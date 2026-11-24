//! Équipe d'une map et avis de faisabilité (#149) : qui travaille sur la map, combien d'heures par semaine et pour
//! quand ; POP compare les heures disponibles jusqu'à la sortie au travail restant (1 h par tâche ouverte, ou le
//! gabarit du genre si la map n'a pas encore de tâches) et dit « Faisable », « Tendu » ou « Pas faisable », avec des
//! corrections chiffrées. Calcul local, sans IA externe.
use crate::{
    access::{map_ctx, notify, MapCtx, Role},
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use time::{macros::format_description, Date, Duration, OffsetDateTime, Weekday};

const FMT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day]");

fn date(s: &str) -> Option<Date> {
    Date::parse(s.trim(), FMT).ok()
}
fn texte(d: Date) -> String {
    d.format(FMT).unwrap_or_default()
}
fn ouvre(d: Date) -> bool {
    !matches!(d.weekday(), Weekday::Saturday | Weekday::Sunday)
}

/// Une personne de l'équipe de la map.
#[derive(Clone, Debug)]
pub struct Personne {
    pub id: i64,
    pub nom: String,
    pub heures: i64,
    pub absences: Vec<(Date, Date)>,
}

/// Jours ouvrés où la personne travaille sur la map, de `debut` (inclus) à `fin` (exclu).
fn jours(p: &Personne, debut: Date, fin: Date) -> i64 {
    let mut n = 0;
    let mut d = debut;
    while d < fin {
        if ouvre(d) && !p.absences.iter().any(|(a, b)| *a <= d && d <= *b) {
            n += 1;
        }
        d += Duration::days(1);
    }
    n
}

fn capacite(equipe: &[Personne], debut: Date, fin: Date) -> f64 {
    equipe
        .iter()
        .map(|p| jours(p, debut, fin) as f64 * p.heures as f64 / 5.0)
        .sum()
}

/// Avis : `taches` heures de travail (1 h par tâche), dont `ia_pct` % confiées à l'IA, marge `marge_pct` %.
/// `a_retirer` : les tâches les moins prioritaires, dans l'ordre où on les retirerait.
pub fn avis(
    aujourd_hui: Date,
    debut: Option<Date>,
    sortie: Option<Date>,
    equipe: &[Personne],
    taches: i64,
    ia_pct: i64,
    marge_pct: i64,
    a_retirer: &[(i64, String)],
) -> Value {
    let Some(sortie) = sortie else {
        return json!({ "status": "sans_date", "message": "Donnez une date de sortie visée pour savoir si c'est faisable." });
    };
    if equipe.is_empty() {
        return json!({ "status": "sans_equipe", "message": "Indiquez qui travaille sur la map et combien d'heures par semaine." });
    }
    let debut = debut.filter(|d| *d > aujourd_hui).unwrap_or(aujourd_hui);
    let dispo = capacite(equipe, debut, sortie);
    let besoin = taches as f64 * (100 - ia_pct.clamp(0, 100)) as f64 / 100.0;
    let cible = besoin * (100 + marge_pct.max(0)) as f64 / 100.0;
    let arrondi = |x: f64| (x * 10.0).round() / 10.0;
    let statut = if dispo >= cible {
        "faisable"
    } else if dispo >= besoin {
        "tendu"
    } else {
        "pas_faisable"
    };
    let mut corrections = vec![];
    if statut != "faisable" {
        let manque = cible - dispo;
        // 1. Repousser la sortie : premier jour où la capacité couvre le travail et la marge.
        let mut fin = sortie;
        for _ in 0..730 {
            fin += Duration::days(1);
            if capacite(equipe, debut, fin) >= cible {
                corrections.push(json!({ "kind": "date", "date": texte(fin), "text": format!("Repousser la sortie au {}", fin.format(format_description!("[day]/[month]/[year]")).unwrap_or_default()) }));
                break;
            }
        }
        // 2. Des heures en plus pour la personne la plus disponible.
        if let Some((p, j)) = equipe
            .iter()
            .map(|p| (p, jours(p, debut, sortie)))
            .filter(|(_, j)| *j > 0)
            .max_by_key(|(p, j)| (*j, p.heures))
        {
            let h = (manque / (j as f64 / 5.0)).ceil() as i64;
            corrections.push(json!({ "kind": "heures", "user_id": p.id, "hours": h, "text": format!("Ajouter {h} h par semaine à {}", p.nom) }));
        }
        // 3 et 4. Une tâche confiée à l'IA ou retirée enlève 1 h de travail et sa marge.
        let n = (manque / ((100 + marge_pct.max(0)) as f64 / 100.0)).ceil() as i64;
        let n = n.min(taches);
        if n > 0 {
            corrections.push(json!({ "kind": "ia", "tasks": n, "text": format!("Confier {n} tâche(s) de plus à l'IA") }));
            let noms: Vec<Value> = a_retirer
                .iter()
                .take(n as usize)
                .map(|(num, t)| json!({ "number": num, "title": t }))
                .collect();
            corrections.push(json!({ "kind": "retirer", "tasks": n, "list": noms, "text": format!("Retirer {n} tâche(s), les moins prioritaires") }));
        }
    }
    json!({
        "status": statut,
        "available_hours": arrondi(dispo),
        "needed_hours": arrondi(besoin),
        "with_margin": arrondi(cible),
        "margin_pct": marge_pct,
        "ai_share_pct": ia_pct,
        "tasks": taches,
        "start": texte(debut),
        "release": texte(sortie),
        "corrections": corrections,
    })
}

