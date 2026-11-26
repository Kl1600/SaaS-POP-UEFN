//! Compilation et session de test d'une map (#83), lues par l'écran Vérifications de Forge (#395) : dernière compilation (erreurs avec fichier et ligne), dernières actions de
//! session (démarrer, arrêter, envoyer), qu'elles viennent d'une IA ou d'un humain. Les boutons passent par
//! `POST /api/maps/{mid}/uefn/action` (journalisé comme toute action UEFN).
use crate::{access::map_ctx, auth::AuthUser, error::ApiResult, AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

/// `GET /api/maps/{mid}/build`
pub async fn etat(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, String, Option<String>, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.action, a.status, a.error, a.at, a.after, us.name, t.name FROM uefn_actions a
         LEFT JOIN users us ON us.id = a.user_id LEFT JOIN api_tokens t ON t.id = a.agent_id
         WHERE a.map_id = ? AND a.action IN ('compile', 'start_session', 'stop_session', 'push_session') ORDER BY a.id DESC LIMIT 30",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let ligne = |r: &(
        i64,
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    )| {
        let after: Value =
            r.5.as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(Value::Null);
        json!({ "id": r.0, "action": r.1, "status": r.2, "error": r.3, "at": r.4, "by": r.7.clone().or(r.6.clone()), "ai": r.7.is_some(), "diagnostics": after["diagnostics"] })
    };
    let compile = rows.iter().find(|r| r.1 == "compile").map(ligne);
    let session: Vec<Value> = rows
        .iter()
        .filter(|r| r.1 != "compile")
        .take(10)
        .map(ligne)
        .collect();
    // Session en cours : le dernier démarrage réussi n'est pas suivi d'un arrêt.
    let en_cours = rows
        .iter()
        .filter(|r| r.2 == "done" && (r.1 == "start_session" || r.1 == "stop_session"))
        .map(|r| r.1 == "start_session")
        .next()
        .unwrap_or(false);
    Ok(Json(
        json!({ "compile": compile, "session": session, "session_running": en_cours, "relay": st.relays.status(mid) }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn derniere_compilation_et_session() {
        let st = crate::test_state().await;
        let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let uid: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('a@a.fr', 'A', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        let ins = |a: &str, after: &str| {
            sqlx::query("INSERT INTO uefn_actions (map_id, user_id, source, action, summary, args, status, after) VALUES (?, ?, 'mcp', ?, '', '{}', 'done', ?)")
                .bind(mid).bind(uid).bind(a.to_string()).bind(after.to_string()).execute(&st.db)
        };
        ins("compile", r#"{"diagnostics":[{"path":"score.verse","line":12,"severity":"error","message":"Identifiant inconnu"}]}"#).await.unwrap();
        ins("start_session", "{}").await.unwrap();
        let u = AuthUser {
            id: uid,
            email: "a@a.fr".into(),
            name: "A".into(),
            sid: String::new(),
        };
        let Json(r) = etat(State(st.clone()), u.clone(), Path(mid)).await.unwrap();
        assert_eq!(r["compile"]["diagnostics"][0]["line"], 12);
        assert_eq!(r["compile"]["diagnostics"][0]["path"], "score.verse");
        assert_eq!(r["session_running"], true);
        ins("stop_session", "{}").await.unwrap();
        let Json(r) = etat(State(st.clone()), u, Path(mid)).await.unwrap();
        assert_eq!(r["session_running"], false);
        assert_eq!(r["session"].as_array().unwrap().len(), 2);
    }
}
