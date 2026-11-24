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
    liberer_silencieux(&st).await?;
    let rows: Vec<(i64, String, String, String, String, Option<String>, i64, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.name, a.kind, u.name, a.scope, a.last_used, a.calls, t.number, t.title, t.status, a.task_since,
                (SELECT x.summary FROM uefn_actions x WHERE x.agent_id = a.id ORDER BY x.id DESC LIMIT 1), a.task_id, a.role
         FROM api_tokens a JOIN users u ON u.id = a.user_id LEFT JOIN tasks t ON t.id = a.task_id
         WHERE a.map_id = ? ORDER BY a.last_used IS NULL, a.last_used DESC, a.id DESC",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let mut out = vec![];
    for r in rows {
        // Étape de la tâche (#298) : écrit, compilé, testé, relu.
        let step = match r.12 {
            Some(tid) => etape(&st, r.0, tid).await?,
            None => None,
        };
        out.push(json!({
            "id": r.0, "name": r.1, "kind": r.2, "owner": r.3, "scope": r.4, "last_used": r.5, "calls": r.6,
            "task": r.7.map(|n| json!({ "number": n, "title": r.8, "status": r.9, "since": r.10, "step": step })),
            "last_action": r.11, "role": r.13,
        }));
    }
    Ok(Json(json!(out)))
}

/// Événement d'agent dans l'historique de la tâche (« IA ») et rafraîchissement du kanban du site (#298).
async fn evenement(st: &AppState, agent: i64, task_id: i64, quoi: &str) -> Result<(), AppError> {
    let r: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT a.user_id, a.name, t.map_id FROM api_tokens a, tasks t WHERE a.id = ? AND t.id = ?",
    )
    .bind(agent)
    .bind(task_id)
    .fetch_optional(&st.db)
    .await?;
    if let Some((uid, nom, mid)) = r {
        sqlx::query("INSERT INTO task_events (task_id, user_id, what, via) VALUES (?, ?, ?, 'IA')")
            .bind(task_id)
            .bind(uid)
            .bind(format!("{quoi} (agent « {nom} »)"))
            .execute(&st.db)
            .await?;
        crate::access::notify(st, mid, json!({ "t": "refresh", "what": "tasks" }));
    }
    Ok(())
}

/// Note un appel d'outil MCP d'un agent sur sa map de travail (#298) : fil en direct de Forge (WebSocket de la map),
/// étape de la tâche, et notifications (tâche bloquée, compilation en échec) au compte de l'agent, sur le site comme
/// dans Forge. `cible` : la tâche visée par comment_task ou set_task_status (sinon la tâche réservée).
pub async fn noter_appel(
    st: &AppState,
    agent: i64,
    mid: i64,
    tool: &str,
    a: &Value,
    res: Result<&Value, ()>,
) {
    let ok = res.is_ok();
    let erreurs = res
        .ok()
        .and_then(|r| r["diagnostics"].as_array())
        .map(|d| d.iter().filter(|x| x["severity"] == "error").count())
        .unwrap_or(0);
    let r: Result<(), AppError> = async {
        let (uid, nom, reservee): (i64, String, Option<i64>) =
            sqlx::query_as("SELECT user_id, name, task_id FROM api_tokens WHERE id = ?")
                .bind(agent)
                .fetch_one(&st.db)
                .await?;
        let cible: Option<i64> = match a["number"].as_i64() {
            Some(n) if matches!(tool, "comment_task" | "set_task_status") => {
                sqlx::query_scalar("SELECT id FROM tasks WHERE map_id = ? AND number = ?")
                    .bind(mid)
                    .bind(n)
                    .fetch_optional(&st.db)
                    .await?
            }
            _ => reservee,
        };
        // Compilation qui passe en échec : on prévient une fois, pas à chaque essai.
        let avant: Option<bool> = sqlx::query_scalar("SELECT ok FROM agent_calls WHERE agent_id = ? AND tool = 'compile_verse' ORDER BY id DESC LIMIT 1")
            .bind(agent)
            .fetch_optional(&st.db)
            .await?;
        let ok_appel = ok && erreurs == 0;
        let id: i64 = sqlx::query_scalar("INSERT INTO agent_calls (agent_id, map_id, task_id, tool, ok) VALUES (?, ?, ?, ?, ?) RETURNING id")
            .bind(agent)
            .bind(mid)
            .bind(cible)
            .bind(tool)
            .bind(ok_appel)
            .fetch_one(&st.db)
            .await?;
        sqlx::query("DELETE FROM agent_calls WHERE map_id = ? AND at < datetime('now', '-7 days')")
            .bind(mid)
            .execute(&st.db)
            .await?;
        crate::access::notify(st, mid, json!({ "t": "agent", "id": id, "agent": agent, "tool": tool, "ok": ok_appel }));
        if tool == "compile_verse" && ok && erreurs > 0 && avant != Some(false) {
            let texte = format!("Compilation en échec ({erreurs} erreur(s)) pour l'agent « {nom} »");
            crate::account::notify_user(st, uid, mid, &texte, &format!("/map/{mid}")).await;
        }
        if tool == "set_task_status" && ok && a["status"] == "blocked" {
            if let (Some(_), Some(n)) = (cible, a["number"].as_i64()) {
                let texte = format!("Tâche #{n} bloquée par l'agent « {nom} »");
                crate::account::notify_user(st, uid, mid, &texte, &format!("/map/{mid}/taches?q=%23{n}")).await;
            }
        }
        Ok(())
    }
    .await;
    if let Err(e) = r {
        tracing::warn!("fil d'activité de l'agent {agent} : {e}");
    }
}

