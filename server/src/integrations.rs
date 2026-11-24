//! Intégrations réelles : Discord (rapport du soir par webhook, commandes /tache-suivante, /fini, /rapport)
//! et GitHub (issues ↔ tâches, synchronisation et webhook).
use crate::{
    access::{log_activity, notify, studio_role, Role},
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) async fn config(st: &AppState, sid: i64, kind: &str) -> Result<Option<Value>, AppError> {
    let r: Option<(i64, String)> =
        sqlx::query_as("SELECT enabled, config FROM integrations WHERE studio_id = ? AND kind = ?")
            .bind(sid)
            .bind(kind)
            .fetch_optional(&st.db)
            .await?;
    Ok(r.filter(|r| r.0 == 1)
        .map(|r| serde_json::from_str(&r.1).unwrap_or(json!({}))))
}

pub(crate) async fn save_config(
    st: &AppState,
    sid: i64,
    kind: &str,
    cfg: &Value,
) -> Result<(), AppError> {
    sqlx::query("UPDATE integrations SET config = ? WHERE studio_id = ? AND kind = ?")
        .bind(cfg.to_string())
        .bind(sid)
        .bind(kind)
        .execute(&st.db)
        .await?;
    Ok(())
}

async fn studio_maps(st: &AppState, sid: i64) -> Result<Vec<(i64, String)>, AppError> {
    Ok(
        sqlx::query_as("SELECT id, name FROM maps WHERE studio_id = ? ORDER BY id")
            .bind(sid)
            .fetch_all(&st.db)
            .await?,
    )
}

fn ext(e: impl std::fmt::Display) -> AppError {
    AppError::Conflict(format!("Service externe : {e}"))
}

// ---------- Discord ----------

async fn post_webhook(st: &AppState, url: &str, text: &str) -> Result<(), AppError> {
    if !url.starts_with("https://discord.com/api/webhooks/")
        && !url.starts_with("https://discordapp.com/api/webhooks/")
    {
        return Err(bad(
            "L’adresse du webhook Discord doit commencer par https://discord.com/api/webhooks/",
        ));
    }
    let r = st.http.post(url).json(&json!({ "content": text.chars().take(1990).collect::<String>(), "allowed_mentions": { "parse": [] } })).send().await.map_err(ext)?;
    if !r.status().is_success() {
        return Err(ext(format!("Discord a refusé le message ({})", r.status())));
    }
    Ok(())
}

/// Envoi immédiat du rapport du soir de chaque map du studio.
pub async fn discord_send(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    let cfg = config(&st, sid, "discord")
        .await?
        .ok_or_else(|| bad("Activez Discord d’abord."))?;
    let url = cfg["webhook"].as_str().unwrap_or("").to_string();
    let mut n = 0;
    for (mid, _) in studio_maps(&st, sid).await? {
        post_webhook(&st, &url, &crate::report_text(&st, mid).await?).await?;
        n += 1;
    }
    Ok(Json(json!({ "sent": n })))
}

/// Tâche de fond : rapport du soir à l'heure choisie (heure de Paris), une fois par jour.
pub async fn scheduler(st: AppState) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        let now = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Paris);
        let today = now.format("%Y-%m-%d").to_string();
        let hm = now.format("%H:%M").to_string();
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT studio_id, config FROM integrations WHERE kind = 'discord' AND enabled = 1",
        )
        .fetch_all(&st.db)
        .await
        .unwrap_or_default();
        for (sid, c) in rows {
            let mut cfg: Value = serde_json::from_str(&c).unwrap_or(json!({}));
            let (Some(url), Some(at)) = (
                cfg["webhook"].as_str().map(String::from),
                cfg["report_time"].as_str().map(String::from),
            ) else {
                continue;
            };
            if at.is_empty() || hm < at || cfg["last_report"].as_str() == Some(today.as_str()) {
                continue;
            }
            cfg["last_report"] = json!(today);
            let _ = save_config(&st, sid, "discord", &cfg).await;
            for (mid, _) in studio_maps(&st, sid).await.unwrap_or_default() {
                if let Ok(t) = crate::report_text(&st, mid).await {
                    if let Err(e) = post_webhook(&st, &url, &t).await {
                        tracing::warn!("rapport Discord : {e:?}");
                    }
                }
            }
        }
    }
}

