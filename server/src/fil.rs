//! Fil d'avancement de la map (#137) : ce que l'IA et POP font, en direct, avec l'annulation à portée de clic.
//! Rassemble le journal UEFN (fichiers Verse, acteurs, compilation, check-in…), l'historique des tâches (prise,
//! changement de statut, check-in) et les suppressions demandées par une IA, qui attendent un humain.
//! Rien de nouveau n'est enregistré : le fil relit le journal existant. Le site se met à jour sur les messages
//! WebSocket déjà envoyés (`uefn`, `agent`, `refresh tasks`).
use crate::{
    access::map_ctx,
    auth::AuthUser,
    error::{ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

/// Nombre d'éléments renvoyés.
const TAILLE: i64 = 60;

pub async fn fil(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let mut items: Vec<(String, Value)> = vec![];

    let actions: Vec<(i64, Option<String>, Option<String>, String, String, String, Option<String>, Option<i64>, bool, String)> = sqlx::query_as(
        "SELECT a.id, u.name, t.name, a.source, a.summary, a.status, a.error, a.task_number, a.undo IS NOT NULL, a.at
         FROM uefn_actions a LEFT JOIN users u ON u.id = a.user_id LEFT JOIN api_tokens t ON t.id = a.agent_id
         WHERE a.map_id = ? ORDER BY a.id DESC LIMIT ?",
    )
    .bind(mid)
    .bind(TAILLE)
    .fetch_all(&st.db)
    .await?;
    for (id, humain, agent, source, resume, statut, erreur, tache, annulable, at) in actions {
        let texte = if source == "undo" {
            format!("Annulation : {resume}")
        } else {
            resume
        };
        items.push((
            at.clone(),
            json!({
                "kind": "uefn", "id": id, "text": texte, "status": statut, "error": erreur, "task": tache, "at": at,
                "who": agent.clone().or(humain), "ia": agent.is_some() || source == "mcp",
                "undoable": annulable && (statut == "done" || statut == "queued") && source != "undo",
            }),
        ));
    }

    let evenements: Vec<(i64, i64, String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT e.id, t.number, t.title, e.what, e.via, u.name, e.at FROM task_events e JOIN tasks t ON t.id = e.task_id
         LEFT JOIN users u ON u.id = e.user_id WHERE t.map_id = ? ORDER BY e.id DESC LIMIT ?",
    )
    .bind(mid)
    .bind(TAILLE)
    .fetch_all(&st.db)
    .await?;
    for (id, n, titre, quoi, via, qui, at) in evenements {
        items.push((
            at.clone(),
            json!({ "kind": "tache", "id": id, "task": n, "title": titre, "text": quoi, "who": qui, "ia": via == "IA", "at": at }),
        ));
    }

    // Suppressions demandées par une IA : en attente jusqu'à « Valider » ou « Refuser » d'un humain.
    let demandes: Vec<(i64, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT a.id, a.summary, a.status, u.name, a.at FROM approvals a LEFT JOIN users u ON u.id = a.decided_by
         WHERE a.map_id = ? ORDER BY a.id DESC LIMIT ?",
    )
    .bind(mid)
    .bind(TAILLE)
    .fetch_all(&st.db)
    .await?;
    for (id, resume, statut, par, at) in demandes {
        items.push((
            at.clone(),
            json!({ "kind": "validation", "id": id, "text": resume, "status": statut, "decided_by": par, "ia": true, "at": at }),
        ));
    }

    // Le plus récent d'abord ; à égalité de seconde, une validation en attente reste visible en tête.
    items.sort_by(|a, b| b.0.cmp(&a.0));
    items.truncate(TAILLE as usize);
    let en_attente = items
        .iter()
        .filter(|(_, v)| v["kind"] == "validation" && v["status"] == "pending")
        .count();
    Ok(
        json!({ "items": items.into_iter().map(|(_, v)| v).collect::<Vec<_>>(), "pending": en_attente }),
    )
}

/// `GET /api/maps/{mid}/fil` : le fil d'avancement de la map, le plus récent d'abord.
pub async fn route(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    Ok(Json(fil(&st, mid).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rassemble_le_journal() {
        let st = crate::test_state().await;
        let q = |s: &'static str| sqlx::query_scalar::<_, i64>(s);
        let sid = q("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let autre: i64 = sqlx::query_scalar(
            "INSERT INTO maps (studio_id, name) VALUES (?, 'Autre') RETURNING id",
        )
        .bind(sid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        let uid = q("INSERT INTO users (email, name, password_hash) VALUES ('f@a.fr', 'Yanis', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        let ag: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, map_id, name, token_hash, kind) VALUES (?, ?, 'Claude', 'h', 'claude_code') RETURNING id").bind(uid).bind(mid).fetch_one(&st.db).await.unwrap();
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tasks (map_id, number, title) VALUES (?, 1, 'Score') RETURNING id",
        )
        .bind(mid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        let ins = |source: &'static str,
                   resume: &'static str,
                   statut: &'static str,
                   undo: Option<&'static str>,
                   agent: Option<i64>,
                   m: i64,
                   at: &'static str| {
            let st = st.clone();
            async move {
                sqlx::query("INSERT INTO uefn_actions (map_id, user_id, source, action, args, summary, undo, status, agent_id, at) VALUES (?, ?, ?, 'write_file', '{}', ?, ?, ?, ?, ?)")
                    .bind(m).bind(uid).bind(source).bind(resume).bind(undo).bind(statut).bind(agent).bind(at)
                    .execute(&st.db).await.unwrap();
            }
        };
        ins(
            "mcp",
            "Fichier Verse écrit : score.verse",
            "done",
            Some("{}"),
            Some(ag),
            mid,
            "2026-11-24 10:00:01",
        )
        .await;
        ins(
            "undo",
            "Fichier Verse retiré : score.verse",
            "done",
            Some("{}"),
            None,
            mid,
            "2026-11-24 10:00:05",
        )
        .await;
        ins(
            "mcp",
            "Acteur posé ailleurs",
            "done",
            Some("{}"),
            Some(ag),
            autre,
            "2026-11-24 10:00:06",
        )
        .await;
        sqlx::query("INSERT INTO task_events (task_id, user_id, what, via, at) VALUES (?, ?, 'Prise (agent « Claude »)', 'IA', '2026-11-24 10:00:00')").bind(tid).bind(uid).execute(&st.db).await.unwrap();
        sqlx::query("INSERT INTO approvals (map_id, user_id, action, args, summary, at) VALUES (?, ?, 'actor_delete', '{}', 'L’IA demande de supprimer : Mur', '2026-11-24 10:00:03')").bind(mid).bind(uid).execute(&st.db).await.unwrap();

        let f = fil(&st, mid).await.unwrap();
        let items = f["items"].as_array().unwrap();
        assert_eq!(items.len(), 4, "{f}");
        assert_eq!(
            items[0]["text"],
            "Annulation : Fichier Verse retiré : score.verse"
        );
        assert_eq!(
            items[0]["undoable"], false,
            "une annulation ne s'annule pas depuis le fil"
        );
        assert_eq!(items[1]["kind"], "validation");
        assert_eq!(f["pending"], 1);
        assert_eq!(items[2]["who"], "Claude");
        assert_eq!(items[2]["undoable"], true);
        assert_eq!(items[2]["ia"], true);
        assert_eq!(items[3]["kind"], "tache");
        assert_eq!(items[3]["task"], 1);
        assert!(
            !items.iter().any(|x| x["text"] == "Acteur posé ailleurs"),
            "rien d'une autre map"
        );
    }
}
