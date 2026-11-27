//! Parler à ses IA depuis Forge (#381) : un fil de messages par agent, et « à toute l'équipe » (copié dans le fil de
//! chacune de ses IA sur la map). L'IA lit ses messages avec `read_messages` (signalés aussi par `next_task`) et
//! répond avec `send_message`. Seul le propriétaire d'une IA lui écrit et lit son fil (#376).
use crate::{
    access::{map_ctx, notify},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

async fn mon_agent(st: &AppState, mid: i64, aid: i64, uid: i64) -> Result<(), AppError> {
    let ok: Option<i64> =
        sqlx::query_scalar("SELECT id FROM api_tokens WHERE id = ? AND map_id = ? AND user_id = ?")
            .bind(aid)
            .bind(mid)
            .bind(uid)
            .fetch_optional(&st.db)
            .await?;
    ok.map(|_| ()).ok_or(AppError::NotFound)
}

/// Fil d'un agent : 100 derniers messages, du plus ancien au plus récent.
pub async fn fil(st: &AppState, aid: i64) -> Result<Value, AppError> {
    let rows: Vec<(i64, Option<String>, bool, bool, String, String, Option<String>)> = sqlx::query_as(
        "SELECT m.id, u.name, m.de_ia, m.equipe, m.body, m.created_at, m.read_at FROM agent_messages m LEFT JOIN users u ON u.id = m.user_id
         WHERE m.agent_id = ? ORDER BY m.id DESC LIMIT 100",
    )
    .bind(aid).fetch_all(&st.db).await?;
    Ok(json!(rows.into_iter().rev().map(|(id, qui, ia, eq, b, at, lu)| json!({ "id": id, "from": if ia { "ia" } else { "humain" }, "author": qui, "team": eq, "body": b, "at": at, "read": lu.is_some() })).collect::<Vec<_>>()))
}

#[derive(Deserialize)]
pub struct MessageIn {
    body: String,
}

fn texte(b: &str) -> Result<String, AppError> {
    let b = b.trim().to_string();
    check_len("Message", &b, 1, 4000)?;
    Ok(b)
}

/// `GET /api/maps/{mid}/agents/{aid}/messages`
pub async fn route_fil(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, aid)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    crate::billing::require_forge(&st, u.id).await?;
    map_ctx(&st, &u, mid).await?;
    mon_agent(&st, mid, aid, u.id).await?;
    Ok(Json(fil(&st, aid).await?))
}

