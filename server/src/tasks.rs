use crate::{
    access::{log_activity, map_ctx, notify, MapCtx, Role},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;

pub const STATUSES: [&str; 4] = ["todo", "doing", "blocked", "done"];
pub const TAGS: [&str; 4] = ["code", "carte", "conception", "test"];

pub fn status_label(s: &str) -> &'static str {
    match s {
        "todo" => "À faire",
        "doing" => "En cours",
        "blocked" => "Bloqué",
        _ => "Fait",
    }
}

#[derive(FromRow, Serialize, Clone)]
pub struct TaskRow {
    pub id: i64,
    pub map_id: i64,
    pub number: i64,
    pub title: String,
    pub description: String,
    pub tag: String,
    pub status: String,
    pub assignee_id: Option<i64>,
    pub assignee_name: Option<String>,
    pub minutes: i64,
    pub planned_day: Option<String>,
    pub lot: String,
    pub ai: i64,
    pub origin: String,
    pub done_at: Option<String>,
}

const TASK_SELECT: &str = "SELECT t.id, t.map_id, t.number, t.title, t.description, t.tag, t.status, t.assignee_id, u.name AS assignee_name, t.minutes, t.planned_day, t.lot, t.ai, t.origin, t.done_at FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id";

async fn deps_of(st: &AppState, task_id: i64) -> Result<Vec<(i64, String)>, AppError> {
    Ok(sqlx::query_as("SELECT d.number, d.status FROM task_deps x JOIN tasks d ON d.id = x.depends_on WHERE x.task_id = ? ORDER BY d.number")
        .bind(task_id)
        .fetch_all(&st.db)
        .await?)
}

pub async fn task_ctx(st: &AppState, u: &AuthUser, task_id: i64) -> Result<(MapCtx, TaskRow), AppError> {
    let t: TaskRow = sqlx::query_as(&format!("{TASK_SELECT} WHERE t.id = ?")).bind(task_id).fetch_one(&st.db).await?;
    let ctx = map_ctx(st, u, t.map_id).await?;
    Ok((ctx, t))
}

