//! Mise en route automatique (#136) : à la dernière réponse du parcours (#135), POP propose « On lance ? » : plan de
//! production du moteur local (gabarit par genre de `plan.rs`, sans IA externe), jalons, et premières tâches données
//! à l'IA qui travaille sur la map (elle les reçoit dans next_task, rubrique « attribuees »). Sur une map qui a déjà
//! des tâches, rien n'est créé sans que le créateur choisisse : ajouter seulement ce qui manque, ou ne rien faire.
use crate::{
    access::{map_ctx, MapCtx, Role},
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Nombre de premières tâches données à l'IA de la map.
const POUR_L_IA: usize = 3;

/// Gabarit de `plan.rs` d'après la réponse « genre » du parcours (texte libre compris).
pub fn gabarit(genre: &str) -> &'static str {
    let g = genre.to_lowercase();
    if g.contains("tycoon") {
        "tycoon"
    } else if g.contains("combat") || g.contains("pvp") || g.contains("box") {
        "boxfight"
    } else if g.contains("parkour") || g.contains("deathrun") {
        "deathrun"
    } else if g.contains("horreur") || g.contains("horror") {
        "horreur"
    } else if g.contains("course") || g.contains("voiture") || g.contains("racing") {
        "course"
    } else if g.contains("gendarme") || g.contains("voleur") || g.contains("braquage") {
        "cr"
    } else {
        ""
    }
}

/// Réponses du parcours (clé → réponse) ; None tant qu'une question de cadrage est sans réponse.
async fn reponses(
    st: &AppState,
    mid: i64,
) -> Result<Option<std::collections::HashMap<String, String>>, AppError> {
    crate::questions::liste(st, mid).await?;
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT cle, reponse FROM map_questions WHERE map_id = ? AND auteur = 'pop'",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    if rows.iter().any(|r| r.1.is_none()) {
        return Ok(None);
    }
    Ok(Some(
        rows.into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect(),
    ))
}

/// Ce qui sera créé : le résultat de l'aperçu du planificateur local, plus l'IA de la map et les tâches existantes.
async fn apercu(st: &AppState, ctx: &MapCtx) -> Result<Value, AppError> {
    let mid = ctx.map_id;
    let Some(r) = reponses(st, mid).await? else {
        return Err(bad("Répondez d'abord aux questions de cadrage."));
    };
    let genre = r.get("genre").cloned().unwrap_or_default();
    // Date de sortie : celle de la map si elle laisse le temps de planifier, sinon dans 8 semaines.
    let sortie: Option<String> = sqlx::query_scalar("SELECT release_date FROM maps WHERE id = ?")
        .bind(mid)
        .fetch_one(&st.db)
        .await?;
    let defaut: String = sqlx::query_scalar("SELECT date('now', '+56 days')")
        .fetch_one(&st.db)
        .await?;
    let assez_loin: Option<String> = match &sortie {
        Some(d) => {
            sqlx::query_scalar("SELECT ? WHERE date(?) > date('now', '+14 days')")
                .bind(d)
                .bind(d)
                .fetch_optional(&st.db)
                .await?
        }
        None => None,
    };
    let date = assez_loin.unwrap_or(defaut);
    // Équipe : seul avec son IA, ou les membres du studio qui peuvent modifier la map.
    let seul = r.get("equipe").is_some_and(|e| e.starts_with("Seul"));
    let membres: Vec<(i64, String, i64)> = if seul {
        vec![(ctx.user.id, String::new(), 10)]
    } else {
        sqlx::query_as("SELECT user_id, specialty, hours_per_week FROM memberships WHERE studio_id = ? AND role IN ('owner','admin','member','freelance')")
            .bind(ctx.studio_id)
            .fetch_all(&st.db)
            .await?
    };
    let agent: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM api_tokens WHERE map_id = ? ORDER BY last_used IS NULL, last_used DESC, id DESC LIMIT 1",
    )
    .bind(mid)
    .fetch_optional(&st.db)
    .await?;
    let gen: crate::plan::GenIn = serde_json::from_value(json!({
        "genre": gabarit(&genre),
        "members": membres.iter().map(|(id, spec, h)| json!({
            "user_id": id,
            "tag": if crate::tasks::TAGS.contains(&spec.as_str()) { spec.as_str() } else { "code" },
            "hours": (*h).max(1),
        })).collect::<Vec<_>>(),
        "ai": agent.is_some(),
        "release_date": date,
        "board_id": null,
    }))
    .map_err(|e| bad(format!("Plan impossible : {e}")))?;
    let Json(plan) =
        crate::plan::preview(State(st.clone()), ctx.user.clone(), Path(mid), Json(gen)).await?;
    let existantes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ?")
        .bind(mid)
        .fetch_one(&st.db)
        .await?;
    let titres: Vec<String> = sqlx::query_scalar("SELECT title FROM tasks WHERE map_id = ?")
        .bind(mid)
        .fetch_all(&st.db)
        .await?;
    let titres: Vec<String> = titres.iter().map(|t| t.to_lowercase()).collect();
    let manquantes = plan["tasks"]
        .as_array()
        .map(|t| {
            t.iter()
                .filter(|x| !titres.contains(&x["title"].as_str().unwrap_or("").to_lowercase()))
                .count()
        })
        .unwrap_or(0);
    Ok(json!({
        "genre": genre, "release_date": date, "plan": plan,
        "tasks": plan["tasks"].as_array().map(|t| t.len()).unwrap_or(0),
        "milestones": plan["milestones"].as_array().map(|t| t.len()).unwrap_or(0),
        "existing": existantes, "missing": manquantes,
        "agent": agent.map(|a| a.1), "for_ai": POUR_L_IA,
    }))
}