/// Étape de la tâche réservée d'un agent, d'après ses appels depuis qu'il l'a prise : écrit, compilé, testé, relu.
async fn etape(st: &AppState, agent: i64, task_id: i64) -> Result<Option<&'static str>, AppError> {
    let outils: Vec<(String, bool)> = sqlx::query_as(
        "SELECT c.tool, c.ok FROM agent_calls c JOIN api_tokens a ON a.id = c.agent_id
         WHERE c.agent_id = ? AND c.task_id = ? AND c.at >= COALESCE(a.task_since, c.at)",
    )
    .bind(agent)
    .bind(task_id)
    .fetch_all(&st.db)
    .await?;
    let relu: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_calls c JOIN api_tokens a ON a.id = c.agent_id WHERE c.task_id = ? AND c.tool = 'comment_task' AND c.ok AND a.role = 'relecteur'",
    )
    .bind(task_id)
    .fetch_one(&st.db)
    .await?;
    let a = |noms: &[&str]| {
        outils
            .iter()
            .any(|(t, ok)| *ok && noms.contains(&t.as_str()))
    };
    Ok(if relu > 0 {
        Some("relu")
    } else if a(&["start_session", "push_to_session", "session_output"]) && a(&["compile_verse"]) {
        Some("testé")
    } else if a(&["compile_verse"]) {
        Some("compilé")
    } else if a(&[
        "write_verse_file",
        "create_widget",
        "update_actors",
        "duplicate_actors",
        "place_device",
        "place_mesh",
        "import_asset",
        "set_island_settings",
    ]) {
        Some("écrit")
    } else {
        None
    })
}

/// Fil d'activité des agents de la map (#298) : les 50 derniers appels, le plus récent d'abord. `?after=` pour la suite.
pub async fn activite(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, i64>>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, Option<String>, String, bool, String, Option<i64>)> = sqlx::query_as(
        "SELECT c.id, a.name, a.role, c.tool, c.ok, c.at, t.number FROM agent_calls c JOIN api_tokens a ON a.id = c.agent_id
         LEFT JOIN tasks t ON t.id = c.task_id WHERE c.map_id = ? AND c.id > ? ORDER BY c.id DESC LIMIT 50",
    )
    .bind(mid)
    .bind(q.get("after").copied().unwrap_or(0))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, nom, role, tool, ok, at, n)| json!({ "id": id, "agent": nom, "role": role, "tool": tool, "ok": ok, "at": at, "task": n }))
        .collect::<Vec<_>>())))
}

/// Délai sans aucun appel MCP au-delà duquel la réservation d'un agent se libère seule (#114).
pub const SILENCE: &str = "-2 hours";

/// Libère les tâches des agents silencieux depuis plus de 2 h, en laissant une note de reprise pour le suivant.
/// Appelée à la demande (claim_task, next_task, liste des agents) : pas de tâche de fond à surveiller.
pub async fn liberer_silencieux(st: &AppState) -> Result<u64, AppError> {
    let mut tx = st.db.begin().await?;
    let r = sqlx::query(
        "INSERT INTO task_progress (task_id, agent_id, agent_name, todo, auto)
         SELECT task_id, id, name, 'Réservation libérée par POP : agent « ' || name || ' » sans activité depuis ' || COALESCE(last_used, task_since) || ' (UTC). Relisez la note précédente et le journal avant de reprendre.', 1
         FROM api_tokens WHERE task_id IS NOT NULL AND COALESCE(last_used, task_since) < datetime('now', ?)",
    )
    .bind(SILENCE)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE api_tokens SET task_id = NULL, task_since = NULL WHERE task_id IS NOT NULL AND COALESCE(last_used, task_since) < datetime('now', ?)")
        .bind(SILENCE)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(r.rows_affected())
}

