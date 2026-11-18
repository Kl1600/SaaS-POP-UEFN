//! Onglet « Discover » de Lancer et suivre : préparer la sortie selon les règles de Discover,
//! puis suivre la map pendant les 14 jours où Epic la teste, et après.
//!
//! - Avant : titre comparé au catalogue (copies, mots trop repris, bourrage de mots-clés), genre officiel conseillé,
//!   vignette (analysée dans le navigateur), bande-annonce, plan pour amener ses propres joueurs.
//! - Pendant les 14 jours : la courbe de la map face aux nouvelles maps de sa niche au même âge, chaque signal
//!   noté par Epic face à la niche, et l'alerte « ça monte » à partir des joueurs heure par heure.
//! - Après : baisse détectée → conseil de mise à jour ; effet de chaque mise à jour publiée.
use crate::{access::{map_ctx, MapCtx, Role}, auth::AuthUser, error::{bad, ApiResult, AppError}, ideas, AppState};
use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// Tout l'onglet suit le plan Studio : celui du compte, ou celui du créateur du studio de la map.
async fn access(st: &AppState, u: &AuthUser, mid: i64) -> Result<MapCtx, AppError> {
    let ctx = map_ctx(st, u, mid).await?;
    if crate::billing::user_analyse(st, u.id).await? || crate::billing::studio_analyse(st, ctx.studio_id).await? { return Ok(ctx) }
    Err(AppError::Guard("plan", "L’onglet Discover est compris dans le plan Studio.".into()))
}

fn words(s: &str) -> Vec<String> { ideas::norm(s).split_whitespace().filter(|w| w.len() >= 2 && !ideas::STOP.contains(w)).map(String::from).collect() }

#[derive(Deserialize)]
pub struct Q { title: Option<String> }

pub async fn get(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Query(q): Query<Q>) -> ApiResult<Value> {
    access(&st, &u, mid).await?;
    let (name, map_genre, code): (String, String, Option<String>) = sqlx::query_as("SELECT name, genre, island_code FROM maps WHERE id = ?").bind(mid).fetch_one(&st.db).await?;
    let saved: Option<(String, String, i64, i64, String)> = sqlx::query_as("SELECT title, thumb, trailer, channels, updates FROM map_discover WHERE map_id = ?").bind(mid).fetch_optional(&st.db).await?;
    let (s_title, thumb, trailer, channels, updates) = saved.unwrap_or_default();
    let island: Option<(String, String)> = match &code { Some(c) => sqlx::query_as("SELECT title, tags FROM fn_islands WHERE code = ?").bind(c).fetch_optional(&st.db).await?, None => None };
    let title = q.title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
        .or_else(|| Some(s_title.clone()).filter(|t| !t.is_empty()))
        .or_else(|| island.as_ref().map(|i| i.0.clone()).filter(|t| !t.is_empty()))
        .unwrap_or(name.clone());
    let tags: Vec<String> = island.as_ref().map(|i| serde_json::from_str(&i.1).unwrap_or_default()).unwrap_or_default();
    let text = ideas::norm(&format!("{title} {}", tags.join(" ")));

    // Niches de la map (thèmes reconnus dans le titre et les tags).
    let themes = ideas::themes();
    let matched: Vec<&ideas::Theme> = themes.iter().filter(|t| t.flag != "risque" && t.pats.iter().any(|p| text.contains(p.as_str()))).collect();
    let all = ideas::niches(&st).await?;
    let niche_of = |key: &str| all["niches"].as_array().and_then(|a| a.iter().find(|n| n["key"] == key).cloned());
    let niches: Vec<Value> = matched.iter().filter_map(|t| niche_of(&t.key)).collect();
    let main = niches.iter().max_by_key(|n| n["islands"].as_i64().unwrap_or(0)).cloned();
    let mut gc: HashMap<&str, usize> = HashMap::new();
    for t in &matched { *gc.entry(t.genre).or_default() += 1 }
    let genre = main.as_ref().and_then(|m| m["genre"].as_str().map(String::from)).or_else(|| gc.into_iter().max_by_key(|x| x.1).map(|x| x.0.to_string()));

    let title_check = check_title(&st, &title, code.as_deref(), main.as_ref(), matched.len()).await?;
    let tracking = match &code { Some(c) => Some(track(&st, mid, c, &matched, main.as_ref(), &updates).await?), None => None };

    Ok(Json(json!({
        "title": title, "map_name": name, "map_genre": map_genre, "island_code": code, "island_title": island.as_ref().map(|i| i.0.clone()),
        "themes": matched.iter().map(|t| json!({ "key": t.key, "fr": t.fr, "en": t.en, "genre": t.genre })).collect::<Vec<_>>(),
        "niche": main, "genre": genre,
        "title_check": title_check,
        "thumb": serde_json::from_str::<Value>(&thumb).unwrap_or(json!({})), "trailer": trailer == 1, "channels": channels == 1,
        "updates": serde_json::from_str::<Value>(&updates).unwrap_or(json!([])),
        "tracking": tracking,
    })))
}

