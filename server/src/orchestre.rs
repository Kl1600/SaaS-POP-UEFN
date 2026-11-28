//! Orchestrateur (#112) : l'agent qui découpe une demande en tâches POP (`plan_tasks`) et les attribue aux agents
//! de la map (`assign_task`). POP n'appelle aucune IA : c'est l'IA de l'utilisateur qui orchestre.
//!
//! Qui est l'Orchestrateur d'une map (décision du 24/11) :
//! 1. l'agent à qui un humain a donné le rôle « Orchestrateur » (le plus récemment actif s'il y en a plusieurs) ;
//! 2. sinon, par défaut, le premier agent Claude lancé sur la map parmi ceux qui sont actifs ; s'il s'arrête (aucun
//!    appel depuis 2 h), le suivant reprend le rôle ;
//! 3. sinon, personne : Forge demandera qui reprend (#94).
//!
//! L'Orchestrateur par défaut garde tous ses outils : il reçoit seulement en plus `plan_tasks` et `assign_task`.
use crate::{
    access::MapCtx,
    error::{bad, AppError},
    AppState,
};
use serde_json::{json, Value};

/// Outils réservés à l'Orchestrateur de la map.
pub const OUTILS: &[&str] = &["plan_tasks", "assign_task"];

/// Identifiant de l'agent Orchestrateur de la map, s'il y en a un.
pub async fn de_la_map(st: &AppState, mid: i64) -> Result<Option<i64>, AppError> {
    // Plus de rôle choisi (#490) : Claude orchestre quand il est là (l'IA connectée par l'humain avant les agents
    // ouverts dans Forge, puis le premier arrivé) ; sans Claude, l'IA active la plus récente.
    Ok(sqlx::query_scalar(
        "SELECT id FROM api_tokens WHERE map_id = ? AND COALESCE(role, '') <> 'relecteur'
           AND COALESCE(last_used, created_at) >= datetime('now', ?)
         ORDER BY (kind LIKE 'claude%' OR lower(name) LIKE '%claude%') DESC,
                  CASE WHEN kind LIKE 'claude%' OR lower(name) LIKE '%claude%' THEN forge END,
                  CASE WHEN kind LIKE 'claude%' OR lower(name) LIKE '%claude%' THEN created_at END,
                  COALESCE(last_used, created_at) DESC, id
         LIMIT 1",
    )
    .bind(mid)
    .bind(crate::agents::SILENCE)
    .fetch_optional(&st.db)
    .await?)
}

/// L'agent est-il l'Orchestrateur de sa map de travail ?
pub async fn est(st: &AppState, agent: i64, mid: Option<i64>) -> bool {
    match mid {
        Some(m) => matches!(de_la_map(st, m).await, Ok(Some(o)) if o == agent),
        None => false,
    }
}

/// Refus clair pour un agent qui n'est pas l'Orchestrateur.
pub fn refus(tool: &str) -> String {
    format!("{tool} est réservé à l'Orchestrateur de la map. Créez vos tâches avec create_task, ou demandez à un humain de vous donner le rôle « Orchestrateur » (Paramètres → IA connectées).")
}

/// Agent de la map désigné par son identifiant ou son nom exact ; son humain doit être encore membre du studio.
async fn agent_de_la_map(
    st: &AppState,
    ctx: &MapCtx,
    v: &Value,
) -> Result<(i64, String), AppError> {
    let r: Option<(i64, String)> = sqlx::query_as(
        "SELECT a.id, a.name FROM api_tokens a JOIN memberships ms ON ms.user_id = a.user_id AND ms.studio_id = ?
         WHERE a.map_id = ? AND (a.id = ? OR a.name = ?) ORDER BY a.id = ? DESC LIMIT 1",
    )
    .bind(ctx.studio_id)
    .bind(ctx.map_id)
    .bind(v.as_i64().unwrap_or(-1))
    .bind(v.as_str().unwrap_or("\u{0}"))
    .bind(v.as_i64().unwrap_or(-1))
    .fetch_optional(&st.db)
    .await?;
    r.ok_or_else(|| {
        bad(format!(
            "Agent {v} introuvable sur cette map. Les agents de la map sont dans list_agents."
        ))
    })
}

