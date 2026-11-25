//! Ce que POP apprend de son propre historique.
//!
//! 1. **Vérification après coup des niches** : chaque jour, la note de chaque niche et ses 5 axes sont gardés
//!    (table fn_niches). 4 semaines plus tard, on regarde si les maps sorties dans la niche pendant les 14 jours
//!    suivants ont vraiment percé. Les axes qui prédisaient bien la réussite prennent plus de poids.
//! 2. **Chance de percer** : un modèle logistique, entraîné sur les nouvelles maps suivies par POP, estime dès
//!    les premiers jours la probabilité qu'une map dépasse 1 000 joueurs par jour avant la fin des 14 jours de test d'Epic.
//! 3. **Mémoire de l'IA** (#70) : chaque version entraînée est gardée avec sa précision, mesurée sur les maps les plus
//!    récentes (jamais vues à l'entraînement, comme l'avenir) ; chaque prédiction du jour est gardée, puis notée une
//!    fois le test d'Epic fini. On voit ainsi, jour après jour, si l'IA progresse.
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
    if due.0 {
        if let Err(e) = calibrate(st).await {
            tracing::warn!("vérification des niches : {e:?}")
        }
    }
    if due.1 {
        if let Err(e) = train(st).await {
            tracing::warn!("modèle de lancement : {e:?}")
        }
        if let Err(e) = grade(st).await {
            tracing::warn!("notation des prédictions : {e:?}")
        }
        if let Err(e) = record_predictions(st).await {
            tracing::warn!("prédictions du jour : {e:?}")
        }
    }
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
    Ok(rows
        .into_iter()
        .map(|(_, t, tags, d, best)| {
            let tags: Vec<String> = serde_json::from_str(&tags).unwrap_or_default();
            (
                ideas::norm(&format!("{t} {}", tags.join(" "))),
                d,
                best.unwrap_or(0),
            )
        })
        .collect())
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
            while j + 1 < idx.len() && v[idx[j + 1]] == v[idx[i]] {
                j += 1
            }
            for k in i..=j {
                r[idx[k]] = (i + j) as f64 / 2.0
            }
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
    if va <= 0.0 || vb <= 0.0 {
        0.0
    } else {
        cov / (va * vb).sqrt()
    }
}

async fn calibrate(st: &AppState) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE idea_model SET calibrated_at = datetime('now') WHERE id = 1")
        .execute(&st.db)
        .await?;
    let snaps: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT day, niche, axes FROM fn_niches WHERE axes IS NOT NULL AND day <= date('now', '-28 days') AND day >= date('now', '-180 days')").fetch_all(&st.db).await?;
    if snaps.is_empty() {
        return Ok(());
    }
    let news = newcomers(st).await?;
    let themes = ideas::themes();
    let pats_of = |key: &str| -> Vec<String> {
        if let Some(w) = key.strip_prefix("w:") {
            return vec![format!(" {w} ")];
        }
        themes
            .iter()
            .find(|t| t.key == key)
            .map(|t| t.pats.clone())
            .unwrap_or_default()
    };
    let global = {
        let n = news.len().max(1) as f64;
        news.iter().filter(|x| x.2 >= 100).count() as f64 / n
    };
    let mut xs: Vec<[f64; 5]> = vec![];
    let mut ys: Vec<f64> = vec![];
    let mut pcache: HashMap<String, Vec<String>> = HashMap::new();
    for (day, niche, axes) in &snaps {
        let Ok(a) = serde_json::from_str::<Vec<f64>>(axes) else {
            continue;
        };
        if a.len() != 5 {
            continue;
        }
        let pats = pcache
            .entry(niche.clone())
            .or_insert_with(|| pats_of(niche))
            .clone();
        if pats.is_empty() {
            continue;
        }
        let end = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
            .map(|d| (d + chrono::Duration::days(14)).to_string())
            .unwrap_or_default();
        let group: Vec<&(String, String, i64)> = news
            .iter()
            .filter(|x| {
                x.1.as_str() >= day.as_str()
                    && x.1 <= end
                    && pats.iter().any(|p| x.0.contains(p.as_str()))
            })
            .collect();
        if group.len() < 3 {
            continue;
        }
        let ok = group.iter().filter(|x| x.2 >= 100).count() as f64;
        xs.push([a[0], a[1], a[2], a[3], a[4]]);
        ys.push((ok + global * 4.0) / (group.len() as f64 + 4.0));
    }
    let n = xs.len();
    let corr: Vec<f64> = (0..5)
        .map(|k| {
            if n >= 2 {
                spearman(&xs.iter().map(|x| x[k]).collect::<Vec<_>>(), &ys)
            } else {
                0.0
            }
        })
        .collect();
    let mut weights = ideas::DEFAULT_WEIGHTS.to_vec();
    let alpha = if n >= 30 {
        (n as f64 / 400.0).min(0.7)
    } else {
        0.0
    };
    let pos: Vec<f64> = corr.iter().map(|c| c.max(0.0)).collect();
    let sp: f64 = pos.iter().sum();
    if alpha > 0.0 && sp > 0.0 {
        weights = weights
            .iter()
            .zip(&pos)
            .map(|(d, p)| (1.0 - alpha) * d + alpha * p / sp)
            .collect();
        // Garde-fou : aucun axe ne décide seul, aucun n'est oublié (entre 5 % et 45 %).
        for _ in 0..5 {
            let s: f64 = weights.iter().sum();
            weights = weights.iter().map(|w| (w / s).clamp(0.05, 0.45)).collect();
        }
        let s: f64 = weights.iter().sum();
        weights = weights.iter().map(|w| w / s).collect();
    }
    sqlx::query("UPDATE idea_model SET weights = ?, calib = ? WHERE id = 1")
        .bind(json!(weights).to_string())
        .bind(json!({ "samples": n, "corr": corr, "alpha": alpha, "global": global }).to_string())
        .execute(&st.db)
        .await?;
    tracing::info!(
        "vérification des niches : {n} échantillons, poids {:?}",
        weights
    );
    Ok(())
}

