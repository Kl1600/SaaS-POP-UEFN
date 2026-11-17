//! Analyse : statistiques publiques des îles Fortnite (Fortnite Data API d'Epic, sans clé)
//! et tendances vidéo autour de Fortnite : TikTok (via Apify) et YouTube Shorts (API officielle, clé gratuite).
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

/// Joueurs du dernier jour mesuré pour une île (None = jamais mesurée ou moins de 5 joueurs).
async fn ensure_island(st: &AppState, code: &str) -> Result<bool, String> {
    let known: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fn_islands WHERE code = ?").bind(code).fetch_one(&st.db).await.map_err(|e| e.to_string())?;
    if known > 0 { return Ok(true) }
    let (status, v) = get_json(st, &format!("{}/islands/{code}", base())).await?;
    if status != 200 { return Ok(false) }
    upsert_island(st, &v, false).await.map_err(|e| e.to_string())?;
    Ok(true)
}

/// Maps les plus jouées en ce moment, d'après fortnite.gg (l'API d'Epic ne sait pas trier par popularité).
/// Sert seulement à savoir QUELLES îles mesurer : les chiffres viennent ensuite de l'API officielle.
async fn seed_popular(st: &AppState) -> Result<usize, String> {
    if std::env::var("POP_FORTNITE_GG").map(|v| v == "0").unwrap_or(false) { return Ok(0) }
    let base = std::env::var("POP_FORTNITE_GG_URL").unwrap_or_else(|_| "https://fortnite.gg".into());
    let re = regex::Regex::new(r"\b(\d{4}-\d{4}-\d{4})\b").unwrap();
    let mut codes: Vec<String> = vec![];
    for page in 1..=4 {
        let url = if page == 1 { format!("{base}/creative") } else { format!("{base}/creative?page={page}") };
        let r = st.http.get(&url)
            .header("user-agent", "Mozilla/5.0 (compatible; POP-UEFN/1.0)")
            .header("accept", "text/html,application/xhtml+xml")
            .header("accept-language", "fr-FR,fr;q=0.9,en;q=0.8")
            .timeout(std::time::Duration::from_secs(20)).send().await.map_err(|e| format!("fortnite.gg injoignable : {e}"))?;
        let status = r.status().as_u16();
        if status != 200 { if page == 1 { return Err(format!("fortnite.gg a refusé la lecture (HTTP {status})")) } else { break } }
        let html = r.text().await.unwrap_or_default();
        let before = codes.len();
        for c in re.captures_iter(&html) { let c = c[1].to_string(); if !codes.contains(&c) { codes.push(c) } }
        if codes.len() == before { break }
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    }
    for (i, c) in codes.iter().enumerate() {
        sqlx::query("INSERT INTO fn_seed (code, source, rank, seen_at) VALUES (?, 'fortnite.gg', ?, datetime('now')) ON CONFLICT(code) DO UPDATE SET rank = excluded.rank, seen_at = excluded.seen_at")
            .bind(c).bind(i as i64 + 1).execute(&st.db).await.map_err(|e| e.to_string())?;
    }
    Ok(codes.len())
}

