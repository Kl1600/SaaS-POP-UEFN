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
/// Tous les signaux connus (#71), dans cet ordre. Les 5 premiers sont la base ; les autres ne sont gardés que s'ils
/// améliorent la précision mesurée sur les maps les plus récentes.
pub const SIGNAUX: &[&str] = &[
    "players", "minutes", "d1", "growth", "no_d1", "plays_pp", "fav_pp", "rec_pp", "d7", "no_d7",
    "creator", "weekend", "genre",
];
const BASE: usize = 5;
/// Signaux candidats, par groupe (un groupe est gardé ou écarté d'un bloc).
const CANDIDATS: &[(&str, &[usize])] = &[
    ("plays_pp", &[5]),
    ("fav_pp", &[6]),
    ("rec_pp", &[7]),
    ("d7", &[8, 9]),
    ("creator", &[10]),
    ("weekend", &[11]),
    ("genre", &[12]),
];
/// Signaux demandés mais sans donnée pour les maps d'apprentissage : notés comme écartés.
const SANS_DONNEE: &[(&str, &str)] = &[
    ("hourly", "Mesurée seulement pour les maps reliées à POP."),
    ("updates", "Connues seulement pour les maps reliées à POP."),
];
/// Gain minimal de précision (AUC) pour garder un signal.
const GAIN_MIN: f64 = 0.002;

/// Chiffres d'un jour de la map (âge = jours depuis la sortie).
#[derive(Default, Clone, Debug)]
pub struct Jour {
    pub players: Option<i64>,
    pub minutes: Option<f64>,
    pub d1: Option<f64>,
    pub d7: Option<f64>,
    pub plays: Option<i64>,
    pub favorites: Option<i64>,
    pub recommends: Option<i64>,
}

/// Ce qu'on sait de la map hors de ses chiffres.
#[derive(Default, Clone, Debug)]
pub struct Contexte {
    /// Une map précédente du même créateur a-t-elle percé ?
    pub createur_a_perce: bool,
    /// Sortie un vendredi, samedi ou dimanche.
    pub weekend: bool,
    /// Taux de réussite des maps du même genre sorties avant (lissé).
    pub genre: f64,
}

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

/// Signaux d'une map (#71), dans l'ordre de SIGNAUX : premiers jours (âges 0 à 2), J+7 s'il existe (âges 0 à 7),
/// et contexte. `None` sans aucun chiffre de joueurs.
pub fn signaux(by_age: &HashMap<i64, Jour>, c: &Contexte) -> Option<Vec<f64>> {
    let mut early: Vec<(i64, &Jour)> = by_age
        .iter()
        .filter(|(a, _)| (0..=2).contains(*a))
        .map(|(a, j)| (*a, j))
        .collect();
    early.sort_by_key(|x| x.0);
    let players: Vec<i64> = early.iter().filter_map(|x| x.1.players).collect();
    if players.is_empty() {
        return None;
    }
    let p = *players.iter().max().unwrap();
    let total: i64 = players.iter().sum();
    let mins = early
        .iter()
        .rev()
        .find_map(|x| x.1.minutes)
        .unwrap_or(0.0)
        .min(120.0);
    let d1 = by_age
        .iter()
        .filter(|(a, _)| (0..=3).contains(*a))
        .filter_map(|(_, x)| x.d1)
        .last();
    let growth = match (players.first(), players.last()) {
        (Some(a), Some(b)) if players.len() >= 2 => ((*b as f64 + 1.0) / (*a as f64 + 1.0)).ln(),
        _ => 0.0,
    };
    let somme =
        |f: fn(&Jour) -> Option<i64>| early.iter().filter_map(|x| f(x.1)).sum::<i64>() as f64;
    let par_joueur = |v: f64| if total > 0 { v / total as f64 } else { 0.0 };
    let d7 = by_age
        .iter()
        .filter(|(a, _)| (0..=7).contains(*a))
        .filter_map(|(_, x)| x.d7)
        .last();
    Some(vec![
        (p as f64).ln_1p(),
        mins / 30.0,
        d1.unwrap_or(0.0),
        growth,
        if d1.is_none() { 1.0 } else { 0.0 },
        par_joueur(somme(|j| j.plays)).ln_1p(),
        par_joueur(somme(|j| j.favorites)) * 100.0,
        par_joueur(somme(|j| j.recommends)) * 100.0,
        d7.unwrap_or(0.0),
        if d7.is_none() { 1.0 } else { 0.0 },
        if c.createur_a_perce { 1.0 } else { 0.0 },
        if c.weekend { 1.0 } else { 0.0 },
        c.genre,
    ])
}