/// Caractéristiques des premiers jours d'une map (âges 0 à 2) : joueurs, temps de jeu, retour à J+1, progression.
pub fn features(
    by_age: &HashMap<i64, (Option<i64>, Option<f64>, Option<f64>)>,
) -> Option<[f64; 5]> {
    let early: Vec<(i64, &(Option<i64>, Option<f64>, Option<f64>))> = {
        let mut v: Vec<_> = by_age
            .iter()
            .filter(|(a, _)| (0..=2).contains(*a))
            .map(|(a, x)| (*a, x))
            .collect();
        v.sort_by_key(|x| x.0);
        v
    };
    let players: Vec<i64> = early.iter().filter_map(|x| x.1 .0).collect();
    if players.is_empty() {
        return None;
    }
    let p = *players.iter().max().unwrap();
    let mins = early
        .iter()
        .rev()
        .find_map(|x| x.1 .1)
        .unwrap_or(0.0)
        .min(120.0);
    let d1 = by_age
        .iter()
        .filter(|(a, _)| (0..=3).contains(*a))
        .filter_map(|(_, x)| x.2)
        .last();
    let growth = match (players.first(), players.last()) {
        (Some(a), Some(b)) if players.len() >= 2 => ((*b as f64 + 1.0) / (*a as f64 + 1.0)).ln(),
        _ => 0.0,
    };
    Some([
        (p as f64).ln_1p(),
        mins / 30.0,
        d1.unwrap_or(0.0),
        growth,
        if d1.is_none() { 1.0 } else { 0.0 },
    ])
}

