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

// ---------- Questions sur les données de la map ----------

#[derive(serde::Deserialize)]
pub struct AskQ { q: String }

fn status_fr(s: &str) -> &'static str {
    match s { "todo" => "à faire", "doing" => "en cours", "blocked" => "bloquée", "done" => "faite", _ => "?" }
}

/// Répond avec les vraies données de la map (retards, personnes, tâche n°, jalon, chiffres de la semaine…).
/// Renvoie `{ "answer": null }` quand la question ne porte pas sur les données : l'interface utilise alors le guide de POP.
pub async fn ask(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, axum::extract::Query(q): axum::extract::Query<AskQ>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let raw = q.q.chars().take(300).collect::<String>();
    let f = crate::account::fold_str(&raw);
    let has = |ws: &[&str]| ws.iter().any(|w| f.contains(w));
    let today = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Paris).date_naive();
    let line = |t: &T| format!("#{} {} · {}{}{}", t.number, t.title, status_fr(&t.status),
        t.assignee.as_deref().map(|a| format!(" · {}", first_name(a))).unwrap_or_default(),
        day(&t.planned_day).map(|d| if d < today && t.status != "done" { format!(" · en retard depuis le {}", fr_day(d)) } else { format!(" · prévue le {}", fr_day(d)) }).unwrap_or_default());
    let item = |t: &T| json!({ "label": line(t), "link": format!("/taches?t={}", t.id) });
    let all: Vec<T> = sqlx::query_as(
        "SELECT t.id, t.number, t.title, t.tag, t.status, t.assignee_id, u.name AS assignee, t.planned_day, (SELECT MAX(e.at) FROM task_events e WHERE e.task_id = t.id) AS since
         FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id WHERE t.map_id = ? ORDER BY t.number",
    ).bind(mid).fetch_all(&st.db).await?;
    let open: Vec<&T> = all.iter().filter(|t| t.status != "done").collect();
    let reply = |text: String, items: Vec<Value>| Ok(Json(json!({ "answer": text, "items": items })));

    // 1. Une tâche précise : « #12 », « tâche 12 ».
    let num_re = regex::Regex::new(r"(?:#|\btache\s*(?:n|numero|no)?\s*)(\d{1,5})\b").unwrap();
    if let Some(c) = num_re.captures(&f) {
        let n: i64 = c[1].parse().unwrap_or(0);
        let Some(t) = all.iter().find(|t| t.number == n) else { return reply(format!("Je ne trouve pas de tâche #{n} dans cette map."), vec![]) };
        let deps: Vec<(i64, String)> = sqlx::query_as("SELECT o.number, o.status FROM task_deps d JOIN tasks o ON o.id = d.depends_on WHERE d.task_id = ?").bind(t.id).fetch_all(&st.db).await?;
        let waiting: Vec<String> = deps.iter().filter(|d| d.1 != "done").map(|d| format!("#{}", d.0)).collect();
        let mut text = format!("#{} « {} » est {}{}.", t.number, t.title, status_fr(&t.status), t.assignee.as_deref().map(|a| format!(", assignée à {}", first_name(a))).unwrap_or_else(|| ", sans personne assignée".into()));
        if let Some(d) = day(&t.planned_day) { let fd = fr_day(d); text += &format!(" Prévue le {fd}{}", if fd.ends_with('.') { "" } else { "." }) }
        if !waiting.is_empty() { text += &format!(" Elle attend {}.", waiting.join(", ")) }
        return reply(text, vec![item(t)]);
    }

    // Une question « comment… / pourquoi… » porte sur l'utilisation de POP : le guide y répond mieux que les données.
    let howto = regex::Regex::new(r"^\s*(comment|pourquoi|c est quoi|qu est ce qu|ou (est|se trouve|sont|trouver)|est ce qu(e|on) (je|on) peu|puis je|peut on|peux tu m expliquer|a quoi sert|how)").unwrap().is_match(&f);
    if howto { return Ok(Json(json!({ "answer": null }))) }
    // 2. Une personne de l'équipe : « sur quoi travaille Hugo », « tâches de Léa », « moi ».
    let people: Vec<(i64, String)> = sqlx::query_as("SELECT u.id, u.name FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ?").bind(ctx.studio_id).fetch_all(&st.db).await?;
    let words: Vec<&str> = f.split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() >= 2).collect();
    let me_q = regex::Regex::new(r"\b(je|j|mes|moi|ma)\b").unwrap().is_match(&f) && has(&["travaill", "tache", "fais", "faire", "bosse", "journee", "dois", "assign", "occupe"]);
    let person = people.iter().find(|(_, n)| {
        let nf = crate::account::fold_str(n);
        let first = nf.split_whitespace().next().unwrap_or("").to_string();
        first.len() >= 2 && words.iter().any(|w| *w == first) || words.windows(2).any(|w| format!("{} {}", w[0], w[1]) == nf)
    }).map(|(id, n)| (*id, n.clone())).or(if me_q { Some((u.id, u.name.clone())) } else { None });
    if let Some((pid, pname)) = person {
        let theirs: Vec<&T> = open.iter().copied().filter(|t| t.assignee_id == Some(pid)).collect();
        let done_w: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ? AND assignee_id = ? AND status = 'done' AND done_at > datetime('now', '-7 days')").bind(mid).bind(pid).fetch_one(&st.db).await?;
        let who = if pid == u.id { "Vous avez".to_string() } else { format!("{} a", first_name(&pname)) };
        let doing = theirs.iter().filter(|t| t.status == "doing").count();
        let late = theirs.iter().filter(|t| day(&t.planned_day).map(|d| d < today).unwrap_or(false)).count();
        let mut text = if theirs.is_empty() { format!("{who} aucune tâche ouverte dans cette map.") }
            else { format!("{who} {} en cours et {} à faire ou bloquée{}.", doing, theirs.len() - doing, if theirs.len() - doing > 1 { "s" } else { "" }) };
        if late > 0 { text += &format!(" Dont {} en retard.", plural(late as i64, "tâche", "tâches")) }
        text += &format!(" {} cette semaine.", if done_w == 0 { "Aucune tâche terminée".to_string() } else { format!("{} terminée{}", plural(done_w, "tâche", "tâches"), if done_w > 1 { "s" } else { "" }) });
        let mut list = theirs.clone(); list.sort_by_key(|t| (t.status != "doing", t.planned_day.clone()));
        return reply(text, list.iter().take(8).map(|t| item(t)).collect());
    }

    // 3. Questions générales sur l'avancement.
    if has(&["retard", "en retard", "late"]) && !has(&["jalon", "sortie", "beta", "deadline"]) {
        let late: Vec<&T> = open.iter().copied().filter(|t| day(&t.planned_day).map(|d| d < today).unwrap_or(false)).collect();
        if late.is_empty() { return reply("Aucune tâche en retard, bravo.".into(), vec![]) }
        return reply(format!("{} en retard :", plural(late.len() as i64, "tâche", "tâches")), late.iter().take(10).map(|t| item(t)).collect());
    }
    if has(&["bloque", "coince", "blocage"]) {
        let b: Vec<&T> = open.iter().copied().filter(|t| t.status == "blocked").collect();
        if b.is_empty() { return reply("Aucune tâche bloquée en ce moment.".into(), vec![]) }
        return reply(format!("{} bloquée{} :", plural(b.len() as i64, "tâche", "tâches"), if b.len() > 1 { "s" } else { "" }), b.iter().take(10).map(|t| item(t)).collect());
    }
    if has(&["qui travaille", "qui fait quoi", "qui bosse", "en cours", "equipe fait"]) {
        let d: Vec<&T> = open.iter().copied().filter(|t| t.status == "doing").collect();
        if d.is_empty() { return reply("Personne n’a de tâche en cours pour l’instant.".into(), vec![]) }
        return reply(format!("{} en cours :", plural(d.len() as i64, "tâche", "tâches")), d.iter().map(|t| item(t)).collect());
    }
    if has(&["sans personne", "non assign", "personne n a pris", "libre", "dispo"]) && has(&["tache"]) {
        let free: Vec<&T> = open.iter().copied().filter(|t| t.assignee_id.is_none() && t.status == "todo").collect();
        if free.is_empty() { return reply("Toutes les tâches à faire ont quelqu’un.".into(), vec![]) }
        return reply(format!("{} sans personne assignée :", plural(free.len() as i64, "tâche", "tâches")), free.iter().take(10).map(|t| item(t)).collect());
    }
    if has(&["jalon", "sortie", "beta", "deadline", "echeance", "on sort quand", "date de sortie"]) {
        let ms: Vec<(String, String)> = sqlx::query_as("SELECT title, date FROM milestones WHERE map_id = ? AND date >= date('now') ORDER BY date LIMIT 3").bind(mid).fetch_all(&st.db).await?;
        let Some((title, date)) = ms.first() else { return reply("Aucun jalon à venir. Ajoutez-en un dans Planifier → Jalons.".into(), vec![json!({ "label": "Ouvrir Planifier", "link": "/planifier" })]) };
        let md = day(&Some(date.clone())).unwrap_or(today);
        let remaining: Vec<&T> = open.iter().copied().filter(|t| day(&t.planned_day).map(|d| d <= md).unwrap_or(false)).collect();
        let done14: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ? AND status = 'done' AND done_at > datetime('now', '-14 days')").bind(mid).fetch_one(&st.db).await?;
        let per = done14 as f64 / 14.0; let left = (md - today).num_days();
        let mut text = format!("Prochain jalon : « {title} » le {} (dans {}). {} à finir avant.", fr_day(md), jours(left), plural(remaining.len() as i64, "tâche", "tâches"));
        if !remaining.is_empty() {
            if per > 0.0 { let need = (remaining.len() as f64 / per).ceil() as i64; text += &if need <= left { format!(" Au rythme actuel ({} par jour), c’est dans les temps.", format!("{per:.1}").replace('.', ",")) } else { format!(" Au rythme actuel, il faudrait {} : retard prévu de {}.", jours(need), jours(need - left)) } }
            else { text += " Aucune tâche terminée ces 14 derniers jours : impossible d’estimer." }
        }
        return reply(text, remaining.iter().take(6).map(|t| item(t)).collect());
    }
    if has(&["combien", "chiffre", "stat", "avancement", "ou en est", "on en est", "progression", "bilan", "semaine"]) {
        let (todo, doing, blocked, done) = all.iter().fold((0, 0, 0, 0), |a, t| match t.status.as_str() { "todo" => (a.0 + 1, a.1, a.2, a.3), "doing" => (a.0, a.1 + 1, a.2, a.3), "blocked" => (a.0, a.1, a.2 + 1, a.3), _ => (a.0, a.1, a.2, a.3 + 1) });
        let week: Vec<(String, i64)> = sqlx::query_as("SELECT COALESCE(u.name, 'Sans personne'), COUNT(*) FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id WHERE t.map_id = ? AND t.status = 'done' AND t.done_at > datetime('now', '-7 days') GROUP BY 1 ORDER BY 2 DESC").bind(mid).fetch_all(&st.db).await?;
        let total = all.len().max(1);
        let mut text = format!("{} au total : {} faite{} ({} %), {} en cours, {} à faire, {} bloquée{}.", plural(all.len() as i64, "tâche", "tâches"), done, if done > 1 { "s" } else { "" }, done * 100 / total, doing, todo, blocked, if blocked > 1 { "s" } else { "" });
        let wk: i64 = week.iter().map(|w| w.1).sum();
        text += &format!(" Cette semaine : {} terminée{}", plural(wk, "tâche", "tâches"), if wk > 1 { "s" } else { "" });
        text += &if week.is_empty() { ".".into() } else { format!(" ({}).", week.iter().map(|w| format!("{} {}", first_name(&w.0), w.1)).collect::<Vec<_>>().join(", ")) };
        return reply(text, vec![]);
    }
    if has(&["relais", "uefn est", "uefn connecte", "uefn link", "pop-link"]) && has(&["connecte", "marche", "en ligne", "statut", "etat", "est"]) {
        let s = st.relays.status(mid);
        let text = match s { "online" => "Le relais UEFN est en ligne : POP parle à UEFN.".to_string(), "offline" => "Le relais UEFN est hors ligne : lancez pop-link.exe sur le PC où UEFN est ouvert.".to_string(), other => format!("Relais UEFN : {other}.") };
        return reply(text, vec![json!({ "label": "Ouvrir l’Organiseur", "link": "/organiseur" })]);
    }
    Ok(Json(json!({ "answer": null })))
}
