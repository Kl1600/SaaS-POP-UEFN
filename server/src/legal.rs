//! Informations légales de l'éditeur (affichées dans les mentions légales, la politique de confidentialité,
//! les CGU et les CGV) et relais des vignettes vidéo (pour qu'aucun visiteur ne contacte TikTok ou YouTube).
use crate::{
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub const FIELDS: &[&str] = &[
    "editor_name",
    "editor_form",
    "capital",
    "address",
    "siren",
    "vat",
    "publisher",
    "email",
    "phone",
    "host_name",
    "host_address",
    "host_phone",
    "mediator_name",
    "mediator_url",
    "updated",
];

/// Lecture publique (les pages légales sont visibles sans compte).
pub async fn get(State(st): State<AppState>) -> ApiResult<Value> {
    let d: String = sqlx::query_scalar("SELECT data FROM legal_info WHERE id = 1")
        .fetch_one(&st.db)
        .await?;
    let mut v: Value = serde_json::from_str(&d).unwrap_or(json!({}));
    v["site"] = json!(crate::mail::public_url());
    Ok(Json(v))
}

pub async fn save(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<Map<String, Value>>,
) -> ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    let mut out = Map::new();
    for k in FIELDS {
        let s = b
            .get(*k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if s.chars().count() > 400 {
            return Err(bad("Champ trop long (400 caractères au plus)."));
        }
        out.insert(k.to_string(), json!(s));
    }
    sqlx::query("UPDATE legal_info SET data = ?, updated_at = datetime('now') WHERE id = 1")
        .bind(Value::Object(out).to_string())
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ImgQ {
    u: String,
}

/// Hôtes des vignettes TikTok et YouTube que POP accepte de relayer (rien d'autre : pas de requête vers une adresse au choix).
const IMG_HOSTS: &[&str] = &[
    "ytimg.com",
    "ggpht.com",
    "tiktokcdn.com",
    "tiktokcdn-us.com",
    "tiktokcdn-eu.com",
    "ibyteimg.com",
    "muscdn.com",
    "byteimg.com",
];

/// Relaie une vignette vidéo : le navigateur du visiteur ne parle qu'à POP (son adresse IP n'est pas transmise à TikTok ou YouTube).
pub async fn image(
    State(st): State<AppState>,
    _u: AuthUser,
    Query(q): Query<ImgQ>,
) -> Result<Response, AppError> {
    let url = reqwest::Url::parse(&q.u).map_err(|_| bad("Adresse d’image invalide."))?;
    let host = url.host_str().unwrap_or("").to_lowercase();
    if url.scheme() != "https"
        || !IMG_HOSTS
            .iter()
            .any(|h| host == *h || host.ends_with(&format!(".{h}")))
    {
        return Err(AppError::Forbidden);
    }
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default()
    });
    let _ = &st;
    let r = client
        .get(url)
        .header("user-agent", "POP-UEFN/1.0")
        .send()
        .await
        .map_err(|_| AppError::NotFound)?;
    if !r.status().is_success() {
        return Err(AppError::NotFound);
    }
    let ct = r
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !ct.starts_with("image/") || ct.contains("svg") {
        return Err(AppError::NotFound);
    }
    let bytes = r.bytes().await.map_err(|_| AppError::NotFound)?;
    if bytes.len() > 3_000_000 {
        return Err(AppError::NotFound);
    }
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, ct),
            (header::CACHE_CONTROL, "private, max-age=86400".to_string()),
        ],
        bytes,
    )
        .into_response())
}

/// Durées de conservation (écrites dans la politique de confidentialité), appliquées une fois par jour :
/// sessions expirées, journal de sécurité au-delà de 12 mois, demandes de bêta refusées au-delà de 12 mois,
/// notifications lues au-delà de 6 mois.
pub async fn purge(st: &AppState) {
    for q in [
        "DELETE FROM sessions WHERE expires_at < datetime('now') OR COALESCE(last_seen, created_at) < datetime('now', '-14 days')",
        "DELETE FROM audit_log WHERE at < datetime('now', '-12 months')",
        "DELETE FROM waitlist WHERE status = 'refused' AND created_at < datetime('now', '-12 months')",
        "DELETE FROM notifications WHERE read_at IS NOT NULL AND at < datetime('now', '-6 months')",
    ] {
        if let Err(e) = sqlx::query(q).execute(&st.db).await { tracing::warn!("purge : {e}") }
    }
}