async fn train(st: &AppState) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE idea_model SET trained_at = datetime('now') WHERE id = 1")
        .execute(&st.db)
        .await?;
    let rows: Vec<(String, String, i64, Option<i64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT m.code, i.first_seen, CAST(julianday(m.day) - julianday(date(i.first_seen)) AS INTEGER), m.unique_players, m.avg_minutes, m.d1
         FROM fn_metrics m JOIN fn_islands i ON i.code = m.code
         WHERE i.seen_new = 1 AND i.first_seen > datetime('now', '-200 days') AND i.first_seen < datetime('now', '-15 days')
           AND m.day BETWEEN date(i.first_seen) AND date(i.first_seen, '+14 days')").fetch_all(&st.db).await?;
    let mut by: HashMap<
        String,
        (
            String,
            HashMap<i64, (Option<i64>, Option<f64>, Option<f64>)>,
        ),
    > = HashMap::new();
    for (c, first, a, p, m, d) in rows {
        by.entry(c)
            .or_insert_with(|| (first, HashMap::new()))
            .1
            .insert(a, (p, m, d));
    }
    // Rangées de la plus ancienne à la plus récente : la vérification se fait sur les plus récentes.
    let mut maps: Vec<&(
        String,
        HashMap<i64, (Option<i64>, Option<f64>, Option<f64>)>,
    )> = by.values().collect();
    maps.sort_by(|a, b| a.0.cmp(&b.0));
    let mut x: Vec<[f64; 5]> = vec![];
    let mut y: Vec<f64> = vec![];
    for (_, ages) in maps {
        let Some(f) = features(ages) else { continue };
        let best = ages.values().filter_map(|v| v.0).max().unwrap_or(0);
        x.push(f);
        y.push(if best >= BREAKOUT { 1.0 } else { 0.0 });
    }
    let n = x.len();
    let pos = y.iter().filter(|v| **v > 0.5).count();
    if n < 40 || pos < 5 || n - pos < 5 {
        sqlx::query("UPDATE idea_model SET launch = ? WHERE id = 1")
            .bind(json!({ "ready": false, "n": n, "positives": pos }).to_string())
            .execute(&st.db)
            .await?;
        return Ok(());
    }
    // Centrage-réduction puis régression logistique (descente de gradient, légère régularisation).
    let mean: Vec<f64> = (0..5)
        .map(|k| x.iter().map(|r| r[k]).sum::<f64>() / n as f64)
        .collect();
    let std: Vec<f64> = (0..5)
        .map(|k| {
            let v = x.iter().map(|r| (r[k] - mean[k]).powi(2)).sum::<f64>() / n as f64;
            v.sqrt().max(1e-6)
        })
        .collect();
    let z: Vec<Vec<f64>> = x
        .iter()
        .map(|r| (0..5).map(|k| (r[k] - mean[k]) / std[k]).collect())
        .collect();
    let fit = |idx: &[usize]| -> (Vec<f64>, f64) {
        let (mut w, mut b) = (vec![0.0; 5], 0.0);
        let m = idx.len().max(1) as f64;
        for _ in 0..800 {
            let mut gw = [0.0; 5];
            let mut gb = 0.0;
            for &i in idx {
                let p = 1.0
                    / (1.0 + (-(b + z[i].iter().zip(&w).map(|(a, c)| a * c).sum::<f64>())).exp());
                let e = p - y[i];
                for k in 0..5 {
                    gw[k] += e * z[i][k]
                }
                gb += e;
            }
            for k in 0..5 {
                w[k] -= 0.3 * (gw[k] / m + 0.01 * w[k])
            }
            b -= 0.3 * gb / m;
        }
        (w, b)
    };
    // Fiabilité mesurée comme pour l'avenir (#70) : on apprend sur les 3/4 les plus anciens et on vérifie sur le
    // quart le plus récent, jamais vu.
    let (train_i, test_i) = temporal_split(n);
    let (wt, bt) = fit(&train_i);
    let probs: Vec<f64> = test_i
        .iter()
        .map(|&i| sigmoid(bt + z[i].iter().zip(&wt).map(|(a, c)| a * c).sum::<f64>()))
        .collect();
    let ys: Vec<f64> = test_i.iter().map(|&i| y[i]).collect();
    let (auc, brier, hit) = (auc(&probs, &ys), brier(&probs, &ys), hit_rate(&probs, &ys));
    let all: Vec<usize> = (0..n).collect();
    let (w, b) = fit(&all);
    let params = json!({ "mean": mean, "std": std, "w": w, "b": b, "features": FEATURES });
    let model_id: i64 = sqlx::query_scalar("INSERT INTO ia_models (n, positives, test_n, auc, brier, hit_rate, params) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(n as i64).bind(pos as i64).bind(test_i.len() as i64).bind(auc).bind(brier).bind(hit).bind(params.to_string())
        .fetch_one(&st.db).await?;
    sqlx::query("UPDATE idea_model SET launch = ? WHERE id = 1")
        .bind(json!({ "ready": true, "n": n, "positives": pos, "mean": mean, "std": std, "w": w, "b": b, "auc": auc, "brier": brier, "threshold": BREAKOUT, "model_id": model_id }).to_string()).execute(&st.db).await?;
    tracing::info!(
        "modèle de lancement : {n} maps, {pos} ont percé, AUC {:.2}",
        auc
    );
    Ok(())
}

fn sigmoid(v: f64) -> f64 {
    1.0 / (1.0 + (-v).exp())
}

/// Indices d'entraînement (les 3/4 les plus anciens) et de vérification (le quart le plus récent), pour `n` maps
/// rangées de la plus ancienne à la plus récente.
pub fn temporal_split(n: usize) -> (Vec<usize>, Vec<usize>) {
    let cut = n - n / 4;
    ((0..cut).collect(), (cut..n).collect())
}

/// AUC : probabilité qu'une map qui a percé soit mieux notée qu'une map qui n'a pas percé (0,5 = au hasard).
pub fn auc(probs: &[f64], ys: &[f64]) -> f64 {
    let (mut good, mut tot) = (0.0, 0.0);
    for (i, &pi) in probs.iter().enumerate() {
        if ys[i] < 0.5 {
            continue;
        }
        for (j, &pj) in probs.iter().enumerate() {
            if ys[j] > 0.5 {
                continue;
            }
            tot += 1.0;
            if pi > pj {
                good += 1.0
            } else if pi == pj {
                good += 0.5
            }
        }
    }
    if tot > 0.0 {
        good / tot
    } else {
        0.5
    }
}

/// Score de Brier : écart moyen au carré entre la probabilité annoncée et ce qui s'est passé (0 = parfait).
pub fn brier(probs: &[f64], ys: &[f64]) -> f64 {
    if probs.is_empty() {
        return 0.0;
    }
    probs
        .iter()
        .zip(ys)
        .map(|(p, y)| (p - y).powi(2))
        .sum::<f64>()
        / probs.len() as f64
}

/// Part des maps annoncées « fortes » (50 % ou plus) qui ont vraiment percé ; rien si aucune n'a été annoncée.
pub fn hit_rate(probs: &[f64], ys: &[f64]) -> Option<f64> {
    let strong: Vec<f64> = probs
        .iter()
        .zip(ys)
        .filter(|(p, _)| **p >= 0.5)
        .map(|(_, y)| *y)
        .collect();
    (!strong.is_empty()).then(|| strong.iter().sum::<f64>() / strong.len() as f64)
}

/// Probabilité donnée par les paramètres d'un modèle (`launch`), et la part de chaque signal.
fn score(m: &Value, f: &[f64; 5]) -> Option<(f64, Vec<f64>)> {
    let g = |k: &str| {
        m[k].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<f64>>())
            .unwrap_or_default()
    };
    let (mean, std, w) = (g("mean"), g("std"), g("w"));
    if mean.len() != 5 || std.len() != 5 || w.len() != 5 {
        return None;
    }
    let contrib: Vec<f64> = (0..5).map(|k| w[k] * (f[k] - mean[k]) / std[k]).collect();
    Some((
        sigmoid(m["b"].as_f64().unwrap_or(0.0) + contrib.iter().sum::<f64>()),
        contrib,
    ))
}

