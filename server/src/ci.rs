//! CI GitHub Actions dans POP (#217, partie serveur) : le webhook GitHub du studio reçoit aussi `workflow_run` et
//! `workflow_job` ; un admin peut reprendre l'historique (50 derniers runs). Pas de doublon (identifiant GitHub) et
//! pas de retour en arrière (date de mise à jour du run, ordre des statuts d'un job). Une branche `issue/N-…` est
//! reliée à la tâche N de la map. 500 runs gardés par map.
use crate::{
    access::{map_ctx, studio_role, Role},
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

const GARDES: i64 = 500;

/// Rang d'un statut GitHub : un événement de rang plus bas qu'un statut déjà connu est ignoré.
fn rang(statut: &str) -> i32 {
    match statut {
        "requested" | "waiting" | "pending" | "queued" => 0,
        "in_progress" => 1,
        "completed" => 2,
        _ => 0,
    }
}

/// `issue/12-mon-sujet` → 12.
pub fn numero_de_branche(branche: &str) -> Option<i64> {
    let reste = branche.strip_prefix("issue/")?;
    reste.split(['-', '/']).next()?.parse().ok()
}

/// Enregistre (ou met à jour) un run. Rend l'id POP du run, ou None s'il était plus récent.
pub async fn run(
    st: &AppState,
    sid: i64,
    mid: Option<i64>,
    r: &Value,
) -> Result<Option<i64>, AppError> {
    let Some(gid) = r["id"].as_i64() else {
        return Ok(None);
    };
    let maj = r["updated_at"].as_str().unwrap_or("").to_string();
    let deja: Option<(i64, String)> =
        sqlx::query_as("SELECT id, updated_at FROM ci_runs WHERE studio_id = ? AND github_id = ?")
            .bind(sid)
            .bind(gid)
            .fetch_optional(&st.db)
            .await?;
    if let Some((id, avant)) = &deja {
        if !maj.is_empty() && maj < *avant {
            return Ok(Some(*id)); // événement plus ancien : le statut connu ne recule pas
        }
    }
    let branche = r["head_branch"].as_str().unwrap_or("");
    let tache: Option<i64> = match (mid, numero_de_branche(branche)) {
        (Some(m), Some(n)) => {
            sqlx::query_scalar("SELECT id FROM tasks WHERE map_id = ? AND number = ?")
                .bind(m)
                .bind(n)
                .fetch_optional(&st.db)
                .await?
        }
        _ => None,
    };
    let pr = r["pull_requests"]
        .as_array()
        .and_then(|p| p.first())
        .and_then(|p| p["number"].as_i64());
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO ci_runs (studio_id, map_id, task_id, github_id, workflow, branch, sha, pr, event, status, conclusion, started_at, updated_at, url)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (studio_id, github_id) DO UPDATE SET map_id = COALESCE(excluded.map_id, map_id), task_id = COALESCE(excluded.task_id, task_id),
           status = excluded.status, conclusion = excluded.conclusion, started_at = COALESCE(excluded.started_at, started_at),
           updated_at = excluded.updated_at, url = excluded.url, pr = COALESCE(excluded.pr, pr)
         RETURNING id",
    )
    .bind(sid)
    .bind(mid)
    .bind(tache)
    .bind(gid)
    .bind(r["name"].as_str().unwrap_or(""))
    .bind(branche)
    .bind(r["head_sha"].as_str().unwrap_or(""))
    .bind(pr)
    .bind(r["event"].as_str().unwrap_or(""))
    .bind(r["status"].as_str().unwrap_or(""))
    .bind(r["conclusion"].as_str())
    .bind(r["run_started_at"].as_str())
    .bind(&maj)
    .bind(r["html_url"].as_str().unwrap_or(""))
    .fetch_one(&st.db)
    .await?;
    if let Some(m) = mid {
        sqlx::query("DELETE FROM ci_runs WHERE map_id = ? AND id NOT IN (SELECT id FROM ci_runs WHERE map_id = ? ORDER BY github_id DESC LIMIT ?)")
            .bind(m)
            .bind(m)
            .bind(GARDES)
            .execute(&st.db)
            .await?;
    }
    Ok(Some(id))
}