/// Titre : copies et titres très proches dans le catalogue, mots trop repris, bourrage de mots-clés.
async fn check_title(st: &AppState, title: &str, own: Option<&str>, niche: Option<&Value>, themes: usize) -> Result<Value, AppError> {
    let mine: HashSet<String> = words(title).into_iter().collect();
    let mine_norm = ideas::norm(title);
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT code, title FROM fn_islands WHERE code NOT LIKE 'playlist_%' AND title <> ''").fetch_all(&st.db).await?;
    let mut close: Vec<(f64, String, String)> = vec![];
    for (c, t) in &rows {
        if Some(c.as_str()) == own { continue }
        let tn = ideas::norm(t);
        if tn == mine_norm && !mine.is_empty() { close.push((1.0, c.clone(), t.clone())); continue }
        let ws: HashSet<String> = words(t).into_iter().collect();
        if ws.is_empty() || mine.is_empty() { continue }
        let inter = mine.intersection(&ws).count();
        let sim = inter as f64 / mine.union(&ws).count() as f64;
        if inter >= 2 && sim >= 0.6 { close.push((sim, c.clone(), t.clone())) }
    }
    close.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let codes: Vec<String> = close.iter().take(6).map(|x| x.1.clone()).collect();
    let mut players: HashMap<String, i64> = HashMap::new();
    for c in &codes {
        let p: Option<i64> = sqlx::query_scalar("SELECT unique_players FROM fn_metrics WHERE code = ? AND unique_players IS NOT NULL ORDER BY day DESC LIMIT 1").bind(c).fetch_optional(&st.db).await?.flatten();
        if let Some(p) = p { players.insert(c.clone(), p); }
    }
    let exact = close.iter().any(|x| x.0 >= 1.0);
    let overused: Vec<String> = niche.and_then(|n| n["title_words"].as_array()).map(|a| a.iter().filter_map(|w| w.as_str()).filter(|w| mine.contains(*w)).map(String::from).collect()).unwrap_or_default();
    // Bourrage : trop de mots, mots répétés, majuscules partout, mots qui « attrapent » la recherche.
    let all_words: Vec<String> = ideas::norm(title).split_whitespace().map(String::from).collect();
    let repeated = all_words.len() != all_words.iter().collect::<HashSet<_>>().len();
    let low = title.to_lowercase();
    let bait = title.contains("#1") || low.contains("vbucks") || low.contains("v-bucks") || all_words.iter().any(|w| ["code", "codes", "xp", "best", "free"].contains(&w.as_str()));
    let letters: Vec<char> = title.chars().filter(|c| c.is_alphabetic()).collect();
    let caps = letters.len() > 6 && letters.iter().filter(|c| c.is_uppercase()).count() as f64 / letters.len() as f64 > 0.6;
    let symbols = title.chars().filter(|c| !c.is_alphanumeric() && !c.is_whitespace() && !"-:'’!?&.,()".contains(*c)).count();
    let long = title.chars().count() > 40;
    let stuffing = all_words.len() > 8 || themes >= 4 || repeated || bait;
    let mut issues: Vec<&str> = vec![];
    if exact { issues.push("copie") } else if !close.is_empty() { issues.push("proche") }
    if stuffing { issues.push("bourrage") }
    if !overused.is_empty() { issues.push("mots_repris") }
    if caps { issues.push("majuscules") }
    if symbols >= 3 { issues.push("symboles") }
    if long { issues.push("trop_long") }
    let mut score: i64 = 100;
    for i in &issues { score -= match *i { "copie" => 60, "proche" => 25, "bourrage" => 30, "mots_repris" => 10, _ => 8 } }
    Ok(json!({
        "score": score.max(0), "issues": issues, "overused": overused, "words": all_words.len(), "chars": title.chars().count(),
        "similar": close.iter().take(6).map(|x| json!({ "code": x.1, "title": x.2, "similarity": (x.0 * 100.0).round(), "players": players.get(&x.1) })).collect::<Vec<_>>(),
    }))
}

