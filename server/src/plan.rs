use crate::{
    access::{log_activity, map_ctx, notify, Role},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    tasks::{insert_task, NewTask},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use time::{macros::format_description, Date, Duration, OffsetDateTime, Weekday};

const FMT: &[time::format_description::FormatItem<'static>] =
    format_description!("[year]-[month]-[day]");

fn today() -> Date {
    OffsetDateTime::now_utc().date()
}
fn parse_date(s: &str) -> Option<Date> {
    Date::parse(s, FMT).ok()
}
fn fmt(d: Date) -> String {
    d.format(FMT).unwrap_or_default()
}
fn is_workday(d: Date) -> bool {
    !matches!(d.weekday(), Weekday::Saturday | Weekday::Sunday)
}
fn next_workday(mut d: Date) -> Date {
    while !is_workday(d) {
        d += Duration::days(1);
    }
    d
}

// ---------- Vue d'ensemble ----------

pub async fn overview(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let (doing, blocked, done_week, total, done): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(status = 'doing'), 0), COALESCE(SUM(status = 'blocked'), 0),
                COALESCE(SUM(status = 'done' AND done_at >= date('now', 'weekday 1', '-7 days')), 0),
                COUNT(*), COALESCE(SUM(status = 'done'), 0)
         FROM tasks WHERE map_id = ?",
    )
    .bind(mid)
    .fetch_one(&st.db)
    .await?;
    let done_prev: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE map_id = ? AND status = 'done' AND done_at >= date('now', 'weekday 1', '-14 days') AND done_at < date('now', 'weekday 1', '-7 days')",
    )
    .bind(mid)
    .fetch_one(&st.db)
    .await?;
    let next: Option<(String, String)> = sqlx::query_as("SELECT title, date FROM milestones WHERE map_id = ? AND date >= date('now') ORDER BY date LIMIT 1").bind(mid).fetch_optional(&st.db).await?;
    let rev: Option<(String, i64, i64)> = sqlx::query_as("SELECT month, engagement_cents, sales_cents FROM revenues WHERE map_id = ? ORDER BY month DESC LIMIT 2").bind(mid).fetch_optional(&st.db).await?;
    let prev_rev: Option<(i64,)> = sqlx::query_as("SELECT engagement_cents + sales_cents FROM revenues WHERE map_id = ? ORDER BY month DESC LIMIT 1 OFFSET 1").bind(mid).fetch_optional(&st.db).await?;
    let playtest: Option<(i64, String, i64, f64)> = sqlx::query_as(
        "SELECT l.id, l.created_at, COUNT(f.id), COALESCE(AVG(f.left_minute), 0.0) FROM playtest_listings l LEFT JOIN playtest_feedback f ON f.listing_id = l.id WHERE l.map_id = ? GROUP BY l.id ORDER BY l.id DESC LIMIT 1",
    )
    .bind(mid)
    .fetch_optional(&st.db)
    .await?;
    let today_tasks: Vec<(i64, i64, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, number, title, tag, status, planned_day FROM tasks WHERE map_id = ? AND assignee_id = ? AND status IN ('todo', 'doing')
         AND (status = 'doing' OR planned_day IS NULL OR planned_day <= date('now', '+1 day')) ORDER BY status = 'doing' DESC, planned_day, number LIMIT 8",
    )
    .bind(mid)
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    let deps: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT x.task_id, d.number, d.status FROM task_deps x JOIN tasks d ON d.id = x.depends_on JOIN tasks t ON t.id = x.task_id WHERE t.map_id = ?",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let activity: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT a.kind, a.title, a.detail, u.name, a.at FROM activity a LEFT JOIN users u ON u.id = a.user_id WHERE a.map_id = ? ORDER BY a.id DESC LIMIT 60",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let ms: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, title, date, kind FROM milestones WHERE map_id = ? ORDER BY date",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "tiles": {
            "doing": doing, "blocked": blocked, "done_week": done_week, "done_prev_week": done_prev, "total": total, "done": done,
            "next_milestone": next.map(|(t, d)| json!({"title": t, "date": d})),
            "revenue": rev.map(|(m, e, s)| json!({"month": m, "cents": e + s, "engagement_cents": e, "sales_cents": s})),
            "revenue_prev_cents": prev_rev.map(|p| p.0),
            "playtest": playtest.map(|(_, at, n, avg)| json!({"at": at, "players": n, "avg_minutes": avg})),
        },
        "today": today_tasks.into_iter().map(|(id, n, title, tag, status, day)| {
            let pending: Vec<i64> = deps.iter().filter(|d| d.0 == id && d.2 != "done").map(|d| d.1).collect();
            json!({"id": id, "number": n, "title": title, "tag": tag, "status": status, "planned_day": day, "waiting_on": pending})
        }).collect::<Vec<_>>(),
        "activity": activity.into_iter().map(|(k, t, d, who, at)| json!({"kind": k, "title": t, "detail": d, "who": who, "at": at})).collect::<Vec<_>>(),
        "milestones": ms.into_iter().map(|(id, t, d, k)| json!({"id": id, "title": t, "date": d, "kind": k})).collect::<Vec<_>>(),
    })))
}

