//! Boussole : guide virtuel du jour. Pas d'IA : des règles simples appliquées aux tâches,
//! aux dépendances et aux jalons pour résumer la journée, signaler les retards et proposer quoi faire.
use crate::{access::map_ctx, auth::AuthUser, error::ApiResult, AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{Datelike, NaiveDate, Timelike};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(sqlx::FromRow, Clone)]
struct T {
    id: i64,
    number: i64,
    title: String,
    tag: String,
    status: String,
    assignee_id: Option<i64>,
    assignee: Option<String>,
    planned_day: Option<String>,
    since: Option<String>,
}

fn day(s: &Option<String>) -> Option<NaiveDate> {
    s.as_deref().and_then(|d| NaiveDate::parse_from_str(&d[..d.len().min(10)], "%Y-%m-%d").ok())
}
fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n > 1 { many } else { one })
}
fn jours(n: i64) -> String { plural(n, "jour", "jours") }
fn first_name(n: &str) -> String { n.split_whitespace().next().unwrap_or(n).to_string() }
fn fr_day(d: NaiveDate) -> String {
    const M: [&str; 12] = ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];
    format!("{} {}", d.day(), M[d.month0() as usize])
}

pub async fn guide(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let me = ctx.user.id;
    let now = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Paris);
    let today = now.date_naive();
    let tomorrow = today.succ_opt().unwrap_or(today);

    let tasks: Vec<T> = sqlx::query_as(
        "SELECT t.id, t.number, t.title, t.tag, t.status, t.assignee_id, u.name AS assignee, t.planned_day,
                (SELECT MAX(e.at) FROM task_events e WHERE e.task_id = t.id) AS since
         FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id WHERE t.map_id = ? AND t.status != 'done'",
    ).bind(mid).fetch_all(&st.db).await?;
    let deps: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT d.task_id, d.depends_on, o.status FROM task_deps d JOIN tasks o ON o.id = d.depends_on JOIN tasks t ON t.id = d.task_id WHERE t.map_id = ?",
    ).bind(mid).fetch_all(&st.db).await?;
    let (done_yesterday_me, done_yesterday, done_today_me, done_14): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(date(done_at) = date('now', '-1 day') AND assignee_id = ?), 0), COALESCE(SUM(date(done_at) = date('now', '-1 day')), 0),
                COALESCE(SUM(date(done_at) = date('now') AND assignee_id = ?), 0), COALESCE(SUM(done_at >= datetime('now', '-14 days')), 0)
         FROM tasks WHERE map_id = ? AND status = 'done'",
    ).bind(me).bind(me).bind(mid).fetch_one(&st.db).await?;
    let milestone: Option<(String, String)> = sqlx::query_as("SELECT title, date FROM milestones WHERE map_id = ? AND date >= date('now') ORDER BY date LIMIT 1").bind(mid).fetch_optional(&st.db).await?;

    // Dépendances : ce qui attend quoi, et combien de tâches chaque tâche débloque.
    let mut waiting: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut unlocks: HashMap<i64, i64> = HashMap::new();
    let num: HashMap<i64, i64> = tasks.iter().map(|t| (t.id, t.number)).collect();
    for (tid, on, s) in &deps {
        if s != "done" {
            waiting.entry(*tid).or_default().push(*num.get(on).unwrap_or(on));
            *unlocks.entry(*on).or_default() += 1;
        }
    }

    let mine = |t: &&T| t.assignee_id == Some(me);
    let my_today: Vec<&T> = tasks.iter().filter(mine).filter(|t| day(&t.planned_day) == Some(today)).collect();
    let my_late: Vec<&T> = tasks.iter().filter(mine).filter(|t| day(&t.planned_day).map(|d| d < today).unwrap_or(false)).collect();
    let my_doing: Vec<&T> = tasks.iter().filter(mine).filter(|t| t.status == "doing").collect();
    let team_late: Vec<&T> = tasks.iter().filter(|t| day(&t.planned_day).map(|d| d < today).unwrap_or(false)).collect();
    let blocked: Vec<&T> = tasks.iter().filter(|t| t.status == "blocked").collect();
    let unassigned = tasks.iter().filter(|t| t.assignee_id.is_none() && t.status == "todo").count() as i64;
    let stale_days = |t: &T| day(&t.since).map(|d| (today - d).num_days()).unwrap_or(0);

    // ---------- Résumé ----------
    let mut points: Vec<Value> = vec![];
    let mut say = |tone: &str, text: String| points.push(json!({ "tone": tone, "text": text }));
    if !my_late.is_empty() {
        let oldest = my_late.iter().filter_map(|t| day(&t.planned_day)).min().unwrap_or(today);
        say("bad", format!("Vous avez {} en retard (la plus ancienne était prévue le {}).", plural(my_late.len() as i64, "tâche", "tâches"), fr_day(oldest)));
    }
    match my_today.len() {
        0 if my_late.is_empty() => say("info", "Rien n’est prévu pour vous aujourd’hui : piochez dans les suggestions ci-dessous.".into()),
        0 => {}
        n => say("info", format!("{} prévue{} pour vous aujourd’hui, soit environ {} de travail.", plural(n as i64, "tâche", "tâches"), if n > 1 { "s" } else { "" }, if n == 1 { "1 heure".into() } else { format!("{n} heures") })),
    }
    for t in my_doing.iter().filter(|t| stale_days(t) >= 2) {
        say("warn", format!("#{} « {} » est en cours depuis {}. Terminez-la, ou passez-la en bloquée si quelque chose coince.", t.number, t.title, jours(stale_days(t))));
    }
    if done_today_me > 0 { say("good", format!("Déjà {} terminée{} aujourd’hui, bravo.", plural(done_today_me, "tâche", "tâches"), if done_today_me > 1 { "s" } else { "" })) }
    else if done_yesterday_me > 0 { say("good", format!("Hier, vous avez terminé {} (l’équipe : {}).", plural(done_yesterday_me, "tâche", "tâches"), done_yesterday)) }
    else if done_yesterday > 0 { say("good", format!("Hier, l’équipe a terminé {}.", plural(done_yesterday, "tâche", "tâches"))) }
    if let Some(b) = blocked.iter().min_by_key(|t| t.since.clone()) {
        say("warn", format!("{} bloquée{} dans l’équipe, dont #{} depuis {}{}.", plural(blocked.len() as i64, "tâche", "tâches"), if blocked.len() > 1 { "s" } else { "" }, b.number, jours(stale_days(b).max(0)), b.assignee.as_deref().map(|a| format!(" ({})", first_name(a))).unwrap_or_default()));
    }
    let others_late = team_late.len() - my_late.len();
    if others_late > 0 { say("warn", format!("L’équipe a {} en retard en plus des vôtres.", plural(others_late as i64, "autre tâche", "autres tâches"))) }
    // Jalon : au rythme des 14 derniers jours, est-ce qu'on y arrive ?
    let mut pace = Value::Null;
    if let Some((title, date)) = &milestone {
        if let Some(md) = day(&Some(date.clone())) {
            let left = (md - today).num_days().max(0);
            let remaining = tasks.iter().filter(|t| day(&t.planned_day).map(|d| d <= md).unwrap_or(false)).count() as i64;
            let per_day = done_14 as f64 / 14.0;
            let need = if per_day > 0.0 { (remaining as f64 / per_day).ceil() as i64 } else if remaining > 0 { i64::MAX } else { 0 };
            let (tone, text) = if remaining == 0 {
                ("good", format!("Jalon « {title} » dans {} : aucune tâche restante prévue avant.", jours(left)))
            } else if need == i64::MAX {
                ("warn", format!("Jalon « {title} » dans {} avec {} à finir avant, et aucune tâche finie ces 14 derniers jours.", jours(left), plural(remaining, "tâche", "tâches")))
            } else if need <= left {
                ("good", format!("Jalon « {title} » dans {} : {} à finir. Au rythme actuel ({:.1} par jour), c’est dans les temps.", jours(left), plural(remaining, "tâche", "tâches"), per_day).replace('.', ","))
            } else {
                ("bad", format!("Jalon « {title} » dans {} : {} à finir, il faudrait environ {} au rythme actuel. Retard prévu : {}.", jours(left), plural(remaining, "tâche", "tâches"), jours(need), jours(need - left)))
            };
            say(tone, text);
            pace = json!({ "title": title, "date": date, "days_left": left, "remaining": remaining, "per_day": (per_day * 10.0).round() / 10.0, "on_track": remaining == 0 || (need != i64::MAX && need <= left) });
        }
    }
    if unassigned > 0 { say("info", format!("{} sans personne assignée.", plural(unassigned, "tâche à faire reste", "tâches à faire restent"))) }

    // ---------- Suggestions ----------
    let md = milestone.as_ref().and_then(|m| day(&Some(m.1.clone())));
    let mut cands: Vec<(i64, Value)> = vec![];
    for t in &tasks {
        let free = t.assignee_id.is_none() || t.assignee_id == Some(me);
        if !free || t.status == "blocked" { continue }
        if t.status == "doing" && t.assignee_id != Some(me) { continue }
        if waiting.contains_key(&t.id) { continue }
        let pd = day(&t.planned_day);
        let mut score: i64 = 0;
        let mut why: Vec<String> = vec![];
        if t.status == "doing" { score += 100; why.push("déjà commencée : finissez-la d’abord".into()) }
        if let Some(d) = pd {
            if d < today { score += 60 + (today - d).num_days().min(10) * 3; why.push(format!("en retard de {}", jours((today - d).num_days()))) }
            else if d == today { score += 45; why.push("prévue aujourd’hui".into()) }
            else if d == tomorrow { score += 20; why.push("prévue demain".into()) }
            else { score -= (d - today).num_days().min(15) }
            if let Some(m) = md { if d <= m && d >= today { score += 15; why.push("compte pour le prochain jalon".into()) } }
        }
        let un = *unlocks.get(&t.id).unwrap_or(&0);
        if un > 0 { score += 18 * un; why.push(format!("débloque {}", plural(un, "autre tâche", "autres tâches"))) }
        if t.assignee_id == Some(me) { score += 10 } else { why.push("personne ne l’a prise".into()) }
        if why.is_empty() { why.push("prête à démarrer".into()) }
        cands.push((score, json!({
            "id": t.id, "number": t.number, "title": t.title, "tag": t.tag, "status": t.status, "mine": t.assignee_id == Some(me),
            "planned_day": t.planned_day, "why": why, "action": if t.status == "doing" { "continue" } else { "take" },
        })));
    }
    cands.sort_by(|a, b| b.0.cmp(&a.0));
    let suggestions: Vec<Value> = cands.into_iter().take(5).map(|c| c.1).collect();

    let hour = now.hour();
    let hello = if hour < 5 || hour >= 18 { "Bonsoir" } else { "Bonjour" };
    let name = first_name(&ctx.user.name);
    let headline = if !my_late.is_empty() { format!("{hello} {name}, on rattrape le retard ?") }
        else if let Some(s) = suggestions.first() { format!("{hello} {name}, je vous propose de commencer par #{}.", s["number"]) }
        else if tasks.is_empty() { format!("{hello} {name}, toutes les tâches sont faites. Ajoutez-en pour la suite !") }
        else { format!("{hello} {name}, rien d’urgent pour vous.") };
    Ok(Json(json!({
        "name": "Boussole", "headline": headline, "points": points, "suggestions": suggestions, "pace": pace,
        "counts": { "today": my_today.len(), "late": my_late.len(), "doing": my_doing.len(), "blocked": blocked.len(), "team_late": team_late.len() },
    })))
}