/// Une passe de collecte :
/// 1. dernières sorties (début de la liste d'Epic, triée de la plus récente à la plus ancienne) ;
/// 2. suite du parcours complet du catalogue, reprise là où la passe précédente s'était arrêtée ;
/// 3. maps populaires repérées ailleurs (fortnite.gg, vidéos) ;
/// 4. mesures, par priorité : maps populaires et suivies, puis jamais mesurées, puis rafraîchissements.
pub async fn crawl(st: &AppState) -> Result<String, String> {
    sqlx::query("UPDATE fn_status SET last_run = datetime('now') WHERE id = 1").execute(&st.db).await.map_err(|e| e.to_string())?;
    let mut listed = 0;
    // 1. Nouveautés.
    let mut url = format!("{}/islands?size=100", base());
    for _ in 0..10 {
        let (status, v) = get_json(st, &url).await?;
        if status == 429 { break }
        if status != 200 { return Err(format!("API Fortnite (liste) : HTTP {status}")) }
        let data = v["data"].as_array().cloned().unwrap_or_default();
        for it in &data { upsert_island(st, it, true).await.map_err(|e| e.to_string())?; listed += 1 }
        match v["links"]["next"].as_str() { Some(n) if !n.is_empty() && !data.is_empty() => url = next_url(n), _ => break }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    // 2. Parcours complet, par tranches.
    let cursor: Option<String> = sqlx::query_scalar("SELECT cursor FROM fn_status WHERE id = 1").fetch_one(&st.db).await.map_err(|e| e.to_string())?;
    let mut url = cursor.filter(|c| !c.is_empty()).unwrap_or_else(|| format!("{}/islands?size=100", base()));
    let mut finished = false;
    for _ in 0..60 {
        let (status, v) = get_json(st, &url).await?;
        if status == 429 { break }
        if status != 200 { url = format!("{}/islands?size=100", base()); break }
        let data = v["data"].as_array().cloned().unwrap_or_default();
        for it in &data { upsert_island(st, it, false).await.map_err(|e| e.to_string())?; listed += 1 }
        match v["links"]["next"].as_str() {
            Some(n) if !n.is_empty() && !data.is_empty() => url = next_url(n),
            _ => { finished = true; url = String::new(); break }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    sqlx::query("UPDATE fn_status SET cursor = ?, sweeps = sweeps + ? WHERE id = 1").bind(&url).bind(finished as i64).execute(&st.db).await.map_err(|e| e.to_string())?;
    // 3. Maps populaires (au plus toutes les 3 heures).
    let due: bool = sqlx::query_scalar("SELECT seed_ok IS NULL OR seed_ok < datetime('now', '-3 hours') FROM fn_status WHERE id = 1").fetch_one(&st.db).await.map_err(|e| e.to_string())?;
    if due {
        match seed_popular(st).await {
            Ok(n) => { let _ = sqlx::query("UPDATE fn_status SET seed_ok = datetime('now'), seed_error = NULL WHERE id = 1").execute(&st.db).await; tracing::info!("analyse : {n} maps populaires repérées") }
            Err(e) => { let _ = sqlx::query("UPDATE fn_status SET seed_ok = datetime('now'), seed_error = ? WHERE id = 1").bind(&e).execute(&st.db).await; tracing::warn!("analyse : {e}") }
        }
    }
    // Codes cités dans les vidéos récentes (TikTok, YouTube) : souvent des maps qui buzzent.
    let texts: Vec<String> = sqlx::query_scalar("SELECT text FROM tt_videos WHERE fetched_at > datetime('now', '-3 days')").fetch_all(&st.db).await.map_err(|e| e.to_string())?;
    let code_re = regex::Regex::new(r"\b\d{4}-\d{4}-\d{4}\b").unwrap();
    for t in &texts { for m in code_re.find_iter(t) {
        let _ = sqlx::query("INSERT INTO fn_seed (code, source, seen_at) VALUES (?, 'vidéos', datetime('now')) ON CONFLICT(code) DO UPDATE SET seen_at = excluded.seen_at").bind(m.as_str()).execute(&st.db).await;
    } }
    let seeds: Vec<String> = sqlx::query_scalar("SELECT code FROM fn_seed WHERE code NOT IN (SELECT code FROM fn_islands) AND seen_at > datetime('now', '-3 days') ORDER BY rank IS NULL, rank")
        .fetch_all(&st.db).await.map_err(|e| e.to_string())?;
    for c in seeds.iter().filter(|c| valid_code(c)).take(150) { let _ = ensure_island(st, c).await; tokio::time::sleep(std::time::Duration::from_millis(150)).await; }

    // 4. Mesures par priorité. Les chiffres d'Epic sont journaliers : inutile de remesurer trop souvent.
    let codes: Vec<String> = sqlx::query_scalar(
        "SELECT code FROM (
           SELECT i.code, CASE
             WHEN i.code IN (SELECT code FROM fn_seed WHERE seen_at > datetime('now', '-3 days')) OR i.code IN (SELECT code FROM fn_watch) OR i.code IN (SELECT island_code FROM maps WHERE island_code IS NOT NULL) THEN 0
             WHEN i.metrics_at IS NULL AND i.seen_new = 1 THEN 1
             WHEN i.code IN (SELECT code FROM fn_metrics WHERE day = (SELECT MAX(day) FROM fn_metrics) AND unique_players >= 100) THEN 2
             WHEN i.metrics_at IS NULL THEN 3
             WHEN i.code IN (SELECT code FROM fn_metrics WHERE unique_players IS NOT NULL) THEN 4
             ELSE 5 END AS prio, i.metrics_at
           FROM fn_islands i
           WHERE i.metrics_at IS NULL
              OR (i.metrics_at < datetime('now', '-3 hours') AND (i.code IN (SELECT code FROM fn_seed) OR i.code IN (SELECT code FROM fn_watch) OR i.code IN (SELECT island_code FROM maps WHERE island_code IS NOT NULL)
                    OR i.code IN (SELECT code FROM fn_metrics WHERE day = (SELECT MAX(day) FROM fn_metrics) AND unique_players >= 100)))
              OR (i.metrics_at < datetime('now', '-20 hours') AND i.code IN (SELECT code FROM fn_metrics WHERE unique_players IS NOT NULL))
              OR i.metrics_at < datetime('now', '-7 days')
         ) ORDER BY prio, metrics_at IS NOT NULL, metrics_at LIMIT 900",
    ).fetch_all(&st.db).await.map_err(|e| e.to_string())?;
    let mut measured = 0;
    let (mut failed, mut in_a_row) = (0, 0);
    for c in &codes {
        // Une requête qui échoue (coupure réseau, délai dépassé) ne doit pas arrêter toute la collecte :
        // on réessaie une fois, puis on passe à l'île suivante. On ne s'arrête que si le réseau semble coupé.
        let mut r = fetch_metrics(st, c).await;
        if matches!(&r, Err(e) if !e.contains("limite")) { tokio::time::sleep(std::time::Duration::from_secs(2)).await; r = fetch_metrics(st, c).await; }
        match r {
            Ok(true) => { measured += 1; in_a_row = 0 }
            Ok(false) => { in_a_row = 0; let _ = sqlx::query("UPDATE fn_islands SET metrics_at = datetime('now') WHERE code = ?").bind(c).execute(&st.db).await; }
            Err(e) if e.contains("limite") => { tokio::time::sleep(std::time::Duration::from_secs(30)).await; break }
            Err(e) => {
                failed += 1; in_a_row += 1;
                tracing::debug!("analyse : {c} ignorée ({e})");
                if in_a_row >= 10 { return Err(format!("API Fortnite injoignable (10 échecs de suite) : {e}")) }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    if failed > 0 { tracing::info!("analyse : {failed} île(s) non mesurée(s) cette fois (réseau), elles seront retentées") }
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
            Ok(m) => { tracing::info!("analyse Fortnite : {m}"); crate::ideas::refresh_all(&st).await }
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

pub async fn status(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    crate::billing::require_market(&st, &u).await?;
    let (last_run, last_ok, err, islands): (Option<String>, Option<String>, Option<String>, i64) = sqlx::query_as("SELECT last_run, last_ok, last_error, islands FROM fn_status WHERE id = 1").fetch_one(&st.db).await?;
    let latest: Option<String> = sqlx::query_scalar("SELECT MAX(day) FROM fn_metrics").fetch_one(&st.db).await?;
    let measured: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT code) FROM fn_metrics").fetch_one(&st.db).await?;
    let active: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT code) FROM fn_metrics WHERE unique_players > 0 AND day >= date((SELECT MAX(day) FROM fn_metrics), '-1 day')").fetch_one(&st.db).await?;
    let (sweeps, cursor, seed_ok, seed_err): (i64, Option<String>, Option<String>, Option<String>) = sqlx::query_as("SELECT sweeps, cursor, seed_ok, seed_error FROM fn_status WHERE id = 1").fetch_one(&st.db).await?;
    let seeds: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fn_seed WHERE seen_at > datetime('now', '-3 days')").fetch_one(&st.db).await?;
    let unmeasured: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fn_islands WHERE metrics_at IS NULL").fetch_one(&st.db).await?;
    Ok(Json(json!({ "last_run": last_run, "last_ok": last_ok, "error": err, "islands": islands, "measured": measured, "active": active, "latest_day": latest,
        "catalog_done": sweeps > 0, "sweeping": cursor.map(|c| !c.is_empty()).unwrap_or(false), "unmeasured": unmeasured,
        "popular_source": { "count": seeds, "last": seed_ok, "error": seed_err } })))
}

static REFRESHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub async fn refresh(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    crate::billing::require_market(&st, &u).await?;
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
pub async fn trending(State(st): State<AppState>, u: AuthUser, Query(q): Query<ListQ>) -> ApiResult<Value> {
    crate::billing::require_analyse(&st, &u).await?;
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
pub async fn launches(State(st): State<AppState>, u: AuthUser, Query(q): Query<ListQ>) -> ApiResult<Value> {
    crate::billing::require_analyse(&st, &u).await?;
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

/// Analyse du marché : pour chaque genre (tag), joueurs, concurrence, rétention, évolution sur 7 jours
/// et un verdict simple (opportunité, très disputé, saturé, niche).
pub async fn market(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    crate::billing::require_market(&st, &u).await?;
    let islands: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT code, title, tags, (seen_new = 1 AND first_seen > datetime('now', '-7 days')) FROM fn_islands WHERE code NOT LIKE 'playlist_%'",
    ).fetch_all(&st.db).await?;
    let metrics: Vec<(String, String, Option<i64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT code, day, unique_players, d1, avg_minutes FROM fn_metrics WHERE code NOT LIKE 'playlist_%' AND day >= date((SELECT MAX(day) FROM fn_metrics), '-7 days')",
    ).fetch_all(&st.db).await?;
    let last_day = metrics.iter().filter(|m| m.2.is_some()).map(|m| m.1.clone()).max();
    let Some(last_day) = last_day else {
        return Ok(Json(json!({ "islands": 0, "total_players": 0, "tags": [], "days": [], "new_per_day": [], "players_per_day": [] })));
    };
    let mut days: Vec<String> = metrics.iter().map(|m| m.1.clone()).collect(); days.sort(); days.dedup();
    let first_day = days.first().cloned().unwrap_or_default();
    // Par île : joueurs par jour, dernière rétention, minutes.
    #[derive(Default)]
    struct I { by_day: HashMap<String, i64>, d1: Option<f64>, mins: Option<f64> }
    let mut per: HashMap<String, I> = HashMap::new();
    for (code, day, up, d1, mins) in &metrics {
        let e = per.entry(code.clone()).or_default();
        if let Some(u) = up { e.by_day.insert(day.clone(), *u); }
        if d1.is_some() && day == &last_day { e.d1 = *d1 }
        if mins.is_some() && day == &last_day { e.mins = *mins }
    }
    #[derive(Default)]
    struct Agg { islands: i64, players: i64, first: i64, same: i64, d1_sum: f64, d1_n: i64, min_sum: f64, min_n: i64, new: i64, by_day: HashMap<String, i64>, top: Vec<(i64, String, String)> }
    let mut by: HashMap<String, Agg> = HashMap::new();
    let mut total_players = 0;
    let mut measured = 0;
    let mut total_by_day: HashMap<String, i64> = HashMap::new();
    let (mut tot_first, mut tot_same) = (0i64, 0i64);
    for (code, title, tags, new) in &islands {
        let Some(m) = per.get(code) else { continue };
        let up = *m.by_day.get(&last_day).unwrap_or(&0);
        // Seules les maps jouées le dernier jour comptent : une map morte ne dit rien du marché actuel.
        if up == 0 { continue }
        measured += 1; total_players += up;
        for (d, v) in &m.by_day { *total_by_day.entry(d.clone()).or_default() += v }
        if let (Some(f), Some(l)) = (m.by_day.get(&first_day), m.by_day.get(&last_day)) { tot_first += f; tot_same += l }
        let tags: Vec<String> = serde_json::from_str(tags).unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        for t in tags.iter().take(6).map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()) {
            if !seen.insert(t.clone()) { continue }
            let a = by.entry(t).or_default();
            a.islands += 1; a.players += up; a.new += *new;
            // Évolution à périmètre constant : seulement les maps mesurées le premier ET le dernier jour.
            if let (Some(f), Some(l)) = (m.by_day.get(&first_day), m.by_day.get(&last_day)) { a.first += f; a.same += l }
            for (d, v) in &m.by_day { *a.by_day.entry(d.clone()).or_default() += v }
            if let Some(d) = m.d1 { a.d1_sum += d; a.d1_n += 1 }
            if let Some(x) = m.mins { a.min_sum += x; a.min_n += 1 }
            a.top.push((up, code.clone(), title.clone()));
        }
    }
    let ready: Vec<(String, Agg)> = by.into_iter().filter(|(_, a)| a.islands >= 3 && a.players > 0).collect();
    let median = |mut v: Vec<f64>| { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); if v.is_empty() { 0.0 } else { v[v.len() / 2] } };
    // Joueurs par map : la moyenne est écrasée par une ou deux maps géantes, la médiane d'une map « normale » est plus parlante.
    let typical = |a: &Agg| { let mut v: Vec<i64> = a.top.iter().map(|x| x.0).collect(); v.sort(); v[v.len() / 2] as f64 };
    let med_typ = median(ready.iter().map(|(_, a)| typical(a)).collect());
    let med_islands = median(ready.iter().map(|(_, a)| a.islands as f64).collect());
    let med_new = median(ready.iter().map(|(_, a)| a.new as f64 / a.islands as f64).collect());
    let mut tags: Vec<Value> = ready.into_iter().map(|(t, mut a)| {
        let avg = a.players as f64 / a.islands as f64;
        let typ = typical(&a);
        let d1 = if a.d1_n > 0 { Some(a.d1_sum / a.d1_n as f64) } else { None };
        let mins = if a.min_n > 0 { Some(a.min_sum / a.min_n as f64) } else { None };
        let trend = if a.first > 0 && first_day != last_day { Some(((a.same as f64 / a.first as f64) - 1.0) * 100.0) } else { None };
        a.top.sort_by(|x, y| y.0.cmp(&x.0));
        let top_share = if a.players > 0 { a.top.iter().take(3).map(|x| x.0).sum::<i64>() as f64 / a.players as f64 } else { 0.0 };
        let crowded = a.islands as f64 > med_islands * 1.5 || a.new as f64 / a.islands as f64 > med_new * 1.5;
        let strong = typ >= med_typ;
        // Verdict en mots simples.
        let verdict = if strong && !crowded && top_share < 0.85 { "opportunite" }
            else if strong && crowded { "dispute" }
            else if top_share >= 0.85 && a.islands >= 5 { "domine" }
            else if !strong && crowded { "sature" }
            else { "niche" };
        let score = typ.ln_1p() * (1.0 + d1.unwrap_or(0.1)) * (1.0 + trend.unwrap_or(0.0).clamp(-50.0, 100.0) / 200.0) / (1.0 + a.new as f64 / a.islands as f64) * (1.0 - top_share * 0.5);
        let series: Vec<Value> = days.iter().map(|d| json!({ "day": d, "players": a.by_day.get(d).copied() })).collect();
        json!({
            "tag": t, "islands": a.islands, "players": a.players, "avg_players": avg.round(), "typical_players": typ.round(), "top_players": a.top.first().map(|x| x.0).unwrap_or(0),
            "d1": d1, "minutes": mins.map(|m| m.round()), "new_7d": a.new, "trend": trend.map(|t| t.round()), "top3_share": (top_share * 100.0).round(),
            "share": if total_players > 0 { a.players as f64 / total_players as f64 * 100.0 } else { 0.0 },
            "score": (score * 10.0).round() / 10.0, "verdict": verdict, "series": series,
            "top": a.top.iter().filter(|x| x.0 > 0).take(5).map(|x| json!({ "code": x.1, "title": x.2, "players": x.0 })).collect::<Vec<_>>(),
        })
    }).collect();
    tags.sort_by(|a, b| b["players"].as_i64().cmp(&a["players"].as_i64()));
    let new_per_day: Vec<(String, i64)> = sqlx::query_as("SELECT date(first_seen), COUNT(*) FROM fn_islands WHERE seen_new = 1 AND first_seen > datetime('now', '-14 days') GROUP BY 1 ORDER BY 1").fetch_all(&st.db).await?;
    Ok(Json(json!({
        "islands": measured, "total_players": total_players, "last_day": last_day, "first_day": first_day, "typical_players": med_typ.round(),
        "total_trend": if tot_first > 0 && first_day != last_day { Some(((tot_same as f64 / tot_first as f64 - 1.0) * 100.0).round()) } else { None },
        "tags": tags,
        "new_per_day": new_per_day.into_iter().map(|(d, n)| json!({ "day": d, "count": n })).collect::<Vec<_>>(),
        "players_per_day": days.iter().map(|d| json!({ "day": d, "players": total_by_day.get(d) })).collect::<Vec<_>>(),
    })))
}

pub async fn island(State(st): State<AppState>, u: AuthUser, Path(code): Path<String>) -> ApiResult<Value> {
    crate::billing::require_market(&st, &u).await?;
    if !valid_code(&code) { return Err(bad("Code d’île invalide (format 1234-5678-9012).")) }
    let meta: Option<(String, String, String, String, String)> = sqlx::query_as("SELECT code, title, creator, category, tags FROM fn_islands WHERE code = ?").bind(&code).fetch_optional(&st.db).await?;
    let meta = match meta {
        Some(m) => m,
        None => {
            // Île jamais vue : on la récupère tout de suite. Si Epic ne la connaît pas, on le dit sans planter.
            let reason = match get_json(&st, &format!("{}/islands/{code}", base())).await {
                Ok((200, v)) => {
                    let mut v = v;
                    if v["code"].as_str().is_none() { v["code"] = json!(code) }
                    upsert_island(&st, &v, false).await?;
                    let _ = fetch_metrics(&st, &code).await;
                    None
                }
                Ok((404, _)) => Some("Cette île n’est pas dans l’API publique d’Epic : elle est privée, non découvrable, retirée, ou le code cité dans la vidéo est faux.".to_string()),
                Ok((429, _)) => Some("L’API Fortnite limite les requêtes en ce moment : réessayez dans une minute.".to_string()),
                Ok((s, _)) => Some(format!("L’API Fortnite a répondu une erreur (HTTP {s}).")),
                Err(e) => Some(format!("API Fortnite injoignable : {e}")),
            };
            let row: Option<(String, String, String, String, String)> = sqlx::query_as("SELECT code, title, creator, category, tags FROM fn_islands WHERE code = ?").bind(&code).fetch_optional(&st.db).await?;
            match row {
                Some(r) => r,
                None => return Ok(Json(json!({ "code": code, "title": "", "creator": "", "tags": [], "series": [], "watched": false,
                    "unavailable": reason.unwrap_or_else(|| "Île introuvable dans l’API Fortnite.".into()) }))),
            }
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
    crate::billing::require_market(&st, &u).await?;
    let code = b.code.trim().to_string();
    if !valid_code(&code) { return Err(bad("Code d’île invalide (format 1234-5678-9012).")) }
    if b.on {
        sqlx::query("INSERT OR IGNORE INTO fn_watch (code, added_by) VALUES (?, ?)").bind(&code).bind(u.id).execute(&st.db).await?;
    } else {
        sqlx::query("DELETE FROM fn_watch WHERE code = ?").bind(&code).execute(&st.db).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn watched(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    crate::billing::require_market(&st, &u).await?;
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

const DEFAULT_TAGS: &[&str] = &["fortnite", "fortnitecreative", "uefn", "fortnitemap", "fortnitemaps", "fortnitecodes", "fortnitemapcode", "fortnitetycoon", "fortniteboxfight", "fortnitezonewars"];
const DEFAULT_KEYWORDS: &[&str] = &["fortnite map code", "fortnite creative map", "uefn map", "best fortnite maps", "fortnite tycoon", "fortnite horror map"];
const NOISE: &[&str] = &["fyp", "foryou", "foryoupage", "viral", "pourtoi", "fypシ", "shorts", "short", "youtubeshorts", "trending", "xyzbca", "fy", "fypage", "fortnite", "fortnitebr", "fortniteclips", "fortnitememes", "fortnitefunny", "gaming", "gamer", "game", "games", "videogames", "epicgames", "fortnitebattleroyale", "fortnitecreative", "fortniteclip", "fortnitegameplay"];

fn split_list(v: &Value) -> Vec<String> {
    v.as_str().unwrap_or("").split([',', '\n']).map(|t| t.trim().to_string()).filter(|t| !t.is_empty() && t.len() < 80).collect()
}
fn followed_tags(cfg: &Value) -> Vec<String> {
    let v: Vec<String> = cfg["hashtags"].as_str().unwrap_or("").split([',', ' ', '\n']).map(|t| t.trim().trim_start_matches('#').to_lowercase()).filter(|t| !t.is_empty() && t.len() < 60).collect();
    if v.is_empty() { DEFAULT_TAGS.iter().map(|s| s.to_string()).collect() } else { v }
}
fn keywords(cfg: &Value) -> Vec<String> {
    let v = split_list(&cfg["keywords"]);
    if v.is_empty() { DEFAULT_KEYWORDS.iter().map(|s| s.to_string()).collect() } else { v }
}

async fn integ_cfg(st: &AppState, sid: i64, kind: &str) -> Result<Value, AppError> {
    let r: Option<(i64, String)> = sqlx::query_as("SELECT enabled, config FROM integrations WHERE studio_id = ? AND kind = ?").bind(sid).bind(kind).fetch_optional(&st.db).await?;
    Ok(r.filter(|r| r.0 == 1).map(|r| serde_json::from_str(&r.1).unwrap_or(json!({}))).unwrap_or(json!({})))
}
fn secret<'a>(cfg: &'a Value, k: &str) -> Option<&'a str> { cfg[k].as_str().filter(|t| !t.is_empty()) }

struct Vid { id: String, platform: &'static str, query: String, text: String, author: String, url: String, music: String, hashtags: Vec<String>, created: Option<String>, plays: i64, likes: i64, shares: i64, comments: i64, thumb: String }

async fn store(st: &AppState, sid: i64, v: &Vid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tt_videos (id, studio_id, query, text, author, url, music, hashtags, created, plays, likes, shares, comments, platform, thumb) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id, studio_id) DO UPDATE SET plays = excluded.plays, likes = excluded.likes, shares = excluded.shares, comments = excluded.comments, thumb = excluded.thumb, fetched_at = datetime('now')",
    )
    .bind(&v.id).bind(sid).bind(&v.query).bind(v.text.chars().take(500).collect::<String>()).bind(&v.author).bind(&v.url)
    .bind(v.music.chars().take(120).collect::<String>()).bind(json!(v.hashtags).to_string()).bind(&v.created)
    .bind(v.plays).bind(v.likes).bind(v.shares).bind(v.comments).bind(v.platform).bind(&v.thumb)
    .execute(&st.db).await?;
    Ok(())
}

fn hashtags_in(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r"#([\p{L}\p{N}_]{2,40})").unwrap();
    re.captures_iter(text).map(|c| c[1].to_lowercase()).collect()
}

/// TikTok via Apify (acteur « clockworks/tiktok-scraper ») : hashtags ET recherches par mots-clés.
async fn collect_tiktok(st: &AppState, sid: i64, cfg: &Value) -> Result<usize, String> {
    let token = secret(cfg, "token").ok_or("pas de jeton Apify")?;
    let mut tags = followed_tags(cfg); tags.truncate(15);
    let mut kw = keywords(cfg); kw.truncate(10);
    let per = cfg["per_tag"].as_i64().unwrap_or(100).clamp(10, 300);
    let input = json!({ "hashtags": tags, "searchQueries": kw, "resultsPerPage": per, "searchSection": "/video", "shouldDownloadVideos": false, "shouldDownloadCovers": false, "shouldDownloadSubtitles": false, "shouldDownloadSlideshowImages": false });
    let r = st.http.post(format!("{}/v2/acts/clockworks~tiktok-scraper/run-sync-get-dataset-items?token={token}&timeout=290", std::env::var("POP_APIFY_API").unwrap_or_else(|_| "https://api.apify.com".into())))
        .json(&input).timeout(std::time::Duration::from_secs(300)).send().await.map_err(|e| format!("Apify injoignable : {e}"))?;
    let status = r.status();
    let body: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() { return Err(format!("Apify : {}", body["error"]["message"].as_str().unwrap_or(status.as_str()))) }
    let mut n = 0;
    for it in body.as_array().cloned().unwrap_or_default() {
        let Some(id) = it["id"].as_str().map(String::from).or(it["id"].as_i64().map(|x| x.to_string())) else { continue };
        let text = it["text"].as_str().unwrap_or("").to_string();
        let mut hs: Vec<String> = it["hashtags"].as_array().cloned().unwrap_or_default().iter().filter_map(|h| h["name"].as_str().or(h.as_str()).map(|s| s.to_lowercase())).filter(|s| !s.is_empty()).collect();
        if hs.is_empty() { hs = hashtags_in(&text) }
        let v = Vid {
            id, platform: "tiktok", query: it["input"].as_str().or(it["searchHashtag"]["name"].as_str()).or(it["searchQuery"].as_str()).unwrap_or("").into(), text,
            author: it["authorMeta"]["name"].as_str().unwrap_or("").into(), url: it["webVideoUrl"].as_str().unwrap_or("").into(),
            music: it["musicMeta"]["musicName"].as_str().unwrap_or("").into(), hashtags: hs, created: it["createTimeISO"].as_str().map(String::from),
            plays: it["playCount"].as_i64().unwrap_or(0), likes: it["diggCount"].as_i64().unwrap_or(0), shares: it["shareCount"].as_i64().unwrap_or(0), comments: it["commentCount"].as_i64().unwrap_or(0),
            thumb: it["videoMeta"]["coverUrl"].as_str().unwrap_or("").into(),
        };
        if store(st, sid, &v).await.is_ok() { n += 1 }
    }
    Ok(n)
}

/// YouTube Shorts via l'API officielle YouTube Data v3 (clé gratuite, 10 000 unités/jour : une collecte en consomme ~900).
async fn collect_youtube(st: &AppState, sid: i64, cfg: &Value, social: &Value) -> Result<usize, String> {
    let key = secret(cfg, "key").ok_or("pas de clé YouTube")?;
    let base = std::env::var("POP_YOUTUBE_API").unwrap_or_else(|_| "https://www.googleapis.com/youtube/v3".into());
    let after = (chrono::Utc::now() - chrono::Duration::days(14)).format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut queries: Vec<String> = keywords(social);
    queries.extend(followed_tags(social).into_iter().take(3).map(|t| format!("#{t}")));
    queries.truncate(9);
    let mut ids: Vec<(String, String)> = vec![];
    for q in &queries {
        for order in ["viewCount", "date"] {
            let url = format!("{base}/search?part=id&type=video&videoDuration=short&maxResults=50&order={order}&publishedAfter={after}&q={}&key={key}", urlencoding(q));
            let (code, v) = get_json(st, &url).await?;
            if code != 200 { return Err(format!("YouTube : {}", v["error"]["message"].as_str().unwrap_or("erreur"))) }
            for it in v["items"].as_array().cloned().unwrap_or_default() {
                if let Some(id) = it["id"]["videoId"].as_str() { if !ids.iter().any(|x| x.0 == id) { ids.push((id.into(), q.clone())) } }
            }
            if order == "date" && queries.len() > 6 { break }
        }
    }
    let mut n = 0;
    for chunk in ids.chunks(50) {
        let list = chunk.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(",");
        let (code, v) = get_json(st, &format!("{base}/videos?part=snippet,statistics&id={list}&key={key}")).await?;
        if code != 200 { return Err(format!("YouTube : {}", v["error"]["message"].as_str().unwrap_or("erreur"))) }
        for it in v["items"].as_array().cloned().unwrap_or_default() {
            let Some(id) = it["id"].as_str() else { continue };
            let sn = &it["snippet"]; let stt = &it["statistics"];
            let title = sn["title"].as_str().unwrap_or(""); let desc = sn["description"].as_str().unwrap_or("");
            let text = if desc.is_empty() { title.to_string() } else { format!("{title}\n{desc}") };
            let mut hs = hashtags_in(&text);
            for t in sn["tags"].as_array().cloned().unwrap_or_default() { if let Some(t) = t.as_str() { let t = t.to_lowercase().replace(' ', ""); if t.len() > 1 && t.len() < 40 { hs.push(t) } } }
            hs.sort(); hs.dedup();
            let num = |k: &str| stt[k].as_str().and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
            let v = Vid {
                id: format!("yt:{id}"), platform: "youtube", query: chunk.iter().find(|x| x.0 == id).map(|x| x.1.clone()).unwrap_or_default(), text,
                author: sn["channelTitle"].as_str().unwrap_or("").into(), url: format!("https://www.youtube.com/shorts/{id}"), music: String::new(), hashtags: hs,
                created: sn["publishedAt"].as_str().map(String::from), plays: num("viewCount"), likes: num("likeCount"), shares: 0, comments: num("commentCount"),
                thumb: sn["thumbnails"]["medium"]["url"].as_str().unwrap_or("").into(),
            };
            if store(st, sid, &v).await.is_ok() { n += 1 }
        }
    }
    Ok(n)
}

fn urlencoding(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

async fn set_status(st: &AppState, sid: i64, platform: &str, r: &Result<usize, String>) {
    let (ok, err, count) = match r { Ok(n) => (true, None, *n as i64), Err(e) => (false, Some(e.clone()), 0) };
    let _ = sqlx::query(
        "INSERT INTO social_status (studio_id, platform, last_run, last_ok, last_error, last_count) VALUES (?, ?, datetime('now'), CASE WHEN ? THEN datetime('now') END, ?, ?)
         ON CONFLICT(studio_id, platform) DO UPDATE SET last_run = excluded.last_run, last_ok = COALESCE(excluded.last_ok, social_status.last_ok), last_error = excluded.last_error, last_count = CASE WHEN ? THEN excluded.last_count ELSE social_status.last_count END",
    ).bind(sid).bind(platform).bind(ok).bind(err).bind(count).bind(ok).execute(&st.db).await;
}

/// Collecte toutes les sources configurées du studio.
async fn collect_all(st: &AppState, sid: i64, with_tiktok: bool) -> Result<Value, AppError> {
    let social = integ_cfg(st, sid, "apify").await?;
    let yt = integ_cfg(st, sid, "youtube").await?;
    let mut out = json!({});
    if with_tiktok && secret(&social, "token").is_some() {
        let r = collect_tiktok(st, sid, &social).await; set_status(st, sid, "tiktok", &r).await;
        out["tiktok"] = match r { Ok(n) => json!(n), Err(e) => json!({ "error": e }) };
    }
    if secret(&yt, "key").is_some() {
        let r = collect_youtube(st, sid, &yt, &social).await; set_status(st, sid, "youtube", &r).await;
        out["youtube"] = match r { Ok(n) => json!(n), Err(e) => json!({ "error": e }) };
    }
    Ok(out)
}

/// Collecte automatique des vidéos : YouTube toutes les 6 h (gratuit) ; TikTok une fois par jour si « auto » est activé.
pub async fn social_loop(st: AppState) {
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    let mut tick: u64 = 0;
    loop {
        let rows: Vec<(i64, String, String)> = sqlx::query_as("SELECT studio_id, kind, config FROM integrations WHERE kind IN ('apify', 'youtube') AND enabled = 1").fetch_all(&st.db).await.unwrap_or_default();
        let mut sids: Vec<i64> = rows.iter().map(|r| r.0).collect(); sids.sort(); sids.dedup();
        for sid in sids {
            let tt_auto = rows.iter().any(|r| r.0 == sid && r.1 == "apify" && serde_json::from_str::<Value>(&r.2).map(|c| c["auto"].as_bool().unwrap_or(false)).unwrap_or(false));
            let _ = collect_all(&st, sid, tt_auto && tick % 4 == 0).await;
        }
        tick += 1;
        tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
    }
}

pub async fn tiktok_refresh(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member { return Err(AppError::Forbidden) }
    crate::billing::require_studio(&st, sid).await?;
    let social = integ_cfg(&st, sid, "apify").await?;
    let yt = integ_cfg(&st, sid, "youtube").await?;
    if secret(&social, "token").is_none() && secret(&yt, "key").is_none() {
        return Err(bad("Ajoutez une source : clé YouTube (gratuite) ou jeton Apify pour TikTok."));
    }
    let out = collect_all(&st, sid, true).await?;
    let total = out["tiktok"].as_i64().unwrap_or(0) + out["youtube"].as_i64().unwrap_or(0);
    let errors: Vec<String> = ["tiktok", "youtube"].iter().filter_map(|k| out[*k]["error"].as_str().map(|e| e.to_string())).collect();
    if total == 0 && !errors.is_empty() { return Err(AppError::Conflict(errors.join(" · "))) }
    Ok(Json(json!({ "videos": total, "detail": out, "errors": errors })))
}

/// Tendances calculées sur les vidéos collectées (TikTok + YouTube Shorts).
pub async fn tiktok(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    crate::billing::require_studio(&st, sid).await?;
    let social = integ_cfg(&st, sid, "apify").await?;
    let yt = integ_cfg(&st, sid, "youtube").await?;
    let rows: Vec<(String, String, String, String, String, String, Option<String>, i64, i64, i64, i64, String, String, String)> = sqlx::query_as(
        "SELECT id, text, author, url, music, hashtags, created, plays, likes, shares, comments, fetched_at, platform, thumb FROM tt_videos WHERE studio_id = ? AND (created IS NULL OR created > datetime('now', '-30 days')) ORDER BY plays DESC LIMIT 5000",
    ).bind(sid).fetch_all(&st.db).await?;
    let now = chrono::Utc::now();
    let age = |c: &Option<String>| c.as_deref().and_then(|c| chrono::DateTime::parse_from_rfc3339(c).ok()).map(|d| now.signed_duration_since(d).num_hours() as f64 / 24.0);
    #[derive(Default)]
    struct T { videos: i64, plays: i64, recent: i64, older: i64, recent_v: i64, platforms: u8 }
    let mut tags: HashMap<String, T> = HashMap::new();
    let mut sounds: HashMap<String, T> = HashMap::new();
    let mut codes: HashMap<String, (i64, i64)> = HashMap::new();
    let followed = followed_tags(&social);
    let code_re = regex::Regex::new(r"\b\d{4}-\d{4}-\d{4}\b").unwrap();
    let mut by_platform: HashMap<String, (i64, i64)> = HashMap::new();
    let (mut g_recent, mut g_all) = (0i64, 0i64);
    for r in &rows {
        let a = age(&r.6);
        let is_recent = a.map(|a| a < 3.0).unwrap_or(false);
        let pbit = if r.12 == "youtube" { 2 } else { 1 };
        g_all += r.7; if is_recent { g_recent += r.7 }
        let bp = by_platform.entry(r.12.clone()).or_default(); bp.0 += 1; bp.1 += r.7;
        let hs: Vec<String> = serde_json::from_str(&r.5).unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        for h in hs.iter().filter(|h| !followed.contains(h) && !NOISE.contains(&h.as_str())) {
            if !seen.insert(h.clone()) { continue }
            let t = tags.entry(h.clone()).or_default();
            t.videos += 1; t.plays += r.7; t.platforms |= pbit;
            if is_recent { t.recent += r.7; t.recent_v += 1 } else { t.older += r.7 }
        }
        let m = r.4.to_lowercase();
        if !r.4.is_empty() && !m.starts_with("original sound") && !m.starts_with("son original") && !m.starts_with("sonido original") {
            let t = sounds.entry(r.4.clone()).or_default();
            t.videos += 1; t.plays += r.7;
            if is_recent { t.recent += r.7; t.recent_v += 1 } else { t.older += r.7 }
        }
        for c in code_re.find_iter(&r.1) {
            let e = codes.entry(c.as_str().to_string()).or_default();
            e.0 += 1; e.1 += r.7;
        }
    }
    // « En hausse » : la part des vues récentes du hashtag dépasse nettement celle de l'ensemble des vidéos.
    let base_share = if g_all > 0 { g_recent as f64 / g_all as f64 } else { 0.0 };
    let rank = |m: HashMap<String, T>, min: i64| {
        let mut v: Vec<Value> = m.into_iter().filter(|(_, t)| t.videos >= min).map(|(k, t)| {
            let momentum = if t.older > 0 { Some((t.recent as f64 / t.older as f64 * 100.0).round()) } else { None };
            json!({ "name": k, "videos": t.videos, "plays": t.plays, "recent_videos": t.recent_v, "momentum": momentum,
                    "rising": t.recent_v >= 2 && t.plays > 0 && (t.recent as f64 / t.plays as f64) > (base_share * 1.4).max(0.15), "platforms": t.platforms })
        }).collect();
        v.sort_by(|a, b| b["plays"].as_i64().cmp(&a["plays"].as_i64()));
        v.truncate(60);
        v
    };
    let mut maps: Vec<Value> = vec![];
    for (c, (vids, plays)) in codes {
        let title: Option<String> = sqlx::query_scalar("SELECT title FROM fn_islands WHERE code = ?").bind(&c).fetch_optional(&st.db).await?;
        maps.push(json!({ "code": c, "title": title, "videos": vids, "plays": plays }));
    }
    maps.sort_by(|a, b| b["plays"].as_i64().cmp(&a["plays"].as_i64()));
    // Vidéos phares : vues par jour depuis la publication, pour que les vidéos récentes qui décollent remontent.
    let mut videos: Vec<(f64, Value)> = rows.iter().filter_map(|r| {
        let a = age(&r.6)?; if a > 7.0 { return None }
        let speed = r.7 as f64 / a.max(1.0);
        Some((speed, json!({ "id": r.0, "text": r.1, "author": r.2, "url": r.3, "music": r.4, "created": r.6, "plays": r.7, "likes": r.8, "shares": r.9, "comments": r.10, "platform": r.12, "thumb": r.13, "per_day": speed.round() })))
    }).collect();
    videos.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let status: Vec<(String, Option<String>, Option<String>, Option<String>, i64)> = sqlx::query_as("SELECT platform, last_run, last_ok, last_error, last_count FROM social_status WHERE studio_id = ?").bind(sid).fetch_all(&st.db).await?;
    let src = |p: &str, on: bool| {
        let s = status.iter().find(|s| s.0 == p);
        json!({ "configured": on, "videos": by_platform.get(p).map(|x| x.0).unwrap_or(0), "last_ok": s.and_then(|s| s.2.clone()), "error": s.and_then(|s| s.3.clone()) })
    };
    let last: Option<String> = rows.iter().map(|r| r.11.clone()).max();
    Ok(Json(json!({
        "configured": secret(&social, "token").is_some() || secret(&yt, "key").is_some(),
        "sources": { "tiktok": src("tiktok", secret(&social, "token").is_some()), "youtube": src("youtube", secret(&yt, "key").is_some()) },
        "followed": followed, "keywords": keywords(&social), "auto_tiktok": social["auto"].as_bool().unwrap_or(false),
        "total": rows.len(), "last_fetch": last, "hashtags": rank(tags, 3), "sounds": rank(sounds, 2), "maps": maps,
        "videos": videos.into_iter().take(36).map(|x| x.1).collect::<Vec<_>>(),
    })))
}
