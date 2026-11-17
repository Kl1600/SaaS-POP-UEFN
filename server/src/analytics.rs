//! Analyse : statistiques publiques des îles Fortnite (Fortnite Data API d'Epic, sans clé)
//! et tendances TikTok autour de Fortnite (via Apify, avec le jeton du studio).
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
use std::collections::HashMap;

const DEFAULT_BASE: &str = "https://api.fortnite.com/ecosystem/v1";

fn base() -> String {
    std::env::var("POP_FORTNITE_API").unwrap_or_else(|_| DEFAULT_BASE.into())
}

fn valid_code(c: &str) -> bool {
    let b = c.as_bytes();
    (b.len() == 14 && b[4] == b'-' && b[9] == b'-' && c.chars().filter(|x| *x != '-').all(|x| x.is_ascii_digit()))
        || (c.starts_with("playlist_") && c.len() < 80 && c.chars().all(|x| x.is_ascii_alphanumeric() || x == '_'))
}

fn next_url(link: &str) -> String {
    if link.starts_with("http") { link.to_string() }
    else if link.starts_with("/ecosystem") && !std::env::var("POP_FORTNITE_API").is_ok() { format!("https://api.fortnite.com{link}") }
    else { format!("{}{}", base(), if link.starts_with('/') { link.to_string() } else { format!("/{link}") }) }
}

async fn get_json(st: &AppState, url: &str) -> Result<(u16, Value), String> {
    let r = st.http.get(url).header("accept", "application/json").header("user-agent", "POP-UEFN/1.0").timeout(std::time::Duration::from_secs(25)).send().await.map_err(|e| e.to_string())?;
    let code = r.status().as_u16();
    let v = r.json::<Value>().await.unwrap_or(Value::Null);
    Ok((code, v))
}

async fn upsert_island(st: &AppState, it: &Value, is_new: bool) -> Result<(), sqlx::Error> {
    let code = it["code"].as_str().unwrap_or("");
    if !valid_code(code) { return Ok(()) }
    sqlx::query(
        "INSERT INTO fn_islands (code, title, creator, category, created_in, tags, seen_new) VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(code) DO UPDATE SET title = excluded.title, creator = excluded.creator, category = excluded.category, created_in = excluded.created_in, tags = excluded.tags, seen_new = MAX(seen_new, excluded.seen_new)",
    )
    .bind(code)
    .bind(it["title"].as_str().unwrap_or("").chars().take(200).collect::<String>())
    .bind(it["creatorCode"].as_str().or(it["displayName"].as_str()).unwrap_or("").chars().take(80).collect::<String>())
    .bind(it["category"].as_str().unwrap_or(""))
    .bind(it["createdIn"].as_str().unwrap_or(""))
    .bind(it["tags"].to_string())
    .bind(is_new as i64)
    .execute(&st.db).await?;
    Ok(())
}

