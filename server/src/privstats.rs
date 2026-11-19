//! Chiffres privés d'une map, importés du Creator Portal (CSV d'analytics), en plus des chiffres publics d'Epic :
//! taux de rebond, nouveaux joueurs, joueurs revenus, impressions, clics… L'import reconnaît les colonnes
//! par leur nom (anglais ou français), au format large (une colonne par mesure) ou long (mesure, date, valeur).
use crate::{access::{map_ctx, Role}, auth::AuthUser, error::{bad, ApiResult}, AppState};
use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// (mesure, mots qui la reconnaissent dans le nom de colonne, valeur en pourcentage ?)
const METRICS: &[(&str, &[&str], bool)] = &[
    ("bounce", &["bounce", "rebond"], true),
    ("d1", &["d1", "day 1", "1-day", "day1", "j+1", "retention 1"], true),
    ("d7", &["d7", "day 7", "7-day", "day7", "j+7", "retention 7"], true),
    ("ctr", &["click-through", "click through", "ctr", "taux de clic"], true),
    ("new_players", &["new player", "new user", "nouveaux joueurs", "first-time", "first time"], false),
    ("returning", &["returning", "lapsed", "reactivated", "revenus", "revenant"], false),
    ("avg_minutes", &["avg minutes", "average minutes", "minutes per player", "avg playtime", "average playtime", "avg. playtime", "temps de jeu moyen", "playtime per player", "minutes per session"], false),
    ("minutes", &["minutes played", "total minutes", "playtime", "temps de jeu", "hours played"], false),
    ("plays", &["plays", "sessions", "parties"], false),
    ("unique_players", &["unique player", "unique users", "joueurs uniques", "players", "joueurs", "users"], false),
    ("peak_ccu", &["peak ccu", "peak concurrent", "ccu", "pic"], false),
    ("impressions", &["impression"], false),
    ("clicks", &["click"], false),
    ("favorites", &["favorite", "favourite", "favori"], false),
    ("recommends", &["recommend"], false),
    ("queue", &["queue", "wait time", "attente"], false),
];

fn metric_of(h: &str) -> Option<(&'static str, bool)> {
    let h = h.to_lowercase();
    // Mots-clés les plus précis d'abord (ordre de la liste).
    METRICS.iter().find(|(_, words, _)| words.iter().any(|w| h.contains(w))).map(|(k, _, p)| (*k, *p))
}

fn parse_date(s: &str) -> Option<String> {
    let s = s.trim().trim_matches('"');
    let head: String = s.chars().take(10).collect();
    if let Ok(d) = chrono::NaiveDate::parse_from_str(&head, "%Y-%m-%d") { return Some(d.to_string()) }
    let parts: Vec<&str> = s.split(|c| c == '/' || c == '.' || c == '-' || c == ' ').filter(|p| !p.is_empty()).collect();
    if parts.len() >= 3 {
        let (a, b, c) = (parts[0].parse::<u32>().ok()?, parts[1].parse::<u32>().ok()?, parts[2].chars().take(4).collect::<String>().parse::<i32>().ok()?);
        let y = if c < 100 { 2000 + c } else { c };
        // JJ/MM/AAAA si le premier nombre dépasse 12, sinon MM/JJ/AAAA (format américain du portail).
        let (d, m) = if a > 12 { (a, b) } else if b > 12 { (b, a) } else { (b, a) };
        return chrono::NaiveDate::from_ymd_opt(y, m, d).map(|x| x.to_string());
    }
    for f in ["%b %d, %Y", "%B %d, %Y", "%d %b %Y", "%d %B %Y"] { if let Ok(d) = chrono::NaiveDate::parse_from_str(s, f) { return Some(d.to_string()) } }
    None
}

fn parse_num(s: &str, percent: bool) -> Option<f64> {
    let t = s.trim().trim_matches('"').replace(['\u{a0}', ' '], "");
    if t.is_empty() || t == "-" { return None }
    let had_pct = t.ends_with('%');
    let t = t.trim_end_matches('%');
    // « 1,234.5 » ou « 1 234,5 » ou « 12,5 »
    let t = if t.contains('.') && t.contains(',') { t.replace(',', "") } else { t.replace(',', ".") };
    let v: f64 = t.parse().ok()?;
    Some(if percent && (had_pct || v > 1.0) { v / 100.0 } else { v })
}

fn split_line(line: &str, sep: char) -> Vec<String> {
    let mut out = vec![]; let mut cur = String::new(); let mut q = false;
    for c in line.chars() {
        match c { '"' => q = !q, c if c == sep && !q => { out.push(cur.trim().to_string()); cur.clear() } c => cur.push(c) }
    }
    out.push(cur.trim().to_string());
    out
}