async fn launch_model(st: &AppState) -> Value {
    let m: Option<String> = sqlx::query_scalar("SELECT launch FROM idea_model WHERE id = 1")
        .fetch_optional(&st.db)
        .await
        .ok()
        .flatten()
        .flatten();
    m.and_then(|m| serde_json::from_str(&m).ok())
        .unwrap_or(json!({}))
}

/// Garde la prédiction du jour de chaque nouvelle map encore dans son test d'Epic (0 à 13 jours).
async fn record_predictions(st: &AppState) -> Result<usize, sqlx::Error> {
    let m = launch_model(st).await;
    if m["ready"].as_bool() != Some(true) {
        return Ok(0);
    }
    let rows: Vec<(String, i64, i64, Option<i64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT m.code, CAST(julianday('now') - julianday(date(i.first_seen)) AS INTEGER),
                CAST(julianday(m.day) - julianday(date(i.first_seen)) AS INTEGER), m.unique_players, m.avg_minutes, m.d1
         FROM fn_metrics m JOIN fn_islands i ON i.code = m.code
         WHERE i.seen_new = 1 AND i.code NOT LIKE 'playlist_%' AND i.first_seen > datetime('now', '-14 days')
           AND m.day BETWEEN date(i.first_seen) AND date('now')",
    )
    .fetch_all(&st.db)
    .await?;
    let mut by: HashMap<String, (i64, HashMap<i64, (Option<i64>, Option<f64>, Option<f64>)>)> =
        HashMap::new();
    for (c, age, a, p, mi, d) in rows {
        by.entry(c)
            .or_insert_with(|| (age, HashMap::new()))
            .1
            .insert(a, (p, mi, d));
    }
    let mut n = 0;
    for (code, (age, ages)) in by {
        let Some(f) = features(&ages) else { continue };
        let Some((p, _)) = score(&m, &f) else {
            continue;
        };
        n += sqlx::query("INSERT OR IGNORE INTO ia_predictions (code, day, age, model_id, prob, features) VALUES (?, date('now'), ?, ?, ?, ?)")
            .bind(&code).bind(age).bind(m["model_id"].as_i64()).bind(p).bind(json!(f).to_string())
            .execute(&st.db).await?.rows_affected() as usize;
    }
    Ok(n)
}