/// `GET /api/maps/{mid}/lancement` : résumé de ce que « Lancer » créera (ou `ready: false` si le parcours n'est
/// pas fini), et `launched` si la mise en route a déjà été faite.
pub async fn route_apercu(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let lance: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ? AND origin = 'parcours'")
            .bind(mid)
            .fetch_one(&st.db)
            .await?;
    if reponses(&st, mid).await?.is_none() {
        return Ok(Json(json!({ "ready": false, "launched": lance > 0 })));
    }
    if ctx.role < Role::Member {
        return Ok(Json(
            json!({ "ready": true, "launched": lance > 0, "can_launch": false }),
        ));
    }
    let mut a = apercu(&st, &ctx).await?;
    a["ready"] = json!(true);
    a["launched"] = json!(lance > 0);
    a["can_launch"] = json!(true);
    Ok(Json(a))
}

#[derive(Deserialize)]
pub struct Lancer {
    /// « tout » sur une map sans tâche ; « manquantes » pour n'ajouter que ce qui n'existe pas encore.
    mode: Option<String>,
}

pub async fn lancer(st: &AppState, ctx: &MapCtx, mode: &str) -> Result<Value, AppError> {
    ctx.require(Role::Member)?;
    let a = apercu(st, ctx).await?;
    let existantes = a["existing"].as_i64().unwrap_or(0);
    if existantes > 0 && mode != "manquantes" {
        return Err(AppError::Conflict(format!(
            "Cette map a déjà {existantes} tâche(s) : choisissez « Ajouter seulement ce qui manque », ou ne lancez pas."
        )));
    }
    let titres: Vec<String> = sqlx::query_scalar("SELECT title FROM tasks WHERE map_id = ?")
        .bind(ctx.map_id)
        .fetch_all(&st.db)
        .await?;
    let jalons: Vec<String> = sqlx::query_scalar("SELECT title FROM milestones WHERE map_id = ?")
        .bind(ctx.map_id)
        .fetch_all(&st.db)
        .await?;
    let titres: Vec<String> = titres.iter().map(|t| t.to_lowercase()).collect();
    let jalons: Vec<String> = jalons.iter().map(|t| t.to_lowercase()).collect();
    let taches: Vec<Value> = a["plan"]["tasks"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|t| !titres.contains(&t["title"].as_str().unwrap_or("").to_lowercase()))
        .collect();
    let milestones: Vec<Value> = a["plan"]["milestones"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|m| !jalons.contains(&m["title"].as_str().unwrap_or("").to_lowercase()))
        .collect();
    if taches.is_empty() && milestones.is_empty() {
        return Ok(
            json!({ "created": 0, "milestones": 0, "for_ai": [], "message": "Rien à ajouter : tout existe déjà." }),
        );
    }
    let avant: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) FROM tasks WHERE map_id = ?")
        .bind(ctx.map_id)
        .fetch_one(&st.db)
        .await?;
    let n_jalons = milestones.len();
    let b: crate::plan::ApplyIn =
        serde_json::from_value(json!({ "tasks": taches, "milestones": milestones }))
            .map_err(|e| bad(format!("Plan illisible : {e}")))?;
    crate::plan::apply(
        State(st.clone()),
        ctx.user.clone(),
        Path(ctx.map_id),
        Json(b),
    )
    .await?;
    sqlx::query("UPDATE tasks SET origin = 'parcours' WHERE map_id = ? AND id > ?")
        .bind(ctx.map_id)
        .bind(avant)
        .execute(&st.db)
        .await?;
    let crees: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ? AND id > ?")
        .bind(ctx.map_id)
        .bind(avant)
        .fetch_one(&st.db)
        .await?;
    // Premières tâches de l'IA de la map : celles marquées pour une IA, puis le code, sans dépendance à attendre.
    let mut pour_ia = vec![];
    let agent: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM api_tokens WHERE map_id = ? ORDER BY last_used IS NULL, last_used DESC, id DESC LIMIT 1",
    )
    .bind(ctx.map_id)
    .fetch_optional(&st.db)
    .await?;
    if let Some(ag) = agent {
        let libres: Vec<(i64, i64, String)> = sqlx::query_as(
            "SELECT t.id, t.number, t.title FROM tasks t WHERE t.map_id = ? AND t.id > ? AND t.agent_id IS NULL
               AND NOT EXISTS (SELECT 1 FROM task_deps d WHERE d.task_id = t.id)
             ORDER BY t.ai DESC, t.tag = 'code' DESC, t.number LIMIT ?",
        )
        .bind(ctx.map_id)
        .bind(avant)
        .bind(POUR_L_IA as i64)
        .fetch_all(&st.db)
        .await?;
        for (id, n, titre) in libres {
            sqlx::query("UPDATE tasks SET agent_id = ? WHERE id = ?")
                .bind(ag)
                .bind(id)
                .execute(&st.db)
                .await?;
            pour_ia.push(json!({ "number": n, "title": titre }));
        }
    }
    crate::access::log_activity(
        st,
        ctx.map_id,
        Some(ctx.user.id),
        "plan",
        "Production lancée",
        &format!("{crees} tâches et {n_jalons} jalons créés depuis le parcours ; {} tâche(s) données à l'IA", pour_ia.len()),
    )
    .await?;
    crate::access::notify(st, ctx.map_id, json!({ "t": "refresh", "what": "tasks" }));
    crate::access::notify(st, ctx.map_id, json!({ "t": "refresh", "what": "plan" }));
    Ok(json!({ "created": crees, "milestones": n_jalons, "for_ai": pour_ia }))
}