/// Métriques journalières (7 jours) d'une île, enregistrées jour par jour.
async fn fetch_metrics(st: &AppState, code: &str) -> Result<bool, String> {
    let mut url = format!("{}/islands/{code}/metrics/day", base());
    let (mut status, mut v) = get_json(st, &url).await?;
    if status == 400 {
        let to = chrono::Utc::now();
        let from = to - chrono::Duration::days(7);
        url = format!("{url}?from={}&to={}", from.format("%Y-%m-%dT00:00:00.000Z"), to.format("%Y-%m-%dT%H:%M:%S.000Z"));
        (status, v) = get_json(st, &url).await?;
    }
    if status == 429 { return Err("limite de l’API Fortnite atteinte".into()) }
    if status == 404 { return Ok(false) }
    if status != 200 { return Err(format!("API Fortnite : HTTP {status}")) }
    let mut days: HashMap<String, HashMap<&str, Value>> = HashMap::new();
    for (field, col) in [("uniquePlayers", "unique_players"), ("plays", "plays"), ("peakCCU", "peak_ccu"), ("minutesPlayed", "minutes"), ("averageMinutesPerPlayer", "avg_minutes"), ("favorites", "favorites"), ("recommendations", "recommends")] {
        let arr = v[field].as_array().or(v[field]["intervals"].as_array()).cloned().unwrap_or_default();
        for p in arr {
            let Some(ts) = p["timestamp"].as_str() else { continue };
            days.entry(ts.chars().take(10).collect()).or_default().insert(col, p["value"].clone());
        }
    }
    for p in v["retention"].as_array().or(v["retention"]["intervals"].as_array()).cloned().unwrap_or_default() {
        let Some(ts) = p["timestamp"].as_str() else { continue };
        let e = days.entry(ts.chars().take(10).collect()).or_default();
        e.insert("d1", p["d1"].clone());
        e.insert("d7", p["d7"].clone());
    }
    for (day, m) in days {
        let g = |k: &str| m.get(k).cloned().unwrap_or(Value::Null);
        sqlx::query(
            "INSERT INTO fn_metrics (code, day, unique_players, plays, peak_ccu, minutes, avg_minutes, favorites, recommends, d1, d7) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(code, day) DO UPDATE SET unique_players = excluded.unique_players, plays = excluded.plays, peak_ccu = excluded.peak_ccu, minutes = excluded.minutes,
             avg_minutes = excluded.avg_minutes, favorites = excluded.favorites, recommends = excluded.recommends, d1 = COALESCE(excluded.d1, d1), d7 = COALESCE(excluded.d7, d7)",
        )
        .bind(code).bind(&day)
        .bind(g("unique_players").as_i64()).bind(g("plays").as_i64()).bind(g("peak_ccu").as_i64()).bind(g("minutes").as_i64())
        .bind(g("avg_minutes").as_f64()).bind(g("favorites").as_i64()).bind(g("recommends").as_i64()).bind(g("d1").as_f64()).bind(g("d7").as_f64())
        .execute(&st.db).await.map_err(|e| e.to_string())?;
    }
    sqlx::query("UPDATE fn_islands SET metrics_at = datetime('now') WHERE code = ?").bind(code).execute(&st.db).await.map_err(|e| e.to_string())?;
    Ok(true)
}