#[derive(Deserialize)]
pub struct ImportIn { csv: String, filename: Option<String> }

pub async fn import(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<ImportIn>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    if b.csv.len() > 3_000_000 { return Err(bad("Fichier trop gros (3 Mo au plus).")) }
    let text = b.csv.trim_start_matches('\u{feff}');
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let sep = [',', ';', '\t'].into_iter().max_by_key(|c| first.matches(*c).count()).unwrap_or(',');
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header = split_line(lines.next().ok_or_else(|| bad("Fichier vide."))?, sep);
    let date_col = header.iter().position(|h| { let h = h.to_lowercase(); h.contains("date") || h == "day" || h.contains("jour") || h.contains("time") || h.contains("period") });
    let date_col = date_col.ok_or_else(|| bad("Colonne de date introuvable. Exportez le CSV d’analytics du Creator Portal (avec une colonne Date)."))?;
    let mut values: BTreeMap<(String, &'static str), f64> = BTreeMap::new();
    let mut unknown: Vec<String> = vec![];
    // Format long : une colonne « mesure » et une colonne « valeur ».
    let name_col = header.iter().position(|h| { let h = h.to_lowercase(); h == "metric" || h == "name" || h == "mesure" || h == "series" });
    let value_col = header.iter().position(|h| { let h = h.to_lowercase(); h == "value" || h == "valeur" || h == "count" });
    let rows: Vec<Vec<String>> = lines.map(|l| split_line(l, sep)).collect();
    if let (Some(nc), Some(vc)) = (name_col, value_col) {
        for r in &rows {
            let (Some(d), Some(name)) = (r.get(date_col).and_then(|d| parse_date(d)), r.get(nc)) else { continue };
            let Some((m, pct)) = metric_of(name) else { if !unknown.contains(name) { unknown.push(name.clone()) } continue };
            if let Some(v) = r.get(vc).and_then(|x| parse_num(x, pct)) { values.insert((d, m), v); }
        }
    } else {
        let cols: Vec<(usize, Option<(&'static str, bool)>)> = header.iter().enumerate().filter(|(i, _)| *i != date_col).map(|(i, h)| (i, metric_of(h))).collect();
        for (i, m) in &cols { if m.is_none() { unknown.push(header[*i].clone()) } }
        for r in &rows {
            let Some(d) = r.get(date_col).and_then(|d| parse_date(d)) else { continue };
            for (i, m) in &cols { if let (Some((k, pct)), Some(v)) = (m, r.get(*i).and_then(|x| parse_num(x, m.map(|x| x.1).unwrap_or(false)))) { let _ = pct; values.insert((d.clone(), k), v); } }
        }
    }
    if values.is_empty() { return Err(bad("Aucun chiffre reconnu dans ce fichier. Envoyez-le à l’équipe POP pour qu’on l’ajoute.")) }
    let mut tx = st.db.begin().await?;
    for ((d, m), v) in &values {
        sqlx::query("INSERT INTO map_private_stats (map_id, day, metric, value) VALUES (?, ?, ?, ?) ON CONFLICT(map_id, day, metric) DO UPDATE SET value = excluded.value")
            .bind(mid).bind(d).bind(*m).bind(*v).execute(&mut *tx).await?;
    }
    let metrics: Vec<&str> = { let mut v: Vec<&str> = values.keys().map(|k| k.1).collect(); v.sort(); v.dedup(); v };
    let days = values.keys().map(|k| k.0.clone()).collect::<std::collections::BTreeSet<_>>();
    sqlx::query("INSERT INTO map_private_imports (map_id, user_id, filename, rows, metrics) VALUES (?, ?, ?, ?, ?)")
        .bind(mid).bind(u.id).bind(b.filename.as_deref().unwrap_or("").chars().take(120).collect::<String>()).bind(values.len() as i64).bind(json!(metrics).to_string()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "values": values.len(), "days": days.len(), "from": days.iter().next(), "to": days.iter().last(), "metrics": metrics, "ignored": unknown })))
}

/// Séries par mesure, et dernière valeur.
pub async fn get(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(String, String, f64)> = sqlx::query_as("SELECT day, metric, value FROM map_private_stats WHERE map_id = ? ORDER BY day").bind(mid).fetch_all(&st.db).await?;
    let mut by: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (d, m, v) in rows { by.entry(m).or_default().push(json!({ "day": d, "value": v })) }
    let last: Option<(String, Option<String>)> = sqlx::query_as("SELECT at, filename FROM map_private_imports WHERE map_id = ? ORDER BY id DESC LIMIT 1").bind(mid).fetch_optional(&st.db).await?;
    Ok(Json(json!({ "series": by, "imported_at": last.as_ref().map(|l| l.0.clone()), "filename": last.and_then(|l| l.1) })))
}