/// L'équipe enregistrée de la map, avec les indisponibilités.
async fn equipe(st: &AppState, mid: i64) -> Result<Vec<Personne>, AppError> {
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT s.user_id, u.name, s.hours_per_week FROM map_staff s JOIN users u ON u.id = s.user_id WHERE s.map_id = ? ORDER BY u.name",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let abs: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT user_id, start_date, end_date FROM map_absences WHERE map_id = ? ORDER BY start_date")
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(id, nom, heures)| Personne {
            id,
            nom,
            heures,
            absences: abs
                .iter()
                .filter(|a| a.0 == id)
                .filter_map(|a| Some((date(&a.1)?, date(&a.2)?)))
                .collect(),
        })
        .collect())
}

/// Heures de chaque membre de l'équipe sur toutes les maps du studio, au-delà de ses heures du studio.
async fn surcharges(st: &AppState, mid: i64) -> Result<Vec<Value>, AppError> {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT u.name, SUM(s.hours_per_week), ms.hours_per_week FROM map_staff s JOIN maps m ON m.id = s.map_id
         JOIN memberships ms ON ms.studio_id = m.studio_id AND ms.user_id = s.user_id JOIN users u ON u.id = s.user_id
         WHERE m.studio_id = (SELECT studio_id FROM maps WHERE id = ?)
           AND s.user_id IN (SELECT user_id FROM map_staff WHERE map_id = ?)
         GROUP BY s.user_id HAVING SUM(s.hours_per_week) > ms.hours_per_week",
    )
    .bind(mid)
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(nom, total, dispo)| json!({ "name": nom, "total": total, "studio_hours": dispo, "text": format!("{nom} : {total} h par semaine sur les maps du studio, pour {dispo} h disponibles") }))
        .collect())
}