pub async fn list(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let tasks: Vec<TaskRow> = sqlx::query_as(&format!("{TASK_SELECT} WHERE t.map_id = ? ORDER BY t.number")).bind(mid).fetch_all(&st.db).await?;
    let deps: Vec<(i64, i64)> = sqlx::query_as("SELECT t.number, d.number FROM task_deps x JOIN tasks t ON t.id = x.task_id JOIN tasks d ON d.id = x.depends_on WHERE t.map_id = ?")
        .bind(mid)
        .fetch_all(&st.db)
        .await?;
    let cm: Vec<(String, i64)> = sqlx::query_as("SELECT target, COUNT(*) FROM comments WHERE map_id = ? AND target LIKE 'task:%' GROUP BY target").bind(mid).fetch_all(&st.db).await?;
    let subs: Vec<(i64, i64, i64)> = sqlx::query_as("SELECT s.task_id, COUNT(*), COALESCE(SUM(s.done), 0) FROM subtasks s JOIN tasks t ON t.id = s.task_id WHERE t.map_id = ? GROUP BY s.task_id").bind(mid).fetch_all(&st.db).await?;
    let out: Vec<Value> = tasks
        .into_iter()
        .map(|t| {
            let d: Vec<i64> = deps.iter().filter(|(n, _)| *n == t.number).map(|(_, d)| *d).collect();
            let c = cm.iter().find(|(k, _)| *k == format!("task:{}", t.id)).map(|x| x.1).unwrap_or(0);
            let s = subs.iter().find(|x| x.0 == t.id).map(|x| (x.1, x.2)).unwrap_or((0, 0));
            let mut v = serde_json::to_value(&t).unwrap();
            v["deps"] = json!(d);
            v["comments"] = json!(c);
            v["subtasks"] = json!({ "total": s.0, "done": s.1 });
            v
        })
        .collect();
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
pub struct NewTask {
    pub title: String,
    pub tag: Option<String>,
    pub description: Option<String>,
    pub assignee_id: Option<i64>,
    pub deps: Option<Vec<i64>>,
    pub planned_day: Option<String>,
    pub lot: Option<String>,
    pub status: Option<String>,
    pub ai: Option<bool>,
    pub origin: Option<String>,
}

async fn check_assignee(st: &AppState, ctx: &MapCtx, a: Option<i64>) -> Result<(), AppError> {
    if let Some(uid) = a {
        let ok: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM memberships WHERE studio_id = ? AND user_id = ?").bind(ctx.studio_id).bind(uid).fetch_optional(&st.db).await?;
        ok.ok_or_else(|| bad("La personne assignée n’est pas membre du studio."))?;
    }
    Ok(())
}

async fn set_deps(st: &AppState, map_id: i64, task_id: i64, numbers: &[i64]) -> Result<(), AppError> {
    sqlx::query("DELETE FROM task_deps WHERE task_id = ?").bind(task_id).execute(&st.db).await?;
    for n in numbers {
        let d: Option<(i64,)> = sqlx::query_as("SELECT id FROM tasks WHERE map_id = ? AND number = ?").bind(map_id).bind(n).fetch_optional(&st.db).await?;
        let (did,) = d.ok_or_else(|| bad(format!("La tâche #{n} n’existe pas.")))?;
        if did == task_id {
            return Err(bad("Une tâche ne peut pas dépendre d’elle-même."));
        }
        // Refus des cycles : la dépendance ne doit pas déjà dépendre (même indirectement) de cette tâche.
        let cyc: i64 = sqlx::query_scalar(
            "WITH RECURSIVE up(id) AS (SELECT ? UNION SELECT x.depends_on FROM task_deps x JOIN up ON x.task_id = up.id) SELECT COUNT(*) FROM up WHERE id = ?",
        )
        .bind(did)
        .bind(task_id)
        .fetch_one(&st.db)
        .await?;
        if cyc > 0 {
            return Err(bad(format!("#{n} dépend déjà de cette tâche : boucle de dépendances.")));
        }
        sqlx::query("INSERT OR IGNORE INTO task_deps (task_id, depends_on) VALUES (?, ?)").bind(task_id).bind(did).execute(&st.db).await?;
    }
    Ok(())
}

pub async fn insert_task(st: &AppState, ctx: &MapCtx, b: &NewTask) -> Result<(i64, i64), AppError> {
    check_len("Titre", &b.title, 1, 140)?;
    let tag = b.tag.clone().unwrap_or_else(|| "code".into());
    if !TAGS.contains(&tag.as_str()) {
        return Err(bad("Étiquette inconnue."));
    }
    let status = b.status.clone().unwrap_or_else(|| "todo".into());
    if !STATUSES.contains(&status.as_str()) {
        return Err(bad("Statut inconnu."));
    }
    check_assignee(st, ctx, b.assignee_id).await?;
    let number: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(number), 0) + 1 FROM tasks WHERE map_id = ?").bind(ctx.map_id).fetch_one(&st.db).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO tasks (map_id, number, title, description, tag, status, assignee_id, planned_day, lot, ai, origin, done_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CASE WHEN ? = 'done' THEN datetime('now') END) RETURNING id",
    )
    .bind(ctx.map_id)
    .bind(number)
    .bind(b.title.trim())
    .bind(b.description.clone().unwrap_or_default())
    .bind(&tag)
    .bind(&status)
    .bind(b.assignee_id)
    .bind(&b.planned_day)
    .bind(b.lot.clone().unwrap_or_default())
    .bind(b.ai.unwrap_or(false) as i64)
    .bind(b.origin.clone().unwrap_or_default())
    .bind(&status)
    .fetch_one(&st.db)
    .await?;
    if let Some(d) = &b.deps {
        set_deps(st, ctx.map_id, id, d).await?;
    }
    sqlx::query("INSERT INTO task_events (task_id, user_id, what, via) VALUES (?, ?, ?, ?)")
        .bind(id)
        .bind(ctx.user.id)
        .bind("Tâche créée")
        .bind(if b.origin.as_deref().unwrap_or("").starts_with("board") { "Tableau blanc" } else { "Bouton" })
        .execute(&st.db)
        .await?;
    Ok((id, number))
}

pub async fn create(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<NewTask>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    let (id, number) = insert_task(&st, &ctx, &b).await?;
    log_activity(&st, mid, Some(u.id), "task", "Tâche créée", &format!("#{number} {}", b.title.trim())).await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "tasks" }));
    Ok(Json(json!({ "id": id, "number": number })))
}