/// Attribue (ou rend libre, `cible` = None) la tâche `tid` ; journalisé dans l'historique de la tâche.
async fn attribuer(
    st: &AppState,
    orch: i64,
    tid: i64,
    number: i64,
    cible: Option<(i64, String)>,
) -> Result<String, AppError> {
    let (status,): (String,) = sqlx::query_as("SELECT status FROM tasks WHERE id = ?")
        .bind(tid)
        .fetch_one(&st.db)
        .await?;
    if status == "done" {
        return Err(bad(format!("La tâche #{number} est déjà faite.")));
    }
    // Une tâche réservée par un autre agent reste à lui : on n'arrache pas le travail en cours.
    let pris: Option<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM api_tokens WHERE task_id = ?")
            .bind(tid)
            .fetch_optional(&st.db)
            .await?;
    if let Some((pid, pnom)) = pris {
        if cible.as_ref().map(|c| c.0) != Some(pid) {
            return Err(AppError::Conflict(format!(
                "La tâche #{number} est en cours chez l'agent « {pnom} ». Attendez qu'il la finisse ou la libère."
            )));
        }
    }
    sqlx::query("UPDATE tasks SET agent_id = ? WHERE id = ?")
        .bind(cible.as_ref().map(|c| c.0))
        .bind(tid)
        .execute(&st.db)
        .await?;
    let quoi = match &cible {
        Some((_, nom)) => format!("Attribuée à l'agent « {nom} » par l'Orchestrateur"),
        None => "Rendue libre par l'Orchestrateur".to_string(),
    };
    crate::agents::evenement(st, orch, tid, &quoi).await?;
    Ok(match cible {
        Some((_, nom)) => format!("Tâche #{number} attribuée à « {nom} »."),
        None => format!("Tâche #{number} libre : n'importe quel agent peut la prendre."),
    })
}

/// `assign_task { number, agent }` : `agent` = identifiant ou nom d'un agent de la map ; vide ou null = libre.
pub async fn assign(st: &AppState, ctx: &MapCtx, orch: i64, a: &Value) -> Result<Value, AppError> {
    ctx.can_edit()?;
    let number = a["number"]
        .as_i64()
        .ok_or_else(|| bad("Numéro de tâche manquant."))?;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM tasks WHERE map_id = ? AND number = ?")
        .bind(ctx.map_id)
        .bind(number)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(|| bad(format!("Tâche #{number} introuvable dans cette map.")))?;
    let cible = match &a["agent"] {
        Value::Null => None,
        Value::String(s) if s.trim().is_empty() => None,
        v => Some(agent_de_la_map(st, ctx, v).await?),
    };
    let message = attribuer(st, orch, tid, number, cible).await?;
    crate::access::notify(st, ctx.map_id, json!({ "t": "refresh", "what": "tasks" }));
    Ok(json!({ "number": number, "message": message }))
}

