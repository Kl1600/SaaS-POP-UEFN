//! Scénarios de test rejouables (#348) : une liste d'étapes (action, résultat attendu) joués par des humains, un
//! verdict par étape (réussi, échoué, non testé) avec commentaire et capture, à chaque version ; on voit ce qui a
//! régressé, et une étape échouée devient une tâche POP en un clic. Une IA peut proposer un scénario (brouillon) :
//! il ne devient jouable qu'une fois validé par un humain.
use crate::{
    access::{log_activity, map_ctx, notify, MapCtx},
    auth::{random_token, AuthUser},
    error::{bad, check_len, ApiResult, AppError},
    s3::S3,
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

const VERDICTS: [&str; 3] = ["pass", "fail", "skip"];

/// Étapes envoyées : `[{ id?, action, expected }]`, de 1 à 50.
fn check_steps(v: &Value) -> Result<Vec<(Option<i64>, String, String)>, AppError> {
    let a = v.as_array().cloned().unwrap_or_default();
    if a.is_empty() || a.len() > 50 {
        return Err(bad("Un scénario a de 1 à 50 étapes."));
    }
    a.iter()
        .enumerate()
        .map(|(i, s)| {
            let action = s["action"].as_str().unwrap_or("").trim().to_string();
            let expected = s["expected"].as_str().unwrap_or("").trim().to_string();
            check_len(&format!("Étape {} : action", i + 1), &action, 1, 300)?;
            check_len(
                &format!("Étape {} : résultat attendu", i + 1),
                &expected,
                0,
                300,
            )?;
            Ok((s["id"].as_i64(), action, expected))
        })
        .collect()
}

async fn ecrire_etapes(
    st: &AppState,
    sid: i64,
    steps: &[(Option<i64>, String, String)],
) -> Result<(), AppError> {
    let mut tx = st.db.begin().await?;
    let anciens: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM scenario_steps WHERE scenario_id = ?")
            .bind(sid)
            .fetch_all(&mut *tx)
            .await?;
    let mut gardes = vec![];
    for (pos, (id, action, expected)) in steps.iter().enumerate() {
        match id.filter(|i| anciens.contains(i)) {
            Some(i) => {
                sqlx::query(
                    "UPDATE scenario_steps SET pos = ?, action = ?, expected = ? WHERE id = ?",
                )
                .bind(pos as i64)
                .bind(action)
                .bind(expected)
                .bind(i)
                .execute(&mut *tx)
                .await?;
                gardes.push(i);
            }
            None => {
                sqlx::query("INSERT INTO scenario_steps (scenario_id, pos, action, expected) VALUES (?, ?, ?, ?)").bind(sid).bind(pos as i64).bind(action).bind(expected).execute(&mut *tx).await?;
            }
        }
    }
    for i in anciens.into_iter().filter(|i| !gardes.contains(i)) {
        sqlx::query("DELETE FROM scenario_steps WHERE id = ?")
            .bind(i)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE scenarios SET updated_at = datetime('now') WHERE id = ?")
        .bind(sid)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Crée un scénario ; `agent` = proposé par une IA, donc brouillon à valider.
pub async fn creer(
    st: &AppState,
    ctx: &MapCtx,
    title: &str,
    steps: &Value,
    agent: Option<i64>,
) -> Result<i64, AppError> {
    ctx.can_edit()?;
    let title = title.trim();
    check_len("Titre", title, 1, 140)?;
    let steps = check_steps(steps)?;
    let sid: i64 = sqlx::query_scalar("INSERT INTO scenarios (map_id, title, status, agent_id, created_by) VALUES (?, ?, ?, ?, ?) RETURNING id")
        .bind(ctx.map_id)
        .bind(title)
        .bind(if agent.is_some() { "draft" } else { "ready" })
        .bind(agent)
        .bind(ctx.user.id)
        .fetch_one(&st.db)
        .await?;
    ecrire_etapes(st, sid, &steps).await?;
    let quoi = if agent.is_some() {
        "Scénario proposé par une IA (à valider)"
    } else {
        "Scénario de test créé"
    };
    log_activity(st, ctx.map_id, Some(ctx.user.id), "scenario", quoi, title).await?;
    notify(
        st,
        ctx.map_id,
        json!({ "t": "refresh", "what": "scenarios" }),
    );
    Ok(sid)
}

/// Les scénarios de la map, avec leurs étapes et le résumé du dernier passage.
pub async fn liste(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let sc: Vec<(i64, String, String, Option<i64>, String)> = sqlx::query_as("SELECT id, title, status, agent_id, updated_at FROM scenarios WHERE map_id = ? ORDER BY id DESC").bind(mid).fetch_all(&st.db).await?;
    let mut out = vec![];
    for (id, title, status, agent, at) in sc {
        let steps: Vec<(i64, String, String)> = sqlx::query_as(
            "SELECT id, action, expected FROM scenario_steps WHERE scenario_id = ? ORDER BY pos",
        )
        .bind(id)
        .fetch_all(&st.db)
        .await?;
        let p = passages(st, id).await?;
        let dernier = p["runs"].as_array().and_then(|r| r.first()).map(|r| json!({ "id": r["id"], "version": r["version"], "at": r["created_at"], "counts": r["counts"], "regressions": r["regressions"].as_array().map(|x| x.len()).unwrap_or(0) }));
        out.push(json!({ "id": id, "title": title, "status": status, "proposed_by_ai": agent.is_some(), "updated_at": at,
            "steps": steps.into_iter().map(|(i, a, e)| json!({ "id": i, "action": a, "expected": e })).collect::<Vec<_>>(), "last_run": dernier }));
    }
    Ok(json!(out))
}

/// Passages d'un scénario, du plus récent au plus ancien ; régression = étape réussie au passage précédent et
/// échouée à celui-ci.
pub async fn passages(st: &AppState, sid: i64) -> Result<Value, AppError> {
    let runs: Vec<(i64, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT r.id, r.version, u.name, r.created_at, r.finished_at FROM scenario_runs r LEFT JOIN users u ON u.id = r.tester_id WHERE r.scenario_id = ? ORDER BY r.id",
    )
    .bind(sid)
    .fetch_all(&st.db)
    .await?;
    let s3 = S3::from_env();
    let mut prec: HashMap<i64, String> = HashMap::new();
    let mut out = vec![];
    for (id, version, who, at, fin) in runs {
        let res: Vec<(i64, String, String, Option<String>, Option<i64>)> = sqlx::query_as(
            "SELECT r.step_id, r.verdict, r.comment, r.capture_key, t.number FROM scenario_results r LEFT JOIN tasks t ON t.id = r.task_id WHERE r.run_id = ?",
        )
        .bind(id)
        .fetch_all(&st.db)
        .await?;
        let regressions: Vec<i64> = res
            .iter()
            .filter(|r| r.1 == "fail" && prec.get(&r.0).map(String::as_str) == Some("pass"))
            .map(|r| r.0)
            .collect();
        let compte = |v: &str| res.iter().filter(|r| r.1 == v).count();
        let counts =
            json!({ "pass": compte("pass"), "fail": compte("fail"), "skip": compte("skip") });
        let results: serde_json::Map<String, Value> = res
            .iter()
            .map(|(step, v, c, k, t)| {
                let capture = k.as_deref().and_then(|k| {
                    s3.as_ref()
                        .map(|s| s.presign("GET", k, 3600, chrono::Utc::now()))
                });
                (
                    step.to_string(),
                    json!({ "verdict": v, "comment": c, "capture": capture, "task": t }),
                )
            })
            .collect();
        for (step, v, ..) in &res {
            prec.insert(*step, v.clone());
        }
        out.push(json!({ "id": id, "version": version, "tester": who, "created_at": at, "finished_at": fin, "counts": counts, "regressions": regressions, "results": results }));
    }
    out.reverse();
    Ok(json!({ "runs": out }))
}

async fn ctx_scenario(
    st: &AppState,
    u: &AuthUser,
    sid: i64,
) -> Result<(MapCtx, String, String), AppError> {
    let (mid, status, title): (i64, String, String) =
        sqlx::query_as("SELECT map_id, status, title FROM scenarios WHERE id = ?")
            .bind(sid)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    Ok((map_ctx(st, u, mid).await?, status, title))
}

async fn ctx_run(st: &AppState, u: &AuthUser, rid: i64) -> Result<(MapCtx, i64, i64), AppError> {
    let (sid, tester): (i64, Option<i64>) =
        sqlx::query_as("SELECT scenario_id, tester_id FROM scenario_runs WHERE id = ?")
            .bind(rid)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    let (ctx, ..) = ctx_scenario(st, u, sid).await?;
    Ok((ctx, sid, tester.unwrap_or(0)))
}

#[derive(Deserialize)]
pub struct ScenarioIn {
    title: String,
    steps: Value,
}

/// `GET /api/maps/{mid}/scenarios`
pub async fn route_liste(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    Ok(Json(liste(&st, mid).await?))
}

/// `POST /api/maps/{mid}/scenarios`
pub async fn route_creer(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<ScenarioIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    Ok(Json(
        json!({ "id": creer(&st, &ctx, &b.title, &b.steps, None).await? }),
    ))
}

/// `PUT /api/scenarios/{sid}` : titre et étapes (les étapes gardent leur identifiant, donc leur historique).
pub async fn route_modifier(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<ScenarioIn>,
) -> ApiResult<Value> {
    let (ctx, ..) = ctx_scenario(&st, &u, sid).await?;
    ctx.can_edit()?;
    check_len("Titre", b.title.trim(), 1, 140)?;
    let steps = check_steps(&b.steps)?;
    sqlx::query("UPDATE scenarios SET title = ? WHERE id = ?")
        .bind(b.title.trim())
        .bind(sid)
        .execute(&st.db)
        .await?;
    ecrire_etapes(&st, sid, &steps).await?;
    notify(
        &st,
        ctx.map_id,
        json!({ "t": "refresh", "what": "scenarios" }),
    );
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/scenarios/{sid}/validate` : un humain valide un brouillon proposé par une IA.
pub async fn route_valider(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    let (ctx, _, title) = ctx_scenario(&st, &u, sid).await?;
    ctx.can_edit()?;
    sqlx::query("UPDATE scenarios SET status = 'ready' WHERE id = ?")
        .bind(sid)
        .execute(&st.db)
        .await?;
    log_activity(
        &st,
        ctx.map_id,
        Some(u.id),
        "scenario",
        "Scénario validé",
        &title,
    )
    .await?;
    notify(
        &st,
        ctx.map_id,
        json!({ "t": "refresh", "what": "scenarios" }),
    );
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/scenarios/{sid}`
pub async fn route_supprimer(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    let (ctx, _, title) = ctx_scenario(&st, &u, sid).await?;
    ctx.can_edit()?;
    sqlx::query("DELETE FROM scenarios WHERE id = ?")
        .bind(sid)
        .execute(&st.db)
        .await?;
    log_activity(
        &st,
        ctx.map_id,
        Some(u.id),
        "scenario",
        "Scénario supprimé",
        &title,
    )
    .await?;
    notify(
        &st,
        ctx.map_id,
        json!({ "t": "refresh", "what": "scenarios" }),
    );
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/scenarios/{sid}/runs`
pub async fn route_passages(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    ctx_scenario(&st, &u, sid).await?;
    Ok(Json(passages(&st, sid).await?))
}

#[derive(Deserialize)]
pub struct RunIn {
    version: Option<String>,
}

/// `POST /api/scenarios/{sid}/runs` : commence un passage (scénario validé seulement).
pub async fn route_jouer(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<RunIn>,
) -> ApiResult<Value> {
    let (ctx, status, _) = ctx_scenario(&st, &u, sid).await?;
    ctx.can_edit()?;
    if status != "ready" {
        return Err(bad(
            "Ce scénario a été proposé par une IA : validez-le avant de le jouer.",
        ));
    }
    let version = b.version.unwrap_or_default().trim().to_string();
    check_len("Version", &version, 0, 60)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO scenario_runs (scenario_id, version, tester_id) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(sid)
    .bind(&version)
    .bind(u.id)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
pub struct ResultIn {
    verdict: String,
    comment: Option<String>,
    capture_key: Option<String>,
}

/// `PUT /api/scenario-runs/{rid}/results/{step}` : verdict d'une étape (par celui qui joue le passage).
pub async fn route_verdict(
    State(st): State<AppState>,
    u: AuthUser,
    Path((rid, step)): Path<(i64, i64)>,
    Json(b): Json<ResultIn>,
) -> ApiResult<Value> {
    let (ctx, sid, tester) = ctx_run(&st, &u, rid).await?;
    ctx.can_edit()?;
    if tester != u.id {
        return Err(bad(
            "Seule la personne qui joue ce passage donne les verdicts.",
        ));
    }
    let ok: Option<i64> =
        sqlx::query_scalar("SELECT id FROM scenario_steps WHERE id = ? AND scenario_id = ?")
            .bind(step)
            .bind(sid)
            .fetch_optional(&st.db)
            .await?;
    ok.ok_or(AppError::NotFound)?;
    if !VERDICTS.contains(&b.verdict.as_str()) {
        return Err(bad("Verdict : réussi, échoué ou non testé."));
    }
    let comment = b.comment.unwrap_or_default();
    check_len("Commentaire", &comment, 0, 1000)?;
    let prefixe = format!("scenarios/{}/{rid}/{step}-", ctx.map_id);
    if b.capture_key
        .as_deref()
        .is_some_and(|k| !k.starts_with(&prefixe) || k.len() > 200)
    {
        return Err(bad("Capture inconnue."));
    }
    sqlx::query(
        "INSERT INTO scenario_results (run_id, step_id, verdict, comment, capture_key) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (run_id, step_id) DO UPDATE SET verdict = excluded.verdict, comment = excluded.comment, capture_key = COALESCE(excluded.capture_key, scenario_results.capture_key)",
    )
    .bind(rid)
    .bind(step)
    .bind(&b.verdict)
    .bind(&comment)
    .bind(&b.capture_key)
    .execute(&st.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/scenario-runs/{rid}/finish`
pub async fn route_finir(
    State(st): State<AppState>,
    u: AuthUser,
    Path(rid): Path<i64>,
) -> ApiResult<Value> {
    let (ctx, sid, tester) = ctx_run(&st, &u, rid).await?;
    if tester != u.id {
        return Err(AppError::Forbidden);
    }
    sqlx::query("UPDATE scenario_runs SET finished_at = datetime('now') WHERE id = ? AND finished_at IS NULL").bind(rid).execute(&st.db).await?;
    let p = passages(&st, sid).await?;
    let r = p["runs"]
        .as_array()
        .and_then(|r| r.iter().find(|x| x["id"] == rid))
        .cloned()
        .unwrap_or(Value::Null);
    let title: String = sqlx::query_scalar("SELECT title FROM scenarios WHERE id = ?")
        .bind(sid)
        .fetch_one(&st.db)
        .await?;
    let detail = format!(
        "{} · {} réussie(s), {} échouée(s), {} régression(s)",
        title,
        r["counts"]["pass"],
        r["counts"]["fail"],
        r["regressions"].as_array().map(|x| x.len()).unwrap_or(0)
    );
    log_activity(
        &st,
        ctx.map_id,
        Some(u.id),
        "scenario",
        "Scénario joué",
        &detail,
    )
    .await?;
    notify(
        &st,
        ctx.map_id,
        json!({ "t": "refresh", "what": "scenarios" }),
    );
    Ok(Json(r))
}

#[derive(Deserialize)]
pub struct CaptureIn {
    step: i64,
}

/// `POST /api/scenario-runs/{rid}/capture` : lien signé pour envoyer la capture d'une étape au stockage objet.
pub async fn route_capture(
    State(st): State<AppState>,
    u: AuthUser,
    Path(rid): Path<i64>,
    Json(b): Json<CaptureIn>,
) -> ApiResult<Value> {
    let (ctx, _, tester) = ctx_run(&st, &u, rid).await?;
    if tester != u.id {
        return Err(AppError::Forbidden);
    }
    let s3 = S3::from_env().ok_or_else(|| {
        bad("Stockage objet non configuré sur ce serveur POP : pas de capture possible.")
    })?;
    let key = format!(
        "scenarios/{}/{rid}/{}-{}.png",
        ctx.map_id,
        b.step,
        &random_token()[..16]
    );
    Ok(Json(
        json!({ "key": key, "url": s3.presign("PUT", &key, 600, chrono::Utc::now()) }),
    ))
}

/// `POST /api/scenario-runs/{rid}/results/{step}/task` : tâche POP pour une étape échouée.
pub async fn route_tache(
    State(st): State<AppState>,
    u: AuthUser,
    Path((rid, step)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    let (ctx, sid, _) = ctx_run(&st, &u, rid).await?;
    ctx.can_edit()?;
    let r: Option<(String, String, Option<i64>, String, String, String, i64)> = sqlx::query_as(
        "SELECT r.verdict, r.comment, r.task_id, s.action, s.expected, ru.version, s.pos FROM scenario_results r JOIN scenario_steps s ON s.id = r.step_id JOIN scenario_runs ru ON ru.id = r.run_id WHERE r.run_id = ? AND r.step_id = ?",
    )
    .bind(rid)
    .bind(step)
    .fetch_optional(&st.db)
    .await?;
    let (verdict, comment, deja, action, expected, version, pos) = r.ok_or(AppError::NotFound)?;
    if verdict != "fail" {
        return Err(bad("Seule une étape échouée devient une tâche."));
    }
    if let Some(t) = deja {
        let n: i64 = sqlx::query_scalar("SELECT number FROM tasks WHERE id = ?")
            .bind(t)
            .fetch_one(&st.db)
            .await?;
        return Ok(Json(json!({ "number": n, "existing": true })));
    }
    let title: String = sqlx::query_scalar("SELECT title FROM scenarios WHERE id = ?")
        .bind(sid)
        .fetch_one(&st.db)
        .await?;
    let v = if version.is_empty() {
        String::new()
    } else {
        format!(" (version {version})")
    };
    let titre: String = format!("Test : « {title} », étape {} échouée{v}", pos + 1)
        .chars()
        .take(140)
        .collect();
    let description = format!(
        "Scénario de test « {title} », passage n° {rid}{v}.\n\nÉtape {} : {action}\nRésultat attendu : {}\nConstat : {}\n\nCapture et historique : Lancer et suivre → Test → Scénarios.",
        pos + 1,
        if expected.is_empty() { "—" } else { &expected },
        if comment.is_empty() { "—" } else { &comment }
    );
    let nt = crate::tasks::NewTask {
        title: titre,
        tag: Some("test".into()),
        description: Some(description),
        assignee_id: None,
        deps: None,
        planned_day: None,
        lot: None,
        status: None,
        ai: None,
        origin: Some("scenario".into()),
        uefn: None,
    };
    let (id, number) = crate::tasks::insert_task(&st, &ctx, &nt).await?;
    sqlx::query("UPDATE scenario_results SET task_id = ? WHERE run_id = ? AND step_id = ?")
        .bind(id)
        .bind(rid)
        .bind(step)
        .execute(&st.db)
        .await?;
    notify(&st, ctx.map_id, json!({ "t": "refresh", "what": "tasks" }));
    Ok(Json(json!({ "number": number, "existing": false })))
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    use super::*;

    async fn compte(st: &AppState, sid: i64, email: &str) -> AuthUser {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
        )
        .bind(email)
        .bind(email)
        .fetch_one(&st.db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')")
            .bind(sid)
            .bind(id)
            .execute(&st.db)
            .await
            .unwrap();
        AuthUser {
            id,
            email: email.into(),
            name: email.into(),
            sid: String::new(),
        }
    }

    #[tokio::test]
    async fn ecrire_jouer_comparer() {
        let st = crate::test_state().await;
        let s1: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('A') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let s2: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('B') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id")
                .bind(s1)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let moi = compte(&st, s1, "moi@a.fr").await;
        let autre = compte(&st, s2, "autre@b.fr").await;
        let etat = || State(st.clone());
        let etapes = json!([{ "action": "Va au spawn", "expected": "On apparaît" }, { "action": "Achète l'arme", "expected": "L'arme est dans l'inventaire" }, { "action": "Gagne une manche", "expected": "Écran de victoire" }]);
        // Validation : pas d'étape, ou une action vide → refus.
        assert!(route_creer(
            etat(),
            moi.clone(),
            Path(mid),
            Json(ScenarioIn {
                title: "Boutique".into(),
                steps: json!([])
            })
        )
        .await
        .is_err());
        assert!(route_creer(
            etat(),
            moi.clone(),
            Path(mid),
            Json(ScenarioIn {
                title: "Boutique".into(),
                steps: json!([{ "action": " " }])
            })
        )
        .await
        .is_err());
        let sid = route_creer(
            etat(),
            moi.clone(),
            Path(mid),
            Json(ScenarioIn {
                title: "Boutique".into(),
                steps: etapes,
            }),
        )
        .await
        .unwrap()
        .0["id"]
            .as_i64()
            .unwrap();
        let l = liste(&st, mid).await.unwrap();
        let ids: Vec<i64> = l[0]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_i64().unwrap())
            .collect();
        assert_eq!(ids.len(), 3);
        // Modification : l'étape 2 garde son identifiant (donc son historique), une étape est ajoutée.
        let modif = json!([{ "id": ids[0], "action": "Va au spawn", "expected": "On apparaît" }, { "id": ids[1], "action": "Achète l'arme (500 or)", "expected": "L'arme est dans l'inventaire" }, { "id": ids[2], "action": "Gagne une manche" }, { "action": "Quitte la partie" }]);
        route_modifier(
            etat(),
            moi.clone(),
            Path(sid),
            Json(ScenarioIn {
                title: "Boutique v2".into(),
                steps: modif,
            }),
        )
        .await
        .unwrap();
        let l = liste(&st, mid).await.unwrap();
        assert_eq!(l[0]["steps"][1]["id"].as_i64().unwrap(), ids[1]);
        assert_eq!(l[0]["steps"].as_array().unwrap().len(), 4);
        // Passage 1 (version 1.0) : tout réussi ; passage 2 (1.1) : l'étape 2 échoue → régression.
        let jouer = |v: &str| {
            route_jouer(
                etat(),
                moi.clone(),
                Path(sid),
                Json(RunIn {
                    version: Some(v.into()),
                }),
            )
        };
        let r1 = jouer("1.0").await.unwrap().0["id"].as_i64().unwrap();
        for s in &ids {
            route_verdict(
                etat(),
                moi.clone(),
                Path((r1, *s)),
                Json(ResultIn {
                    verdict: "pass".into(),
                    comment: None,
                    capture_key: None,
                }),
            )
            .await
            .unwrap();
        }
        route_finir(etat(), moi.clone(), Path(r1)).await.unwrap();
        let r2 = jouer("1.1").await.unwrap().0["id"].as_i64().unwrap();
        assert!(route_verdict(
            etat(),
            moi.clone(),
            Path((r2, ids[1])),
            Json(ResultIn {
                verdict: "oups".into(),
                comment: None,
                capture_key: None
            })
        )
        .await
        .is_err());
        assert!(
            route_verdict(
                etat(),
                moi.clone(),
                Path((r2, ids[1])),
                Json(ResultIn {
                    verdict: "fail".into(),
                    comment: None,
                    capture_key: Some("autre/clef.png".into())
                })
            )
            .await
            .is_err(),
            "capture d'ailleurs refusée"
        );
        route_verdict(
            etat(),
            moi.clone(),
            Path((r2, ids[0])),
            Json(ResultIn {
                verdict: "pass".into(),
                comment: None,
                capture_key: None,
            }),
        )
        .await
        .unwrap();
        let cle = format!("scenarios/{mid}/{r2}/{}-abc.png", ids[1]);
        route_verdict(
            etat(),
            moi.clone(),
            Path((r2, ids[1])),
            Json(ResultIn {
                verdict: "fail".into(),
                comment: Some("Le prix ne baisse pas".into()),
                capture_key: Some(cle),
            }),
        )
        .await
        .unwrap();
        route_verdict(
            etat(),
            moi.clone(),
            Path((r2, ids[2])),
            Json(ResultIn {
                verdict: "skip".into(),
                comment: None,
                capture_key: None,
            }),
        )
        .await
        .unwrap();
        let fin = route_finir(etat(), moi.clone(), Path(r2)).await.unwrap().0;
        assert_eq!(fin["regressions"], json!([ids[1]]));
        assert_eq!(fin["counts"], json!({ "pass": 1, "fail": 1, "skip": 1 }));
        assert_eq!(
            liste(&st, mid).await.unwrap()[0]["last_run"]["regressions"],
            1
        );
        // Étape échouée → tâche POP (une seule, même en recliquant) ; une étape réussie → refus.
        assert!(route_tache(etat(), moi.clone(), Path((r2, ids[0])))
            .await
            .is_err());
        let t = route_tache(etat(), moi.clone(), Path((r2, ids[1])))
            .await
            .unwrap()
            .0;
        let (titre, desc, tag): (String, String, String) = sqlx::query_as(
            "SELECT title, description, tag FROM tasks WHERE map_id = ? AND number = ?",
        )
        .bind(mid)
        .bind(t["number"].as_i64().unwrap())
        .fetch_one(&st.db)
        .await
        .unwrap();
        assert_eq!(
            titre,
            "Test : « Boutique v2 », étape 2 échouée (version 1.1)"
        );
        assert!(desc.contains("Le prix ne baisse pas") && desc.contains("500 or"));
        assert_eq!(tag, "test");
        assert_eq!(
            route_tache(etat(), moi.clone(), Path((r2, ids[1])))
                .await
                .unwrap()
                .0["existing"],
            true
        );
        assert_eq!(
            passages(&st, sid).await.unwrap()["runs"][0]["results"][ids[1].to_string()]["task"],
            t["number"]
        );
        // Un membre d'un autre studio ne voit ni ne modifie rien.
        assert!(route_liste(etat(), autre.clone(), Path(mid)).await.is_err());
        assert!(route_passages(etat(), autre.clone(), Path(sid))
            .await
            .is_err());
        assert!(route_supprimer(etat(), autre.clone(), Path(sid))
            .await
            .is_err());
        assert!(route_jouer(
            etat(),
            autre.clone(),
            Path(sid),
            Json(RunIn { version: None })
        )
        .await
        .is_err());
        assert!(route_verdict(
            etat(),
            autre.clone(),
            Path((r2, ids[0])),
            Json(ResultIn {
                verdict: "fail".into(),
                comment: None,
                capture_key: None
            })
        )
        .await
        .is_err());
        // Brouillon proposé par une IA : pas jouable avant validation par un humain.
        let ctx = map_ctx(&st, &moi, mid).await.unwrap();
        let ia = creer(
            &st,
            &ctx,
            "Proposé",
            &json!([{ "action": "Ouvre le coffre" }]),
            Some(7),
        )
        .await
        .unwrap();
        assert!(
            route_jouer(etat(), moi.clone(), Path(ia), Json(RunIn { version: None }))
                .await
                .is_err()
        );
        route_valider(etat(), moi.clone(), Path(ia)).await.unwrap();
        assert!(
            route_jouer(etat(), moi.clone(), Path(ia), Json(RunIn { version: None }))
                .await
                .is_ok()
        );
    }
}
