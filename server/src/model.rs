//! Ce que POP apprend de son propre historique.
//!
//! 1. **Vérification après coup des niches** : chaque jour, la note de chaque niche et ses 5 axes sont gardés
//!    (table fn_niches). 4 semaines plus tard, on regarde si les maps sorties dans la niche pendant les 14 jours
//!    suivants ont vraiment percé. Les axes qui prédisaient bien la réussite prennent plus de poids.
//! 2. **Chance de percer** : un modèle logistique, entraîné sur les nouvelles maps suivies par POP, estime dès
//!    les premiers jours la probabilité qu'une map dépasse 1 000 joueurs par jour avant la fin des 14 jours de test d'Epic.
use crate::{ideas, AppState};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Seuil de réussite pour « percer » pendant le test d'Epic.
pub const BREAKOUT: i64 = 1000;
const FEATURES: &[&str] = &["players", "minutes", "d1", "growth", "no_d1"];

/// Une fois par jour : vérification des niches et entraînement du modèle de lancement.
pub async fn daily(st: &AppState) {
    let due: (bool, bool) = sqlx::query_as("SELECT calibrated_at IS NULL OR calibrated_at < datetime('now', '-20 hours'), trained_at IS NULL OR trained_at < datetime('now', '-20 hours') FROM idea_model WHERE id = 1")
        .fetch_one(&st.db).await.unwrap_or((false, false));
    if due.0 { if let Err(e) = calibrate(st).await { tracing::warn!("vérification des niches : {e:?}") } }
    if due.1 { if let Err(e) = train(st).await { tracing::warn!("modèle de lancement : {e:?}") } }
}

/// Plus haut nombre de joueurs de chaque nouvelle map pendant ses 14 premiers jours.
async fn newcomers(st: &AppState) -> Result<Vec<(String, String, i64)>, sqlx::Error> {
    // (texte du titre et des tags, jour de sortie, plus haut)
    let rows: Vec<(String, String, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT i.code, i.title, i.tags, date(i.first_seen), MAX(m.unique_players) FROM fn_islands i
         JOIN fn_metrics m ON m.code = i.code AND m.day BETWEEN date(i.first_seen) AND date(i.first_seen, '+14 days')
         WHERE i.seen_new = 1 AND i.first_seen > datetime('now', '-200 days') AND i.code NOT LIKE 'playlist_%'
         GROUP BY i.code",
    ).fetch_all(&st.db).await?;
    Ok(rows.into_iter().map(|(_, t, tags, d, best)| {
        let tags: Vec<String> = serde_json::from_str(&tags).unwrap_or_default();
        (ideas::norm(&format!("{t} {}", tags.join(" "))), d, best.unwrap_or(0))
    }).collect())
}

/// Corrélation de rang (Spearman) entre deux séries.
fn spearman(a: &[f64], b: &[f64]) -> f64 {
    let rank = |v: &[f64]| -> Vec<f64> {
        let mut idx: Vec<usize> = (0..v.len()).collect();
        idx.sort_by(|&i, &j| v[i].partial_cmp(&v[j]).unwrap());
        let mut r = vec![0.0; v.len()];
        let mut i = 0;
        while i < idx.len() {
            let mut j = i;
            while j + 1 < idx.len() && v[idx[j + 1]] == v[idx[i]] { j += 1 }
            for k in i..=j { r[idx[k]] = (i + j) as f64 / 2.0 }
            i = j + 1;
        }
        r
    };
    let (ra, rb) = (rank(a), rank(b));
    let n = a.len() as f64;
    let (ma, mb) = (ra.iter().sum::<f64>() / n, rb.iter().sum::<f64>() / n);
    let cov: f64 = ra.iter().zip(&rb).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = ra.iter().map(|x| (x - ma).powi(2)).sum();
    let vb: f64 = rb.iter().map(|y| (y - mb).powi(2)).sum();
    if va <= 0.0 || vb <= 0.0 { 0.0 } else { cov / (va * vb).sqrt() }
}