/// Genre d'une map : sa catégorie, sinon son premier tag.
fn genre_de(category: &str, tags: &str) -> String {
    if !category.trim().is_empty() {
        return category.trim().to_lowercase();
    }
    serde_json::from_str::<Vec<String>>(tags)
        .ok()
        .and_then(|t| t.into_iter().next())
        .unwrap_or_default()
        .to_lowercase()
}

/// Les maps suivies, avec leurs jours et leur contexte. `depuis`/`jusqu` bornent la date de sortie (SQLite).
/// Le contexte n'utilise que des maps sorties avant chacune (pas de triche sur l'avenir).
async fn maps_suivies(
    st: &AppState,
    depuis: &str,
    jusqu: &str,
    code: Option<&str>,
) -> Result<Vec<(String, String, i64, HashMap<i64, Jour>, Contexte)>, sqlx::Error> {
    // Historique complet (sortie, créateur, genre, meilleur score des 14 jours) pour le contexte.
    let hist: Vec<(String, String, String, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT i.code, i.first_seen, i.creator, i.category, i.tags,
                (SELECT MAX(m.unique_players) FROM fn_metrics m WHERE m.code = i.code AND m.day BETWEEN date(i.first_seen) AND date(i.first_seen, '+14 days'))
         FROM fn_islands i WHERE i.seen_new = 1 AND i.code NOT LIKE 'playlist_%' ORDER BY i.first_seen",
    )
    .fetch_all(&st.db)
    .await?;
    let rows: Vec<(String, String, i64, i64, Option<i64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT m.code, i.first_seen, CAST(julianday('now') - julianday(date(i.first_seen)) AS INTEGER),
                CAST(julianday(m.day) - julianday(date(i.first_seen)) AS INTEGER),
                m.unique_players, m.avg_minutes, m.d1, m.d7, m.plays, m.favorites, m.recommends
         FROM fn_metrics m JOIN fn_islands i ON i.code = m.code
         WHERE i.seen_new = 1 AND i.code NOT LIKE 'playlist_%' AND i.first_seen > datetime('now', ?) AND i.first_seen < datetime('now', ?)
           AND (? IS NULL OR i.code = ?)
           AND m.day BETWEEN date(i.first_seen) AND date(i.first_seen, '+14 days')",
    )
    .bind(depuis)
    .bind(jusqu)
    .bind(code)
    .bind(code)
    .fetch_all(&st.db)
    .await?;
    let mut by: HashMap<String, (String, i64, HashMap<i64, Jour>)> = HashMap::new();
    for (c, first, age, a, p, mi, d1, d7, pl, fa, re) in rows {
        by.entry(c)
            .or_insert_with(|| (first, age, HashMap::new()))
            .2
            .insert(
                a,
                Jour {
                    players: p,
                    minutes: mi,
                    d1,
                    d7,
                    plays: pl,
                    favorites: fa,
                    recommends: re,
                },
            );
    }
    let mut out = vec![];
    for (c, (first, age, jours)) in by {
        let date = first.get(..10).unwrap_or(&first).to_string();
        let mut ctx = Contexte::default();
        let mine = hist.iter().find(|h| h.0 == c);
        let (createur, genre) = mine
            .map(|h| (h.2.trim().to_lowercase(), genre_de(&h.3, &h.4)))
            .unwrap_or_default();
        // Maps sorties avant, dont le test d'Epic (14 jours) était fini à la sortie de celle-ci.
        let fini = |h: &(String, String, String, String, String, Option<i64>)| {
            h.0 != c
                && h.1
                    .get(..10)
                    .map(|d| jour_plus(d, 15) <= date)
                    .unwrap_or(false)
        };
        ctx.createur_a_perce = !createur.is_empty()
            && hist.iter().any(|h| {
                fini(h) && h.2.trim().to_lowercase() == createur && h.5.unwrap_or(0) >= BREAKOUT
            });
        let (mut n, mut r) = (0.0, 0.0);
        if !genre.is_empty() {
            for h in hist
                .iter()
                .filter(|h| fini(h) && genre_de(&h.3, &h.4) == genre)
            {
                n += 1.0;
                if h.5.unwrap_or(0) >= BREAKOUT {
                    r += 1.0;
                }
            }
        }
        // Lissage vers un taux de base de 10 % (5 maps fictives).
        ctx.genre = (r + 0.5) / (n + 5.0);
        ctx.weekend = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map(|d| {
                matches!(
                    chrono::Datelike::weekday(&d),
                    chrono::Weekday::Fri | chrono::Weekday::Sat | chrono::Weekday::Sun
                )
            })
            .unwrap_or(false);
        out.push((c, first, age, jours, ctx));
    }
    Ok(out)
}