/// L'avis de la map, d'après ses tâches ouvertes (ou le gabarit du genre si elle n'en a pas encore).
pub async fn avis_de_la_map(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let (sortie, debut, marge, ia, genre): (Option<String>, Option<String>, i64, i64, String) =
        sqlx::query_as("SELECT release_date, start_date, margin_pct, ai_share_pct, genre FROM maps WHERE id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await?;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ?")
        .bind(mid)
        .fetch_one(&st.db)
        .await?;
    let ouvertes: Vec<(i64, String)> = sqlx::query_as(
        "SELECT number, title FROM tasks WHERE map_id = ? AND status <> 'done'
         ORDER BY status = 'doing', planned_day IS NOT NULL, planned_day DESC, number DESC",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let (taches, source) = if total == 0 {
        (
            crate::plan::taille_du_gabarit(&crate::lancement::gabarit(&genre)) as i64,
            "gabarit",
        )
    } else {
        (ouvertes.len() as i64, "taches")
    };
    let a_retirer: Vec<(i64, String)> = ouvertes
        .into_iter()
        .filter(|_| source == "taches")
        .collect();
    let mut a = avis(
        OffsetDateTime::now_utc().date(),
        debut.as_deref().and_then(date),
        sortie.as_deref().and_then(date),
        &equipe(st, mid).await?,
        taches,
        ia,
        marge,
        &a_retirer,
    );
    a["source"] = json!(source);
    a["overloads"] = json!(surcharges(st, mid).await?);
    Ok(a)
}

#[derive(Deserialize, Default)]
pub struct Absence {
    start: String,
    end: String,
}

#[derive(Deserialize)]
pub struct Membre {
    user_id: i64,
    hours: i64,
    #[serde(default)]
    absences: Vec<Absence>,
}

#[derive(Deserialize)]
pub struct EquipeIn {
    pub start_date: Option<String>,
    pub margin_pct: Option<i64>,
    pub ai_share_pct: Option<i64>,
    pub staff: Vec<Membre>,
}