// ---------- Objectifs ----------

#[derive(Deserialize)]
pub struct MilestoneIn {
    title: String,
    date: String,
    kind: Option<String>,
    /// Heure facultative « HH:MM » (#261).
    time: Option<String>,
    detail: Option<String>,
}

/// Heure « HH:MM » valide, ou rien (champ vide = pas d'heure).
fn parse_time(t: &Option<String>) -> Result<Option<String>, AppError> {
    let Some(t) = t.as_deref().map(str::trim).filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    let ok = t.len() == 5
        && t.as_bytes()[2] == b':'
        && t[..2].parse::<u8>().is_ok_and(|h| h < 24)
        && t[3..].parse::<u8>().is_ok_and(|m| m < 60);
    if ok {
        Ok(Some(t.to_string()))
    } else {
        Err(bad("Heure invalide (format HH:MM)."))
    }
}

pub async fn add_milestone(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<MilestoneIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    check_len("Titre", &b.title, 1, 80)?;
    parse_date(&b.date).ok_or_else(|| bad("Date invalide."))?;
    let time = parse_time(&b.time)?;
    let detail = b.detail.as_deref().unwrap_or("").trim().to_string();
    check_len("Détail", &detail, 0, 2000)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO milestones (map_id, title, date, kind, time, detail) VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(mid)
    .bind(b.title.trim())
    .bind(&b.date)
    .bind(b.kind.unwrap_or_else(|| "jalon".into()))
    .bind(&time)
    .bind(&detail)
    .fetch_one(&st.db)
    .await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "plan" }));
    Ok(Json(json!({ "id": id })))
}