/// `POST /api/maps/{mid}/agents/{aid}/messages` : écrire à une de ses IA.
pub async fn route_ecrire(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, aid)): Path<(i64, i64)>,
    Json(b): Json<MessageIn>,
) -> ApiResult<Value> {
    crate::billing::require_forge(&st, u.id).await?;
    map_ctx(&st, &u, mid).await?;
    mon_agent(&st, mid, aid, u.id).await?;
    let b = texte(&b.body)?;
    sqlx::query("INSERT INTO agent_messages (map_id, agent_id, user_id, body) VALUES (?, ?, ?, ?)")
        .bind(mid)
        .bind(aid)
        .bind(u.id)
        .bind(&b)
        .execute(&st.db)
        .await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "messages" }));
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/maps/{mid}/agents/messages` : écrire à toutes ses IA de la map.
pub async fn route_equipe(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<MessageIn>,
) -> ApiResult<Value> {
    crate::billing::require_forge(&st, u.id).await?;
    map_ctx(&st, &u, mid).await?;
    let b = texte(&b.body)?;
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM api_tokens WHERE map_id = ? AND user_id = ? AND scope <> 'off'",
    )
    .bind(mid)
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    if ids.is_empty() {
        return Err(bad("Aucune de vos IA ne travaille sur cette map."));
    }
    for a in &ids {
        sqlx::query("INSERT INTO agent_messages (map_id, agent_id, user_id, equipe, body) VALUES (?, ?, ?, 1, ?)").bind(mid).bind(a).bind(u.id).bind(&b).execute(&st.db).await?;
    }
    notify(&st, mid, json!({ "t": "refresh", "what": "messages" }));
    Ok(Json(json!({ "ok": true, "agents": ids.len() })))
}

/// MCP `read_messages` : messages non lus de l'humain pour cet agent (marqués lus).
pub async fn lire_pour_agent(st: &AppState, agent: i64) -> Result<Value, AppError> {
    let rows: Vec<(i64, Option<String>, bool, String, String)> = sqlx::query_as(
        "SELECT m.id, u.name, m.equipe, m.body, m.created_at FROM agent_messages m LEFT JOIN users u ON u.id = m.user_id
         WHERE m.agent_id = ? AND m.de_ia = 0 AND m.read_at IS NULL ORDER BY m.id",
    )
    .bind(agent).fetch_all(&st.db).await?;
    sqlx::query("UPDATE agent_messages SET read_at = datetime('now') WHERE agent_id = ? AND de_ia = 0 AND read_at IS NULL").bind(agent).execute(&st.db).await?;
    let mid: Option<i64> = sqlx::query_scalar("SELECT map_id FROM api_tokens WHERE id = ?")
        .bind(agent)
        .fetch_optional(&st.db)
        .await?
        .flatten();
    if let (Some(m), false) = (mid, rows.is_empty()) {
        notify(st, m, json!({ "t": "refresh", "what": "messages" }));
    }
    Ok(json!({
        "messages": rows.into_iter().map(|(id, qui, eq, b, at)| json!({ "id": id, "from": qui, "to": if eq { "toute l'équipe" } else { "vous" }, "body": b, "at": at })).collect::<Vec<_>>(),
        "consigne": "Répondez avec send_message, puis faites ce qui est demandé (en créant ou en prenant une tâche si besoin).",
    }))
}

/// Messages non lus joints à la réponse d'un outil (#464) : lus et marqués comme lus ; `None` s'il n'y en a pas.
pub async fn joindre(st: &AppState, agent: i64) -> Option<Value> {
    if non_lus(st, agent).await.ok()? == 0 {
        return None;
    }
    lire_pour_agent(st, agent).await.ok()
}

/// Nombre de messages non lus pour cet agent (signalé par next_task).
pub async fn non_lus(st: &AppState, agent: i64) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_messages WHERE agent_id = ? AND de_ia = 0 AND read_at IS NULL",
    )
    .bind(agent)
    .fetch_one(&st.db)
    .await?)
}

/// MCP `send_message` : réponse de l'IA dans son fil.
pub async fn ecrire_par_agent(
    st: &AppState,
    agent: i64,
    mid: i64,
    a: &Value,
) -> Result<Value, AppError> {
    let b = texte(a["body"].as_str().unwrap_or(""))?;
    sqlx::query("INSERT INTO agent_messages (map_id, agent_id, de_ia, body) VALUES (?, ?, 1, ?)")
        .bind(mid)
        .bind(agent)
        .bind(&b)
        .execute(&st.db)
        .await?;
    notify(st, mid, json!({ "t": "refresh", "what": "messages" }));
    Ok(
        json!({ "ok": true, "message": "Message envoyé à votre humain (fil de l'agent dans POP Forge)." }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fil_humain_ia() {
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
        let mut us = vec![];
        for e in ["moi@a.fr", "ben@a.fr"] {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO users (email, name, password_hash, plan) VALUES (?, ?, 'x', 'pro') RETURNING id",
            )
            .bind(e)
            .bind(e)
            .fetch_one(&st.db)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')",
            )
            .bind(sid)
            .bind(id)
            .execute(&st.db)
            .await
            .unwrap();
            us.push(AuthUser {
                id,
                email: e.into(),
                name: e.into(),
                sid: String::new(),
            });
        }
        let mut ag = vec![];
        for (i, n) in [(0, "A1"), (0, "A2"), (1, "B1")] {
            let id: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, map_id, name, token_hash) VALUES (?, ?, ?, ?) RETURNING id").bind(us[i].id).bind(mid).bind(n).bind(n).fetch_one(&st.db).await.unwrap();
            ag.push(id);
        }
        let s = || State(st.clone());
        let m = |t: &str| Json(MessageIn { body: t.into() });
        let _ = route_ecrire(
            s(),
            us[0].clone(),
            Path((mid, ag[0])),
            m("Fais la boutique"),
        )
        .await
        .unwrap();
        assert_eq!(non_lus(&st, ag[0]).await.unwrap(), 1);
        let r = lire_pour_agent(&st, ag[0]).await.unwrap();
        assert_eq!(r["messages"][0]["body"], "Fais la boutique");
        assert_eq!(non_lus(&st, ag[0]).await.unwrap(), 0);
        ecrire_par_agent(&st, ag[0], mid, &json!({ "body": "Je m'en occupe" }))
            .await
            .unwrap();
        let Json(f) = route_fil(s(), us[0].clone(), Path((mid, ag[0])))
            .await
            .unwrap();
        assert_eq!(f[1]["from"], "ia");
        assert_eq!(f[0]["read"], true);
        // À toute l'équipe : mes deux IA, pas celle de Ben.
        let Json(e) = route_equipe(s(), us[0].clone(), Path(mid), m("Pause à 18 h"))
            .await
            .unwrap();
        assert_eq!(e["agents"], 2);
        assert_eq!(non_lus(&st, ag[1]).await.unwrap(), 1);
        assert_eq!(non_lus(&st, ag[2]).await.unwrap(), 0);
        // Ben ne lit ni n'écrit dans le fil de mes IA.
        assert!(route_fil(s(), us[1].clone(), Path((mid, ag[0])))
            .await
            .is_err());
        assert!(
            route_ecrire(s(), us[1].clone(), Path((mid, ag[0])), m("coucou"))
                .await
                .is_err()
        );
        assert!(
            route_ecrire(s(), us[0].clone(), Path((mid, ag[0])), m("  "))
                .await
                .is_err()
        );
    }
}