async fn calibrate(st: &AppState) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE idea_model SET calibrated_at = datetime('now') WHERE id = 1").execute(&st.db).await?;
    let snaps: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT day, niche, axes FROM fn_niches WHERE axes IS NOT NULL AND day <= date('now', '-28 days') AND day >= date('now', '-180 days')").fetch_all(&st.db).await?;
    if snaps.is_empty() { return Ok(()) }
    let news = newcomers(st).await?;
    let themes = ideas::themes();
    let pats_of = |key: &str| -> Vec<String> {
        if let Some(w) = key.strip_prefix("w:") { return vec![format!(" {w} ")] }
        themes.iter().find(|t| t.key == key).map(|t| t.pats.clone()).unwrap_or_default()
    };
    let global = { let n = news.len().max(1) as f64; news.iter().filter(|x| x.2 >= 100).count() as f64 / n };
    let mut xs: Vec<[f64; 5]> = vec![];
    let mut ys: Vec<f64> = vec![];
    let mut pcache: HashMap<String, Vec<String>> = HashMap::new();
    for (day, niche, axes) in &snaps {
        let Ok(a) = serde_json::from_str::<Vec<f64>>(axes) else { continue };
        if a.len() != 5 { continue }
        let pats = pcache.entry(niche.clone()).or_insert_with(|| pats_of(niche)).clone();
        if pats.is_empty() { continue }
        let end = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").map(|d| (d + chrono::Duration::days(14)).to_string()).unwrap_or_default();
        let group: Vec<&(String, String, i64)> = news.iter().filter(|x| x.1.as_str() >= day.as_str() && x.1 <= end && pats.iter().any(|p| x.0.contains(p.as_str()))).collect();
        if group.len() < 3 { continue }
        let ok = group.iter().filter(|x| x.2 >= 100).count() as f64;
        xs.push([a[0], a[1], a[2], a[3], a[4]]);
        ys.push((ok + global * 4.0) / (group.len() as f64 + 4.0));
    }
    let n = xs.len();
    let corr: Vec<f64> = (0..5).map(|k| if n >= 2 { spearman(&xs.iter().map(|x| x[k]).collect::<Vec<_>>(), &ys) } else { 0.0 }).collect();
    let mut weights = ideas::DEFAULT_WEIGHTS.to_vec();
    let alpha = if n >= 30 { (n as f64 / 400.0).min(0.7) } else { 0.0 };
    let pos: Vec<f64> = corr.iter().map(|c| c.max(0.0)).collect();
    let sp: f64 = pos.iter().sum();
    if alpha > 0.0 && sp > 0.0 {
        weights = weights.iter().zip(&pos).map(|(d, p)| (1.0 - alpha) * d + alpha * p / sp).collect();
        // Garde-fou : aucun axe ne décide seul, aucun n'est oublié (entre 5 % et 45 %).
        for _ in 0..5 {
            let s: f64 = weights.iter().sum(); weights = weights.iter().map(|w| (w / s).clamp(0.05, 0.45)).collect();
        }
        let s: f64 = weights.iter().sum(); weights = weights.iter().map(|w| w / s).collect();
    }
    sqlx::query("UPDATE idea_model SET weights = ?, calib = ? WHERE id = 1")
        .bind(json!(weights).to_string()).bind(json!({ "samples": n, "corr": corr, "alpha": alpha, "global": global }).to_string()).execute(&st.db).await?;
    tracing::info!("vérification des niches : {n} échantillons, poids {:?}", weights);
    Ok(())
}

/// Caractéristiques des premiers jours d'une map (âges 0 à 2) : joueurs, temps de jeu, retour à J+1, progression.
pub fn features(by_age: &HashMap<i64, (Option<i64>, Option<f64>, Option<f64>)>) -> Option<[f64; 5]> {
    let early: Vec<(i64, &(Option<i64>, Option<f64>, Option<f64>))> = { let mut v: Vec<_> = by_age.iter().filter(|(a, _)| (0..=2).contains(*a)).map(|(a, x)| (*a, x)).collect(); v.sort_by_key(|x| x.0); v };
    let players: Vec<i64> = early.iter().filter_map(|x| x.1 .0).collect();
    if players.is_empty() { return None }
    let p = *players.iter().max().unwrap();
    let mins = early.iter().rev().find_map(|x| x.1 .1).unwrap_or(0.0).min(120.0);
    let d1 = by_age.iter().filter(|(a, _)| (0..=3).contains(*a)).filter_map(|(_, x)| x.2).last();
    let growth = match (players.first(), players.last()) { (Some(a), Some(b)) if players.len() >= 2 => ((*b as f64 + 1.0) / (*a as f64 + 1.0)).ln(), _ => 0.0 };
    Some([(p as f64).ln_1p(), mins / 30.0, d1.unwrap_or(0.0), growth, if d1.is_none() { 1.0 } else { 0.0 }])
}

