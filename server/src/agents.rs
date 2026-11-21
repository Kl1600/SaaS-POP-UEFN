//! Registre des agents IA (#93) : chaque jeton MCP de POP est un agent (Claude Code, Codex, Cursor…). Un agent
//! réserve une tâche POP à la fois (`claim_task`) et une tâche n'a qu'un agent ; un agent ne modifie pas un fichier
//! Verse qu'un autre agent a écrit pour sa tâche en cours. Ses actions UEFN portent son identifiant dans le journal.
use crate::{
    access::map_ctx,
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

/// Agents de la map : type, humain responsable, droits, dernier appel, tâche réservée, dernière action UEFN.
pub async fn list(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, String, String, String, Option<String>, i64, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.name, a.kind, u.name, a.scope, a.last_used, a.calls, t.number, t.title, t.status, a.task_since,
                (SELECT x.summary FROM uefn_actions x WHERE x.agent_id = a.id ORDER BY x.id DESC LIMIT 1)
         FROM api_tokens a JOIN users u ON u.id = a.user_id LEFT JOIN tasks t ON t.id = a.task_id
         WHERE a.map_id = ? ORDER BY a.last_used IS NULL, a.last_used DESC, a.id DESC",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|r| json!({
            "id": r.0, "name": r.1, "kind": r.2, "owner": r.3, "scope": r.4, "last_used": r.5, "calls": r.6,
            "task": r.7.map(|n| json!({ "number": n, "title": r.8, "status": r.9, "since": r.10 })),
            "last_action": r.11,
        }))
        .collect::<Vec<_>>())))
}

/// `claim_task` : l'agent réserve la tâche n° `number` de la map (et libère sa tâche précédente).
pub async fn claim(st: &AppState, agent: i64, mid: i64, number: i64) -> Result<Value, AppError> {
    let t: Option<(i64, String, String)> =
        sqlx::query_as("SELECT id, title, status FROM tasks WHERE map_id = ? AND number = ?")
            .bind(mid)
            .bind(number)
            .fetch_optional(&st.db)
            .await?;
    let (tid, title, status) =
        t.ok_or_else(|| bad(format!("Tâche #{number} introuvable dans cette map.")))?;
    if status == "done" {
        return Err(bad(format!("La tâche #{number} est déjà faite.")));
    }
    let autre: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT a.id, a.name, u.name FROM api_tokens a JOIN users u ON u.id = a.user_id WHERE a.task_id = ? AND a.id <> ?",
    )
    .bind(tid)
    .bind(agent)
    .fetch_optional(&st.db)
    .await?;
    if let Some((_, nom, humain)) = autre {
        return Err(AppError::Conflict(format!(
            "La tâche #{number} est déjà prise par l'agent « {nom} » ({humain}). Choisissez-en une autre (next_task)."
        )));
    }
    // L'index unique sur task_id tranche une course entre deux agents : le second reçoit une erreur.
    sqlx::query("UPDATE api_tokens SET task_id = ?, task_since = datetime('now') WHERE id = ?")
        .bind(tid)
        .bind(agent)
        .execute(&st.db)
        .await
        .map_err(|_| {
            AppError::Conflict(format!(
                "La tâche #{number} vient d'être prise par un autre agent."
            ))
        })?;
    Ok(
        json!({ "number": number, "title": title, "message": format!("Tâche #{number} réservée pour cet agent.") }),
    )
}

/// `release_task` : l'agent libère sa tâche.
pub async fn release(st: &AppState, agent: i64) -> Result<Value, AppError> {
    sqlx::query("UPDATE api_tokens SET task_id = NULL, task_since = NULL WHERE id = ?")
        .bind(agent)
        .execute(&st.db)
        .await?;
    Ok(json!({ "message": "Tâche libérée." }))
}