/// La note de reprise la plus récente d'une tâche, s'il y en a une, avec la précédente note d'un agent quand la
/// dernière vient de POP (libération automatique).
pub async fn reprise(st: &AppState, task_id: i64) -> Result<Value, AppError> {
    let rows: Vec<(String, String, String, String, bool, String)> = sqlx::query_as(
        "SELECT agent_name, done, todo, files, auto, created_at FROM task_progress WHERE task_id = ? ORDER BY id DESC LIMIT 2",
    )
    .bind(task_id)
    .fetch_all(&st.db)
    .await?;
    let note = |r: &(String, String, String, String, bool, String)| json!({ "par": r.0, "fait": r.1, "reste": r.2, "fichiers": serde_json::from_str::<Value>(&r.3).unwrap_or(json!([])), "automatique": r.4, "le": r.5 });
    Ok(match rows.as_slice() {
        [] => Value::Null,
        [d, p, ..] if d.4 && !p.4 => json!({ "derniere": note(d), "precedente": note(p) }),
        [d, ..] => json!({ "derniere": note(d) }),
    })
}

/// `save_progress` : l'agent note où il en est sur sa tâche réservée (fait, reste, fichiers).
pub async fn save_progress(st: &AppState, agent: i64, a: &Value) -> Result<Value, AppError> {
    let t: Option<(Option<i64>, String)> =
        sqlx::query_as("SELECT task_id, name FROM api_tokens WHERE id = ?")
            .bind(agent)
            .fetch_optional(&st.db)
            .await?;
    let (Some(tid), nom) = t.unwrap_or((None, String::new())) else {
        return Err(bad(
            "Aucune tâche réservée : réservez-la d'abord (claim_task).",
        ));
    };
    let texte = |k: &str| -> Result<String, AppError> {
        let v = a[k].as_str().unwrap_or("").trim().to_string();
        if v.chars().count() > 2000 {
            return Err(bad(format!("{k} : 2 000 caractères au plus.")));
        }
        Ok(v)
    };
    let (fait, reste) = (texte("done")?, texte("todo")?);
    if fait.is_empty() && reste.is_empty() {
        return Err(bad(
            "Écrivez au moins ce qui est fait (done) ou ce qui reste (todo).",
        ));
    }
    let fichiers: Vec<String> = a["files"]
        .as_array()
        .map(|f| {
            f.iter()
                .filter_map(|x| x.as_str())
                .take(50)
                .map(|x| x.chars().take(300).collect())
                .collect()
        })
        .unwrap_or_default();
    sqlx::query("INSERT INTO task_progress (task_id, agent_id, agent_name, done, todo, files) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(tid)
        .bind(agent)
        .bind(&nom)
        .bind(&fait)
        .bind(&reste)
        .bind(json!(fichiers).to_string())
        .execute(&st.db)
        .await?;
    Ok(
        json!({ "message": "Progression enregistrée : un agent qui reprend cette tâche la recevra." }),
    )
}

/// La tâche réservée par l'agent sur cette map, avec sa note de reprise ; et les tâches libres interrompues (avec
/// une note), à reprendre de préférence. Pour next_task.
pub async fn pour_next_task(st: &AppState, agent: i64, mid: i64) -> Result<Value, AppError> {
    liberer_silencieux(st).await?;
    let mienne: Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT t.id, t.number, t.title FROM api_tokens a JOIN tasks t ON t.id = a.task_id WHERE a.id = ? AND t.map_id = ?",
    )
    .bind(agent)
    .bind(mid)
    .fetch_optional(&st.db)
    .await?;
    let mienne = match mienne {
        Some((id, n, titre)) => {
            json!({ "number": n, "title": titre, "reprise": reprise(st, id).await? })
        }
        None => Value::Null,
    };
    let libres: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT t.id, t.number, t.title FROM tasks t WHERE t.map_id = ? AND t.status <> 'done'
           AND EXISTS (SELECT 1 FROM task_progress p WHERE p.task_id = t.id)
           AND NOT EXISTS (SELECT 1 FROM api_tokens a WHERE a.task_id = t.id)
         ORDER BY t.number LIMIT 10",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let mut a_reprendre = vec![];
    for (id, n, titre) in libres {
        a_reprendre.push(json!({ "number": n, "title": titre, "reprise": reprise(st, id).await? }));
    }
    Ok(json!({ "ta_tache": mienne, "a_reprendre": a_reprendre }))
}

