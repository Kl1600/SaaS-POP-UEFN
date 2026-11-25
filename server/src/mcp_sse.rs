//! Ancien transport MCP « HTTP + SSE » (spécification 2024-11-05), pour les clients qui ne parlent pas encore
//! Streamable HTTP (#375). Le client ouvre `GET /sse` (flux d'événements), reçoit l'événement `endpoint` qui lui
//! donne l'adresse `/messages?session_id=…`, y envoie ses requêtes JSON-RPC en POST, et lit les réponses dans le
//! flux. Chaque requête passe par le même traitement que `POST /mcp` : mêmes outils, mêmes droits, même journal.
use crate::{
    mcp::{mcp, McpQ},
    AppState,
};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tokio::sync::mpsc;

/// Une session SSE : où pousser les réponses, et le jeton de l'IA qui l'a ouverte.
struct Session {
    tx: mpsc::UnboundedSender<String>,
    token: String,
}

fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static S: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Retire la session quand le flux se ferme (le client s'est déconnecté).
struct Fin(String);
impl Drop for Fin {
    fn drop(&mut self) {
        if let Ok(mut s) = sessions().lock() {
            s.remove(&self.0);
        }
    }
}

fn jeton(headers: &HeaderMap, q: Option<String>) -> String {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(String::from)
        .or(q)
        .unwrap_or_default()
}

fn non_autorise() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(
            axum::http::header::WWW_AUTHENTICATE,
            crate::oauth::www_authenticate_for("sse"),
        )],
        Json(json!({ "error": "Jeton POP invalide ou expiré." })),
    )
        .into_response()
}

/// `GET /sse` : ouvre le flux. Sans jeton valide, 401 et l'adresse de la connexion OAuth, comme `/mcp`.
pub async fn ouvrir(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<McpQ>,
) -> Response {
    let token = jeton(&headers, q.token);
    let ok: Option<i64> = sqlx::query_scalar(
        "SELECT t.id FROM api_tokens t JOIN users u ON u.id = t.user_id WHERE t.token_hash = ? AND u.disabled = 0 AND (t.expires_at IS NULL OR t.expires_at > datetime('now'))",
    )
    .bind(crate::auth::sha256_hex(&token))
    .fetch_optional(&st.db)
    .await
    .ok()
    .flatten();
    if token.is_empty() || ok.is_none() {
        return non_autorise();
    }
    let id = crate::auth::random_token();
    let (tx, rx) = mpsc::unbounded_channel::<String>();
    if let Ok(mut s) = sessions().lock() {
        s.insert(id.clone(), Session { tx, token });
    }
    let fin = Fin(id.clone());
    let premier = Event::default()
        .event("endpoint")
        .data(format!("/messages?session_id={id}"));
    let flux = futures::stream::unfold((Some(premier), rx, fin), |(p, mut rx, fin)| async move {
        if let Some(e) = p {
            return Some((Ok::<Event, Infallible>(e), (None, rx, fin)));
        }
        let m = rx.recv().await?;
        Some((
            Ok(Event::default().event("message").data(m)),
            (None, rx, fin),
        ))
    });
    Sse::new(flux)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
        .into_response()
}

#[derive(Deserialize)]
pub struct MessagesQ {
    session_id: Option<String>,
}

/// `POST /messages?session_id=…` : traite la requête comme `POST /mcp` et pousse la réponse dans le flux.
pub async fn message(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<MessagesQ>,
    Json(req): Json<Value>,
) -> Response {
    let id = q.session_id.unwrap_or_default();
    let (tx, token) = match sessions()
        .lock()
        .ok()
        .and_then(|s| s.get(&id).map(|x| (x.tx.clone(), x.token.clone())))
    {
        Some(v) => v,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Session SSE inconnue ou fermée : rouvrez /sse." })),
            )
                .into_response()
        }
    };
    // Le jeton de la session ; un jeton envoyé avec la requête doit être le même.
    let envoye = jeton(&headers, None);
    if !envoye.is_empty() && envoye != token {
        return non_autorise();
    }
    let mut h = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&format!("Bearer {token}")) {
        h.insert("authorization", v);
    }
    let res = mcp(State(st), h, Query(McpQ { token: None }), Json(req)).await;
    let corps = axum::body::to_bytes(res.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap_or_default();
    if !corps.is_empty() {
        let _ = tx.send(String::from_utf8_lossy(&corps).into_owned());
    }
    StatusCode::ACCEPTED.into_response()
}

#[cfg(test)]
mod tests_sse {
    use super::*;

    #[test]
    fn session_retiree_a_la_fermeture() {
        let (tx, _rx) = mpsc::unbounded_channel();
        sessions().lock().unwrap().insert(
            "s-test".into(),
            Session {
                tx,
                token: "t".into(),
            },
        );
        drop(Fin("s-test".into()));
        assert!(!sessions().lock().unwrap().contains_key("s-test"));
    }
}