/// Enregistre (ou met à jour) un job et ses étapes ; crée un run minimal s'il n'est pas encore connu.
pub async fn job(st: &AppState, sid: i64, mid: Option<i64>, j: &Value) -> Result<(), AppError> {
    let (Some(gid), Some(run_gid)) = (j["id"].as_i64(), j["run_id"].as_i64()) else {
        return Ok(());
    };
    let run_id: i64 = match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM ci_runs WHERE studio_id = ? AND github_id = ?",
    )
    .bind(sid)
    .bind(run_gid)
    .fetch_optional(&st.db)
    .await?
    {
        Some(id) => id,
        None => {
            let r = json!({ "id": run_gid, "name": j["workflow_name"], "head_branch": j["head_branch"], "head_sha": j["head_sha"], "status": "queued", "updated_at": "" });
            match run(st, sid, mid, &r).await? {
                Some(id) => id,
                None => return Ok(()),
            }
        }
    };
    let statut = j["status"].as_str().unwrap_or("");
    let avant: Option<String> =
        sqlx::query_scalar("SELECT status FROM ci_jobs WHERE github_id = ?")
            .bind(gid)
            .fetch_optional(&st.db)
            .await?;
    if avant.as_deref().is_some_and(|a| rang(a) > rang(statut)) {
        return Ok(());
    }
    let etapes: Vec<Value> = j["steps"]
        .as_array()
        .map(|s| s.iter().map(|x| json!({ "name": x["name"], "status": x["status"], "conclusion": x["conclusion"], "number": x["number"], "started_at": x["started_at"], "completed_at": x["completed_at"] })).collect())
        .unwrap_or_default();
    sqlx::query(
        "INSERT INTO ci_jobs (run_id, github_id, name, status, conclusion, started_at, completed_at, steps, url) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (github_id) DO UPDATE SET status = excluded.status, conclusion = excluded.conclusion, started_at = COALESCE(excluded.started_at, started_at),
           completed_at = COALESCE(excluded.completed_at, completed_at), steps = excluded.steps, url = excluded.url",
    )
    .bind(run_id)
    .bind(gid)
    .bind(j["name"].as_str().unwrap_or(""))
    .bind(statut)
    .bind(j["conclusion"].as_str())
    .bind(j["started_at"].as_str())
    .bind(j["completed_at"].as_str())
    .bind(json!(etapes).to_string())
    .bind(j["html_url"].as_str().unwrap_or(""))
    .execute(&st.db)
    .await?;
    Ok(())
}

/// Événement du webhook (signature et dépôt déjà vérifiés par `integrations::github_hook`).
pub async fn evenement(
    st: &AppState,
    sid: i64,
    cfg: &Value,
    quoi: &str,
    v: &Value,
) -> Result<Value, AppError> {
    let mid = map_de_l_integration(st, sid, cfg).await?;
    match quoi {
        "workflow_run" => {
            run(st, sid, mid, &v["workflow_run"]).await?;
        }
        "workflow_job" => job(st, sid, mid, &v["workflow_job"]).await?,
        _ => return Ok(json!({ "ignored": true })),
    }
    if let Some(m) = mid {
        crate::access::notify(st, m, json!({ "t": "refresh", "what": "ci" }));
    }
    Ok(json!({ "ok": true }))
}

/// Map de l'intégration : celle choisie à la dernière synchronisation, sinon la seule map du studio.
async fn map_de_l_integration(
    st: &AppState,
    sid: i64,
    cfg: &Value,
) -> Result<Option<i64>, AppError> {
    if let Some(m) = cfg["map"].as_i64() {
        let ok: Option<i64> =
            sqlx::query_scalar("SELECT id FROM maps WHERE id = ? AND studio_id = ?")
                .bind(m)
                .bind(sid)
                .fetch_optional(&st.db)
                .await?;
        if ok.is_some() {
            return Ok(ok);
        }
    }
    let maps: Vec<i64> = sqlx::query_scalar("SELECT id FROM maps WHERE studio_id = ?")
        .bind(sid)
        .fetch_all(&st.db)
        .await?;
    Ok((maps.len() == 1).then(|| maps[0]))
}

#[derive(Deserialize)]
pub struct MapQ {
    map: i64,
}