/// Enregistre l'équipe de la map (remplace l'ancienne). Membres du studio seulement.
pub async fn enregistrer(st: &AppState, ctx: &MapCtx, b: &EquipeIn) -> Result<(), AppError> {
    ctx.require(Role::Admin)?;
    if let Some(d) = b.start_date.as_deref().filter(|d| !d.is_empty()) {
        date(d).ok_or_else(|| bad("Date de début invalide (AAAA-MM-JJ)."))?;
    }
    if b.margin_pct.is_some_and(|m| !(0..=100).contains(&m))
        || b.ai_share_pct.is_some_and(|m| !(0..=100).contains(&m))
    {
        return Err(bad("Marge et part de l'IA : entre 0 et 100 %."));
    }
    if b.staff.len() > 50 {
        return Err(bad("50 personnes au plus."));
    }
    for m in &b.staff {
        if !(1..=80).contains(&m.hours) {
            return Err(bad("Heures par semaine : entre 1 et 80."));
        }
        let membre: Option<i64> = sqlx::query_scalar(
            "SELECT user_id FROM memberships WHERE studio_id = ? AND user_id = ?",
        )
        .bind(ctx.studio_id)
        .bind(m.user_id)
        .fetch_optional(&st.db)
        .await?;
        if membre.is_none() {
            return Err(bad(
                "Seuls les membres du studio peuvent être dans l'équipe de la map.",
            ));
        }
        for a in &m.absences {
            match (date(&a.start), date(&a.end)) {
                (Some(x), Some(y)) if x <= y => {}
                _ => {
                    return Err(bad(
                        "Indisponibilité : dates de début et de fin valides, dans l'ordre.",
                    ))
                }
            }
        }
    }
    let mut tx = st.db.begin().await?;
    sqlx::query("UPDATE maps SET start_date = ?, margin_pct = COALESCE(?, margin_pct), ai_share_pct = COALESCE(?, ai_share_pct) WHERE id = ?")
        .bind(b.start_date.as_deref().filter(|d| !d.is_empty()))
        .bind(b.margin_pct)
        .bind(b.ai_share_pct)
        .bind(ctx.map_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM map_staff WHERE map_id = ?")
        .bind(ctx.map_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM map_absences WHERE map_id = ?")
        .bind(ctx.map_id)
        .execute(&mut *tx)
        .await?;
    for m in &b.staff {
        sqlx::query(
            "INSERT OR REPLACE INTO map_staff (map_id, user_id, hours_per_week) VALUES (?, ?, ?)",
        )
        .bind(ctx.map_id)
        .bind(m.user_id)
        .bind(m.hours)
        .execute(&mut *tx)
        .await?;
        for a in &m.absences {
            sqlx::query("INSERT INTO map_absences (map_id, user_id, start_date, end_date) VALUES (?, ?, ?, ?)")
                .bind(ctx.map_id)
                .bind(m.user_id)
                .bind(a.start.trim())
                .bind(a.end.trim())
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    notify(st, ctx.map_id, json!({ "t": "refresh", "what": "plan" }));
    Ok(())
}

/// `GET /api/maps/{mid}/faisabilite` : l'avis, l'équipe enregistrée et les membres du studio (pour l'éditer).
pub async fn route_lire(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let (debut, marge, ia): (Option<String>, i64, i64) =
        sqlx::query_as("SELECT start_date, margin_pct, ai_share_pct FROM maps WHERE id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await?;
    let eq = equipe(&st, mid).await?;
    let membres: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT u.id, u.name, m.hours_per_week FROM memberships m JOIN users u ON u.id = m.user_id
         WHERE m.studio_id = ? AND m.role <> 'viewer' ORDER BY u.name",
    )
    .bind(ctx.studio_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "avis": avis_de_la_map(&st, mid).await?,
        "start_date": debut, "margin_pct": marge, "ai_share_pct": ia,
        "staff": eq.iter().map(|p| json!({ "user_id": p.id, "name": p.nom, "hours": p.heures,
            "absences": p.absences.iter().map(|(a, b)| json!({ "start": texte(*a), "end": texte(*b) })).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        "members": membres.into_iter().map(|(id, n, h)| json!({ "user_id": id, "name": n, "studio_hours": h })).collect::<Vec<_>>(),
        "can_edit": ctx.role >= Role::Admin,
    })))
}

/// `PUT /api/maps/{mid}/faisabilite` : enregistre l'équipe, rend l'avis recalculé et les surcharges.
pub async fn route_ecrire(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<EquipeIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    enregistrer(&st, &ctx, &b).await?;
    Ok(Json(avis_de_la_map(&st, mid).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        date(s).unwrap()
    }
    fn p(id: i64, h: i64) -> Personne {
        Personne {
            id,
            nom: format!("P{id}"),
            heures: h,
            absences: vec![],
        }
    }
    // Lundi 30 novembre 2026 ; sortie 3 semaines plus tard = 15 jours ouvrés.
    const LUNDI: &str = "2026-11-30";
    const SORTIE: &str = "2026-12-21";

    #[test]
    fn faisable_tendu_pas_faisable() {
        let eq = [p(1, 5), p(2, 5)]; // 2 × 5 h × 3 semaines = 30 h
        let a = avis(d(LUNDI), None, Some(d(SORTIE)), &eq, 19, 0, 20, &[]);
        assert_eq!(a["status"], "faisable", "{a}");
        assert_eq!(a["available_hours"], 30.0);
        // 27 tâches : 27 h couvertes, pas 32,4 h avec la marge.
        let a = avis(d(LUNDI), None, Some(d(SORTIE)), &eq, 27, 0, 20, &[]);
        assert_eq!(a["status"], "tendu", "{a}");
        assert!(!a["corrections"].as_array().unwrap().is_empty());
        let retirer: Vec<(i64, String)> =
            (1..=60).rev().map(|n| (n, format!("Tâche {n}"))).collect();
        let a = avis(d(LUNDI), None, Some(d(SORTIE)), &eq, 60, 0, 20, &retirer);
        assert_eq!(a["status"], "pas_faisable", "{a}");
        // Chaque correction, appliquée, rend le plan faisable ; une de moins ne suffit pas.
        let c = a["corrections"].as_array().unwrap();
        let par = |k: &str| c.iter().find(|x| x["kind"] == k).unwrap().clone();
        let nouvelle = d(par("date")["date"].as_str().unwrap());
        assert_eq!(
            avis(d(LUNDI), None, Some(nouvelle), &eq, 60, 0, 20, &[])["status"],
            "faisable"
        );
        assert_ne!(
            avis(
                d(LUNDI),
                None,
                Some(nouvelle - Duration::days(1)),
                &eq,
                60,
                0,
                20,
                &[]
            )["status"],
            "faisable"
        );
        let h = par("heures")["hours"].as_i64().unwrap();
        let plus = |x: i64| [p(1, 5 + x), p(2, 5)];
        assert_eq!(
            avis(d(LUNDI), None, Some(d(SORTIE)), &plus(h), 60, 0, 20, &[])["status"],
            "faisable"
        );
        assert_ne!(
            avis(
                d(LUNDI),
                None,
                Some(d(SORTIE)),
                &plus(h - 1),
                60,
                0,
                20,
                &[]
            )["status"],
            "faisable"
        );
        let n = par("retirer")["tasks"].as_i64().unwrap();
        assert_eq!(
            n, 35,
            "60 h + 20 % = 72 h pour 30 h : retirer 35 tâches (25 h × 1,2 = 30 h)"
        );
        assert_eq!(
            avis(d(LUNDI), None, Some(d(SORTIE)), &eq, 60 - n, 0, 20, &[])["status"],
            "faisable"
        );
        assert_ne!(
            avis(d(LUNDI), None, Some(d(SORTIE)), &eq, 60 - n + 1, 0, 20, &[])["status"],
            "faisable"
        );
        assert_eq!(
            par("retirer")["list"][0]["number"],
            60,
            "la moins prioritaire d'abord"
        );
        assert_eq!(par("ia")["tasks"], n);
    }

    #[test]
    fn indisponibilites_et_ia() {
        let mut a = p(1, 10);
        a.absences = vec![(d("2026-12-07"), d("2026-12-11"))]; // une semaine de congé : 10 h de moins
        let r = avis(
            d(LUNDI),
            None,
            Some(d(SORTIE)),
            &[a, p(2, 5)],
            10,
            0,
            20,
            &[],
        );
        assert_eq!(r["available_hours"], 35.0, "{r}");
        // La part de l'IA réduit le travail humain.
        let r = avis(d(LUNDI), None, Some(d(SORTIE)), &[p(1, 5)], 30, 50, 0, &[]);
        assert_eq!(
            (r["needed_hours"].clone(), r["status"].clone()),
            (json!(15.0), json!("faisable"))
        );
        // Sans date ni équipe : rien à calculer.
        assert_eq!(
            avis(d(LUNDI), None, None, &[p(1, 5)], 3, 0, 20, &[])["status"],
            "sans_date"
        );
        assert_eq!(
            avis(d(LUNDI), None, Some(d(SORTIE)), &[], 3, 0, 20, &[])["status"],
            "sans_equipe"
        );
    }

    #[tokio::test]
    async fn equipe_et_surcharge() {
        let st = crate::test_state().await;
        let q = |s: &'static str| sqlx::query_scalar::<_, i64>(s);
        let sid = q("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let uid = q("INSERT INTO users (email, name, password_hash) VALUES ('f@a.fr', 'Yanis', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        let autre = q("INSERT INTO users (email, name, password_hash) VALUES ('o@a.fr', 'Hors', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role, hours_per_week) VALUES (?, ?, 'owner', 10)").bind(sid).bind(uid).execute(&st.db).await.unwrap();
        let u = AuthUser {
            id: uid,
            email: "f@a.fr".into(),
            name: "Yanis".into(),
            sid: String::new(),
        };
        let mut maps = vec![];
        for n in ["A", "B"] {
            maps.push(
                crate::studio::insert_map(&st, sid, n, "Tycoon", Some("2027-06-01"))
                    .await
                    .unwrap(),
            );
        }
        let ctx = map_ctx(&st, &u, maps[0]).await.unwrap();
        let eq = |h: i64, user: i64| -> EquipeIn {
            serde_json::from_value(json!({ "start_date": "", "margin_pct": 20, "staff": [{ "user_id": user, "hours": h, "absences": [{ "start": "2027-01-04", "end": "2027-01-08" }] }] })).unwrap()
        };
        assert!(
            enregistrer(&st, &ctx, &eq(5, autre)).await.is_err(),
            "hors du studio"
        );
        enregistrer(&st, &ctx, &eq(6, uid)).await.unwrap();
        let a = avis_de_la_map(&st, maps[0]).await.unwrap();
        assert_eq!(a["source"], "gabarit", "sans tâche : le gabarit du genre");
        assert_eq!(a["overloads"].as_array().unwrap().len(), 0);
        let ctx_b = map_ctx(&st, &u, maps[1]).await.unwrap();
        enregistrer(&st, &ctx_b, &eq(6, uid)).await.unwrap();
        let a = avis_de_la_map(&st, maps[1]).await.unwrap();
        assert_eq!(a["overloads"][0]["total"], 12, "{a}");
        let Json(l) = route_lire(State(st.clone()), u.clone(), Path(maps[0]))
            .await
            .unwrap();
        assert_eq!(l["staff"][0]["hours"], 6);
        assert_eq!(l["staff"][0]["absences"][0]["start"], "2027-01-04");
    }

    /// Critère 4 : le plan répartit selon les heures de la map ; jamais plus d'heures par semaine que l'affectation.
    #[tokio::test]
    async fn plan_selon_les_heures_de_la_map() {
        let st = crate::test_state().await;
        let q = |s: &'static str| sqlx::query_scalar::<_, i64>(s);
        let sid = q("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let a = q("INSERT INTO users (email, name, password_hash) VALUES ('a@a.fr', 'A', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        let b = q("INSERT INTO users (email, name, password_hash) VALUES ('b@a.fr', 'B', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        for u in [a, b] {
            sqlx::query("INSERT INTO memberships (studio_id, user_id, role, hours_per_week) VALUES (?, ?, 'owner', 10)").bind(sid).bind(u).execute(&st.db).await.unwrap();
        }
        let mid = crate::studio::insert_map(&st, sid, "M", "", None)
            .await
            .unwrap();
        let u = AuthUser {
            id: a,
            email: "a@a.fr".into(),
            name: "A".into(),
            sid: String::new(),
        };
        let ctx = map_ctx(&st, &u, mid).await.unwrap();
        let eq: EquipeIn = serde_json::from_value(
            json!({ "staff": [{ "user_id": a, "hours": 3 }, { "user_id": b, "hours": 7 }] }),
        )
        .unwrap();
        enregistrer(&st, &ctx, &eq).await.unwrap();
        let sortie = texte(OffsetDateTime::now_utc().date() + Duration::days(120));
        let gen: crate::plan::GenIn = serde_json::from_value(json!({ "genre": "tycoon", "ai": false, "release_date": sortie, "board_id": null,
            "members": [{ "user_id": a, "tag": "code", "hours": 10 }, { "user_id": b, "tag": "code", "hours": 10 }] })).unwrap();
        let Json(p) = crate::plan::preview(State(st.clone()), u.clone(), Path(mid), Json(gen))
            .await
            .unwrap();
        // Les membres ne sont pas assignés d'office : on vérifie la charge par jour planifiée, toutes personnes
        // confondues, qui ne dépasse pas la somme des heures de la semaine (3 + 7 = 10 h).
        let mut par_semaine: std::collections::HashMap<Date, i64> = Default::default();
        for t in p["tasks"].as_array().unwrap() {
            let d = date(t["planned_day"].as_str().unwrap()).unwrap();
            *par_semaine
                .entry(d - Duration::days(d.weekday().number_days_from_monday() as i64))
                .or_default() += 1;
        }
        assert!(par_semaine.values().all(|n| *n <= 10), "{par_semaine:?}");
        assert!(
            par_semaine.values().any(|n| *n > 5),
            "les heures de la map sont bien utilisées"
        );
    }
}