/// Joueurs heure par heure de l'île (au plus toutes les 30 minutes), pour repérer une montée inhabituelle.
async fn refresh_hourly(st: &AppState, mid: i64, code: &str) {
    let due: bool = sqlx::query_scalar("SELECT COALESCE((SELECT hourly_at < datetime('now', '-30 minutes') OR hourly_at IS NULL FROM map_discover WHERE map_id = ?), 1)").bind(mid).fetch_one(&st.db).await.unwrap_or(false);
    if !due || std::env::var("POP_ANALYTICS").map(|v| v == "0").unwrap_or(false) { return }
    let _ = sqlx::query("INSERT INTO map_discover (map_id, hourly_at) VALUES (?, datetime('now')) ON CONFLICT(map_id) DO UPDATE SET hourly_at = datetime('now')").bind(mid).execute(&st.db).await;
    let _ = crate::analytics::fetch_hourly(st, code).await;
    // Les chiffres journaliers de la map, au passage.
    let _ = crate::analytics::fetch_metrics(st, code).await;
}

async fn track(st: &AppState, mid: i64, code: &str, matched: &[&ideas::Theme], niche: Option<&Value>, updates: &str) -> Result<Value, AppError> {
    refresh_hourly(st, mid, code).await;
    let series: Vec<(String, Option<i64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT day, unique_players, avg_minutes, d1, d7, recommends, peak_ccu FROM fn_metrics WHERE code = ? ORDER BY day").bind(code).fetch_all(&st.db).await?;
    let first_seen: Option<String> = sqlx::query_scalar("SELECT date(first_seen) FROM fn_islands WHERE code = ?").bind(code).fetch_optional(&st.db).await?.flatten();
    let first_metric = series.iter().find(|s| s.1.unwrap_or(0) > 0).map(|s| s.0.clone());
    let start = match (first_seen, first_metric.clone()) { (Some(a), Some(b)) => Some(a.min(b)), (a, b) => a.or(b) };
    let parse = |d: &str| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok();
    let today = chrono::Utc::now().date_naive();
    let age = start.as_deref().and_then(parse).map(|d| (today - d).num_days());
    let age_of = |day: &str| -> Option<i64> { Some((parse(day)? - parse(start.as_deref()?)?).num_days()) };

    // Repères : nouvelles maps de la même niche, joueurs selon leur âge (jours depuis la sortie).
    let mut bench: Vec<Value> = vec![];
    let mut my_pct: Option<f64> = None;
    if !matched.is_empty() {
        let cohort: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT code, title, tags, date(first_seen) FROM fn_islands WHERE seen_new = 1 AND first_seen > datetime('now', '-90 days') AND code <> ? AND code NOT LIKE 'playlist_%'").bind(code).fetch_all(&st.db).await?;
        let mut starts: HashMap<String, chrono::NaiveDate> = HashMap::new();
        for (c, t, tg, fs) in &cohort {
            let tags: Vec<String> = serde_json::from_str(tg).unwrap_or_default();
            let tx = ideas::norm(&format!("{t} {}", tags.join(" ")));
            if matched.iter().any(|th| th.pats.iter().any(|p| tx.contains(p.as_str()))) { if let Some(d) = parse(fs) { starts.insert(c.clone(), d); } }
        }
        let mut by_age: HashMap<i64, Vec<f64>> = HashMap::new();
        if !starts.is_empty() {
            let rows: Vec<(String, String, Option<i64>)> = sqlx::query_as("SELECT code, day, unique_players FROM fn_metrics WHERE day > date('now', '-100 days') AND code IN (SELECT code FROM fn_islands WHERE seen_new = 1 AND first_seen > datetime('now', '-90 days'))").fetch_all(&st.db).await?;
            for (c, d, p) in rows {
                let (Some(s), Some(d), Some(p)) = (starts.get(&c), parse(&d), p) else { continue };
                let a = (d - *s).num_days();
                if (0..=21).contains(&a) { by_age.entry(a).or_default().push(p as f64) }
            }
        }
        for a in 0..=21 {
            if let Some(v) = by_age.get(&a) { if v.len() >= 3 {
                bench.push(json!({ "age": a, "n": v.len(), "p25": ideas::quantile(v.clone(), 0.25), "p50": ideas::quantile(v.clone(), 0.5), "p75": ideas::quantile(v.clone(), 0.75) }));
            } }
        }
        // Où se place la map à son dernier âge mesuré.
        if let Some((day, Some(p))) = series.iter().rev().find(|s| s.1.is_some()).map(|s| (s.0.clone(), s.1)) {
            if let Some(a) = age_of(&day) { if let Some(v) = by_age.get(&a) { if v.len() >= 3 { my_pct = Some(ideas::pct(v, p as f64)) } } }
        }
    }

    // Signaux notés par Epic, face à la niche.
    let last = |f: &dyn Fn(&(String, Option<i64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>, Option<i64>)) -> Option<f64>| series.iter().rev().find_map(|s| f(s));
    let minutes = last(&|s| s.2);
    let d1 = last(&|s| s.3);
    let d7 = last(&|s| s.4);
    let rec = last(&|s| match (s.5, s.1) { (Some(r), Some(u)) if u > 0 => Some(r as f64 / u as f64), _ => None });
    let grade = |v: Option<f64>, mean: Option<f64>, target: Option<f64>| -> &'static str {
        match (v, mean) {
            (None, _) => "inconnu",
            (Some(v), _) if target.map(|t| v >= t).unwrap_or(false) => "top",
            (Some(v), Some(m)) if v >= m => "bon",
            (Some(v), Some(m)) if v >= m * 0.75 => "moyen",
            (Some(_), Some(_)) => "faible",
            (Some(_), None) => "inconnu",
        }
    };
    let n = |k: &str| niche.and_then(|x| x[k].as_f64());
    let t = |k: &str| niche.and_then(|x| x["targets"][k].as_f64());
    let signals = json!([
        { "key": "minutes", "value": minutes, "niche": n("minutes"), "target": t("minutes"), "grade": grade(minutes, n("minutes"), t("minutes")) },
        { "key": "d1", "value": d1, "niche": n("d1"), "target": t("d1"), "grade": grade(d1, n("d1"), t("d1")) },
        { "key": "d7", "value": d7, "niche": n("d7"), "target": Value::Null, "grade": grade(d7, n("d7"), None) },
        { "key": "rec", "value": rec, "niche": n("rec_rate"), "target": Value::Null, "grade": grade(rec, n("rec_rate"), None) },
    ]);

    // Montée inhabituelle (même idée que la rangée Trending) : les 3 dernières heures face aux mêmes heures des jours d'avant.
    let hours: Vec<(String, Option<i64>)> = sqlx::query_as("SELECT ts, ccu FROM fn_hourly WHERE code = ? AND ts > strftime('%Y-%m-%dT%H', 'now', '-8 days') ORDER BY ts").bind(code).fetch_all(&st.db).await?;
    let mut surge: Value = Value::Null;
    if hours.len() >= 48 {
        let recent: Vec<&(String, Option<i64>)> = hours.iter().rev().take(3).collect();
        let now_avg = recent.iter().filter_map(|h| h.1).sum::<i64>() as f64 / 3.0;
        let hod: HashSet<String> = recent.iter().map(|h| h.0.chars().skip(11).take(2).collect()).collect();
        let last_day = recent[0].0.chars().take(10).collect::<String>();
        let past: Vec<f64> = hours.iter().filter(|h| hod.contains(&h.0.chars().skip(11).take(2).collect::<String>()) && !h.0.starts_with(&last_day)).filter_map(|h| h.1.map(|v| v as f64)).collect();
        if !past.is_empty() {
            let base = past.iter().sum::<f64>() / past.len() as f64;
            let ratio = if base > 0.0 { now_avg / base } else { 0.0 };
            surge = json!({ "now": now_avg.round(), "usual": base.round(), "ratio": (ratio * 100.0).round() / 100.0, "rising": ratio >= 1.3 && now_avg >= 5.0 });
        }
    }

    // Tendance des derniers jours (baisse → mise à jour conseillée).
    let ups: Vec<f64> = series.iter().filter_map(|s| s.1.map(|v| v as f64)).collect();
    let trend3 = if ups.len() >= 6 { let a: f64 = ups[ups.len() - 3..].iter().sum(); let b: f64 = ups[ups.len() - 6..ups.len() - 3].iter().sum(); (b > 0.0).then(|| (a / b - 1.0) * 100.0) } else { None };

    // Effet de chaque mise à jour : moyenne des 3 jours avant et après.
    let ups_by_day: HashMap<String, f64> = series.iter().filter_map(|s| s.1.map(|v| (s.0.clone(), v as f64))).collect();
    let upd: Vec<String> = serde_json::from_str(updates).unwrap_or_default();
    let effects: Vec<Value> = upd.iter().filter_map(|d| {
        let d0 = parse(d)?;
        let avg = |r: std::ops::RangeInclusive<i64>| { let v: Vec<f64> = r.filter_map(|i| ups_by_day.get(&(d0 + chrono::Duration::days(i)).to_string()).copied()).collect(); if v.is_empty() { None } else { Some(v.iter().sum::<f64>() / v.len() as f64) } };
        let (b, a) = (avg(-3..=-1), avg(1..=3));
        Some(json!({ "day": d, "before": b.map(|x| x.round()), "after": a.map(|x| x.round()), "change": match (b, a) { (Some(b), Some(a)) if b > 0.0 => Some(((a / b - 1.0) * 100.0).round()), _ => None } }))
    }).collect();

    // Chance de percer (modèle appris sur les nouvelles maps suivies par POP), pendant les 14 jours de test.
    let mut by_age: HashMap<i64, (Option<i64>, Option<f64>, Option<f64>)> = HashMap::new();
    for s in &series { if let Some(a) = age_of(&s.0) { by_age.insert(a, (s.1, s.2, s.3)); } }
    let launch = if age.map(|a| (0..=14).contains(&a)).unwrap_or(false) { crate::model::predict(st, &by_age).await } else { Value::Null };

    let phase = match age { None => "avant", Some(a) if first_metric.is_none() && a <= 1 => "avant", Some(a) if a <= 14 => "test", _ => "apres" };
    Ok(json!({
        "phase": phase, "age": age, "start": start,
        "series": series.iter().map(|s| json!({ "day": s.0, "age": age_of(&s.0), "players": s.1, "minutes": s.2, "d1": s.3, "d7": s.4, "ccu": s.6 })).collect::<Vec<_>>(),
        "bench": bench, "percentile": my_pct.map(|p| (p * 100.0).round()),
        "signals": signals, "surge": surge, "launch": launch, "trend3": trend3.map(|x| x.round()), "effects": effects,
    }))
}