/// Enregistre les commandes slash auprès de Discord (identifiant d'application + jeton du bot).
pub async fn discord_register(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    let cfg = config(&st, sid, "discord")
        .await?
        .ok_or_else(|| bad("Activez Discord d’abord."))?;
    let app = cfg["app_id"]
        .as_str()
        .filter(|s| s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty())
        .ok_or_else(|| bad("Identifiant d’application Discord manquant."))?;
    let bot = cfg["bot_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("Jeton du bot manquant."))?;
    let cmds = json!([
        { "name": "tache-suivante", "description": "Ta prochaine tâche POP", "type": 1 },
        { "name": "fini", "description": "Marquer une tâche POP comme faite", "type": 1, "options": [{ "name": "numero", "description": "Numéro de la tâche", "type": 4, "required": true }] },
        { "name": "rapport", "description": "Rapport du jour de la map", "type": 1 }
    ]);
    let r = st
        .http
        .put(format!(
            "https://discord.com/api/v10/applications/{app}/commands"
        ))
        .header("authorization", format!("Bot {bot}"))
        .json(&cmds)
        .send()
        .await
        .map_err(ext)?;
    if !r.status().is_success() {
        return Err(ext(format!(
            "Discord : {}",
            r.text()
                .await
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
        )));
    }
    Ok(Json(json!({ "ok": true })))
}

fn verify_discord(key_hex: &str, h: &HeaderMap, body: &[u8]) -> bool {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let (Some(sig), Some(ts)) = (
        h.get("x-signature-ed25519").and_then(|v| v.to_str().ok()),
        h.get("x-signature-timestamp").and_then(|v| v.to_str().ok()),
    ) else {
        return false;
    };
    let (Ok(k), Ok(s)) = (hex::decode(key_hex), hex::decode(sig)) else {
        return false;
    };
    let (Ok(k), Ok(s)): (Result<[u8; 32], _>, Result<[u8; 64], _>) = (k.try_into(), s.try_into())
    else {
        return false;
    };
    let Ok(vk) = VerifyingKey::from_bytes(&k) else {
        return false;
    };
    let mut msg = ts.as_bytes().to_vec();
    msg.extend_from_slice(body);
    vk.verify(&msg, &Signature::from_bytes(&s)).is_ok()
}

fn reply(text: &str) -> Json<Value> {
    Json(
        json!({ "type": 4, "data": { "content": text.chars().take(1990).collect::<String>(), "flags": 64, "allowed_mentions": { "parse": [] } } }),
    )
}

/// Point d'entrée des commandes Discord (« Interactions Endpoint URL » de l'application).
pub async fn discord_hook(
    State(st): State<AppState>,
    Path(sid): Path<i64>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    let cfg = config(&st, sid, "discord")
        .await?
        .ok_or(AppError::NotFound)?;
    if !verify_discord(cfg["public_key"].as_str().unwrap_or(""), &headers, &body) {
        return Err(AppError::Unauthorized);
    }
    let v: Value = serde_json::from_slice(&body).map_err(|_| bad("JSON invalide"))?;
    if v["type"] == 1 {
        return Ok(Json(json!({ "type": 1 })));
    }
    let maps = studio_maps(&st, sid).await?;
    let mid = cfg["map_id"]
        .as_i64()
        .filter(|m| maps.iter().any(|x| x.0 == *m))
        .or(maps.first().map(|m| m.0))
        .ok_or(AppError::NotFound)?;
    let duser = &v["member"]["user"];
    let handle = duser["username"].as_str().unwrap_or("").to_lowercase();
    // Pseudo Discord → membre POP (renseigné par un admin dans Équipe et intégrations).
    let uid: Option<i64> = cfg["handles"].as_object().and_then(|m| {
        m.iter()
            .find(|(_, h)| {
                h.as_str()
                    .map(|h| h.trim_start_matches('@').to_lowercase() == handle)
                    .unwrap_or(false)
            })
            .and_then(|(k, _)| k.parse().ok())
    });
    let uid = match uid {
        Some(u) => {
            sqlx::query_scalar::<_, i64>(
                "SELECT user_id FROM memberships WHERE studio_id = ? AND user_id = ?",
            )
            .bind(sid)
            .bind(u)
            .fetch_optional(&st.db)
            .await?
        }
        None => None,
    };
    let cmd = v["data"]["name"].as_str().unwrap_or("");
    match cmd {
        "rapport" => Ok(reply(&crate::report_text(&st, mid).await?)),
        "tache-suivante" => {
            let Some(uid) = uid else {
                return Ok(reply(&format!("Ton pseudo Discord « {handle} » n’est relié à aucun membre POP. Un admin peut le relier dans Équipe et intégrations → Discord.")));
            };
            let t: Option<(i64, String, String)> = sqlx::query_as("SELECT number, title, status FROM tasks WHERE map_id = ? AND assignee_id = ? AND status IN ('doing', 'todo') ORDER BY status = 'doing' DESC, planned_day IS NULL, planned_day, number LIMIT 1").bind(mid).bind(uid).fetch_optional(&st.db).await?;
            Ok(reply(&match t {
                Some((n, title, s)) => format!(
                    "{} #{n} · {title}",
                    if s == "doing" {
                        "En cours :"
                    } else {
                        "Prochaine :"
                    }
                ),
                None => "Aucune tâche à faire pour toi. 🎉".into(),
            }))
        }
        "fini" => {
            let Some(uid) = uid else {
                return Ok(reply("Ton pseudo Discord n’est relié à aucun membre POP."));
            };
            let role: String = sqlx::query_scalar(
                "SELECT role FROM memberships WHERE studio_id = ? AND user_id = ?",
            )
            .bind(sid)
            .bind(uid)
            .fetch_one(&st.db)
            .await?;
            if role == "viewer" {
                return Ok(reply(
                    "Les invités en lecture ne peuvent pas modifier les tâches.",
                ));
            }
            let n = v["data"]["options"]
                .as_array()
                .and_then(|o| o.iter().find(|x| x["name"] == "numero"))
                .and_then(|x| x["value"].as_i64())
                .unwrap_or(0);
            let t: Option<(i64, String)> =
                sqlx::query_as("SELECT id, title FROM tasks WHERE map_id = ? AND number = ?")
                    .bind(mid)
                    .bind(n)
                    .fetch_optional(&st.db)
                    .await?;
            let Some((tid, title)) = t else {
                return Ok(reply(&format!("Tâche #{n} introuvable.")));
            };
            // Une tâche UEFN sans check-in Lore est refusée (#127) : Discord reçoit l'explication.
            match crate::tasks::set_status_internal(&st, mid, tid, uid, "done", "Discord").await {
                Err(AppError::Conflict(m)) => Ok(reply(&m)),
                r => {
                    r?;
                    Ok(reply(&format!("✅ #{n} {title} : faite.")))
                }
            }
        }
        _ => Ok(reply("Commande inconnue.")),
    }
}

// ---------- GitHub ----------

pub(crate) fn gh(
    st: &AppState,
    method: reqwest::Method,
    url: String,
    token: &str,
) -> reqwest::RequestBuilder {
    st.http
        .request(method, url)
        .header("authorization", format!("Bearer {token}"))
        .header("accept", "application/vnd.github+json")
        .header("user-agent", "pop-uefn")
        .header("x-github-api-version", "2022-11-28")
}

pub(crate) fn repo_ok(r: &str) -> bool {
    let p: Vec<&str> = r.split('/').collect();
    p.len() == 2
        && p.iter().all(|x| {
            !x.is_empty()
                && x.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
}

#[derive(Deserialize)]
pub struct SyncQ {
    map: i64,
}

/// Synchronisation dans les deux sens pour une map : tâches → issues, issues → tâches, états ouverts/fermés.
pub async fn github_sync(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Query(q): Query<SyncQ>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    if !studio_maps(&st, sid).await?.iter().any(|m| m.0 == q.map) {
        return Err(AppError::NotFound);
    }
    let cfg = config(&st, sid, "github")
        .await?
        .ok_or_else(|| bad("Activez GitHub d’abord."))?;
    let repo = cfg["repo"]
        .as_str()
        .filter(|r| repo_ok(r))
        .ok_or_else(|| bad("Dépôt GitHub invalide (format : propriétaire/nom)."))?
        .to_string();
    let token = cfg["token"]
        .as_str()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| bad("Jeton GitHub manquant."))?
        .to_string();
    let base = format!("https://api.github.com/repos/{repo}");
    // 1. Issues existantes (ouvertes et fermées), sans les pull requests.
    let mut issues: Vec<Value> = vec![];
    for page in 1..=10 {
        let r = gh(
            &st,
            reqwest::Method::GET,
            format!("{base}/issues?state=all&per_page=100&page={page}"),
            &token,
        )
        .send()
        .await
        .map_err(ext)?;
        if !r.status().is_success() {
            return Err(ext(format!(
                "GitHub ({}) : vérifiez le dépôt et le jeton (droit Issues en lecture/écriture).",
                r.status()
            )));
        }
        let batch: Vec<Value> = r.json().await.map_err(ext)?;
        let len = batch.len();
        issues.extend(
            batch
                .into_iter()
                .filter(|i| i.get("pull_request").is_none()),
        );
        if len < 100 {
            break;
        }
    }
    let tasks: Vec<(i64, i64, String, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT id, number, title, description, status, github_issue FROM tasks WHERE map_id = ?",
    )
    .bind(q.map)
    .fetch_all(&st.db)
    .await?;
    let (mut created, mut imported, mut closed, mut done) = (0, 0, 0, 0);
    // 2. Tâches sans issue → nouvelle issue.
    for (tid, num, title, desc, status, gi) in &tasks {
        match gi {
            None => {
                let r = gh(&st, reqwest::Method::POST, format!("{base}/issues"), &token).json(&json!({ "title": format!("#{num} {title}"), "body": format!("{desc}\n\n— Tâche POP #{num}") })).send().await.map_err(ext)?;
                if !r.status().is_success() {
                    return Err(ext(format!("création d’issue refusée ({})", r.status())));
                }
                let i: Value = r.json().await.map_err(ext)?;
                let n = i["number"].as_i64().unwrap_or(0);
                sqlx::query("UPDATE tasks SET github_issue = ? WHERE id = ?")
                    .bind(n)
                    .bind(tid)
                    .execute(&st.db)
                    .await?;
                if status == "done" {
                    gh(
                        &st,
                        reqwest::Method::PATCH,
                        format!("{base}/issues/{n}"),
                        &token,
                    )
                    .json(&json!({ "state": "closed" }))
                    .send()
                    .await
                    .map_err(ext)?;
                }
                created += 1;
            }
            Some(n) => {
                let Some(i) = issues.iter().find(|i| i["number"].as_i64() == Some(*n)) else {
                    continue;
                };
                let open = i["state"] == "open";
                if status == "done" && open {
                    gh(
                        &st,
                        reqwest::Method::PATCH,
                        format!("{base}/issues/{n}"),
                        &token,
                    )
                    .json(&json!({ "state": "closed" }))
                    .send()
                    .await
                    .map_err(ext)?;
                    closed += 1;
                } else if status != "done" && !open {
                    // Une tâche UEFN reste ouverte tant que son check-in Lore manque (#127).
                    match crate::tasks::set_status_internal(
                        &st, q.map, *tid, u.id, "done", "GitHub",
                    )
                    .await
                    {
                        Err(AppError::Conflict(_)) => {}
                        r => {
                            r?;
                            done += 1;
                        }
                    }
                }
            }
        }
    }
    // 3. Issues ouvertes inconnues → tâches.
    let linked: Vec<i64> = sqlx::query_scalar(
        "SELECT github_issue FROM tasks WHERE map_id = ? AND github_issue IS NOT NULL",
    )
    .bind(q.map)
    .fetch_all(&st.db)
    .await?;
    for i in issues.iter().filter(|i| i["state"] == "open") {
        let n = i["number"].as_i64().unwrap_or(0);
        if linked.contains(&n) || i["body"].as_str().unwrap_or("").contains("— Tâche POP #") {
            continue;
        }
        let number: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(number), 0) + 1 FROM tasks WHERE map_id = ?")
                .bind(q.map)
                .fetch_one(&st.db)
                .await?;
        let title: String = i["title"]
            .as_str()
            .unwrap_or("Issue")
            .chars()
            .take(140)
            .collect();
        let tid: i64 = sqlx::query_scalar("INSERT INTO tasks (map_id, number, title, description, origin, github_issue) VALUES (?, ?, ?, ?, ?, ?) RETURNING id")
            .bind(q.map).bind(number).bind(&title).bind(i["body"].as_str().unwrap_or("").chars().take(5000).collect::<String>()).bind(format!("github:{n}")).bind(n).fetch_one(&st.db).await?;
        sqlx::query("INSERT INTO task_events (task_id, user_id, what, via) VALUES (?, ?, 'Tâche créée', 'GitHub')").bind(tid).bind(u.id).execute(&st.db).await?;
        imported += 1;
    }
    notify(&st, q.map, json!({ "t": "refresh", "what": "tasks" }));
    log_activity(&st, q.map, Some(u.id), "task", "Synchronisation GitHub", &format!("{created} issue(s) créée(s), {imported} importée(s), {closed} fermée(s), {done} tâche(s) faite(s)")).await?;
    Ok(Json(
        json!({ "created": created, "imported": imported, "closed": closed, "done": done }),
    ))
}

/// Webhook GitHub (événement « Issues ») : une issue fermée marque la tâche faite, rouverte la remet à faire.
pub async fn github_hook(
    State(st): State<AppState>,
    Path(sid): Path<i64>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    use hmac::{Hmac, Mac};
    let cfg = config(&st, sid, "github")
        .await?
        .ok_or(AppError::NotFound)?;
    let secret = cfg["secret"].as_str().unwrap_or("");
    let sig = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("sha256="))
        .and_then(|s| hex::decode(s).ok())
        .ok_or(AppError::Unauthorized)?;
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::Unauthorized)?;
    mac.update(&body);
    if secret.is_empty() || mac.verify_slice(&sig).is_err() {
        return Err(AppError::Unauthorized);
    }
    let v: Value = serde_json::from_slice(&body).map_err(|_| bad("JSON invalide"))?;
    if v["repository"]["full_name"].as_str() != cfg["repo"].as_str() {
        return Ok(Json(json!({ "ignored": true })));
    }
    // CI GitHub Actions (#217) : runs et jobs.
    let quoi = headers
        .get("x-github-event")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if matches!(quoi, "workflow_run" | "workflow_job") {
        return Ok(Json(crate::ci::evenement(&st, sid, &cfg, quoi, &v).await?));
    }
    let (action, n) = (
        v["action"].as_str().unwrap_or(""),
        v["issue"]["number"].as_i64().unwrap_or(0),
    );
    let row: Option<(i64, i64)> = sqlx::query_as("SELECT t.id, t.map_id FROM tasks t JOIN maps m ON m.id = t.map_id WHERE m.studio_id = ? AND t.github_issue = ?").bind(sid).bind(n).fetch_optional(&st.db).await?;
    let Some((tid, mid)) = row else {
        return Ok(Json(json!({ "ignored": true })));
    };
    let owner: i64 = sqlx::query_scalar(
        "SELECT user_id FROM memberships WHERE studio_id = ? AND role = 'owner' LIMIT 1",
    )
    .bind(sid)
    .fetch_one(&st.db)
    .await?;
    match action {
        // Une tâche UEFN reste ouverte tant que son check-in Lore manque (#127).
        "closed" => {
            match crate::tasks::set_status_internal(&st, mid, tid, owner, "done", "GitHub").await {
                Err(AppError::Conflict(m)) => {
                    return Ok(Json(json!({ "ignored": true, "reason": m })))
                }
                r => r?,
            }
        }
        "reopened" => {
            crate::tasks::set_status_internal(&st, mid, tid, owner, "todo", "GitHub").await?
        }
        _ => {}
    }
    Ok(Json(json!({ "ok": true })))
}