/// Note les prédictions des maps dont le test d'Epic (14 jours) est fini : ont-elles percé ?
async fn grade(st: &AppState) -> Result<u64, sqlx::Error> {
    let n = sqlx::query(
        "UPDATE ia_predictions SET graded_at = datetime('now'),
           best = (SELECT MAX(m.unique_players) FROM fn_metrics m JOIN fn_islands i ON i.code = m.code
                   WHERE m.code = ia_predictions.code AND m.day BETWEEN date(i.first_seen) AND date(i.first_seen, '+14 days'))
         WHERE graded_at IS NULL AND code IN (SELECT code FROM fn_islands WHERE first_seen < datetime('now', '-15 days'))",
    )
    .execute(&st.db)
    .await?
    .rows_affected();
    sqlx::query("UPDATE ia_predictions SET outcome = (best >= ?) WHERE graded_at IS NOT NULL AND outcome IS NULL AND best IS NOT NULL")
        .bind(BREAKOUT)
        .execute(&st.db)
        .await?;
    Ok(n)
}

/// Administration → Apprentissage de l'IA : versions du modèle, précision réelle des prédictions vérifiées.
pub async fn admin_learning(
    axum::extract::State(st): axum::extract::State<AppState>,
    u: crate::auth::AuthUser,
) -> crate::error::ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    let models: Vec<(i64, String, i64, i64, Option<i64>, Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT id, trained_at, n, positives, test_n, auc, brier, hit_rate FROM ia_models ORDER BY id DESC LIMIT 90",
    ).fetch_all(&st.db).await?;
    // Précision réelle : prédictions faites au 3e jour (âge 2, premiers jours complets), notées, par semaine.
    let graded: Vec<(String, f64, i64)> = sqlx::query_as(
        "SELECT strftime('%Y-%W', day), prob, outcome FROM ia_predictions WHERE age = 2 AND outcome IS NOT NULL ORDER BY day",
    ).fetch_all(&st.db).await?;
    let mut weeks: Vec<(String, Vec<f64>, Vec<f64>)> = vec![];
    for (w, p, o) in graded {
        if weeks.last().map(|x| x.0 != w).unwrap_or(true) {
            weeks.push((w.clone(), vec![], vec![]));
        }
        let last = weeks.last_mut().unwrap();
        last.1.push(p);
        last.2.push(o as f64);
    }
    let recent: Vec<(String, String, i64, f64, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT p.code, i.title, p.age, p.prob, p.best, p.outcome FROM ia_predictions p JOIN fn_islands i ON i.code = p.code
         WHERE p.age = 2 AND p.graded_at IS NOT NULL ORDER BY p.graded_at DESC, p.prob DESC LIMIT 20",
    ).fetch_all(&st.db).await?;
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(graded_at IS NOT NULL), 0) FROM ia_predictions",
    )
    .fetch_one(&st.db)
    .await?;
    Ok(axum::Json(json!({
        "threshold": BREAKOUT,
        "predictions": counts.0, "graded": counts.1,
        "models": models.into_iter().map(|r| json!({ "id": r.0, "at": r.1, "n": r.2, "positives": r.3, "test_n": r.4, "auc": r.5, "brier": r.6, "hit_rate": r.7 })).collect::<Vec<_>>(),
        "weeks": weeks.iter().map(|(w, p, y)| json!({ "week": w, "n": p.len(),
            // L'AUC n'a de sens que si la semaine a des maps qui ont percé et d'autres non.
            "auc": (y.iter().any(|v| *v > 0.5) && y.iter().any(|v| *v < 0.5)).then(|| auc(p, y)), "brier": brier(p, y),
            "accuracy": p.iter().zip(y).filter(|(a, b)| (**a >= 0.5) == (**b > 0.5)).count() as f64 / p.len() as f64 })).collect::<Vec<_>>(),
        "recent": recent.into_iter().map(|r| json!({ "code": r.0, "title": r.1, "prob": r.3, "best": r.4, "outcome": r.5 })).collect::<Vec<_>>(),
    })))
}