/// `claim_task` : l'agent réserve la tâche n° `number` de la map (et libère sa tâche précédente).
pub async fn claim(st: &AppState, agent: i64, mid: i64, number: i64) -> Result<Value, AppError> {
    liberer_silencieux(st).await?;
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
    evenement(st, agent, tid, "Prise").await?;
    let reprise = reprise(st, tid).await?;
    let message = if reprise.is_null() {
        format!("Tâche #{number} réservée pour cet agent.")
    } else {
        format!("Tâche #{number} réservée pour cet agent. Elle a été commencée : lisez la note de reprise avant de travailler.")
    };
    Ok(json!({ "number": number, "title": title, "message": message, "reprise": reprise }))
}

/// `release_task` : l'agent libère sa tâche.
pub async fn release(st: &AppState, agent: i64) -> Result<Value, AppError> {
    let t: Option<i64> = sqlx::query_scalar("SELECT task_id FROM api_tokens WHERE id = ?")
        .bind(agent)
        .fetch_optional(&st.db)
        .await?
        .flatten();
    if let Some(tid) = t {
        evenement(st, agent, tid, "Libérée").await?;
    }
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

    /// #114 : note de reprise rendue à l'agent qui reprend ; réservation silencieuse libérée après 2 h, avec une note.
    #[tokio::test]
    async fn reprise_d_une_tache() {
        let m = monde().await;
        // Sans tâche réservée : refus clair.
        assert!(save_progress(&m.st, m.a, &json!({ "done": "x" }))
            .await
            .is_err());
        claim(&m.st, m.a, m.mid, 1).await.unwrap();
        assert!(save_progress(&m.st, m.a, &json!({})).await.is_err());
        save_progress(&m.st, m.a, &json!({ "done": "Minuteur écrit", "todo": "Brancher le HUD", "files": ["minuteur.verse"] })).await.unwrap();
        let n = pour_next_task(&m.st, m.a, m.mid).await.unwrap();
        assert_eq!(
            n["ta_tache"]["reprise"]["derniere"]["reste"],
            "Brancher le HUD"
        );
        release(&m.st, m.a).await.unwrap();

        // Un autre agent reprend la tâche : il reçoit la note, et next_task la lui proposait.
        let n = pour_next_task(&m.st, m.b, m.mid).await.unwrap();
        assert_eq!(n["a_reprendre"][0]["number"], 1);
        let r = claim(&m.st, m.b, m.mid, 1).await.unwrap();
        assert_eq!(r["reprise"]["derniere"]["fait"], "Minuteur écrit");
        assert_eq!(r["reprise"]["derniere"]["fichiers"][0], "minuteur.verse");
        assert!(r["message"].as_str().unwrap().contains("note de reprise"));

        // Silencieux depuis plus de 2 h : la tâche se libère seule, avec une note automatique et la note de l'agent.
        sqlx::query("UPDATE api_tokens SET last_used = datetime('now', '-3 hours'), task_since = datetime('now', '-3 hours') WHERE id = ?")
            .bind(m.b).execute(&m.st.db).await.unwrap();
        save_progress(&m.st, m.b, &json!({ "todo": "Tester en session" }))
            .await
            .unwrap();
        // Actif il y a 1 h : rien ne bouge.
        sqlx::query("UPDATE api_tokens SET last_used = datetime('now', '-1 hours') WHERE id = ?")
            .bind(m.b)
            .execute(&m.st.db)
            .await
            .unwrap();
        assert_eq!(liberer_silencieux(&m.st).await.unwrap(), 0);
        sqlx::query("UPDATE api_tokens SET last_used = datetime('now', '-3 hours') WHERE id = ?")
            .bind(m.b)
            .execute(&m.st.db)
            .await
            .unwrap();
        let r = claim(&m.st, m.a, m.mid, 1).await.unwrap();
        assert_eq!(r["reprise"]["derniere"]["automatique"], true, "{r}");
        assert_eq!(r["reprise"]["precedente"]["reste"], "Tester en session");
        let libre: Option<i64> = sqlx::query_scalar("SELECT task_id FROM api_tokens WHERE id = ?")
            .bind(m.b)
            .fetch_one(&m.st.db)
            .await
            .unwrap();
        assert_eq!(libre, None);
    }

    /// #298 : historique « IA » et kanban prévenus, fil d'activité, étape de la tâche, notifications.
    #[tokio::test]
    async fn fil_d_activite() {
        let m = monde().await;
        let mut rx = m.st.hub.subscribe(m.mid);
        claim(&m.st, m.a, m.mid, 1).await.unwrap();
        let hist: Vec<(String, String)> = sqlx::query_as("SELECT what, via FROM task_events e JOIN tasks t ON t.id = e.task_id WHERE t.number = 1")
            .fetch_all(&m.st.db).await.unwrap();
        assert!(
            hist.iter()
                .any(|(w, v)| w.starts_with("Prise") && w.contains("Claude") && v == "IA"),
            "{hist:?}"
        );
        let mut vu = false;
        while let Ok(msg) = rx.try_recv() {
            vu |= msg.contains("\"tasks\"");
        }
        assert!(vu, "le kanban du site doit être prévenu");

        // Écrit, puis compilé avec une erreur (notification une seule fois), puis compilé sans erreur, puis testé.
        let ok = json!({});
        noter_appel(&m.st, m.a, m.mid, "write_verse_file", &json!({}), Ok(&ok)).await;
        let l = list(State(m.st.clone()), m.humain.clone(), Path(m.mid))
            .await
            .unwrap()
            .0;
        assert_eq!(
            l.as_array()
                .unwrap()
                .iter()
                .find(|x| x["id"] == m.a)
                .unwrap()["task"]["step"],
            "écrit"
        );
        let ko = json!({ "diagnostics": [{ "severity": "error" }] });
        noter_appel(&m.st, m.a, m.mid, "compile_verse", &json!({}), Ok(&ko)).await;
        noter_appel(&m.st, m.a, m.mid, "compile_verse", &json!({}), Ok(&ko)).await;
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE text LIKE 'Compilation en échec%'",
        )
        .fetch_one(&m.st.db)
        .await
        .unwrap();
        assert_eq!(n, 1);
        noter_appel(&m.st, m.a, m.mid, "compile_verse", &json!({}), Ok(&ok)).await;
        let l = list(State(m.st.clone()), m.humain.clone(), Path(m.mid))
            .await
            .unwrap()
            .0;
        assert_eq!(
            l.as_array()
                .unwrap()
                .iter()
                .find(|x| x["id"] == m.a)
                .unwrap()["task"]["step"],
            "compilé"
        );
        noter_appel(&m.st, m.a, m.mid, "start_session", &json!({}), Ok(&ok)).await;
        let l = list(State(m.st.clone()), m.humain.clone(), Path(m.mid))
            .await
            .unwrap()
            .0;
        assert_eq!(
            l.as_array()
                .unwrap()
                .iter()
                .find(|x| x["id"] == m.a)
                .unwrap()["task"]["step"],
            "testé"
        );

        // Un relecteur commente la tâche : relu.
        sqlx::query("UPDATE api_tokens SET role = 'relecteur' WHERE id = ?")
            .bind(m.b)
            .execute(&m.st.db)
            .await
            .unwrap();
        noter_appel(
            &m.st,
            m.b,
            m.mid,
            "comment_task",
            &json!({ "number": 1 }),
            Ok(&ok),
        )
        .await;
        let l = list(State(m.st.clone()), m.humain.clone(), Path(m.mid))
            .await
            .unwrap()
            .0;
        assert_eq!(
            l.as_array()
                .unwrap()
                .iter()
                .find(|x| x["id"] == m.a)
                .unwrap()["task"]["step"],
            "relu"
        );

        // Tâche bloquée par un agent : notification.
        noter_appel(
            &m.st,
            m.a,
            m.mid,
            "set_task_status",
            &json!({ "number": 1, "status": "blocked" }),
            Ok(&ok),
        )
        .await;
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE text = 'Tâche #1 bloquée par l''agent « Claude »'").fetch_one(&m.st.db).await.unwrap();
        assert_eq!(n, 1);

        // Le fil : le plus récent d'abord, avec l'agent et la tâche.
        let f = activite(
            State(m.st.clone()),
            m.humain.clone(),
            Path(m.mid),
            axum::extract::Query(Default::default()),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(f[0]["tool"], "set_task_status");
        assert_eq!(f.as_array().unwrap().len(), 7);
        assert!(f
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["tool"] == "compile_verse" && x["ok"] == false));
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