/// `POST /api/maps/{mid}/lancement`
pub async fn route_lancer(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<Lancer>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    Ok(Json(
        lancer(&st, &ctx, b.mode.as_deref().unwrap_or("tout")).await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genres() {
        assert_eq!(gabarit("Tycoon"), "tycoon");
        assert_eq!(gabarit("Combat (PvP)"), "boxfight");
        assert_eq!(gabarit("Parkour / Deathrun"), "deathrun");
        assert_eq!(gabarit("Course de voitures"), "course");
        assert_eq!(gabarit("Simulateur / Roleplay"), "");
    }

    #[tokio::test]
    async fn lancer_depuis_le_parcours() {
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
        let uid = q("INSERT INTO users (email, name, password_hash) VALUES ('l@a.fr', 'L', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        let ag: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, map_id, name, token_hash, kind, last_used) VALUES (?, ?, 'Claude', 'h', 'claude_code', datetime('now')) RETURNING id").bind(uid).bind(mid).fetch_one(&st.db).await.unwrap();
        let u = AuthUser {
            id: uid,
            email: "l@a.fr".into(),
            name: "L".into(),
            sid: String::new(),
        };
        let ctx = map_ctx(&st, &u, mid).await.unwrap();

        // Parcours pas fini : rien.
        let Json(a) = route_apercu(State(st.clone()), u.clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(a["ready"], false);
        assert!(lancer(&st, &ctx, "tout").await.is_err());
        for x in crate::questions::liste(&st, mid).await.unwrap() {
            let rep = x["choices"][0].as_str().unwrap().to_string();
            crate::questions::repondre(&st, &ctx, x["id"].as_i64().unwrap(), &rep)
                .await
                .unwrap();
        }
        let Json(a) = route_apercu(State(st.clone()), u.clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(a["ready"], true, "{a}");
        assert_eq!(a["agent"], "Claude");
        let n = a["tasks"].as_u64().unwrap();
        assert!(n > 10 && a["milestones"].as_u64().unwrap() == 4, "{a}");

        let r = lancer(&st, &ctx, "tout").await.unwrap();
        assert_eq!(r["created"].as_u64().unwrap(), n);
        assert_eq!(r["milestones"], 4);
        assert_eq!(r["for_ai"].as_array().unwrap().len(), POUR_L_IA);
        let tycoon: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE map_id = ? AND title = 'Générateur de ressource'",
        )
        .bind(mid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        assert_eq!(tycoon, 1, "gabarit du genre répondu");
        // L'IA reçoit ses premières tâches dans next_task.
        let nt = crate::agents::pour_next_task(&st, ag, mid).await.unwrap();
        assert_eq!(
            nt["attribuees"].as_array().unwrap().len(),
            POUR_L_IA,
            "{nt}"
        );

        // Relancer : refus sans choix, puis « manquantes » ne crée aucun doublon.
        let Json(a) = route_apercu(State(st.clone()), u.clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(a["launched"], true);
        assert_eq!(a["missing"], 0);
        assert!(matches!(
            lancer(&st, &ctx, "tout").await,
            Err(AppError::Conflict(_))
        ));
        let r = lancer(&st, &ctx, "manquantes").await.unwrap();
        assert_eq!(r["created"], 0);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(total as u64, n);
        let j: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM milestones WHERE map_id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(j, 4);
    }
}