/// Refuse l'écriture d'un fichier Verse qu'un autre agent a écrit depuis qu'il a réservé sa tâche en cours.
pub async fn check_file(st: &AppState, agent: i64, mid: i64, path: &str) -> Result<(), AppError> {
    let autre: Option<(String, String, i64)> = sqlx::query_as(
        "SELECT a.name, u.name, t.number FROM uefn_actions x
         JOIN api_tokens a ON a.id = x.agent_id JOIN users u ON u.id = a.user_id JOIN tasks t ON t.id = a.task_id
         WHERE x.map_id = ? AND x.action = 'write_file' AND x.agent_id <> ? AND json_extract(x.args, '$.path') = ?
           AND x.at >= a.task_since AND x.status <> 'undone'
         ORDER BY x.id DESC LIMIT 1",
    )
    .bind(mid)
    .bind(agent)
    .bind(path)
    .fetch_optional(&st.db)
    .await?;
    match autre {
        Some((nom, humain, n)) => Err(AppError::Conflict(format!(
            "{path} est en cours de modification par l'agent « {nom} » ({humain}) pour la tâche #{n}."
        ))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Monde {
        st: AppState,
        mid: i64,
        humain: AuthUser,
        a: i64,
        b: i64,
    }

    async fn monde() -> Monde {
        let st = crate::test_state().await;
        let q = |s: &'static str| sqlx::query_scalar::<_, i64>(s);
        let sid = q("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid = sqlx::query_scalar::<_, i64>(
            "INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id",
        )
        .bind(sid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        let uid = q("INSERT INTO users (email, name, password_hash) VALUES ('h@a.fr', 'Yanis', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        for n in 1..=2 {
            sqlx::query("INSERT INTO tasks (map_id, number, title) VALUES (?, ?, ?)")
                .bind(mid)
                .bind(n)
                .bind(format!("Tâche {n}"))
                .execute(&st.db)
                .await
                .unwrap();
        }
        let jeton = |nom: &'static str, kind: &'static str| {
            let st = st.clone();
            async move {
                sqlx::query_scalar::<_, i64>("INSERT INTO api_tokens (user_id, map_id, name, token_hash, kind) VALUES (?, ?, ?, ?, ?) RETURNING id")
                    .bind(uid).bind(mid).bind(nom).bind(format!("h-{nom}")).bind(kind).fetch_one(&st.db).await.unwrap()
            }
        };
        let a = jeton("Claude", "claude_code").await;
        let b = jeton("Codex", "codex").await;
        let humain = AuthUser {
            id: uid,
            email: "h@a.fr".into(),
            name: "Yanis".into(),
            sid: String::new(),
        };
        Monde {
            st,
            mid,
            humain,
            a,
            b,
        }
    }

    #[tokio::test]
    async fn une_tache_par_agent() {
        let m = monde().await;
        claim(&m.st, m.a, m.mid, 1).await.unwrap();
        // Le second agent est refusé sur la même tâche, avec le nom du premier.
        match claim(&m.st, m.b, m.mid, 1).await {
            Err(AppError::Conflict(msg)) => assert!(msg.contains("Claude"), "{msg}"),
            _ => panic!("la tâche #1 devait être refusée au second agent"),
        }
        claim(&m.st, m.b, m.mid, 2).await.unwrap();
        // Changer de tâche libère la précédente.
        claim(&m.st, m.a, m.mid, 1).await.unwrap();
        release(&m.st, m.b).await.unwrap();
        claim(&m.st, m.a, m.mid, 2).await.unwrap();
        claim(&m.st, m.b, m.mid, 1).await.unwrap();
        assert!(claim(&m.st, m.a, m.mid, 99).await.is_err());
    }

    #[tokio::test]
    async fn liste_journal_et_fichiers() {
        let m = monde().await;
        claim(&m.st, m.a, m.mid, 1).await.unwrap();
        sqlx::query("UPDATE api_tokens SET last_used = datetime('now'), calls = 3 WHERE id = ?")
            .bind(m.a)
            .execute(&m.st.db)
            .await
            .unwrap();
        for (agent, path) in [(m.a, "Content/jeu.verse"), (m.b, "Content/hud.verse")] {
            sqlx::query("INSERT INTO uefn_actions (map_id, user_id, source, action, args, summary, status, agent_id, at) VALUES (?, ?, 'mcp', 'write_file', ?, ?, 'done', ?, datetime('now', '+1 second'))")
                .bind(m.mid).bind(m.humain.id).bind(json!({ "path": path }).to_string()).bind(format!("Écrit {path}")).bind(agent)
                .execute(&m.st.db).await.unwrap();
        }
        // Liste : tâche, dernier appel, dernière action.
        let Json(l) = list(State(m.st.clone()), m.humain.clone(), Path(m.mid))
            .await
            .unwrap();
        let claude = l
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == m.a)
            .unwrap();
        assert_eq!(claude["task"]["number"], 1);
        assert_eq!(claude["calls"], 3);
        assert_eq!(claude["last_action"], "Écrit Content/jeu.verse");
        // Journal filtré par agent.
        let q = axum::extract::Query(serde_json::from_value(json!({ "agent": m.b })).unwrap());
        let Json(j) = crate::uefn::journal(State(m.st.clone()), m.humain.clone(), Path(m.mid), q)
            .await
            .unwrap();
        assert_eq!(j.as_array().unwrap().len(), 1);
        assert_eq!(j[0]["agent_name"], "Codex");
        // Fichier écrit par Claude pour sa tâche : Codex ne peut pas l'écrire ; Claude oui.
        assert!(matches!(
            check_file(&m.st, m.b, m.mid, "Content/jeu.verse").await,
            Err(AppError::Conflict(_))
        ));
        check_file(&m.st, m.a, m.mid, "Content/jeu.verse")
            .await
            .unwrap();
        // Codex n'a pas de tâche : son fichier ne bloque personne.
        check_file(&m.st, m.a, m.mid, "Content/hud.verse")
            .await
            .unwrap();
        // Claude libère sa tâche : le fichier est de nouveau libre.
        release(&m.st, m.a).await.unwrap();
        check_file(&m.st, m.b, m.mid, "Content/jeu.verse")
            .await
            .unwrap();
    }
}