/// Chance de percer d'une map d'après ses premiers jours, avec ce qui la fait monter ou baisser.
pub async fn predict(
    st: &AppState,
    by_age: &HashMap<i64, (Option<i64>, Option<f64>, Option<f64>)>,
) -> Value {
    let Some(f) = features(by_age) else {
        return Value::Null;
    };
    let m = launch_model(st).await;
    if m["ready"].as_bool() != Some(true) {
        return json!({ "ready": false, "n": m["n"], "positives": m["positives"], "threshold": BREAKOUT });
    }
    let Some((p, contrib)) = score(&m, &f) else {
        return Value::Null;
    };
    let mut factors: Vec<(usize, f64)> = contrib
        .iter()
        .copied()
        .enumerate()
        .filter(|(k, c)| *k != 4 && c.abs() >= 0.15)
        .collect();
    factors.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    json!({
        "ready": true, "prob": (p * 100.0).round(), "threshold": BREAKOUT, "n": m["n"], "positives": m["positives"], "auc": m["auc"],
        "factors": factors.into_iter().take(3).map(|(k, c)| json!({ "key": FEATURES[k], "effect": if c > 0.0 { "plus" } else { "moins" } })).collect::<Vec<_>>(),
    })
}

/// Fiabilité des modèles (pour l'administration et le bas de la page Idées).
pub async fn status(st: &AppState) -> Value {
    let r: Option<(
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> =
        sqlx::query_as("SELECT weights, calib, launch, calibrated_at FROM idea_model WHERE id = 1")
            .fetch_optional(&st.db)
            .await
            .ok()
            .flatten();
    let (w, c, l, at) = r.unwrap_or_default();
    let p = |s: Option<String>| {
        s.and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or(Value::Null)
    };
    json!({ "weights": p(w), "default_weights": ideas::DEFAULT_WEIGHTS, "calibration": p(c), "launch": p(l), "calibrated_at": at })
}

/// Administration : état des modèles, et recalcul immédiat.
pub async fn admin_status(
    axum::extract::State(st): axum::extract::State<AppState>,
    u: crate::auth::AuthUser,
) -> crate::error::ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    Ok(axum::Json(status(&st).await))
}
pub async fn admin_refresh(
    axum::extract::State(st): axum::extract::State<AppState>,
    u: crate::auth::AuthUser,
) -> crate::error::ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    sqlx::query("UPDATE idea_model SET calibrated_at = NULL, trained_at = NULL WHERE id = 1")
        .execute(&st.db)
        .await?;
    daily(&st).await;
    Ok(axum::Json(status(&st).await))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoupage_dans_le_temps_et_mesures() {
        let (tr, te) = temporal_split(8);
        assert_eq!(tr, (0..6).collect::<Vec<_>>());
        assert_eq!(
            te,
            vec![6, 7],
            "la vérification porte sur les maps les plus récentes"
        );
        assert_eq!(auc(&[0.9, 0.8, 0.1], &[1.0, 1.0, 0.0]), 1.0);
        assert_eq!(auc(&[0.1, 0.9], &[1.0, 0.0]), 0.0);
        assert!((brier(&[1.0, 0.0], &[1.0, 0.0])).abs() < 1e-9);
        assert!((brier(&[0.5, 0.5], &[1.0, 0.0]) - 0.25).abs() < 1e-9);
        assert_eq!(hit_rate(&[0.2, 0.3], &[1.0, 0.0]), None);
        assert_eq!(hit_rate(&[0.9, 0.6, 0.1], &[1.0, 0.0, 1.0]), Some(0.5));
    }

    /// Une map de test : `strong` décide si ses premiers jours sont bons et si elle perce ensuite.
    async fn island(st: &AppState, code: &str, days_ago: i64, strong: bool, noise: i64) {
        sqlx::query("INSERT INTO fn_islands (code, title, first_seen, seen_new) VALUES (?, ?, datetime('now', ?), 1)")
            .bind(code).bind(format!("Map {code}")).bind(format!("-{days_ago} days")).execute(&st.db).await.unwrap();
        for age in 0..=14i64 {
            if days_ago - age < 1 {
                break;
            }
            let base = if strong { 300 + 120 * age } else { 40 + noise };
            let players = if strong && age >= 6 { 1500 } else { base };
            sqlx::query("INSERT INTO fn_metrics (code, day, unique_players, avg_minutes, d1) VALUES (?, date('now', ?), ?, ?, ?)")
                .bind(code).bind(format!("-{} days", days_ago - age)).bind(players)
                .bind(if strong { 25.0 } else { 8.0 }).bind(if strong { 0.3 } else { 0.08 })
                .execute(&st.db).await.unwrap();
        }
    }

    #[tokio::test]
    async fn entraine_garde_les_predictions_et_les_note() {
        let st = crate::test_state().await;
        // 80 maps sorties il y a 20 à 180 jours ; une sur quatre perce.
        for k in 0..80i64 {
            island(
                &st,
                &format!("1000-0000-{k:04}"),
                20 + 2 * k,
                k % 4 == 0,
                k % 7,
            )
            .await;
        }
        train(&st).await.unwrap();
        let (n, test_n, a): (i64, i64, f64) =
            sqlx::query_as("SELECT n, test_n, auc FROM ia_models ORDER BY id DESC LIMIT 1")
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(n, 80);
        assert_eq!(test_n, 20, "un quart, les plus récentes");
        assert!(
            a > 0.9,
            "sur des données nettes, l'IA sépare bien les maps : AUC {a}"
        );

        // Deux nouvelles maps encore en test : leur prédiction du jour est gardée.
        island(&st, "2000-0000-0001", 3, true, 0).await;
        island(&st, "2000-0000-0002", 3, false, 3).await;
        assert_eq!(record_predictions(&st).await.unwrap(), 2);
        assert_eq!(
            record_predictions(&st).await.unwrap(),
            0,
            "une seule prédiction par map et par jour"
        );
        let (strong, weak): (f64, f64) = sqlx::query_as(
            "SELECT (SELECT prob FROM ia_predictions WHERE code = '2000-0000-0001'), (SELECT prob FROM ia_predictions WHERE code = '2000-0000-0002')",
        ).fetch_one(&st.db).await.unwrap();
        assert!(
            strong > weak,
            "la bonne map est mieux notée ({strong} > {weak})"
        );

        // Une prédiction faite il y a longtemps pour une map dont le test est fini est notée.
        sqlx::query("INSERT INTO ia_predictions (code, day, age, prob, features) VALUES ('1000-0000-0000', date('now', '-30 days'), 2, 0.8, '[]'), ('1000-0000-0001', date('now', '-30 days'), 2, 0.7, '[]'), ('2000-0000-0001', date('now', '-1 days'), 2, 0.6, '[]')")
            .execute(&st.db).await.unwrap();
        grade(&st).await.unwrap();
        let o: Vec<(String, Option<i64>)> =
            sqlx::query_as("SELECT code, outcome FROM ia_predictions WHERE age = 2 ORDER BY code")
                .fetch_all(&st.db)
                .await
                .unwrap();
        assert_eq!(
            o,
            vec![
                ("1000-0000-0000".into(), Some(1)),
                ("1000-0000-0001".into(), Some(0)),
                ("2000-0000-0001".into(), None)
            ],
            "percée, ratée, et pas encore notée tant que son test n'est pas fini"
        );
    }
}