/// « AAAA-MM-JJ » + n jours.
fn jour_plus(d: &str, n: i64) -> String {
    chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
        .map(|x| (x + chrono::Duration::days(n)).to_string())
        .unwrap_or_default()
}

/// Régression logistique sur les colonnes `cols` (centrées-réduites), lignes `idx`.
fn ajuster(z: &[Vec<f64>], y: &[f64], cols: &[usize], idx: &[usize]) -> (Vec<f64>, f64) {
    let k = cols.len();
    let (mut w, mut b) = (vec![0.0; k], 0.0);
    let m = idx.len().max(1) as f64;
    for _ in 0..800 {
        let mut gw = vec![0.0; k];
        let mut gb = 0.0;
        for &i in idx {
            let v: f64 = cols.iter().zip(&w).map(|(c, wk)| z[i][*c] * wk).sum();
            let e = sigmoid(b + v) - y[i];
            for (j, c) in cols.iter().enumerate() {
                gw[j] += e * z[i][*c]
            }
            gb += e;
        }
        for j in 0..k {
            w[j] -= 0.3 * (gw[j] / m + 0.01 * w[j])
        }
        b -= 0.3 * gb / m;
    }
    (w, b)
}

/// Précision sur l'avenir d'un jeu de colonnes : appris sur les 3/4 les plus anciens, testé sur le quart récent.
fn precision(z: &[Vec<f64>], y: &[f64], cols: &[usize]) -> (f64, f64, Option<f64>, usize) {
    let (tr_i, te_i) = temporal_split(z.len());
    let (w, b) = ajuster(z, y, cols, &tr_i);
    let probs: Vec<f64> = te_i
        .iter()
        .map(|&i| {
            sigmoid(
                b + cols
                    .iter()
                    .zip(&w)
                    .map(|(c, wk)| z[i][*c] * wk)
                    .sum::<f64>(),
            )
        })
        .collect();
    let ys: Vec<f64> = te_i.iter().map(|&i| y[i]).collect();
    (
        auc(&probs, &ys),
        brier(&probs, &ys),
        hit_rate(&probs, &ys),
        te_i.len(),
    )
}