async fn train(st: &AppState) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE idea_model SET trained_at = datetime('now') WHERE id = 1").execute(&st.db).await?;
    let rows: Vec<(String, i64, Option<i64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT m.code, CAST(julianday(m.day) - julianday(date(i.first_seen)) AS INTEGER), m.unique_players, m.avg_minutes, m.d1
         FROM fn_metrics m JOIN fn_islands i ON i.code = m.code
         WHERE i.seen_new = 1 AND i.first_seen > datetime('now', '-200 days') AND i.first_seen < datetime('now', '-15 days')
           AND m.day BETWEEN date(i.first_seen) AND date(i.first_seen, '+14 days')").fetch_all(&st.db).await?;
    let mut by: HashMap<String, HashMap<i64, (Option<i64>, Option<f64>, Option<f64>)>> = HashMap::new();
    for (c, a, p, m, d) in rows { by.entry(c).or_default().insert(a, (p, m, d)); }
    let mut x: Vec<[f64; 5]> = vec![];
    let mut y: Vec<f64> = vec![];
    for ages in by.values() {
        let Some(f) = features(ages) else { continue };
        let best = ages.values().filter_map(|v| v.0).max().unwrap_or(0);
        x.push(f); y.push(if best >= BREAKOUT { 1.0 } else { 0.0 });
    }
    let n = x.len();
    let pos = y.iter().filter(|v| **v > 0.5).count();
    if n < 40 || pos < 5 || n - pos < 5 {
        sqlx::query("UPDATE idea_model SET launch = ? WHERE id = 1").bind(json!({ "ready": false, "n": n, "positives": pos }).to_string()).execute(&st.db).await?;
        return Ok(())
    }
    // Centrage-réduction puis régression logistique (descente de gradient, légère régularisation).
    let mean: Vec<f64> = (0..5).map(|k| x.iter().map(|r| r[k]).sum::<f64>() / n as f64).collect();
    let std: Vec<f64> = (0..5).map(|k| { let v = x.iter().map(|r| (r[k] - mean[k]).powi(2)).sum::<f64>() / n as f64; v.sqrt().max(1e-6) }).collect();
    let z: Vec<Vec<f64>> = x.iter().map(|r| (0..5).map(|k| (r[k] - mean[k]) / std[k]).collect()).collect();
    let fit = |idx: &[usize]| -> (Vec<f64>, f64) {
        let (mut w, mut b) = (vec![0.0; 5], 0.0);
        let m = idx.len().max(1) as f64;
        for _ in 0..800 {
            let mut gw = vec![0.0; 5]; let mut gb = 0.0;
            for &i in idx {
                let p = 1.0 / (1.0 + (-(b + z[i].iter().zip(&w).map(|(a, c)| a * c).sum::<f64>())).exp());
                let e = p - y[i];
                for k in 0..5 { gw[k] += e * z[i][k] } gb += e;
            }
            for k in 0..5 { w[k] -= 0.3 * (gw[k] / m + 0.01 * w[k]) } b -= 0.3 * gb / m;
        }
        (w, b)
    };
    // Fiabilité mesurée honnêtement : on apprend sur 3 maps sur 4 et on vérifie sur la 4e, jamais vue.
    // AUC = probabilité qu'une map qui a percé soit mieux notée qu'une map qui n'a pas percé.
    let (train_i, test_i): (Vec<usize>, Vec<usize>) = (0..n).partition(|i| i % 4 != 0);
    let (wt, bt) = fit(&train_i);
    let score = |i: usize| bt + z[i].iter().zip(&wt).map(|(a, c)| a * c).sum::<f64>();
    let (mut good, mut tot) = (0.0, 0.0);
    for &i in &test_i { if y[i] > 0.5 { for &j in &test_i { if y[j] < 0.5 { tot += 1.0; let (a, c) = (score(i), score(j)); if a > c { good += 1.0 } else if a == c { good += 0.5 } } } } }
    let auc = if tot > 0.0 { good / tot } else { 0.5 };
    let all: Vec<usize> = (0..n).collect();
    let (w, b) = fit(&all);
    sqlx::query("UPDATE idea_model SET launch = ? WHERE id = 1")
        .bind(json!({ "ready": true, "n": n, "positives": pos, "mean": mean, "std": std, "w": w, "b": b, "auc": auc, "threshold": BREAKOUT }).to_string()).execute(&st.db).await?;
    tracing::info!("modèle de lancement : {n} maps, {pos} ont percé, AUC {:.2}", auc);
    Ok(())
}