#[derive(Deserialize)]
pub struct SaveIn { title: Option<String>, thumb: Option<Value>, trailer: Option<bool>, channels: Option<bool>, update_day: Option<String>, remove_update: Option<String> }

pub async fn save(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<SaveIn>) -> ApiResult<Value> {
    access(&st, &u, mid).await?.require(Role::Member)?;
    sqlx::query("INSERT OR IGNORE INTO map_discover (map_id) VALUES (?)").bind(mid).execute(&st.db).await?;
    if let Some(t) = &b.title { if t.chars().count() > 80 { return Err(bad("Titre trop long.")) } sqlx::query("UPDATE map_discover SET title = ? WHERE map_id = ?").bind(t.trim()).bind(mid).execute(&st.db).await?; }
    if let Some(t) = &b.thumb { let s = t.to_string(); if s.len() > 4000 { return Err(bad("Analyse trop longue.")) } sqlx::query("UPDATE map_discover SET thumb = ? WHERE map_id = ?").bind(s).bind(mid).execute(&st.db).await?; }
    if let Some(v) = b.trailer { sqlx::query("UPDATE map_discover SET trailer = ? WHERE map_id = ?").bind(v as i64).bind(mid).execute(&st.db).await?; }
    if let Some(v) = b.channels { sqlx::query("UPDATE map_discover SET channels = ? WHERE map_id = ?").bind(v as i64).bind(mid).execute(&st.db).await?; }
    if b.update_day.is_some() || b.remove_update.is_some() {
        let cur: String = sqlx::query_scalar("SELECT updates FROM map_discover WHERE map_id = ?").bind(mid).fetch_one(&st.db).await?;
        let mut v: Vec<String> = serde_json::from_str(&cur).unwrap_or_default();
        if let Some(d) = &b.update_day {
            if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() { return Err(bad("Date invalide (AAAA-MM-JJ).")) }
            if !v.contains(d) { v.push(d.clone()); v.sort(); }
        }
        if let Some(d) = &b.remove_update { v.retain(|x| x != d) }
        sqlx::query("UPDATE map_discover SET updates = ? WHERE map_id = ?").bind(json!(v).to_string()).bind(mid).execute(&st.db).await?;
    }
    Ok(Json(json!({ "ok": true })))
}