async fn train(st: &AppState) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE idea_model SET trained_at = datetime('now') WHERE id = 1")
        .execute(&st.db)
        .await?;
    let mut maps = maps_suivies(st, "-200 days", "-15 days", None).await?;
    // De la plus ancienne à la plus récente : la vérification se fait sur les plus récentes.
    maps.sort_by(|a, b| a.1.cmp(&b.1));
    let mut x: Vec<Vec<f64>> = vec![];
    let mut y: Vec<f64> = vec![];
    for (_, _, _, jours, ctx) in &maps {
        let Some(f) = signaux(jours, ctx) else {
            continue;
        };
        let best = jours.values().filter_map(|v| v.players).max().unwrap_or(0);
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
    let d = SIGNAUX.len();
    let mean: Vec<f64> = (0..d)
        .map(|k| x.iter().map(|r| r[k]).sum::<f64>() / n as f64)
        .collect();
    let std: Vec<f64> = (0..d)
        .map(|k| {
            (x.iter().map(|r| (r[k] - mean[k]).powi(2)).sum::<f64>() / n as f64)
                .sqrt()
                .max(1e-6)
        })
        .collect();
    let z: Vec<Vec<f64>> = x
        .iter()
        .map(|r| (0..d).map(|k| (r[k] - mean[k]) / std[k]).collect())
        .collect();
    // Choix des signaux (#71) : on part de la base ; chaque candidat n'est gardé que s'il améliore la précision sur
    // l'avenir. Un signal qui ne varie pas (aucune donnée) est écarté d'office.
    let mut cols: Vec<usize> = (0..BASE).collect();
    let (auc_base, ..) = precision(&z, &y, &cols);
    let mut meilleure = auc_base;
    let mut choix: Vec<Value> = vec![];
    for (nom, groupe) in CANDIDATS {
        if groupe.iter().all(|k| std[*k] <= 1e-6) {
            choix.push(json!({ "key": nom, "kept": false, "gain": null, "why": "Pas de donnée pour ces maps." }));
            continue;
        }
        let essai: Vec<usize> = cols.iter().chain(groupe.iter()).copied().collect();
        let (a, ..) = precision(&z, &y, &essai);
        let gain = a - meilleure;
        if gain > GAIN_MIN {
            cols = essai;
            meilleure = a;
            choix.push(json!({ "key": nom, "kept": true, "gain": gain }));
        } else {
            choix.push(json!({ "key": nom, "kept": false, "gain": gain, "why": "N'améliore pas la précision sur l'avenir." }));
        }
    }
    for (nom, why) in SANS_DONNEE {
        choix.push(json!({ "key": nom, "kept": false, "gain": null, "why": why }));
    }
    let (auc, brier, hit, test_n) = precision(&z, &y, &cols);
    let all: Vec<usize> = (0..n).collect();
    let (w, b) = ajuster(&z, &y, &cols, &all);
    let noms: Vec<&str> = cols.iter().map(|k| SIGNAUX[*k]).collect();
    let mean_c: Vec<f64> = cols.iter().map(|k| mean[*k]).collect();
    let std_c: Vec<f64> = cols.iter().map(|k| std[*k]).collect();
    let params = json!({ "mean": mean_c, "std": std_c, "w": w, "b": b, "features": noms, "signaux": choix, "auc_base": auc_base });
    let model_id: i64 = sqlx::query_scalar("INSERT INTO ia_models (n, positives, test_n, auc, brier, hit_rate, params) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(n as i64).bind(pos as i64).bind(test_n as i64).bind(auc).bind(brier).bind(hit).bind(params.to_string())
        .fetch_one(&st.db).await?;
    sqlx::query("UPDATE idea_model SET launch = ? WHERE id = 1")
        .bind(json!({ "ready": true, "n": n, "positives": pos, "mean": mean_c, "std": std_c, "w": w, "b": b, "features": noms, "signaux": choix,
            "auc": auc, "auc_base": auc_base, "brier": brier, "threshold": BREAKOUT, "model_id": model_id }).to_string()).execute(&st.db).await?;
    tracing::info!("modèle de lancement : {n} maps, {pos} ont percé, AUC {auc:.2} (base {auc_base:.2}), signaux {noms:?}");
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

/// Probabilité donnée par les paramètres d'un modèle (`launch`), et la part de chaque signal utilisé, par nom.
/// `f` suit l'ordre de SIGNAUX ; un ancien modèle sans liste de signaux utilise la base.
fn score(m: &Value, f: &[f64]) -> Option<(f64, Vec<(&'static str, f64)>)> {
    let g = |k: &str| {
        m[k].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<f64>>())
            .unwrap_or_default()
    };
    let (mean, std, w) = (g("mean"), g("std"), g("w"));
    let noms: Vec<String> = m["features"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| SIGNAUX[..BASE].iter().map(|s| s.to_string()).collect());
    if mean.len() != noms.len() || std.len() != noms.len() || w.len() != noms.len() {
        return None;
    }
    let mut contrib = vec![];
    for (j, nom) in noms.iter().enumerate() {
        let k = SIGNAUX.iter().position(|s| s == nom)?;
        contrib.push((SIGNAUX[k], w[j] * (f.get(k)? - mean[j]) / std[j]));
    }
    Some((
        sigmoid(m["b"].as_f64().unwrap_or(0.0) + contrib.iter().map(|c| c.1).sum::<f64>()),
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
    let mut n = 0;
    for (code, _, age, jours, ctx) in maps_suivies(st, "-14 days", "+1 days", None).await? {
        // Les jours pas encore arrivés ne comptent pas.
        let jours: HashMap<i64, Jour> = jours.into_iter().filter(|(a, _)| *a <= age).collect();
        let Some(f) = signaux(&jours, &ctx) else {
            continue;
        };
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
    let l = launch_model(&st).await;
    let poids: Vec<Value> = l["features"]
        .as_array()
        .zip(l["w"].as_array())
        .map(|(f, w)| {
            f.iter()
                .zip(w)
                .map(|(k, v)| json!({ "key": k, "weight": v }))
                .collect()
        })
        .unwrap_or_default();
    Ok(axum::Json(json!({
        "threshold": BREAKOUT,
        "signaux": l["signaux"], "poids": poids, "auc_base": l["auc_base"],
        "predictions": counts.0, "graded": counts.1,
        "models": models.into_iter().map(|r| json!({ "id": r.0, "at": r.1, "n": r.2, "positives": r.3, "test_n": r.4, "auc": r.5, "brier": r.6, "hit_rate": r.7 })).collect::<Vec<_>>(),
        "weeks": weeks.iter().map(|(w, p, y)| json!({ "week": w, "n": p.len(),
            // L'AUC n'a de sens que si la semaine a des maps qui ont percé et d'autres non.
            "auc": (y.iter().any(|v| *v > 0.5) && y.iter().any(|v| *v < 0.5)).then(|| auc(p, y)), "brier": brier(p, y),
            "accuracy": p.iter().zip(y).filter(|(a, b)| (**a >= 0.5) == (**b > 0.5)).count() as f64 / p.len() as f64 })).collect::<Vec<_>>(),
        "recent": recent.into_iter().map(|r| json!({ "code": r.0, "title": r.1, "prob": r.3, "best": r.4, "outcome": r.5 })).collect::<Vec<_>>(),
    })))
}

/// Chance de percer d'une île d'après ses premiers jours, avec les signaux qui la font monter ou baisser (#71).
pub async fn predict(st: &AppState, code: &str) -> Value {
    let Ok(mut l) = maps_suivies(st, "-400 days", "+1 days", Some(code)).await else {
        return Value::Null;
    };
    let Some((_, _, _, jours, ctx)) = l.pop() else {
        return Value::Null;
    };
    let Some(f) = signaux(&jours, &ctx) else {
        return Value::Null;
    };
    let m = launch_model(st).await;
    if m["ready"].as_bool() != Some(true) {
        return json!({ "ready": false, "n": m["n"], "positives": m["positives"], "threshold": BREAKOUT });
    }
    let Some((p, contrib)) = score(&m, &f) else {
        return Value::Null;
    };
    // Les drapeaux « pas de donnée » ne s'expliquent pas à l'utilisateur.
    let mut factors: Vec<(&str, f64)> = contrib
        .into_iter()
        .filter(|(k, c)| !k.starts_with("no_") && c.abs() >= 0.15)
        .collect();
    factors.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    json!({
        "ready": true, "prob": (p * 100.0).round(), "threshold": BREAKOUT, "n": m["n"], "positives": m["positives"], "auc": m["auc"],
        "factors": factors.into_iter().take(4).map(|(k, c)| json!({ "key": k, "effect": if c > 0.0 { "plus" } else { "moins" } })).collect::<Vec<_>>(),
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

    /// #71 : un signal utile (le créateur a déjà percé) est gardé et cité ; un signal sans lien (genre tiré au
    /// hasard) est écarté ; la précision n'est jamais en dessous de la base.
    #[tokio::test]
    async fn garde_seulement_les_signaux_utiles() {
        let st = crate::test_state().await;
        for k in 0..120i64 {
            let star = k % 3 == 0;
            let code = format!("3000-0000-{k:04}");
            let days_ago = 20 + k;
            sqlx::query("INSERT INTO fn_islands (code, title, creator, category, first_seen, seen_new) VALUES (?, 'M', ?, ?, datetime('now', ?), 1)")
                .bind(&code).bind(if star { "Star" } else { "Autre" }).bind(if k % 2 == 0 { "a" } else { "b" })
                .bind(format!("-{days_ago} days")).execute(&st.db).await.unwrap();
            for age in 0..=14i64 {
                if days_ago - age < 1 {
                    break;
                }
                // Premiers jours semblables pour tous : seules les maps du créateur « Star » percent ensuite.
                let players = if star && age >= 6 {
                    1500
                } else {
                    100 + (k * 7 + age * 3) % 40
                };
                sqlx::query("INSERT INTO fn_metrics (code, day, unique_players, avg_minutes, d1) VALUES (?, date('now', ?), ?, 10, 0.1)")
                    .bind(&code).bind(format!("-{} days", days_ago - age)).bind(players).execute(&st.db).await.unwrap();
            }
        }
        train(&st).await.unwrap();
        let l = launch_model(&st).await;
        assert_eq!(l["ready"], true, "{l}");
        let choix = l["signaux"].as_array().unwrap();
        let garde = |k: &str| {
            choix
                .iter()
                .find(|c| c["key"] == k)
                .map(|c| c["kept"] == true)
                .unwrap()
        };
        assert!(
            garde("creator"),
            "le créateur prédit la réussite : {choix:?}"
        );
        assert!(!garde("genre"), "le genre est tiré au hasard : {choix:?}");
        assert!(
            choix
                .iter()
                .any(|c| c["key"] == "hourly" && c["kept"] == false),
            "les signaux sans donnée sont notés"
        );
        assert!(l["auc"].as_f64().unwrap() >= l["auc_base"].as_f64().unwrap());
        assert!(l["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "creator"));
        // La chance de percer d'une nouvelle map du créateur « Star » cite ce signal.
        sqlx::query("INSERT INTO fn_islands (code, title, creator, first_seen, seen_new) VALUES ('4000-0000-0001', 'N', 'Star', datetime('now', '-2 days'), 1)").execute(&st.db).await.unwrap();
        for age in 0..2 {
            sqlx::query("INSERT INTO fn_metrics (code, day, unique_players, avg_minutes, d1) VALUES ('4000-0000-0001', date('now', ?), 110, 10, 0.1)")
                .bind(format!("-{} days", 2 - age)).execute(&st.db).await.unwrap();
        }
        let p = predict(&st, "4000-0000-0001").await;
        assert!(
            p["factors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["key"] == "creator" && f["effect"] == "plus"),
            "{p}"
        );
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