/// `POST /api/studios/{sid}/github/ci/sync?map=` : les 50 derniers runs et leurs jobs, par un admin du studio.
pub async fn synchroniser(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Query(q): Query<MapQ>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    let ok: Option<i64> = sqlx::query_scalar("SELECT id FROM maps WHERE id = ? AND studio_id = ?")
        .bind(q.map)
        .bind(sid)
        .fetch_optional(&st.db)
        .await?;
    ok.ok_or(AppError::NotFound)?;
    let mut cfg = crate::integrations::config(&st, sid, "github")
        .await?
        .ok_or_else(|| bad("Activez GitHub d’abord (Équipe et intégrations → GitHub)."))?;
    let repo = cfg["repo"]
        .as_str()
        .filter(|r| crate::integrations::repo_ok(r))
        .ok_or_else(|| bad("Dépôt GitHub invalide (format : propriétaire/nom)."))?
        .to_string();
    let token = cfg["token"]
        .as_str()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| bad("Jeton GitHub manquant."))?
        .to_string();
    // La map choisie devient celle de l'intégration : les événements du webhook y seront rangés.
    cfg["map"] = json!(q.map);
    crate::integrations::save_config(&st, sid, "github", &cfg).await?;
    let base = format!("https://api.github.com/repos/{repo}/actions");
    let refus = |s: reqwest::StatusCode| {
        match s.as_u16() {
        401 | 403 => bad("GitHub refuse l'accès aux Actions : ajoutez la permission « Actions : lecture » au jeton (GitHub → Settings → Developer settings → Fine-grained tokens)."),
        404 => bad("Dépôt GitHub introuvable : vérifiez son nom (propriétaire/nom) et que le jeton y a accès."),
        _ => bad(format!("GitHub a répondu {s}.")),
    }
    };
    let r = crate::integrations::gh(
        &st,
        reqwest::Method::GET,
        format!("{base}/runs?per_page=50"),
        &token,
    )
    .send()
    .await
    .map_err(|e| bad(format!("GitHub injoignable : {e}")))?;
    if !r.status().is_success() {
        return Err(refus(r.status()));
    }
    let v: Value = r.json().await.map_err(|e| bad(format!("GitHub : {e}")))?;
    let runs = v["workflow_runs"].as_array().cloned().unwrap_or_default();
    let mut jobs = 0;
    for x in &runs {
        run(&st, sid, Some(q.map), x).await?;
        let Some(gid) = x["id"].as_i64() else {
            continue;
        };
        let r = crate::integrations::gh(
            &st,
            reqwest::Method::GET,
            format!("{base}/runs/{gid}/jobs?per_page=100"),
            &token,
        )
        .send()
        .await
        .map_err(|e| bad(format!("GitHub injoignable : {e}")))?;
        if !r.status().is_success() {
            return Err(refus(r.status()));
        }
        let jv: Value = r.json().await.map_err(|e| bad(format!("GitHub : {e}")))?;
        for j in jv["jobs"].as_array().cloned().unwrap_or_default() {
            job(&st, sid, Some(q.map), &j).await?;
            jobs += 1;
        }
    }
    crate::access::notify(&st, q.map, json!({ "t": "refresh", "what": "ci" }));
    Ok(Json(json!({ "runs": runs.len(), "jobs": jobs })))
}

fn ligne_run(r: &LigneRun) -> Value {
    json!({ "id": r.0, "github_id": r.1, "workflow": r.2, "branch": r.3, "sha": r.4, "pr": r.5, "event": r.6, "status": r.7,
        "conclusion": r.8, "started_at": r.9, "updated_at": r.10, "url": r.11, "task": r.12 })
}

/// Une ligne de `ci_jobs` : id GitHub, nom, statut, conclusion, début, fin, étapes (JSON), lien.
type LigneJob = (
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
);
/// Une ligne de `ci_runs` lue avec `COLONNES`.
type LigneRun = (
    i64,
    i64,
    String,
    String,
    String,
    Option<i64>,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<i64>,
);

const COLONNES: &str = "r.id, r.github_id, r.workflow, r.branch, r.sha, r.pr, r.event, r.status, r.conclusion, r.started_at, r.updated_at, r.url, t.number";

#[derive(Deserialize)]
pub struct ListeQ {
    branch: Option<String>,
    status: Option<String>,
    page: Option<i64>,
}