/// Modifier un objectif (#261) : titre, date, heure, détail.
pub async fn update_milestone(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<MilestoneIn>,
) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM milestones WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    map_ctx(&st, &u, mid).await?.require(Role::Member)?;
    check_len("Titre", &b.title, 1, 80)?;
    parse_date(&b.date).ok_or_else(|| bad("Date invalide."))?;
    let time = parse_time(&b.time)?;
    let detail = b.detail.as_deref().unwrap_or("").trim().to_string();
    check_len("Détail", &detail, 0, 2000)?;
    sqlx::query("UPDATE milestones SET title = ?, date = ?, time = ?, detail = ? WHERE id = ?")
        .bind(b.title.trim())
        .bind(&b.date)
        .bind(&time)
        .bind(&detail)
        .bind(id)
        .execute(&st.db)
        .await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "plan" }));
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_milestone(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM milestones WHERE id = ?")
        .bind(id)
        .fetch_one(&st.db)
        .await?;
    map_ctx(&st, &u, mid).await?.require(Role::Member)?;
    sqlx::query("DELETE FROM milestones WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------- Générateur de projet ----------

#[derive(Deserialize)]
pub struct GenMember {
    user_id: i64,
    tag: String,
    hours: i64,
}

#[derive(Deserialize)]
pub struct GenIn {
    genre: String,
    members: Vec<GenMember>,
    ai: bool,
    release_date: String,
    board_id: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PlanTask {
    key: usize,
    title: String,
    tag: String,
    deps: Vec<usize>,
    planned_day: Option<String>,
    assignee_id: Option<i64>,
    ai: bool,
    lot: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PlanMilestone {
    title: String,
    date: String,
    kind: String,
}

type T = (&'static str, &'static str, &'static [usize], bool); // titre, étiquette, dépendances (index local au gabarit), délégable à une IA

// Une dépendance = une tâche sans laquelle celle-ci ne peut être ni faite ni testée, pas l'ordre du jeu (#160).
// Exemple : construire les niveaux 6 à 10 n'attend pas les niveaux 1 à 5 ; tester tous les niveaux, si.

fn genre_loop(genre: &str) -> Vec<T> {
    match genre {
        "cr" => vec![
            ("Équipes gendarmes / voleurs", "code", &[], false),
            ("Coffre-fort et minuteur de braquage", "code", &[], false),
            ("Prison et système d’arrestation", "code", &[0], false),
            ("Véhicules de police", "carte", &[], false),
            ("Route d’évasion", "carte", &[], false),
            (
                "Tester une manche complète",
                "test",
                &[0, 1, 2, 3, 4],
                false,
            ),
        ],
        "tycoon" => vec![
            ("Générateur de ressource", "code", &[], false),
            ("Transformation et vente", "code", &[0], false),
            ("Améliorations achetables", "code", &[1], false),
            ("Zone de production habillée", "carte", &[], true),
            ("Tester 15 min de progression", "test", &[2, 3], false),
        ],
        "boxfight" => vec![
            ("Manches et score", "code", &[], false),
            ("Arène et murs", "carte", &[], false),
            ("Choix des armes par manche", "code", &[0], false),
            ("Tester en 2v2", "test", &[1, 2], false),
        ],
        "deathrun" => vec![
            ("Checkpoints et réapparition", "code", &[], false),
            ("Niveaux 1 à 5", "carte", &[], false),
            ("Niveaux 6 à 10", "carte", &[], false),
            ("Chronomètre et classement", "code", &[], false),
            ("Tester tous les niveaux", "test", &[0, 1, 2, 3], false),
        ],
        "horreur" => vec![
            ("Scénario et énigmes", "conception", &[], false),
            ("Ambiance sonore et lumières", "carte", &[], false),
            ("Créature et poursuite", "code", &[], false),
            ("Tester la peur (5 joueurs)", "test", &[1, 2], false),
        ],
        "course" => vec![
            ("Circuit principal", "carte", &[], false),
            ("Véhicules et points de départ", "carte", &[0], false),
            ("Tours, chrono et arrivée", "code", &[], false),
            ("Tester 3 courses", "test", &[1, 2], false),
        ],
        _ => vec![
            ("Rotation des mini-jeux", "code", &[], false),
            ("Mini-jeu 1", "carte", &[], false),
            ("Mini-jeu 2", "carte", &[], false),
            ("Score global", "code", &[0], false),
            ("Tester une soirée complète", "test", &[1, 2, 3], false),
        ],
    }
}

fn skeleton(genre: &str) -> Vec<(&'static str, Vec<T>)> {
    vec![
        (
            "Lot 1 · Fondations",
            vec![
                ("Créer le projet UEFN et le dépôt", "code", &[], false),
                ("Blocking de l’île", "carte", &[], false),
                ("Écrire la boucle de jeu (1 page)", "conception", &[], false),
                ("Règles et conventions de nommage", "conception", &[], true),
            ],
        ),
        ("Lot 2 · Boucle de jeu", genre_loop(genre)),
        (
            "Lot 3 · Économie et récompenses",
            vec![
                ("Tableau des prix et des dépenses", "conception", &[], false),
                ("Boutique en jeu", "code", &[], false),
                ("Première offre en jeu", "code", &[1], false),
                ("Interface de la boutique", "conception", &[1], true),
            ],
        ),
        (
            "Lot 4 · Finitions et publication",
            vec![
                ("Tutoriel et textes", "conception", &[], true),
                ("Optimisation mémoire", "carte", &[], false),
                ("Vignette, titre et description", "conception", &[], true),
                ("Traductions FR / EN", "conception", &[0], true),
                ("Playtest à plusieurs", "test", &[], false),
                ("Checklist de soumission Epic", "test", &[2], false),
            ],
        ),
    ]
}

pub async fn preview(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<GenIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    let release = parse_date(&b.release_date).ok_or_else(|| bad("Date de sortie invalide."))?;
    let start = next_workday(today() + Duration::days(1));
    if release <= start + Duration::days(3) {
        return Err(bad("La date de sortie est trop proche pour planifier."));
    }
    // Liste plate des tâches avec dépendances globales.
    let mut tasks: Vec<PlanTask> = vec![];
    for (lot, list) in skeleton(&b.genre) {
        let base = tasks.len();
        for (i, (title, tag, deps, ai)) in list.iter().enumerate() {
            tasks.push(PlanTask {
                key: base + i,
                title: title.to_string(),
                tag: tag.to_string(),
                deps: deps.iter().map(|d| base + d).collect(),
                planned_day: None,
                assignee_id: None,
                ai: b.ai && *ai,
                lot: lot.to_string(),
            });
        }
    }
    // Idées votées du tableau blanc.
    if let Some(bid) = b.board_id {
        let (bmid, objects): (i64, String) =
            sqlx::query_as("SELECT map_id, objects FROM boards WHERE id = ?")
                .bind(bid)
                .fetch_one(&st.db)
                .await?;
        if bmid == mid {
            let objs: Value = serde_json::from_str(&objects).unwrap_or(json!({}));
            let mut voted: Vec<(i64, String)> = objs
                .as_object()
                .map(|m| {
                    m.values()
                        .filter(|o| o["kind"] == "sticky" && !o["task_id"].is_i64())
                        .filter_map(|o| {
                            let v: i64 = o["votes"]
                                .as_object()
                                .map(|x| x.values().filter_map(|n| n.as_i64()).sum())
                                .unwrap_or(0);
                            (v > 0).then(|| {
                                (
                                    v,
                                    o["text"].as_str().unwrap_or("").chars().take(140).collect(),
                                )
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            voted.sort_by(|a, b| b.0.cmp(&a.0));
            for (_, t) in voted.into_iter().take(10) {
                let k = tasks.len();
                tasks.push(PlanTask {
                    key: k,
                    title: t,
                    tag: "conception".into(),
                    deps: vec![],
                    planned_day: None,
                    assignee_id: None,
                    ai: false,
                    lot: "Idées votées du tableau".into(),
                });
            }
        }
    }
    // Placement jour par jour selon la capacité de l'équipe. Les spécialités servent à répartir la charge,
    // mais personne n'est assigné d'office : chaque membre prend ses tâches lui-même (#157).
    let deadline = release - Duration::days(4);
    let mut cap: HashMap<i64, f64> = HashMap::new();
    for m in &b.members {
        cap.insert(m.user_id, (m.hours.max(1) as f64) / 5.0);
    }
    let mut used: HashMap<(i64, Date), f64> = HashMap::new();
    let mut rr = 0usize;
    for i in 0..tasks.len() {
        let tag = tasks[i].tag.clone();
        let cands: Vec<&GenMember> = b.members.iter().filter(|m| m.tag == tag).collect();
        let who = if !cands.is_empty() {
            cands[i % cands.len()].user_id
        } else if !b.members.is_empty() {
            rr += 1;
            b.members[rr % b.members.len()].user_id
        } else {
            u.id
        };
        let mut day = start;
        for d in tasks[i].deps.clone() {
            if let Some(dd) = tasks[d].planned_day.as_deref().and_then(parse_date) {
                if dd > day {
                    day = dd;
                }
            }
        }
        let c = *cap.get(&who).unwrap_or(&2.0);
        if !tasks[i].ai {
            while used.get(&(who, day)).copied().unwrap_or(0.0) + 1.0 > c.max(1.0) {
                day = next_workday(day + Duration::days(1));
            }
            *used.entry((who, day)).or_default() += 1.0;
        }
        tasks[i].planned_day = Some(fmt(day));
    }
    let last = tasks
        .iter()
        .filter_map(|t| t.planned_day.as_deref().and_then(parse_date))
        .max()
        .unwrap_or(start);
    let human: usize = tasks.iter().filter(|t| !t.ai).count();
    let milestones = vec![
        PlanMilestone {
            title: "Gel du contenu".into(),
            date: fmt(release - Duration::days(7)),
            kind: "gel".into(),
        },
        PlanMilestone {
            title: "Playtest à plusieurs".into(),
            date: fmt(release - Duration::days(5)),
            kind: "test".into(),
        },
        PlanMilestone {
            title: "Soumission à Epic".into(),
            date: fmt(release - Duration::days(4)),
            kind: "soumission".into(),
        },
        PlanMilestone {
            title: "Sortie".into(),
            date: fmt(release),
            kind: "sortie".into(),
        },
    ];
    Ok(Json(json!({
        "tasks": tasks, "milestones": milestones,
        "stats": { "tasks": tasks.len(), "human": human, "ai": tasks.len() - human, "end": fmt(last), "late": last > deadline }
    })))
}

#[derive(Deserialize)]
pub struct ApplyIn {
    tasks: Vec<PlanTask>,
    milestones: Vec<PlanMilestone>,
}

pub async fn apply(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<ApplyIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    if b.tasks.len() > 300 {
        return Err(bad("Trop de tâches."));
    }
    let mut numbers: HashMap<usize, i64> = HashMap::new();
    for t in &b.tasks {
        let deps: Vec<i64> = t
            .deps
            .iter()
            .filter_map(|k| numbers.get(k).copied())
            .collect();
        let (_, n) = insert_task(
            &st,
            &ctx,
            &NewTask {
                title: t.title.clone(),
                tag: Some(t.tag.clone()),
                description: None,
                assignee_id: t.assignee_id,
                deps: Some(deps),
                planned_day: t.planned_day.clone(),
                lot: Some(t.lot.clone()),
                status: None,
                ai: Some(t.ai),
                origin: Some("plan".into()),
                uefn: None,
            },
        )
        .await?;
        numbers.insert(t.key, n);
    }
    for m in &b.milestones {
        if parse_date(&m.date).is_some() {
            sqlx::query("INSERT INTO milestones (map_id, title, date, kind) VALUES (?, ?, ?, ?)")
                .bind(mid)
                .bind(&m.title)
                .bind(&m.date)
                .bind(&m.kind)
                .execute(&st.db)
                .await?;
        }
    }
    log_activity(
        &st,
        mid,
        Some(u.id),
        "plan",
        "Plan de projet validé",
        &format!("{} tâches, {} objectifs", b.tasks.len(), b.milestones.len()),
    )
    .await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "tasks" }));
    Ok(Json(json!({ "created": b.tasks.len() })))
}

// ---------- Planning : chronologie, chemin critique, recalcul ----------

#[derive(sqlx::FromRow)]
struct PT {
    id: i64,
    number: i64,
    title: String,
    tag: String,
    status: String,
    assignee_id: Option<i64>,
    name: Option<String>,
    planned_day: Option<String>,
    minutes: i64,
}

async fn timeline_data(st: &AppState, mid: i64) -> Result<(Vec<PT>, Vec<(i64, i64)>), AppError> {
    let tasks: Vec<PT> = sqlx::query_as(
        "SELECT t.id, t.number, t.title, t.tag, t.status, t.assignee_id, u.name, t.planned_day, t.minutes FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id WHERE t.map_id = ? ORDER BY t.planned_day IS NULL, t.planned_day, t.number",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let deps: Vec<(i64, i64)> = sqlx::query_as("SELECT x.task_id, x.depends_on FROM task_deps x JOIN tasks t ON t.id = x.task_id WHERE t.map_id = ?").bind(mid).fetch_all(&st.db).await?;
    Ok((tasks, deps))
}

pub async fn timeline(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let (tasks, deps) = timeline_data(&st, mid).await?;
    // Chemin critique : plus longue chaîne de tâches non finies reliées par des dépendances.
    let open: HashMap<i64, &PT> = tasks
        .iter()
        .filter(|t| t.status != "done")
        .map(|t| (t.id, t))
        .collect();
    let mut memo: HashMap<i64, (i64, Option<i64>)> = HashMap::new();
    fn longest(
        id: i64,
        deps: &[(i64, i64)],
        open: &HashMap<i64, &PT>,
        memo: &mut HashMap<i64, (i64, Option<i64>)>,
        depth: u32,
    ) -> i64 {
        if let Some(v) = memo.get(&id) {
            return v.0;
        }
        if depth > 500 {
            return 1;
        }
        let mut best = (1, None);
        for (t, d) in deps.iter().filter(|(t, _)| *t == id) {
            let _ = t;
            if open.contains_key(d) {
                let l = longest(*d, deps, open, memo, depth + 1) + 1;
                if l > best.0 {
                    best = (l, Some(*d));
                }
            }
        }
        memo.insert(id, best);
        best.0
    }
    let mut end = None;
    let mut best = 0;
    for id in open.keys() {
        let l = longest(*id, &deps, &open, &mut memo, 0);
        if l > best {
            best = l;
            end = Some(*id);
        }
    }
    let mut critical = vec![];
    while let Some(id) = end {
        critical.push(id);
        end = memo.get(&id).and_then(|v| v.1);
    }
    let ms: Vec<(i64, String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, title, date, kind, time, detail FROM milestones WHERE map_id = ? ORDER BY date, time",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let release: Option<(Option<String>,)> =
        sqlx::query_as("SELECT release_date FROM maps WHERE id = ?")
            .bind(mid)
            .fetch_optional(&st.db)
            .await?;
    // Charge par personne sur la semaine en cours.
    let members: Vec<(i64, String, i64)> = sqlx::query_as("SELECT u.id, u.name, m.hours_per_week FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ? ORDER BY u.name").bind(ctx.studio_id).fetch_all(&st.db).await?;
    let t0 = today();
    let monday = t0 - Duration::days(t0.weekday().number_days_from_monday() as i64);
    let sunday = monday + Duration::days(6);
    let load: Vec<Value> = members.iter().map(|(id, name, hpw)| {
        let mins: i64 = tasks.iter().filter(|t| t.assignee_id == Some(*id) && t.status != "done").filter(|t| t.planned_day.as_deref().and_then(parse_date).map(|d| d >= monday && d <= sunday).unwrap_or(false)).map(|t| t.minutes).sum();
        json!({ "id": id, "name": name, "planned_h": mins / 60, "capacity_h": hpw, "overload": mins / 60 > *hpw })
    }).collect();
    Ok(Json(json!({
        "today": fmt(t0),
        "release_date": release.and_then(|r| r.0),
        "tasks": tasks.iter().map(|t| json!({
            "id": t.id, "number": t.number, "title": t.title, "tag": t.tag, "status": t.status, "assignee_id": t.assignee_id, "assignee": t.name,
            "planned_day": t.planned_day, "deps": deps.iter().filter(|(x, _)| *x == t.id).map(|(_, d)| d).collect::<Vec<_>>(), "critical": critical.contains(&t.id)
        })).collect::<Vec<_>>(),
        "milestones": ms.into_iter().map(|(id, t, d, k, h, det)| json!({"id": id, "title": t, "date": d, "kind": k, "time": h, "detail": det})).collect::<Vec<_>>(),
        "load": load,
    })))
}

#[derive(Deserialize)]
pub struct MoveIn {
    task_id: i64,
    planned_day: String,
}

/// Déplace une barre puis recalcule : les tâches qui en dépendent glissent, et on signale les objectifs dépassés.
pub async fn move_and_recalc(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<MoveIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    parse_date(&b.planned_day).ok_or_else(|| bad("Date invalide."))?;
    sqlx::query("UPDATE tasks SET planned_day = ? WHERE id = ? AND map_id = ?")
        .bind(&b.planned_day)
        .bind(b.task_id)
        .bind(mid)
        .execute(&st.db)
        .await?;
    let out = recalc_inner(&st, mid).await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "plan" }));
    Ok(Json(out))
}

pub async fn recalc(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?.can_edit()?;
    let out = recalc_inner(&st, mid).await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "plan" }));
    Ok(Json(out))
}

async fn recalc_inner(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let (tasks, deps) = timeline_data(st, mid).await?;
    let t0 = today();
    let mut day: HashMap<i64, Date> = tasks
        .iter()
        .filter_map(|t| {
            t.planned_day
                .as_deref()
                .and_then(parse_date)
                .map(|d| (t.id, d))
        })
        .collect();
    let status: HashMap<i64, &str> = tasks.iter().map(|t| (t.id, t.status.as_str())).collect();
    let mut shifted = vec![];
    // Les tâches non finies prévues dans le passé reviennent à aujourd'hui.
    for t in &tasks {
        if t.status != "done" {
            if let Some(d) = day.get(&t.id).copied() {
                if d < t0 {
                    day.insert(t.id, next_workday(t0));
                    shifted.push(t.number);
                }
            }
        }
    }
    // Propagation : une tâche commence au plus tôt le jour ouvré qui suit ses dépendances non finies.
    for _ in 0..50 {
        let mut changed = false;
        for (tid, did) in &deps {
            if status.get(tid) == Some(&"done") || status.get(did) == Some(&"done") {
                continue;
            }
            if let (Some(dd), Some(td)) = (day.get(did).copied(), day.get(tid).copied()) {
                let min = next_workday(dd + Duration::days(1));
                if td < min {
                    day.insert(*tid, min);
                    if let Some(t) = tasks.iter().find(|t| t.id == *tid) {
                        if !shifted.contains(&t.number) {
                            shifted.push(t.number);
                        }
                    }
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for t in &tasks {
        if let Some(d) = day.get(&t.id) {
            let s = fmt(*d);
            if t.planned_day.as_deref() != Some(s.as_str()) {
                sqlx::query("UPDATE tasks SET planned_day = ? WHERE id = ?")
                    .bind(&s)
                    .bind(t.id)
                    .execute(&st.db)
                    .await?;
            }
        }
    }
    let ms: Vec<(String, String)> = sqlx::query_as(
        "SELECT title, date FROM milestones WHERE map_id = ? AND date >= date('now') ORDER BY date",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let mut alerts = vec![];
    // Alerte quand une tâche décalée passe de l'autre côté d'un objectif.
    for t in &tasks {
        let (Some(old), Some(new)) = (
            t.planned_day.as_deref().and_then(parse_date),
            day.get(&t.id).copied(),
        ) else {
            continue;
        };
        if new <= old {
            continue;
        }
        for (title, date) in &ms {
            if let Some(d) = parse_date(date) {
                if old <= d && new > d {
                    alerts.push(format!(
                        "#{} {} glisse au {}, après « {title} » ({date}).",
                        t.number,
                        t.title,
                        fmt(new)
                    ));
                }
            }
        }
    }
    if !alerts.is_empty() {
        log_activity(st, mid, None, "alert", "Planning en retard", &alerts[0]).await?;
    }
    shifted.sort();
    Ok(json!({ "shifted": shifted, "alerts": alerts }))
}

/// Calendrier des mises à jour et des saisons : objectifs de la map + repères Fortnite à vérifier.
pub async fn seasons(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let ms: Vec<(String, String, String)> =
        sqlx::query_as("SELECT title, date, kind FROM milestones WHERE map_id = ? ORDER BY date")
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
    let mut ev: Vec<Value> = ms
        .into_iter()
        .map(|(t, d, k)| json!({"date": d, "title": t, "kind": "map", "sub": k}))
        .collect();
    // Repères génériques : à confirmer avec les annonces d'Epic.
    for (d, t, k) in [
        ("2026-11-26", "Thanksgiving (US) : pic de joueurs", "aim"),
        ("2026-12-19", "Vacances de Noël (France)", "aim"),
        (
            "2026-12-21",
            "Fêtes : modération Epic souvent plus lente (à confirmer)",
            "avoid",
        ),
        ("2027-02-01", "Ventes en jeu reversées à 50 %", "money"),
        (
            "2027-02-13",
            "Vacances d’hiver (France, zone variable)",
            "aim",
        ),
    ] {
        ev.push(json!({"date": d, "title": t, "kind": k}));
    }
    ev.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    Ok(Json(json!(ev)))
}

#[cfg(test)]
mod tests_assignation {
    use super::*;

    /// #157 : un plan généré pour une équipe de 2 ne pré-assigne personne.
    #[tokio::test]
    async fn plan_genere_sans_assignation() {
        let st = crate::test_state().await;
        let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let mut ids = vec![];
        for (e, r) in [("a@a.fr", "owner"), ("b@a.fr", "member")] {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
            )
            .bind(e)
            .bind(e)
            .fetch_one(&st.db)
            .await
            .unwrap();
            sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, ?)")
                .bind(sid)
                .bind(id)
                .bind(r)
                .execute(&st.db)
                .await
                .unwrap();
            ids.push(id);
        }
        let u = AuthUser {
            id: ids[0],
            email: "a@a.fr".into(),
            name: "A".into(),
            sid: String::new(),
        };
        let release = fmt(today() + Duration::days(60));
        let b: GenIn = serde_json::from_value(json!({
            "genre": "autre", "ai": true, "release_date": release, "board_id": null,
            "members": [{ "user_id": ids[0], "tag": "code", "hours": 20 }, { "user_id": ids[1], "tag": "carte", "hours": 10 }]
        }))
        .unwrap();
        let r = preview(State(st.clone()), u, Path(mid), Json(b))
            .await
            .unwrap()
            .0;
        let tasks = r["tasks"].as_array().unwrap();
        assert!(!tasks.is_empty());
        assert!(tasks.iter().all(|t| t["assignee_id"].is_null()));
        assert!(tasks.iter().all(|t| t["planned_day"].is_string()));
    }
}

#[cfg(test)]
mod tests_dependances {
    use super::*;

    /// #160 : dans chaque gabarit, une dépendance pointe vers une tâche plus haut, et celles qui
    /// n'étaient que l'ordre du jeu ont été retirées.
    #[test]
    fn squelettes_sans_ordre_du_jeu() {
        let ordre_du_jeu = [
            "Coffre-fort et minuteur de braquage",
            "Route d’évasion",
            "Niveaux 6 à 10",
            "Chronomètre et classement",
            "Créature et poursuite",
            "Règles et conventions de nommage",
            "Boutique en jeu",
            "Playtest à plusieurs",
        ];
        for g in [
            "cr", "tycoon", "boxfight", "deathrun", "horreur", "course", "autre",
        ] {
            for (lot, list) in skeleton(g) {
                for (i, (title, _, deps, _)) in list.iter().enumerate() {
                    assert!(deps.iter().all(|d| *d < i), "{lot} · {title}");
                    if ordre_du_jeu.contains(title) {
                        assert!(deps.is_empty(), "{title} dépend encore de {deps:?}");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests_dashboard {
    use super::*;

    /// #255 : une demande de test sans aucun retour ne doit pas faire planter le Dashboard
    /// (AVG sans ligne donnait l'entier 0, illisible en f64).
    #[tokio::test]
    async fn dashboard_avec_demande_de_test_sans_retour() {
        let st = crate::test_state().await;
        let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO users (email, name, password_hash) VALUES ('a@a.fr', 'Ana', 'x') RETURNING id",
        )
        .fetch_one(&st.db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO playtest_listings (map_id, owner_id, minutes, reward, slots, island_code) VALUES (?, ?, 15, 20, 4, '')")
            .bind(mid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        let u = AuthUser {
            id: uid,
            email: "a@a.fr".into(),
            name: "Ana".into(),
            sid: String::new(),
        };
        let Json(v) = overview(State(st.clone()), u, Path(mid)).await.unwrap();
        assert_eq!(v["tiles"]["playtest"]["players"], 0);
        assert_eq!(v["tiles"]["playtest"]["avg_minutes"], 0.0);
    }
}

#[cfg(test)]
mod tests_objectifs {
    use super::*;

    #[test]
    fn heure_des_objectifs() {
        assert_eq!(parse_time(&None).unwrap(), None);
        assert_eq!(parse_time(&Some(" ".into())).unwrap(), None);
        assert_eq!(
            parse_time(&Some("09:30".into())).unwrap(),
            Some("09:30".into())
        );
        for t in ["24:00", "9:30", "09:60", "ab:cd", "09h30"] {
            assert!(parse_time(&Some(t.into())).is_err(), "{t}");
        }
    }

    /// #261 : un objectif garde son heure et son détail, modifiables ensuite ; la fiche des objectifs les renvoie.
    #[tokio::test]
    async fn objectif_avec_heure_et_detail() {
        let st = crate::test_state().await;
        let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO users (email, name, password_hash) VALUES ('a@a.fr', 'Ana', 'x') RETURNING id",
        )
        .fetch_one(&st.db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        let u = || AuthUser {
            id: uid,
            email: "a@a.fr".into(),
            name: "Ana".into(),
            sid: String::new(),
        };
        let mk = |t: &str| MilestoneIn {
            title: "Démo".into(),
            date: "2026-12-01".into(),
            kind: None,
            time: Some(t.into()),
            detail: Some("Avec le studio".into()),
        };
        let Json(r) = add_milestone(State(st.clone()), u(), Path(mid), Json(mk("14:00")))
            .await
            .unwrap();
        let id = r["id"].as_i64().unwrap();
        assert!(
            add_milestone(State(st.clone()), u(), Path(mid), Json(mk("25:00")))
                .await
                .is_err()
        );
        update_milestone(State(st.clone()), u(), Path(id), Json(mk("16:30")))
            .await
            .unwrap();
        let (h, d): (Option<String>, String) =
            sqlx::query_as("SELECT time, detail FROM milestones WHERE id = ?")
                .bind(id)
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(h.as_deref(), Some("16:30"));
        assert_eq!(d, "Avec le studio");
    }
}