/// Chance de percer d'une map d'après ses premiers jours, avec ce qui la fait monter ou baisser.
pub async fn predict(st: &AppState, by_age: &HashMap<i64, (Option<i64>, Option<f64>, Option<f64>)>) -> Value {
    let Some(f) = features(by_age) else { return Value::Null };
    let m: Option<String> = sqlx::query_scalar("SELECT launch FROM idea_model WHERE id = 1").fetch_optional(&st.db).await.ok().flatten().flatten();
    let m: Value = m.and_then(|m| serde_json::from_str(&m).ok()).unwrap_or(json!({}));
    if m["ready"].as_bool() != Some(true) {
        return json!({ "ready": false, "n": m["n"], "positives": m["positives"], "threshold": BREAKOUT });
    }
    let g = |k: &str| m[k].as_array().map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<f64>>()).unwrap_or_default();
    let (mean, std, w) = (g("mean"), g("std"), g("w"));
    if mean.len() != 5 || std.len() != 5 || w.len() != 5 { return Value::Null }
    let b = m["b"].as_f64().unwrap_or(0.0);
    let contrib: Vec<f64> = (0..5).map(|k| w[k] * (f[k] - mean[k]) / std[k]).collect();
    let p = 1.0 / (1.0 + (-(b + contrib.iter().sum::<f64>())).exp());
    let mut factors: Vec<(usize, f64)> = contrib.iter().copied().enumerate().filter(|(k, c)| *k != 4 && c.abs() >= 0.15).collect();
    factors.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    json!({
        "ready": true, "prob": (p * 100.0).round(), "threshold": BREAKOUT, "n": m["n"], "positives": m["positives"], "auc": m["auc"],
        "factors": factors.into_iter().take(3).map(|(k, c)| json!({ "key": FEATURES[k], "effect": if c > 0.0 { "plus" } else { "moins" } })).collect::<Vec<_>>(),
    })
}

/// Fiabilité des modèles (pour l'administration et le bas de la page Idées).
pub async fn status(st: &AppState) -> Value {
    let r: Option<(Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as("SELECT weights, calib, launch, calibrated_at FROM idea_model WHERE id = 1").fetch_optional(&st.db).await.ok().flatten();
    let (w, c, l, at) = r.unwrap_or_default();
    let p = |s: Option<String>| s.and_then(|s| serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null);
    json!({ "weights": p(w), "default_weights": ideas::DEFAULT_WEIGHTS, "calibration": p(c), "launch": p(l), "calibrated_at": at })
}

/// Administration : état des modèles, et recalcul immédiat.
pub async fn admin_status(axum::extract::State(st): axum::extract::State<AppState>, u: crate::auth::AuthUser) -> crate::error::ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    Ok(axum::Json(status(&st).await))
}
pub async fn admin_refresh(axum::extract::State(st): axum::extract::State<AppState>, u: crate::auth::AuthUser) -> crate::error::ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    sqlx::query("UPDATE idea_model SET calibrated_at = NULL, trained_at = NULL WHERE id = 1").execute(&st.db).await?;
    daily(&st).await;
    Ok(axum::Json(status(&st).await))
}