pub async fn detail(State(st): State<AppState>, u: AuthUser, Path(tid): Path<i64>) -> ApiResult<Value> {
    let (_ctx, t) = task_ctx(&st, &u, tid).await?;
    let deps = deps_of(&st, tid).await?;
    let subtasks: Vec<(i64, String, i64)> = sqlx::query_as("SELECT id, title, done FROM subtasks WHERE task_id = ? ORDER BY id").bind(tid).fetch_all(&st.db).await?;
    let comments: Vec<(i64, Option<String>, String, i64, String)> = sqlx::query_as(
        "SELECT c.id, u.name, c.body, c.resolved, c.at FROM comments c LEFT JOIN users u ON u.id = c.user_id WHERE c.map_id = ? AND c.target = ? ORDER BY c.id",
    )
    .bind(t.map_id)
    .bind(format!("task:{tid}"))
    .fetch_all(&st.db)
    .await?;
    let events: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT e.what, e.via, u.name, e.at FROM task_events e LEFT JOIN users u ON u.id = e.user_id WHERE e.task_id = ? ORDER BY e.id DESC LIMIT 50",
    )
    .bind(tid)
    .fetch_all(&st.db)
    .await?;
    let locks: Vec<(String,)> = sqlx::query_as("SELECT path FROM file_locks WHERE map_id = ? AND task_number = ?").bind(t.map_id).bind(t.number).fetch_all(&st.db).await?;
    Ok(Json(json!({
        "task": t,
        "deps": deps.into_iter().map(|(n, s)| json!({"number": n, "status": s})).collect::<Vec<_>>(),
        "subtasks": subtasks.into_iter().map(|(id, title, done)| json!({"id": id, "title": title, "done": done == 1})).collect::<Vec<_>>(),
        "comments": comments.into_iter().map(|(id, who, body, res, at)| json!({"id": id, "who": who, "body": body, "resolved": res == 1, "at": at})).collect::<Vec<_>>(),
        "events": events.into_iter().map(|(what, via, who, at)| json!({"what": what, "via": via, "who": who, "at": at})).collect::<Vec<_>>(),
        "files": locks.into_iter().map(|(p,)| p).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct TaskPatch {
    title: Option<String>,
    description: Option<String>,
    tag: Option<String>,
    assignee_id: Option<Option<i64>>,
    planned_day: Option<Option<String>>,
    deps: Option<Vec<i64>>,
}

pub async fn update(State(st): State<AppState>, u: AuthUser, Path(tid): Path<i64>, Json(b): Json<TaskPatch>) -> ApiResult<Value> {
    let (ctx, t) = task_ctx(&st, &u, tid).await?;
    ctx.can_edit()?;
    if ctx.role == Role::Freelance && t.assignee_id != Some(u.id) {
        return Err(AppError::Forbidden);
    }
    if let Some(x) = &b.title {
        check_len("Titre", x, 1, 140)?;
        sqlx::query("UPDATE tasks SET title = ? WHERE id = ?").bind(x.trim()).bind(tid).execute(&st.db).await?;
    }
    if let Some(x) = &b.description {
        check_len("Description", x, 0, 20000)?;
        sqlx::query("UPDATE tasks SET description = ? WHERE id = ?").bind(x).bind(tid).execute(&st.db).await?;
    }
    if let Some(x) = &b.tag {
        if !TAGS.contains(&x.as_str()) {
            return Err(bad("Étiquette inconnue."));
        }
        sqlx::query("UPDATE tasks SET tag = ? WHERE id = ?").bind(x).bind(tid).execute(&st.db).await?;
    }
    if let Some(a) = b.assignee_id {
        check_assignee(&st, &ctx, a).await?;
        sqlx::query("UPDATE tasks SET assignee_id = ? WHERE id = ?").bind(a).bind(tid).execute(&st.db).await?;
        if let Some(a) = a.filter(|a| *a != u.id) {
            crate::account::notify_user(&st, a, t.map_id, &format!("{} vous a assigné #{} {}", u.name, t.number, t.title), &format!("/taches?t={tid}")).await;
        }
    }
    if let Some(d) = &b.planned_day {
        sqlx::query("UPDATE tasks SET planned_day = ? WHERE id = ?").bind(d).bind(tid).execute(&st.db).await?;
    }
    if let Some(d) = &b.deps {
        set_deps(&st, t.map_id, tid, d).await?;
    }
    notify(&st, t.map_id, json!({ "t": "refresh", "what": "tasks" }));
    Ok(Json(json!({ "ok": true })))
}

/// Change le statut d'une tâche en appliquant la règle : « En cours » seulement si les dépendances sont faites.
pub async fn set_status_internal(st: &AppState, map_id: i64, tid: i64, uid: i64, status: &str, via: &str) -> Result<(), AppError> {
    if !STATUSES.contains(&status) {
        return Err(bad("Statut inconnu."));
    }
    let (number, title, old): (i64, String, String) = sqlx::query_as("SELECT number, title, status FROM tasks WHERE id = ?").bind(tid).fetch_one(&st.db).await?;
    if old == status {
        return Ok(());
    }
    if status == "doing" {
        let pending: Vec<(i64, String)> = deps_of(st, tid).await?.into_iter().filter(|(_, s)| s != "done").collect();
        if !pending.is_empty() {
            let list = pending.iter().map(|(n, _)| format!("#{n}")).collect::<Vec<_>>().join(", ");
            return Err(AppError::Conflict(format!("#{number} ne peut pas passer « En cours » : {list} n’est pas encore fait.")));
        }
    }
    sqlx::query("UPDATE tasks SET status = ?, done_at = CASE WHEN ? = 'done' THEN datetime('now') ELSE NULL END WHERE id = ?")
        .bind(status)
        .bind(status)
        .bind(tid)
        .execute(&st.db)
        .await?;
    sqlx::query("INSERT INTO task_events (task_id, user_id, what, via) VALUES (?, ?, ?, ?)")
        .bind(tid)
        .bind(uid)
        .bind(format!("{} → {}", status_label(&old), status_label(status)))
        .bind(via)
        .execute(&st.db)
        .await?;
    let kind = match status {
        "done" => "done",
        "blocked" => "blocked",
        _ => "task",
    };
    log_activity(st, map_id, Some(uid), kind, &format!("#{number} {}", status_label(status)), &title).await?;
    if status == "done" {
        // Débloque automatiquement les tâches dont toutes les dépendances sont maintenant faites.
        let freed: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT t.id, t.number FROM tasks t JOIN task_deps x ON x.task_id = t.id WHERE x.depends_on = ? AND t.status = 'blocked'
             AND NOT EXISTS (SELECT 1 FROM task_deps y JOIN tasks d ON d.id = y.depends_on WHERE y.task_id = t.id AND d.status <> 'done')",
        )
        .bind(tid)
        .fetch_all(&st.db)
        .await?;
        for (fid, fnum) in freed {
            sqlx::query("UPDATE tasks SET status = 'todo' WHERE id = ?").bind(fid).execute(&st.db).await?;
            let who: Option<(Option<i64>, String)> = sqlx::query_as("SELECT assignee_id, title FROM tasks WHERE id = ?").bind(fid).fetch_optional(&st.db).await?;
            if let Some((Some(a), ftitle)) = who {
                crate::account::notify_user(st, a, map_id, &format!("#{fnum} {ftitle} est débloquée : vous pouvez commencer"), &format!("/taches?t={fid}")).await;
            }
            sqlx::query("INSERT INTO task_events (task_id, user_id, what, via) VALUES (?, NULL, ?, 'Automatique')")
                .bind(fid)
                .bind(format!("Débloquée : #{number} est fait"))
                .execute(&st.db)
                .await?;
            log_activity(st, map_id, None, "task", &format!("#{fnum} débloquée"), &format!("#{number} est fait")).await?;
        }
    }
    notify(st, map_id, json!({ "t": "refresh", "what": "tasks" }));
    Ok(())
}

#[derive(Deserialize)]
pub struct StatusIn {
    status: String,
}

pub async fn set_status(State(st): State<AppState>, u: AuthUser, Path(tid): Path<i64>, Json(b): Json<StatusIn>) -> ApiResult<Value> {
    let (ctx, t) = task_ctx(&st, &u, tid).await?;
    ctx.can_edit()?;
    if ctx.role == Role::Freelance && t.assignee_id != Some(u.id) {
        return Err(AppError::Forbidden);
    }
    set_status_internal(&st, t.map_id, tid, u.id, &b.status, "Bouton").await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete(State(st): State<AppState>, u: AuthUser, Path(tid): Path<i64>) -> ApiResult<Value> {
    let (ctx, t) = task_ctx(&st, &u, tid).await?;
    ctx.require(Role::Admin)?;
    sqlx::query("DELETE FROM tasks WHERE id = ?").bind(tid).execute(&st.db).await?;
    notify(&st, t.map_id, json!({ "t": "refresh", "what": "tasks" }));
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct TitleIn {
    title: String,
}

pub async fn add_subtask(State(st): State<AppState>, u: AuthUser, Path(tid): Path<i64>, Json(b): Json<TitleIn>) -> ApiResult<Value> {
    let (ctx, _) = task_ctx(&st, &u, tid).await?;
    ctx.can_edit()?;
    check_len("Sous-tâche", &b.title, 1, 140)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO subtasks (task_id, title) VALUES (?, ?) RETURNING id").bind(tid).bind(b.title.trim()).fetch_one(&st.db).await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
pub struct SubPatch {
    done: bool,
}

pub async fn toggle_subtask(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>, Json(b): Json<SubPatch>) -> ApiResult<Value> {
    let (tid,): (i64,) = sqlx::query_as("SELECT task_id FROM subtasks WHERE id = ?").bind(sid).fetch_one(&st.db).await?;
    let (ctx, _) = task_ctx(&st, &u, tid).await?;
    ctx.can_edit()?;
    sqlx::query("UPDATE subtasks SET done = ? WHERE id = ?").bind(b.done as i64).bind(sid).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct CommentIn {
    pub body: String,
}

pub async fn comment(State(st): State<AppState>, u: AuthUser, Path(tid): Path<i64>, Json(b): Json<CommentIn>) -> ApiResult<Value> {
    let (ctx, t) = task_ctx(&st, &u, tid).await?;
    ctx.require(Role::Freelance)?;
    check_len("Commentaire", &b.body, 1, 4000)?;
    sqlx::query("INSERT INTO comments (map_id, target, user_id, body) VALUES (?, ?, ?, ?)")
        .bind(t.map_id)
        .bind(format!("task:{tid}"))
        .bind(u.id)
        .bind(b.body.trim())
        .execute(&st.db)
        .await?;
    log_activity(&st, t.map_id, Some(u.id), "comment", &format!("Commentaire sur #{}", t.number), &b.body.chars().take(120).collect::<String>()).await?;
    let mut to = crate::account::mentioned(&st, t.map_id, &b.body).await;
    if let Some(a) = t.assignee_id { to.push(a) }
    to.sort(); to.dedup();
    for uid in to.into_iter().filter(|x| *x != u.id) {
        crate::account::notify_user(&st, uid, t.map_id, &format!("{} sur #{} : {}", u.name, t.number, b.body.chars().take(100).collect::<String>()), &format!("/taches?t={tid}")).await;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn locks(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(String, Option<String>, Option<i64>, String, f64)> = sqlx::query_as(
        "SELECT l.path, u.name, l.task_number, l.since, (julianday('now') - julianday(l.since)) * 24 FROM file_locks l LEFT JOIN users u ON u.id = l.user_id WHERE l.map_id = ? ORDER BY l.since",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows.into_iter().map(|(p, who, n, since, hours)| json!({"path": p, "who": who, "task": n, "since": since, "hours": hours, "stale": hours > 24.0})).collect::<Vec<_>>())))
}

/// Vue par personne et chaînes « qui bloque qui ».
pub async fn people(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let members: Vec<(i64, String, String, String, i64)> = sqlx::query_as(
        "SELECT u.id, u.name, m.role, m.specialty, m.hours_per_week FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ? ORDER BY u.name",
    )
    .bind(ctx.studio_id)
    .fetch_all(&st.db)
    .await?;
    let mut people = vec![];
    for (uid, name, role, spec, hpw) in members {
        let (wip, planned, done, late): (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(status = 'doing'), 0),
                    COALESCE(SUM(CASE WHEN planned_day BETWEEN date('now', 'weekday 1', '-7 days') AND date('now', 'weekday 0') THEN minutes END), 0),
                    COALESCE(SUM(CASE WHEN status = 'done' AND done_at >= date('now', 'weekday 1', '-7 days') THEN minutes END), 0),
                    COALESCE(SUM(status <> 'done' AND planned_day < date('now')), 0)
             FROM tasks WHERE map_id = ? AND assignee_id = ?",
        )
        .bind(mid)
        .bind(uid)
        .fetch_one(&st.db)
        .await?;
        people.push(json!({ "id": uid, "name": name, "role": role, "specialty": spec, "hours_per_week": hpw,
            "wip": wip, "planned_h": planned / 60, "done_h": done / 60, "late": late, "overload": planned / 60 > hpw }));
    }
    let chains: Vec<(i64, String, Option<String>, String, i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT d.number, d.title, du.name, d.status, t.number, t.title, tu.name, t.status
         FROM task_deps x JOIN tasks t ON t.id = x.task_id JOIN tasks d ON d.id = x.depends_on
         LEFT JOIN users du ON du.id = d.assignee_id LEFT JOIN users tu ON tu.id = t.assignee_id
         WHERE t.map_id = ? AND d.status <> 'done' AND t.status <> 'done' ORDER BY d.number",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "people": people,
        "chains": chains.into_iter().map(|c| json!({"from": c.0, "from_title": c.1, "from_who": c.2, "from_status": c.3, "to": c.4, "to_title": c.5, "to_who": c.6, "to_status": c.7})).collect::<Vec<_>>()
    })))
}