/// Une passe de collecte : dernières sorties, puis métriques des îles à suivre.
pub async fn crawl(st: &AppState) -> Result<String, String> {
    sqlx::query("UPDATE fn_status SET last_run = datetime('now') WHERE id = 1").execute(&st.db).await.map_err(|e| e.to_string())?;
    // 1. Les dernières îles sorties (l'API les classe de la plus récente à la plus ancienne).
    let mut url = format!("{}/islands?size=100", base());
    let mut listed = 0;
    for _ in 0..15 {
        let (status, v) = get_json(st, &url).await?;
        if status == 429 { break }
        if status != 200 { return Err(format!("API Fortnite (liste) : HTTP {status}")) }
        let data = v["data"].as_array().cloned().unwrap_or_default();
        for it in &data {
            upsert_island(st, it, true).await.map_err(|e| e.to_string())?;
            listed += 1;
        }
        match v["links"]["next"].as_str() {
            Some(n) if !n.is_empty() && !data.is_empty() => url = next_url(n),
            _ => break,
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    // 2. Métriques : îles jamais mesurées, sorties récentes, îles suivies, meilleures îles.
    let codes: Vec<String> = sqlx::query_scalar(
        "SELECT code FROM fn_islands i WHERE metrics_at IS NULL OR metrics_at < datetime('now', '-50 minutes') AND (
            (seen_new = 1 AND first_seen > datetime('now', '-14 days'))
            OR code IN (SELECT code FROM fn_watch)
            OR code IN (SELECT island_code FROM maps WHERE island_code IS NOT NULL)
            OR code IN (SELECT code FROM fn_metrics WHERE day = (SELECT MAX(day) FROM fn_metrics) ORDER BY unique_players DESC LIMIT 200))
         ORDER BY metrics_at IS NOT NULL, metrics_at LIMIT 400",
    ).fetch_all(&st.db).await.map_err(|e| e.to_string())?;
    let mut measured = 0;
    for c in &codes {
        match fetch_metrics(st, c).await {
            Ok(true) => measured += 1,
            Ok(false) => {}
            Err(e) if e.contains("limite") => { tokio::time::sleep(std::time::Duration::from_secs(30)).await; break }
            Err(e) => return Err(e),
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fn_islands").fetch_one(&st.db).await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE fn_status SET last_ok = datetime('now'), last_error = NULL, islands = ? WHERE id = 1").bind(total).execute(&st.db).await.map_err(|e| e.to_string())?;
    Ok(format!("{listed} îles listées, {measured} mesurées"))
}

/// Collecte automatique toutes les 30 minutes (POP_ANALYTICS=0 pour la couper).
pub async fn crawler(st: AppState) {
    if std::env::var("POP_ANALYTICS").map(|v| v == "0").unwrap_or(false) { return }
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    loop {
        match crawl(&st).await {
            Ok(m) => tracing::info!("analyse Fortnite : {m}"),
            Err(e) => {
                tracing::warn!("analyse Fortnite : {e}");
                let _ = sqlx::query("UPDATE fn_status SET last_error = ? WHERE id = 1").bind(&e).execute(&st.db).await;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1800)).await;
    }
}

// ---------- Lecture ----------

#[derive(Deserialize)]
pub struct ListQ {
    q: Option<String>,
    tag: Option<String>,
    epic: Option<bool>,
}

fn island_json(r: &(String, String, String, String, String)) -> Value {
    json!({ "code": r.0, "title": r.1, "creator": r.2, "category": r.3, "tags": serde_json::from_str::<Value>(&r.4).unwrap_or(json!([])) })
}

pub async fn status(State(st): State<AppState>, _u: AuthUser) -> ApiResult<Value> {
    let (last_run, last_ok, err, islands): (Option<String>, Option<String>, Option<String>, i64) = sqlx::query_as("SELECT last_run, last_ok, last_error, islands FROM fn_status WHERE id = 1").fetch_one(&st.db).await?;
    let latest: Option<String> = sqlx::query_scalar("SELECT MAX(day) FROM fn_metrics").fetch_one(&st.db).await?;
    let measured: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT code) FROM fn_metrics").fetch_one(&st.db).await?;
    Ok(Json(json!({ "last_run": last_run, "last_ok": last_ok, "error": err, "islands": islands, "measured": measured, "latest_day": latest })))
}

static REFRESHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub async fn refresh(State(st): State<AppState>, _u: AuthUser) -> ApiResult<Value> {
    if REFRESHING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Ok(Json(json!({ "running": true })));
    }
    let st2 = st.clone();
    tokio::spawn(async move {
        if let Err(e) = crawl(&st2).await {
            let _ = sqlx::query("UPDATE fn_status SET last_error = ? WHERE id = 1").bind(&e).execute(&st2.db).await;
        }
        REFRESHING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
    Ok(Json(json!({ "running": true })))
}

/// Maps qui marchent en ce moment : joueurs du dernier jour, évolution sur la veille.
pub async fn trending(State(st): State<AppState>, _u: AuthUser, Query(q): Query<ListQ>) -> ApiResult<Value> {
    let rows: Vec<(String, String, String, String, String, Option<i64>, Option<i64>, Option<i64>, Option<f64>, Option<f64>, Option<i64>, String)> = sqlx::query_as(
        "WITH d AS (SELECT MAX(day) AS last FROM fn_metrics WHERE unique_players IS NOT NULL)
         SELECT i.code, i.title, i.creator, i.category, i.tags, m.unique_players, m.peak_ccu, m.plays, m.avg_minutes,
                (SELECT d1 FROM fn_metrics r WHERE r.code = i.code AND r.d1 IS NOT NULL ORDER BY r.day DESC LIMIT 1),
                (SELECT p.unique_players FROM fn_metrics p WHERE p.code = i.code AND p.day = date(m.day, '-1 day')), m.day
         FROM fn_islands i JOIN fn_metrics m ON m.code = i.code, d WHERE m.day = d.last AND m.unique_players IS NOT NULL
         ORDER BY m.unique_players DESC LIMIT 600",
    ).fetch_all(&st.db).await?;
    let ql = q.q.unwrap_or_default().to_lowercase();
    let tag = q.tag.unwrap_or_default().to_lowercase();
    let epic = q.epic.unwrap_or(false);
    let out: Vec<Value> = rows.into_iter().filter(|r| {
        (epic || !(r.0.starts_with("playlist_") || r.2.eq_ignore_ascii_case("epic")))
            && (ql.is_empty() || r.1.to_lowercase().contains(&ql) || r.0.contains(&ql) || r.2.to_lowercase().contains(&ql))
            && (tag.is_empty() || r.4.to_lowercase().contains(&format!("\"{tag}\"")))
    }).take(100).map(|r| {
        let growth = match (r.5, r.10) { (Some(a), Some(b)) if b > 0 => Some(((a - b) as f64 / b as f64 * 100.0).round()), _ => None };
        let mut v = island_json(&(r.0.clone(), r.1.clone(), r.2.clone(), r.3.clone(), r.4.clone()));
        v["players"] = json!(r.5); v["peak_ccu"] = json!(r.6); v["plays"] = json!(r.7); v["avg_minutes"] = json!(r.8); v["d1"] = json!(r.9); v["growth"] = json!(growth); v["day"] = json!(r.11);
        v
    }).collect();
    Ok(Json(json!(out)))
}

/// Meilleurs démarrages : îles sorties récemment, classées sur leur premier jour mesuré.
pub async fn launches(State(st): State<AppState>, _u: AuthUser, Query(q): Query<ListQ>) -> ApiResult<Value> {
    let rows: Vec<(String, String, String, String, String, String, String, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<f64>)> = sqlx::query_as(
        "SELECT i.code, i.title, i.creator, i.category, i.tags, i.first_seen, f.day, f.unique_players, f.peak_ccu, f.plays,
                (SELECT l.unique_players FROM fn_metrics l WHERE l.code = i.code AND l.unique_players IS NOT NULL ORDER BY l.day DESC LIMIT 1),
                (SELECT r.d1 FROM fn_metrics r WHERE r.code = i.code AND r.d1 IS NOT NULL ORDER BY r.day LIMIT 1)
         FROM fn_islands i JOIN fn_metrics f ON f.code = i.code AND f.day = (SELECT MIN(day) FROM fn_metrics x WHERE x.code = i.code AND x.unique_players IS NOT NULL)
         WHERE i.seen_new = 1 AND i.first_seen > datetime('now', '-30 days') AND f.day > (SELECT MIN(day) FROM fn_metrics WHERE day > date('now', '-8 days'))
         ORDER BY f.unique_players DESC LIMIT 300",
    ).fetch_all(&st.db).await?;
    let tag = q.tag.unwrap_or_default().to_lowercase();
    let out: Vec<Value> = rows.into_iter().filter(|r| tag.is_empty() || r.4.to_lowercase().contains(&format!("\"{tag}\""))).take(100).map(|r| {
        let mut v = island_json(&(r.0.clone(), r.1.clone(), r.2.clone(), r.3.clone(), r.4.clone()));
        v["first_seen"] = json!(r.5); v["launch_day"] = json!(r.6); v["launch_players"] = json!(r.7); v["launch_ccu"] = json!(r.8); v["launch_plays"] = json!(r.9); v["players_now"] = json!(r.10); v["d1"] = json!(r.11);
        v
    }).collect();
    Ok(Json(json!(out)))
}

/// Analyse du marché : genres (tags) par joueurs, concurrence et rétention.
pub async fn market(State(st): State<AppState>, _u: AuthUser) -> ApiResult<Value> {
    let rows: Vec<(String, String, Option<i64>, Option<f64>, i64)> = sqlx::query_as(
        "WITH d AS (SELECT MAX(day) AS last FROM fn_metrics WHERE unique_players IS NOT NULL)
         SELECT i.code, i.tags, m.unique_players, (SELECT d1 FROM fn_metrics r WHERE r.code = i.code AND r.d1 IS NOT NULL ORDER BY r.day DESC LIMIT 1),
                (i.seen_new = 1 AND i.first_seen > datetime('now', '-7 days'))
         FROM fn_islands i JOIN fn_metrics m ON m.code = i.code, d WHERE m.day = d.last AND i.code NOT LIKE 'playlist_%'",
    ).fetch_all(&st.db).await?;
    #[derive(Default)]
    struct Agg { islands: i64, players: i64, d1_sum: f64, d1_n: i64, new: i64, top: i64 }
    let mut by: HashMap<String, Agg> = HashMap::new();
    let mut total_players = 0;
    for (_, tags, up, d1, new) in &rows {
        let up = up.unwrap_or(0);
        total_players += up;
        let tags: Vec<String> = serde_json::from_str(tags).unwrap_or_default();
        for t in tags.iter().take(6) {
            let a = by.entry(t.to_lowercase()).or_default();
            a.islands += 1; a.players += up; a.new += *new;
            a.top = a.top.max(up);
            if let Some(d) = d1 { a.d1_sum += d; a.d1_n += 1 }
        }
    }
    let mut tags: Vec<Value> = by.into_iter().filter(|(_, a)| a.islands >= 3).map(|(t, a)| {
        let avg = a.players as f64 / a.islands as f64;
        let d1 = if a.d1_n > 0 { Some(a.d1_sum / a.d1_n as f64) } else { None };
        // Opportunité : beaucoup de joueurs par île, peu de concurrence nouvelle, bonne rétention.
        let score = avg.ln_1p() * (1.0 + d1.unwrap_or(0.1)) / (1.0 + a.new as f64 / a.islands as f64);
        json!({ "tag": t, "islands": a.islands, "players": a.players, "avg_players": avg.round(), "top_players": a.top, "d1": d1, "new_7d": a.new, "share": if total_players > 0 { a.players as f64 / total_players as f64 * 100.0 } else { 0.0 }, "score": (score * 10.0).round() / 10.0 })
    }).collect();
    tags.sort_by(|a, b| b["players"].as_i64().cmp(&a["players"].as_i64()));
    let new_per_day: Vec<(String, i64)> = sqlx::query_as("SELECT date(first_seen), COUNT(*) FROM fn_islands WHERE seen_new = 1 AND first_seen > datetime('now', '-14 days') GROUP BY 1 ORDER BY 1").fetch_all(&st.db).await?;
    let series: Vec<(String, Option<i64>)> = sqlx::query_as("SELECT day, SUM(unique_players) FROM fn_metrics WHERE code NOT LIKE 'playlist_%' GROUP BY day ORDER BY day").fetch_all(&st.db).await?;
    Ok(Json(json!({
        "islands": rows.len(), "total_players": total_players, "tags": tags,
        "new_per_day": new_per_day.into_iter().map(|(d, n)| json!({ "day": d, "count": n })).collect::<Vec<_>>(),
        "players_per_day": series.into_iter().map(|(d, n)| json!({ "day": d, "players": n })).collect::<Vec<_>>(),
    })))
}

pub async fn island(State(st): State<AppState>, u: AuthUser, Path(code): Path<String>) -> ApiResult<Value> {
    if !valid_code(&code) { return Err(bad("Code d’île invalide (format 1234-5678-9012).")) }
    let meta: Option<(String, String, String, String, String)> = sqlx::query_as("SELECT code, title, creator, category, tags FROM fn_islands WHERE code = ?").bind(&code).fetch_optional(&st.db).await?;
    let meta = match meta {
        Some(m) => m,
        None => {
            // Île jamais vue : on la récupère tout de suite.
            let (status, v) = get_json(&st, &format!("{}/islands/{code}", base())).await.map_err(|e| AppError::Conflict(format!("API Fortnite injoignable : {e}")))?;
            if status != 200 { return Err(AppError::Conflict("Île introuvable dans l’API Fortnite (elle doit être publique et découvrable).".into())) }
            upsert_island(&st, &v, false).await?;
            let _ = fetch_metrics(&st, &code).await;
            sqlx::query_as("SELECT code, title, creator, category, tags FROM fn_islands WHERE code = ?").bind(&code).fetch_one(&st.db).await?
        }
    };
    let series: Vec<(String, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<f64>, Option<i64>, Option<i64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT day, unique_players, plays, peak_ccu, minutes, avg_minutes, favorites, recommends, d1, d7 FROM fn_metrics WHERE code = ? ORDER BY day",
    ).bind(&code).fetch_all(&st.db).await?;
    let watched: bool = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM fn_watch WHERE code = ?").bind(&code).fetch_one(&st.db).await? > 0;
    let _ = u;
    let mut v = island_json(&meta);
    v["watched"] = json!(watched);
    v["series"] = json!(series.into_iter().map(|s| json!({ "day": s.0, "players": s.1, "plays": s.2, "peak_ccu": s.3, "minutes": s.4, "avg_minutes": s.5, "favorites": s.6, "recommends": s.7, "d1": s.8, "d7": s.9 })).collect::<Vec<_>>());
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct WatchIn {
    code: String,
    on: bool,
}

pub async fn watch(State(st): State<AppState>, u: AuthUser, Json(b): Json<WatchIn>) -> ApiResult<Value> {
    let code = b.code.trim().to_string();
    if !valid_code(&code) { return Err(bad("Code d’île invalide (format 1234-5678-9012).")) }
    if b.on {
        sqlx::query("INSERT OR IGNORE INTO fn_watch (code, added_by) VALUES (?, ?)").bind(&code).bind(u.id).execute(&st.db).await?;
    } else {
        sqlx::query("DELETE FROM fn_watch WHERE code = ?").bind(&code).execute(&st.db).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn watched(State(st): State<AppState>, _u: AuthUser) -> ApiResult<Value> {
    let rows: Vec<(String, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT w.code, i.title, (SELECT unique_players FROM fn_metrics m WHERE m.code = w.code AND unique_players IS NOT NULL ORDER BY day DESC LIMIT 1) FROM fn_watch w LEFT JOIN fn_islands i ON i.code = w.code ORDER BY w.added_at DESC",
    ).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|(c, t, p)| json!({ "code": c, "title": t, "players": p })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct IslandCodeIn {
    code: Option<String>,
}

/// Code de l'île publiée d'une map POP, pour la comparer au marché.
pub async fn set_map_island(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<IslandCodeIn>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?.require(Role::Admin)?;
    let code = b.code.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    if let Some(c) = &code { if !valid_code(c) { return Err(bad("Code d’île invalide (format 1234-5678-9012).")) } }
    sqlx::query("UPDATE maps SET island_code = ? WHERE id = ?").bind(&code).bind(mid).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn map_island(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let code: Option<String> = sqlx::query_scalar("SELECT island_code FROM maps WHERE id = ?").bind(mid).fetch_one(&st.db).await?;
    Ok(Json(json!({ "code": code })))
}

// ---------- TikTok ----------

const DEFAULT_TAGS: &[&str] = &["fortnite", "fortnitecreative", "uefn", "fortnitemap", "fortnitemaps"];

async fn apify_cfg(st: &AppState, sid: i64) -> Result<Value, AppError> {
    let r: Option<(i64, String)> = sqlx::query_as("SELECT enabled, config FROM integrations WHERE studio_id = ? AND kind = 'apify'").bind(sid).fetch_optional(&st.db).await?;
    Ok(r.filter(|r| r.0 == 1).map(|r| serde_json::from_str(&r.1).unwrap_or(json!({}))).unwrap_or(json!({})))
}

/// Récupère les vidéos TikTok récentes des hashtags suivis (acteur Apify « clockworks/tiktok-scraper »).
pub async fn tiktok_refresh(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member { return Err(AppError::Forbidden) }
    let cfg = apify_cfg(&st, sid).await?;
    let token = cfg["token"].as_str().filter(|t| !t.is_empty()).ok_or_else(|| bad("Ajoutez votre jeton Apify (Équipe → Intégrations → TikTok) pour collecter les tendances."))?.to_string();
    let mut tags: Vec<String> = cfg["hashtags"].as_str().unwrap_or("").split([',', ' ', '\n']).map(|t| t.trim().trim_start_matches('#').to_lowercase()).filter(|t| !t.is_empty() && t.len() < 60).collect();
    if tags.is_empty() { tags = DEFAULT_TAGS.iter().map(|s| s.to_string()).collect() }
    tags.truncate(8);
    let per = cfg["per_tag"].as_i64().unwrap_or(30).clamp(5, 100);
    let input = json!({ "hashtags": tags, "resultsPerPage": per, "shouldDownloadVideos": false, "shouldDownloadCovers": false, "shouldDownloadSubtitles": false, "shouldDownloadSlideshowImages": false });
    let r = st.http.post(format!("{}/v2/acts/clockworks~tiktok-scraper/run-sync-get-dataset-items?token={token}&timeout=280", std::env::var("POP_APIFY_API").unwrap_or_else(|_| "https://api.apify.com".into())))
        .json(&input).timeout(std::time::Duration::from_secs(300)).send().await.map_err(|e| AppError::Conflict(format!("Apify injoignable : {e}")))?;
    let status = r.status();
    let body: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(AppError::Conflict(format!("Apify : {}", body["error"]["message"].as_str().unwrap_or(status.as_str()))));
    }
    let items = body.as_array().cloned().unwrap_or_default();
    let mut n = 0;
    for it in &items {
        let Some(id) = it["id"].as_str().map(String::from).or(it["id"].as_i64().map(|x| x.to_string())) else { continue };
        let hashtags: Vec<String> = it["hashtags"].as_array().cloned().unwrap_or_default().iter().filter_map(|h| h["name"].as_str().or(h.as_str()).map(|s| s.to_lowercase())).filter(|s| !s.is_empty()).collect();
        sqlx::query(
            "INSERT INTO tt_videos (id, studio_id, query, text, author, url, music, hashtags, created, plays, likes, shares, comments) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id, studio_id) DO UPDATE SET plays = excluded.plays, likes = excluded.likes, shares = excluded.shares, comments = excluded.comments, fetched_at = datetime('now')",
        )
        .bind(&id).bind(sid).bind(it["input"].as_str().or(it["searchHashtag"]["name"].as_str()).unwrap_or(""))
        .bind(it["text"].as_str().unwrap_or("").chars().take(500).collect::<String>())
        .bind(it["authorMeta"]["name"].as_str().unwrap_or(""))
        .bind(it["webVideoUrl"].as_str().unwrap_or(""))
        .bind(it["musicMeta"]["musicName"].as_str().unwrap_or("").chars().take(120).collect::<String>())
        .bind(json!(hashtags).to_string())
        .bind(it["createTimeISO"].as_str())
        .bind(it["playCount"].as_i64().unwrap_or(0)).bind(it["diggCount"].as_i64().unwrap_or(0)).bind(it["shareCount"].as_i64().unwrap_or(0)).bind(it["commentCount"].as_i64().unwrap_or(0))
        .execute(&st.db).await?;
        n += 1;
    }
    Ok(Json(json!({ "videos": n, "hashtags": tags })))
}

/// Tendances calculées sur les vidéos collectées : hashtags et sons qui montent, vidéos phares, codes de maps cités.
pub async fn tiktok(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    let cfg = apify_cfg(&st, sid).await?;
    let rows: Vec<(String, String, String, String, String, String, Option<String>, i64, i64, i64, i64, String)> = sqlx::query_as(
        "SELECT id, text, author, url, music, hashtags, created, plays, likes, shares, comments, fetched_at FROM tt_videos WHERE studio_id = ? AND (created IS NULL OR created > datetime('now', '-30 days')) ORDER BY plays DESC LIMIT 2000",
    ).bind(sid).fetch_all(&st.db).await?;
    let now = chrono::Utc::now();
    let recent = |c: &Option<String>, days: i64| c.as_deref().and_then(|c| chrono::DateTime::parse_from_rfc3339(c).ok()).map(|d| now.signed_duration_since(d).num_days() < days).unwrap_or(false);
    #[derive(Default)]
    struct T { videos: i64, plays: i64, recent_plays: i64, older_plays: i64 }
    let mut tags: HashMap<String, T> = HashMap::new();
    let mut sounds: HashMap<String, T> = HashMap::new();
    let mut codes: HashMap<String, (i64, i64)> = HashMap::new();
    let followed: Vec<String> = cfg["hashtags"].as_str().map(|s| s.split([',', ' ', '\n']).map(|t| t.trim().trim_start_matches('#').to_lowercase()).filter(|t| !t.is_empty()).collect()).filter(|v: &Vec<String>| !v.is_empty()).unwrap_or_else(|| DEFAULT_TAGS.iter().map(|s| s.to_string()).collect());
    let code_re = regex::Regex::new(r"\b\d{4}-\d{4}-\d{4}\b").unwrap();
    for r in &rows {
        let is_recent = recent(&r.6, 3);
        let hs: Vec<String> = serde_json::from_str(&r.5).unwrap_or_default();
        for h in hs.iter().filter(|h| !followed.contains(h) && !["fyp", "foryou", "foryoupage", "viral", "pourtoi", "fypシ"].contains(&h.as_str())) {
            let t = tags.entry(h.clone()).or_default();
            t.videos += 1; t.plays += r.7;
            if is_recent { t.recent_plays += r.7 } else { t.older_plays += r.7 }
        }
        if !r.4.is_empty() && !r.4.to_lowercase().starts_with("original sound") && !r.4.to_lowercase().starts_with("son original") {
            let t = sounds.entry(r.4.clone()).or_default();
            t.videos += 1; t.plays += r.7;
            if is_recent { t.recent_plays += r.7 } else { t.older_plays += r.7 }
        }
        for m in code_re.find_iter(&r.1) {
            let e = codes.entry(m.as_str().to_string()).or_default();
            e.0 += 1; e.1 += r.7;
        }
    }
    let rank = |m: HashMap<String, T>| {
        let mut v: Vec<Value> = m.into_iter().filter(|(_, t)| t.videos >= 2).map(|(k, t)| json!({ "name": k, "videos": t.videos, "plays": t.plays, "momentum": if t.older_plays > 0 { Some((t.recent_plays as f64 / t.older_plays as f64 * 100.0).round()) } else { None } })).collect();
        v.sort_by(|a, b| b["plays"].as_i64().cmp(&a["plays"].as_i64()));
        v.truncate(40);
        v
    };
    let mut maps: Vec<Value> = vec![];
    for (c, (vids, plays)) in codes {
        let title: Option<String> = sqlx::query_scalar("SELECT title FROM fn_islands WHERE code = ?").bind(&c).fetch_optional(&st.db).await?;
        maps.push(json!({ "code": c, "title": title, "videos": vids, "plays": plays }));
    }
    maps.sort_by(|a, b| b["plays"].as_i64().cmp(&a["plays"].as_i64()));
    let videos: Vec<Value> = rows.iter().filter(|r| recent(&r.6, 7)).take(30).map(|r| json!({ "id": r.0, "text": r.1, "author": r.2, "url": r.3, "music": r.4, "created": r.6, "plays": r.7, "likes": r.8, "shares": r.9, "comments": r.10 })).collect();
    let last: Option<String> = rows.iter().map(|r| r.11.clone()).max();
    Ok(Json(json!({ "configured": cfg["token"].as_str().map(|t| !t.is_empty()).unwrap_or(false), "followed": followed, "total": rows.len(), "last_fetch": last, "hashtags": rank(tags), "sounds": rank(sounds), "maps": maps, "videos": videos })))
}