/// `GET /api/maps/{mid}/ci/runs?branch=&status=&page=` : 50 runs par page, le plus récent d'abord.
/// `status` : queued, in_progress, completed, ou une conclusion (success, failure…).
pub async fn liste(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Query(q): Query<ListeQ>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let page = q.page.unwrap_or(1).max(1);
    let rows: Vec<LigneRun> = sqlx::query_as(&format!(
        "SELECT {COLONNES} FROM ci_runs r LEFT JOIN tasks t ON t.id = r.task_id WHERE r.map_id = ?
           AND (? IS NULL OR r.branch = ?) AND (? IS NULL OR r.status = ? OR r.conclusion = ?)
         ORDER BY r.github_id DESC LIMIT 50 OFFSET ?"
    ))
    .bind(mid)
    .bind(&q.branch)
    .bind(&q.branch)
    .bind(&q.status)
    .bind(&q.status)
    .bind(&q.status)
    .bind((page - 1) * 50)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        json!({ "page": page, "runs": rows.iter().map(ligne_run).collect::<Vec<_>>() }),
    ))
}

/// `GET /api/maps/{mid}/ci/runs/{id}` : un run avec ses jobs et leurs étapes.
pub async fn detail(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let r: Option<LigneRun> =
        sqlx::query_as(&format!("SELECT {COLONNES} FROM ci_runs r LEFT JOIN tasks t ON t.id = r.task_id WHERE r.id = ? AND r.map_id = ?"))
            .bind(id)
            .bind(mid)
            .fetch_optional(&st.db)
            .await?;
    let r = r.ok_or(AppError::NotFound)?;
    let jobs: Vec<LigneJob> = sqlx::query_as(
        "SELECT github_id, name, status, conclusion, started_at, completed_at, steps, url FROM ci_jobs WHERE run_id = ? ORDER BY github_id",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    let mut v = ligne_run(&r);
    v["jobs"] = json!(jobs.into_iter().map(|j| json!({ "github_id": j.0, "name": j.1, "status": j.2, "conclusion": j.3, "started_at": j.4,
        "completed_at": j.5, "steps": serde_json::from_str::<Value>(&j.6).unwrap_or(json!([])), "url": j.7 })).collect::<Vec<_>>());
    Ok(Json(v))
}

/// `GET /api/maps/{mid}/ci/task/{number}` : dernier statut CI de la tâche (run le plus récent de sa branche).
pub async fn de_la_tache(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, n)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let r: Option<LigneRun> = sqlx::query_as(&format!(
        "SELECT {COLONNES} FROM ci_runs r JOIN tasks t ON t.id = r.task_id WHERE r.map_id = ? AND t.number = ? ORDER BY r.github_id DESC LIMIT 1"
    ))
    .bind(mid)
    .bind(n)
    .fetch_optional(&st.db)
    .await?;
    Ok(Json(r.as_ref().map(ligne_run).unwrap_or(Value::Null)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Bytes, http::HeaderMap};
    use hmac::{Hmac, Mac};

    async fn monde() -> (AppState, i64, i64, AuthUser) {
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
        let uid = q("INSERT INTO users (email, name, password_hash) VALUES ('c@a.fr', 'C', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tasks (map_id, number, title) VALUES (?, 12, 'Score')")
            .bind(mid)
            .execute(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO integrations (studio_id, kind, enabled, config) VALUES (?, 'github', 1, ?)")
            .bind(sid)
            .bind(json!({ "repo": "pop/test", "secret": "s3cret", "token": "x" }).to_string())
            .execute(&st.db)
            .await
            .unwrap();
        (
            st,
            sid,
            mid,
            AuthUser {
                id: uid,
                email: "c@a.fr".into(),
                name: "C".into(),
                sid: String::new(),
            },
        )
    }

    async fn envoyer(
        st: &AppState,
        sid: i64,
        quoi: &str,
        v: Value,
        secret: &str,
    ) -> Result<Value, AppError> {
        let body = v.to_string();
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body.as_bytes());
        let mut h = HeaderMap::new();
        h.insert(
            "x-hub-signature-256",
            format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
                .parse()
                .unwrap(),
        );
        h.insert("x-github-event", quoi.parse().unwrap());
        crate::integrations::github_hook(State(st.clone()), Path(sid), h, Bytes::from(body))
            .await
            .map(|j| j.0)
    }

    fn evt_run(statut: &str, conclusion: Option<&str>, maj: &str) -> Value {
        json!({ "repository": { "full_name": "pop/test" }, "workflow_run": {
            "id": 555, "name": "CI", "head_branch": "issue/12-score", "head_sha": "abc", "event": "push",
            "status": statut, "conclusion": conclusion, "updated_at": maj, "run_started_at": "2026-11-24T10:00:00Z", "html_url": "https://github.com/pop/test/actions/runs/555" } })
    }

    #[tokio::test]
    async fn reception_anti_doublon_et_tache() {
        let (st, sid, mid, u) = monde().await;
        assert_eq!(numero_de_branche("issue/12-score"), Some(12));
        assert_eq!(numero_de_branche("main"), None);
        // Signature fausse : 401. Autre dépôt : ignoré.
        assert!(matches!(
            envoyer(
                &st,
                sid,
                "workflow_run",
                evt_run("queued", None, "2026-11-24T10:00:00Z"),
                "faux"
            )
            .await,
            Err(AppError::Unauthorized)
        ));
        let mut autre = evt_run("queued", None, "2026-11-24T10:00:00Z");
        autre["repository"]["full_name"] = json!("x/y");
        assert_eq!(
            envoyer(&st, sid, "workflow_run", autre, "s3cret")
                .await
                .unwrap()["ignored"],
            true
        );
        // En file → en cours (job avec étapes) → réussi.
        envoyer(
            &st,
            sid,
            "workflow_run",
            evt_run("queued", None, "2026-11-24T10:00:00Z"),
            "s3cret",
        )
        .await
        .unwrap();
        let job = |statut: &str, c: Option<&str>| {
            json!({ "repository": { "full_name": "pop/test" }, "workflow_job": {
            "id": 777, "run_id": 555, "name": "serveur", "status": statut, "conclusion": c, "html_url": "u",
            "steps": [{ "name": "cargo test", "status": statut, "conclusion": c, "number": 1 }] } })
        };
        envoyer(&st, sid, "workflow_job", job("in_progress", None), "s3cret")
            .await
            .unwrap();
        envoyer(
            &st,
            sid,
            "workflow_job",
            job("completed", Some("success")),
            "s3cret",
        )
        .await
        .unwrap();
        envoyer(
            &st,
            sid,
            "workflow_run",
            evt_run("completed", Some("success"), "2026-11-24T10:05:00Z"),
            "s3cret",
        )
        .await
        .unwrap();
        // Redelivery et événements anciens : pas de doublon, pas de retour en arrière.
        envoyer(
            &st,
            sid,
            "workflow_run",
            evt_run("completed", Some("success"), "2026-11-24T10:05:00Z"),
            "s3cret",
        )
        .await
        .unwrap();
        envoyer(
            &st,
            sid,
            "workflow_run",
            evt_run("in_progress", None, "2026-11-24T10:01:00Z"),
            "s3cret",
        )
        .await
        .unwrap();
        envoyer(&st, sid, "workflow_job", job("in_progress", None), "s3cret")
            .await
            .unwrap();
        let n: (i64, i64) =
            sqlx::query_as("SELECT (SELECT COUNT(*) FROM ci_runs), (SELECT COUNT(*) FROM ci_jobs)")
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(n, (1, 1));
        let Json(l) = liste(
            State(st.clone()),
            u.clone(),
            Path(mid),
            Query(ListeQ {
                branch: None,
                status: Some("success".into()),
                page: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(l["runs"].as_array().unwrap().len(), 1, "{l}");
        assert_eq!(
            (l["runs"][0]["status"].clone(), l["runs"][0]["task"].clone()),
            (json!("completed"), json!(12))
        );
        let Json(d) = detail(
            State(st.clone()),
            u.clone(),
            Path((mid, l["runs"][0]["id"].as_i64().unwrap())),
        )
        .await
        .unwrap();
        assert_eq!(
            (
                d["jobs"][0]["status"].clone(),
                d["jobs"][0]["steps"][0]["name"].clone()
            ),
            (json!("completed"), json!("cargo test"))
        );
        let Json(t) = de_la_tache(State(st.clone()), u.clone(), Path((mid, 12)))
            .await
            .unwrap();
        assert_eq!(t["conclusion"], "success");
        // Un membre d'un autre studio : rien.
        let x: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('x@a.fr', 'X', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        let ux = AuthUser {
            id: x,
            email: "x@a.fr".into(),
            name: "X".into(),
            sid: String::new(),
        };
        assert!(liste(
            State(st.clone()),
            ux,
            Path(mid),
            Query(ListeQ {
                branch: None,
                status: None,
                page: None
            })
        )
        .await
        .is_err());
        // Un membre (pas admin) ne lance pas la synchronisation.
        assert!(matches!(
            synchroniser(State(st.clone()), u, Path(sid), Query(MapQ { map: mid })).await,
            Err(AppError::Forbidden)
        ));
    }
}