/// `plan_tasks { tasks: [{ key, title, description, tag, lot, uefn, depends_on, agent }] }` : crée les tâches dans
/// l'ordre. `depends_on` mêle numéros de tâches existantes et `key` d'une tâche plus haut dans le même plan.
/// Tout est vérifié avant de créer quoi que ce soit.
pub async fn plan(st: &AppState, ctx: &MapCtx, orch: i64, a: &Value) -> Result<Value, AppError> {
    ctx.can_edit()?;
    let items = a["tasks"]
        .as_array()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| bad("Liste de tâches manquante (tasks)."))?;
    if items.len() > 30 {
        return Err(bad(
            "30 tâches au plus par plan : découpez la demande en plusieurs plans.",
        ));
    }
    // Vérifications d'abord : titres, clés, dépendances (vers une tâche existante ou plus haut), agents.
    let mut cles: Vec<String> = vec![];
    let mut agents: Vec<Option<(i64, String)>> = vec![];
    for (i, t) in items.iter().enumerate() {
        let titre = t["title"].as_str().unwrap_or("").trim();
        if titre.is_empty() {
            return Err(bad(format!("Tâche {} du plan : titre manquant.", i + 1)));
        }
        for d in t["depends_on"].as_array().into_iter().flatten() {
            match d {
                Value::Number(n) => {
                    let ok: Option<i64> = sqlx::query_scalar(
                        "SELECT id FROM tasks WHERE map_id = ? AND number = ?",
                    )
                    .bind(ctx.map_id)
                    .bind(n.as_i64().unwrap_or(-1))
                    .fetch_optional(&st.db)
                    .await?;
                    if ok.is_none() {
                        return Err(bad(format!("« {titre} » : la tâche #{n} n'existe pas.")));
                    }
                }
                Value::String(k) if cles.contains(k) => {}
                other => {
                    return Err(bad(format!("« {titre} » : dépendance {other} inconnue. Donnez un numéro de tâche existante, ou la clé (key) d'une tâche placée plus haut dans le plan.")))
                }
            }
        }
        agents.push(match &t["agent"] {
            Value::Null => None,
            Value::String(s) if s.trim().is_empty() => None,
            v => Some(agent_de_la_map(st, ctx, v).await?),
        });
        let cle = t["key"].as_str().unwrap_or("").trim().to_string();
        if !cle.is_empty() && cles.contains(&cle) {
            return Err(bad(format!(
                "Clé « {cle} » utilisée deux fois dans le plan."
            )));
        }
        cles.push(cle);
    }
    // Création, dans l'ordre : une dépendance par clé vise une tâche déjà créée.
    let mut numeros: Vec<i64> = vec![];
    let mut faites = vec![];
    for (i, t) in items.iter().enumerate() {
        let deps: Vec<i64> = t["depends_on"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| match d {
                Value::Number(n) => n.as_i64(),
                Value::String(k) => cles.iter().position(|c| c == k).map(|p| numeros[p]),
                _ => None,
            })
            .collect();
        let b = crate::tasks::NewTask {
            title: t["title"].as_str().unwrap_or("").to_string(),
            tag: t["tag"].as_str().map(String::from),
            description: t["description"].as_str().map(String::from),
            assignee_id: None,
            deps: Some(deps),
            planned_day: None,
            lot: t["lot"]
                .as_str()
                .map(|l| l.trim().chars().take(60).collect()),
            status: None,
            ai: Some(true),
            origin: Some("ia".into()),
            uefn: t["uefn"].as_bool(),
        };
        let (tid, number) = crate::tasks::insert_task(st, ctx, &b).await?;
        numeros.push(number);
        let mut ligne = json!({ "number": number, "title": b.title.trim() });
        if let Some(c) = agents[i].clone() {
            ligne["agent"] = json!(c.1);
            attribuer(st, orch, tid, number, Some(c)).await?;
        }
        faites.push(ligne);
    }
    crate::access::notify(st, ctx.map_id, json!({ "t": "refresh", "what": "tasks" }));
    Ok(json!({
        "tasks": faites,
        "message": format!("{} tâche(s) créée(s). Chaque agent trouve les siennes avec next_task.", numeros.len()),
    }))
}

/// Agents de la map pour l'Orchestrateur : identifiant, nom, rôle, tâche en cours, actif ou non.
pub async fn liste(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let orch = de_la_map(st, mid).await?;
    let rows: Vec<(i64, String, String, Option<String>, Option<i64>, bool)> = sqlx::query_as(
        "SELECT a.id, a.name, a.kind, a.role, t.number, COALESCE(a.last_used, a.created_at) >= datetime('now', ?)
         FROM api_tokens a LEFT JOIN tasks t ON t.id = a.task_id WHERE a.map_id = ? ORDER BY a.id",
    )
    .bind(crate::agents::SILENCE)
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(json!(rows
        .into_iter()
        .map(|r| json!({
            "id": r.0, "name": r.1, "kind": r.2,
            "role": r.3.as_deref().and_then(crate::roles::get).map(|x| x.nom),
            "orchestrator": Some(r.0) == orch, "current_task": r.4, "active": r.5,
        }))
        .collect::<Vec<_>>()))
}
