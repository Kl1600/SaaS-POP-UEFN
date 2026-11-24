//! Popy : guide virtuel du jour. Pas d'IA : des règles simples appliquées aux tâches,
//! aux dépendances et aux objectifs pour résumer la journée, signaler les retards et proposer quoi faire.
use crate::{access::map_ctx, auth::AuthUser, error::ApiResult, AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};

/// Texte en français ou en anglais (`?lang=en`), avec les mêmes arguments.
macro_rules! tf {
    ($en:expr, $fr:literal, $e:literal $(, $a:expr)* $(,)?) => { if $en { format!($e $(, $a)*) } else { format!($fr $(, $a)*) } };
}
fn ls(en: bool, fr: &str, e: &str) -> String {
    if en {
        e.to_string()
    } else {
        fr.to_string()
    }
}

#[derive(serde::Deserialize, Default)]
pub struct LangQ {
    lang: Option<String>,
}
fn is_en(l: &Option<String>) -> bool {
    l.as_deref().map(|x| x.starts_with("en")).unwrap_or(false)
}
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
    s.as_deref()
        .and_then(|d| NaiveDate::parse_from_str(&d[..d.len().min(10)], "%Y-%m-%d").ok())
}
fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n > 1 { many } else { one })
}
fn jours(en: bool, n: i64) -> String {
    if en {
        plural(n, "day", "days")
    } else {
        plural(n, "jour", "jours")
    }
}
fn taches(en: bool, n: i64) -> String {
    if en {
        plural(n, "task", "tasks")
    } else {
        plural(n, "tâche", "tâches")
    }
}
fn s_fr(en: bool, n: i64) -> &'static str {
    if !en && n > 1 {
        "s"
    } else {
        ""
    }
}
fn first_name(n: &str) -> String {
    n.split_whitespace().next().unwrap_or(n).to_string()
}
fn fr_day(en: bool, d: NaiveDate) -> String {
    const M: [&str; 12] = [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
        "déc.",
    ];
    const E: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{} {}",
        d.day(),
        if en { E } else { M }[d.month0() as usize]
    )
}

pub async fn guide(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Query(lq): Query<LangQ>,
) -> ApiResult<Value> {
    let en = is_en(&lq.lang);
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
            waiting
                .entry(*tid)
                .or_default()
                .push(*num.get(on).unwrap_or(on));
            *unlocks.entry(*on).or_default() += 1;
        }
    }

    let mine = |t: &&T| t.assignee_id == Some(me);
    let my_today: Vec<&T> = tasks
        .iter()
        .filter(mine)
        .filter(|t| day(&t.planned_day) == Some(today))
        .collect();
    let my_late: Vec<&T> = tasks
        .iter()
        .filter(mine)
        .filter(|t| day(&t.planned_day).map(|d| d < today).unwrap_or(false))
        .collect();
    let my_doing: Vec<&T> = tasks
        .iter()
        .filter(mine)
        .filter(|t| t.status == "doing")
        .collect();
    let team_late: Vec<&T> = tasks
        .iter()
        .filter(|t| day(&t.planned_day).map(|d| d < today).unwrap_or(false))
        .collect();
    let blocked: Vec<&T> = tasks.iter().filter(|t| t.status == "blocked").collect();
    let unassigned = tasks
        .iter()
        .filter(|t| t.assignee_id.is_none() && t.status == "todo")
        .count() as i64;
    let stale_days = |t: &T| day(&t.since).map(|d| (today - d).num_days()).unwrap_or(0);

    // ---------- Résumé ----------
    let mut points: Vec<Value> = vec![];
    let mut say = |tone: &str, text: String| points.push(json!({ "tone": tone, "text": text }));
    if !my_late.is_empty() {
        let oldest = my_late
            .iter()
            .filter_map(|t| day(&t.planned_day))
            .min()
            .unwrap_or(today);
        say(
            "bad",
            tf!(
                en,
                "Vous avez {} en retard (la plus ancienne était prévue le {}).",
                "You have {} late (the oldest was due on {}).",
                taches(en, my_late.len() as i64),
                fr_day(en, oldest)
            ),
        );
    }
    match my_today.len() {
        0 if my_late.is_empty() => say(
            "info",
            ls(
                en,
                "Rien n’est prévu pour vous aujourd’hui : piochez dans les suggestions ci-dessous.",
                "Nothing is planned for you today: pick from the suggestions below.",
            ),
        ),
        0 => {}
        n => say(
            "info",
            tf!(
                en,
                "{} prévue{} pour vous aujourd’hui, soit environ {} de travail.",
                "{}{} planned for you today, about {} of work.",
                taches(en, n as i64),
                s_fr(en, n as i64),
                if en {
                    if n == 1 {
                        "1 hour".into()
                    } else {
                        format!("{n} hours")
                    }
                } else if n == 1 {
                    "1 heure".into()
                } else {
                    format!("{n} heures")
                }
            ),
        ),
    }
    for t in my_doing.iter().filter(|t| stale_days(t) >= 2) {
        say("warn", tf!(en, "#{} « {} » est en cours depuis {}. Terminez-la, ou passez-la en bloquée si quelque chose coince.", "#{} “{}” has been in progress for {}. Finish it, or mark it blocked if something is stuck.", t.number, t.title, jours(en, stale_days(t))));
    }
    if done_today_me > 0 {
        say(
            "good",
            tf!(
                en,
                "Déjà {} terminée{} aujourd’hui, bravo.",
                "Already {}{} done today, well done.",
                taches(en, done_today_me),
                s_fr(en, done_today_me)
            ),
        )
    } else if done_yesterday_me > 0 {
        say(
            "good",
            tf!(
                en,
                "Hier, vous avez terminé {} (l’équipe : {}).",
                "Yesterday you finished {} (team: {}).",
                taches(en, done_yesterday_me),
                done_yesterday
            ),
        )
    } else if done_yesterday > 0 {
        say(
            "good",
            tf!(
                en,
                "Hier, l’équipe a terminé {}.",
                "Yesterday the team finished {}.",
                taches(en, done_yesterday)
            ),
        )
    }
    if let Some(b) = blocked.iter().min_by_key(|t| t.since.clone()) {
        say(
            "warn",
            tf!(
                en,
                "{} bloquée{} dans l’équipe, dont #{} depuis {}{}.",
                "{}{} blocked in the team, including #{} for {}{}.",
                taches(en, blocked.len() as i64),
                s_fr(en, blocked.len() as i64),
                b.number,
                jours(en, stale_days(b).max(0)),
                b.assignee
                    .as_deref()
                    .map(|a| format!(" ({})", first_name(a)))
                    .unwrap_or_default()
            ),
        );
    }
    let others_late = team_late.len() - my_late.len();
    if others_late > 0 {
        say(
            "warn",
            tf!(
                en,
                "L’équipe a {} en retard en plus des vôtres.",
                "The team has {} late on top of yours.",
                if en {
                    plural(others_late as i64, "other task", "other tasks")
                } else {
                    plural(others_late as i64, "autre tâche", "autres tâches")
                }
            ),
        )
    }
    // Objectif : au rythme des 14 derniers jours, est-ce qu'on y arrive ?
    let mut pace = Value::Null;
    if let Some((title, date)) = &milestone {
        if let Some(md) = day(&Some(date.clone())) {
            let left = (md - today).num_days().max(0);
            let remaining = tasks
                .iter()
                .filter(|t| day(&t.planned_day).map(|d| d <= md).unwrap_or(false))
                .count() as i64;
            let per_day = done_14 as f64 / 14.0;
            let need = if per_day > 0.0 {
                (remaining as f64 / per_day).ceil() as i64
            } else if remaining > 0 {
                i64::MAX
            } else {
                0
            };
            let (tone, text) = if remaining == 0 {
                (
                    "good",
                    tf!(
                        en,
                        "Objectif « {title} » dans {} : aucune tâche restante prévue avant.",
                        "Milestone “{title}” in {}: no tasks left planned before it.",
                        jours(en, left)
                    ),
                )
            } else if need == i64::MAX {
                ("warn", tf!(en, "Objectif « {title} » dans {} avec {} à finir avant, et aucune tâche finie ces 14 derniers jours.", "Milestone “{title}” in {} with {} to finish before it, and no tasks finished in the last 14 days.", jours(en, left), taches(en, remaining)))
            } else if need <= left {
                (
                    "good",
                    if en {
                        format!("Milestone “{title}” in {}: {} to finish. At the current pace ({:.1} per day), you’re on track.", jours(en, left), taches(en, remaining), per_day)
                    } else {
                        format!("Objectif « {title} » dans {} : {} à finir. Au rythme actuel ({:.1} par jour), c’est dans les temps.", jours(en, left), taches(en, remaining), per_day).replace('.', ",")
                    },
                )
            } else {
                ("bad", tf!(en, "Objectif « {title} » dans {} : {} à finir, il faudrait environ {} au rythme actuel. Retard prévu : {}.", "Milestone “{title}” in {}: {} to finish, about {} needed at the current pace. Expected delay: {}.", jours(en, left), taches(en, remaining), jours(en, need), jours(en, need - left)))
            };
            say(tone, text);
            pace = json!({ "title": title, "date": date, "days_left": left, "remaining": remaining, "per_day": (per_day * 10.0).round() / 10.0, "on_track": remaining == 0 || (need != i64::MAX && need <= left) });
        }
    }
    if unassigned > 0 {
        say(
            "info",
            if en {
                format!(
                    "{} up for grabs: nobody has taken them yet.",
                    plural(unassigned, "task to do is", "tasks to do are")
                )
            } else {
                format!(
                    "{} à prendre : personne ne s’en occupe encore.",
                    plural(unassigned, "tâche à faire est", "tâches à faire sont")
                )
            },
        )
    }

    // ---------- Suggestions ----------
    let md = milestone.as_ref().and_then(|m| day(&Some(m.1.clone())));
    let mut cands: Vec<(i64, Value)> = vec![];
    for t in &tasks {
        let free = t.assignee_id.is_none() || t.assignee_id == Some(me);
        if !free || t.status == "blocked" {
            continue;
        }
        if t.status == "doing" && t.assignee_id != Some(me) {
            continue;
        }
        if waiting.contains_key(&t.id) {
            continue;
        }
        let pd = day(&t.planned_day);
        let mut score: i64 = 0;
        let mut why: Vec<String> = vec![];
        if t.status == "doing" {
            score += 100;
            why.push(ls(
                en,
                "déjà commencée : finissez-la d’abord",
                "already started: finish it first",
            ))
        }
        if let Some(d) = pd {
            if d < today {
                score += 60 + (today - d).num_days().min(10) * 3;
                why.push(tf!(
                    en,
                    "en retard de {}",
                    "{} late",
                    jours(en, (today - d).num_days())
                ))
            } else if d == today {
                score += 45;
                why.push(ls(en, "prévue aujourd’hui", "due today"))
            } else if d == tomorrow {
                score += 20;
                why.push(ls(en, "prévue demain", "due tomorrow"))
            } else {
                score -= (d - today).num_days().min(15)
            }
            if let Some(m) = md {
                if d <= m && d >= today {
                    score += 15;
                    why.push(ls(
                        en,
                        "compte pour le prochain objectif",
                        "counts for the next milestone",
                    ))
                }
            }
        }
        let un = *unlocks.get(&t.id).unwrap_or(&0);
        if un > 0 {
            score += 18 * un;
            why.push(if en {
                format!("unblocks {}", plural(un, "other task", "other tasks"))
            } else {
                format!("débloque {}", plural(un, "autre tâche", "autres tâches"))
            })
        }
        if t.assignee_id == Some(me) {
            score += 10
        } else {
            why.push(ls(en, "personne ne l’a prise", "nobody has taken it"))
        }
        if why.is_empty() {
            why.push(ls(en, "prête à démarrer", "ready to start"))
        }
        cands.push((score, json!({
            "id": t.id, "number": t.number, "title": t.title, "tag": t.tag, "status": t.status, "mine": t.assignee_id == Some(me),
            "planned_day": t.planned_day, "why": why, "action": if t.status == "doing" { "continue" } else { "take" },
        })));
    }
    cands.sort_by(|a, b| b.0.cmp(&a.0));
    let suggestions: Vec<Value> = cands.into_iter().take(5).map(|c| c.1).collect();

    let hour = now.hour();
    let hello = if en {
        if hour < 5 || hour >= 18 {
            "Good evening"
        } else {
            "Hi"
        }
    } else if hour < 5 || hour >= 18 {
        "Bonsoir"
    } else {
        "Bonjour"
    };
    let name = first_name(&ctx.user.name);
    let headline = if !my_late.is_empty() {
        tf!(
            en,
            "{hello} {name}, on rattrape le retard ?",
            "{hello} {name}, shall we catch up?"
        )
    } else if let Some(s) = suggestions.first() {
        tf!(
            en,
            "{hello} {name}, je vous propose de commencer par #{}.",
            "{hello} {name}, I suggest starting with #{}.",
            s["number"]
        )
    } else if tasks.is_empty() {
        tf!(
            en,
            "{hello} {name}, toutes les tâches sont faites. Ajoutez-en pour la suite !",
            "{hello} {name}, all tasks are done. Add some for what’s next!"
        )
    } else {
        tf!(
            en,
            "{hello} {name}, rien d’urgent pour vous.",
            "{hello} {name}, nothing urgent for you."
        )
    };
    Ok(Json(json!({
        "name": "Popy", "headline": headline, "points": points, "suggestions": suggestions, "pace": pace,
        "counts": { "today": my_today.len(), "late": my_late.len(), "doing": my_doing.len(), "blocked": blocked.len(), "team_late": team_late.len() },
    })))
}

// ---------- Questions sur les données de la map ----------

#[derive(serde::Deserialize)]
pub struct AskQ {
    q: String,
    lang: Option<String>,
}

fn status_fr(en: bool, s: &str) -> &'static str {
    if en {
        return match s {
            "todo" => "to do",
            "doing" => "in progress",
            "blocked" => "blocked",
            "done" => "done",
            _ => "?",
        };
    }
    match s {
        "todo" => "à faire",
        "doing" => "en cours",
        "blocked" => "bloquée",
        "done" => "faite",
        _ => "?",
    }
}

/// Répond avec les vraies données de la map (retards, personnes, tâche n°, objectif, chiffres de la semaine…).
/// Renvoie `{ "answer": null }` quand la question ne porte pas sur les données : l'interface utilise alors le guide de POP.
pub async fn ask(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<AskQ>,
) -> ApiResult<Value> {
    let en = is_en(&q.lang);
    let ctx = map_ctx(&st, &u, mid).await?;
    let raw = q.q.chars().take(300).collect::<String>();
    let f = crate::account::fold_str(&raw);
    let has = |ws: &[&str]| ws.iter().any(|w| f.contains(w));
    let today = chrono::Utc::now()
        .with_timezone(&chrono_tz::Europe::Paris)
        .date_naive();
    let line = |t: &T| {
        format!(
            "#{} {} · {}{}{}",
            t.number,
            t.title,
            status_fr(en, &t.status),
            t.assignee
                .as_deref()
                .map(|a| format!(" · {}", first_name(a)))
                .unwrap_or_default(),
            day(&t.planned_day)
                .map(|d| if d < today && t.status != "done" {
                    tf!(
                        en,
                        " · en retard depuis le {}",
                        " · late since {}",
                        fr_day(en, d)
                    )
                } else {
                    tf!(en, " · prévue le {}", " · due {}", fr_day(en, d))
                })
                .unwrap_or_default()
        )
    };
    let item = |t: &T| json!({ "label": line(t), "link": format!("/taches?t={}", t.id) });
    let all: Vec<T> = sqlx::query_as(
        "SELECT t.id, t.number, t.title, t.tag, t.status, t.assignee_id, u.name AS assignee, t.planned_day, (SELECT MAX(e.at) FROM task_events e WHERE e.task_id = t.id) AS since
         FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id WHERE t.map_id = ? ORDER BY t.number",
    ).bind(mid).fetch_all(&st.db).await?;
    let open: Vec<&T> = all.iter().filter(|t| t.status != "done").collect();
    let reply =
        |text: String, items: Vec<Value>| Ok(Json(json!({ "answer": text, "items": items })));

    // 1. Une tâche précise : « #12 », « tâche 12 ».
    let num_re =
        regex::Regex::new(r"(?:#|\b(?:tache|task)\s*(?:n|numero|no|number)?\s*)(\d{1,5})\b")
            .unwrap();
    if let Some(c) = num_re.captures(&f) {
        let n: i64 = c[1].parse().unwrap_or(0);
        let Some(t) = all.iter().find(|t| t.number == n) else {
            return reply(
                tf!(
                    en,
                    "Je ne trouve pas de tâche #{n} dans cette map.",
                    "I can’t find task #{n} in this map."
                ),
                vec![],
            );
        };
        let deps: Vec<(i64, String)> = sqlx::query_as("SELECT o.number, o.status FROM task_deps d JOIN tasks o ON o.id = d.depends_on WHERE d.task_id = ?").bind(t.id).fetch_all(&st.db).await?;
        let waiting: Vec<String> = deps
            .iter()
            .filter(|d| d.1 != "done")
            .map(|d| format!("#{}", d.0))
            .collect();
        let mut text = tf!(
            en,
            "#{} « {} » est {}{}.",
            "#{} “{}” is {}{}.",
            t.number,
            t.title,
            status_fr(en, &t.status),
            t.assignee
                .as_deref()
                .map(|a| tf!(en, ", assignée à {}", ", assigned to {}", first_name(a)))
                .unwrap_or_else(|| ls(en, ", sans personne assignée", ", with nobody assigned"))
        );
        if let Some(d) = day(&t.planned_day) {
            let fd = fr_day(en, d);
            text += &tf!(
                en,
                " Prévue le {fd}{}",
                " Due {fd}{}",
                if fd.ends_with('.') { "" } else { "." }
            )
        }
        if !waiting.is_empty() {
            text += &tf!(
                en,
                " Elle attend {}.",
                " It’s waiting for {}.",
                waiting.join(", ")
            )
        }
        return reply(text, vec![item(t)]);
    }

    // Une question « comment… / pourquoi… » porte sur l'utilisation de POP : le guide y répond mieux que les données.
    let howto = regex::Regex::new(r"^\s*(comment|pourquoi|c est quoi|qu est ce qu|ou (est|se trouve|sont|trouver)|est ce qu(e|on) (je|on) peu|puis je|peut on|peux tu m expliquer|a quoi sert|how|why|what is|where (is|are|do|can)|can i|can you explain)").unwrap().is_match(&f);
    if howto {
        return Ok(Json(json!({ "answer": null })));
    }
    // 2. Une personne de l'équipe : « sur quoi travaille Hugo », « tâches de Léa », « moi ».
    let people: Vec<(i64, String)> = sqlx::query_as("SELECT u.id, u.name FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ?").bind(ctx.studio_id).fetch_all(&st.db).await?;
    let words: Vec<&str> = f
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .collect();
    let me_q = regex::Regex::new(r"\b(je|j|mes|moi|ma|i|my|me|mine)\b")
        .unwrap()
        .is_match(&f)
        && has(&[
            "travaill", "tache", "fais", "faire", "bosse", "journee", "dois", "assign", "occupe",
            "task", "work", "doing", "do ", "day", "should",
        ]);
    let person = people
        .iter()
        .find(|(_, n)| {
            let nf = crate::account::fold_str(n);
            let first = nf.split_whitespace().next().unwrap_or("").to_string();
            first.len() >= 2 && words.iter().any(|w| *w == first)
                || words.windows(2).any(|w| format!("{} {}", w[0], w[1]) == nf)
        })
        .map(|(id, n)| (*id, n.clone()))
        .or(if me_q {
            Some((u.id, u.name.clone()))
        } else {
            None
        });
    if let Some((pid, pname)) = person {
        let theirs: Vec<&T> = open
            .iter()
            .copied()
            .filter(|t| t.assignee_id == Some(pid))
            .collect();
        let done_w: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ? AND assignee_id = ? AND status = 'done' AND done_at > datetime('now', '-7 days')").bind(mid).bind(pid).fetch_one(&st.db).await?;
        let who = if pid == u.id {
            ls(en, "Vous avez", "You have")
        } else {
            tf!(en, "{} a", "{} has", first_name(&pname))
        };
        let doing = theirs.iter().filter(|t| t.status == "doing").count();
        let late = theirs
            .iter()
            .filter(|t| day(&t.planned_day).map(|d| d < today).unwrap_or(false))
            .count();
        let mut text = if theirs.is_empty() {
            tf!(
                en,
                "{who} aucune tâche ouverte dans cette map.",
                "{who} no open tasks in this map."
            )
        } else {
            tf!(
                en,
                "{who} {} en cours et {} à faire ou bloquée{}.",
                "{who} {} in progress and {} to do or blocked{}.",
                doing,
                theirs.len() - doing,
                s_fr(en, (theirs.len() - doing) as i64)
            )
        };
        if late > 0 {
            text += &tf!(
                en,
                " Dont {} en retard.",
                " {} of them late.",
                taches(en, late as i64)
            )
        }
        text += &if en {
            format!(
                " {} this week.",
                if done_w == 0 {
                    "No tasks finished".to_string()
                } else {
                    format!("{} finished", taches(en, done_w))
                }
            )
        } else {
            format!(
                " {} cette semaine.",
                if done_w == 0 {
                    "Aucune tâche terminée".to_string()
                } else {
                    format!("{} terminée{}", taches(en, done_w), s_fr(en, done_w))
                }
            )
        };
        let mut list = theirs.clone();
        list.sort_by_key(|t| (t.status != "doing", t.planned_day.clone()));
        return reply(text, list.iter().take(8).map(|t| item(t)).collect());
    }

    // 3. Questions générales sur l'avancement.
    if has(&["retard", "en retard", "late", "overdue", "behind"])
        && !has(&[
            "jalon",
            "sortie",
            "beta",
            "deadline",
            "milestone",
            "release",
        ])
    {
        let late: Vec<&T> = open
            .iter()
            .copied()
            .filter(|t| day(&t.planned_day).map(|d| d < today).unwrap_or(false))
            .collect();
        if late.is_empty() {
            return reply(
                ls(
                    en,
                    "Aucune tâche en retard, bravo.",
                    "No late tasks, well done.",
                ),
                vec![],
            );
        }
        return reply(
            tf!(
                en,
                "{} en retard :",
                "{} late:",
                taches(en, late.len() as i64)
            ),
            late.iter().take(10).map(|t| item(t)).collect(),
        );
    }
    if has(&["bloque", "coince", "blocage", "blocked", "stuck"]) {
        let b: Vec<&T> = open
            .iter()
            .copied()
            .filter(|t| t.status == "blocked")
            .collect();
        if b.is_empty() {
            return reply(
                ls(
                    en,
                    "Aucune tâche bloquée en ce moment.",
                    "No blocked tasks right now.",
                ),
                vec![],
            );
        }
        return reply(
            tf!(
                en,
                "{} bloquée{} :",
                "{}{} blocked:",
                taches(en, b.len() as i64),
                s_fr(en, b.len() as i64)
            ),
            b.iter().take(10).map(|t| item(t)).collect(),
        );
    }
    if has(&[
        "qui travaille",
        "qui fait quoi",
        "qui bosse",
        "en cours",
        "equipe fait",
        "who is working",
        "who s working",
        "who works",
        "in progress",
        "team doing",
    ]) {
        let d: Vec<&T> = open
            .iter()
            .copied()
            .filter(|t| t.status == "doing")
            .collect();
        if d.is_empty() {
            return reply(
                ls(
                    en,
                    "Personne n’a de tâche en cours pour l’instant.",
                    "Nobody has a task in progress right now.",
                ),
                vec![],
            );
        }
        return reply(
            tf!(
                en,
                "{} en cours :",
                "{} in progress:",
                taches(en, d.len() as i64)
            ),
            d.iter().map(|t| item(t)).collect(),
        );
    }
    if has(&[
        "sans personne",
        "non assign",
        "personne n a pris",
        "libre",
        "dispo",
        "unassigned",
        "nobody",
        "free",
        "available",
    ]) && has(&["tache", "task"])
    {
        let free: Vec<&T> = open
            .iter()
            .copied()
            .filter(|t| t.assignee_id.is_none() && t.status == "todo")
            .collect();
        if free.is_empty() {
            return reply(
                ls(
                    en,
                    "Toutes les tâches à faire ont quelqu’un.",
                    "Every task to do has someone.",
                ),
                vec![],
            );
        }
        return reply(
            tf!(
                en,
                "{} sans personne assignée :",
                "{} with nobody assigned:",
                taches(en, free.len() as i64)
            ),
            free.iter().take(10).map(|t| item(t)).collect(),
        );
    }
    if has(&[
        "jalon",
        "sortie",
        "beta",
        "deadline",
        "echeance",
        "on sort quand",
        "date de sortie",
        "milestone",
        "release",
        "launch",
    ]) {
        let ms: Vec<(String, String)> = sqlx::query_as("SELECT title, date FROM milestones WHERE map_id = ? AND date >= date('now') ORDER BY date LIMIT 3").bind(mid).fetch_all(&st.db).await?;
        let Some((title, date)) = ms.first() else {
            return reply(
                ls(
                    en,
                    "Aucun objectif à venir. Ajoutez-en un dans Planifier → Objectifs.",
                    "No upcoming milestone. Add one in Plan → Milestones.",
                ),
                vec![
                    json!({ "label": ls(en, "Ouvrir Planifier", "Open Plan"), "link": "/planifier" }),
                ],
            );
        };
        let md = day(&Some(date.clone())).unwrap_or(today);
        let remaining: Vec<&T> = open
            .iter()
            .copied()
            .filter(|t| day(&t.planned_day).map(|d| d <= md).unwrap_or(false))
            .collect();
        let done14: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ? AND status = 'done' AND done_at > datetime('now', '-14 days')").bind(mid).fetch_one(&st.db).await?;
        let per = done14 as f64 / 14.0;
        let left = (md - today).num_days();
        let mut text = tf!(
            en,
            "Prochain objectif : « {title} » le {} (dans {}). {} à finir avant.",
            "Next milestone: “{title}” on {} (in {}). {} to finish before it.",
            fr_day(en, md),
            jours(en, left),
            taches(en, remaining.len() as i64)
        );
        if !remaining.is_empty() {
            if per > 0.0 {
                let need = (remaining.len() as f64 / per).ceil() as i64;
                text += &if need <= left {
                    if en {
                        format!(" At the current pace ({per:.1} per day), you’re on track.")
                    } else {
                        format!(
                            " Au rythme actuel ({} par jour), c’est dans les temps.",
                            format!("{per:.1}").replace('.', ",")
                        )
                    }
                } else {
                    tf!(
                        en,
                        " Au rythme actuel, il faudrait {} : retard prévu de {}.",
                        " At the current pace, it would take {}: expected delay of {}.",
                        jours(en, need),
                        jours(en, need - left)
                    )
                }
            } else {
                text += &ls(
                    en,
                    " Aucune tâche terminée ces 14 derniers jours : impossible d’estimer.",
                    " No tasks finished in the last 14 days: can’t estimate.",
                )
            }
        }
        return reply(text, remaining.iter().take(6).map(|t| item(t)).collect());
    }
    if has(&[
        "combien",
        "chiffre",
        "stat",
        "avancement",
        "ou en est",
        "on en est",
        "progression",
        "bilan",
        "semaine",
        "how many",
        "progress",
        "summary",
        "week",
        "numbers",
    ]) {
        let (todo, doing, blocked, done) =
            all.iter()
                .fold((0, 0, 0, 0), |a, t| match t.status.as_str() {
                    "todo" => (a.0 + 1, a.1, a.2, a.3),
                    "doing" => (a.0, a.1 + 1, a.2, a.3),
                    "blocked" => (a.0, a.1, a.2 + 1, a.3),
                    _ => (a.0, a.1, a.2, a.3 + 1),
                });
        let week: Vec<(String, i64)> = sqlx::query_as("SELECT COALESCE(u.name, '-'), COUNT(*) FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id WHERE t.map_id = ? AND t.status = 'done' AND t.done_at > datetime('now', '-7 days') GROUP BY 1 ORDER BY 2 DESC").bind(mid).fetch_all(&st.db).await?;
        let total = all.len().max(1);
        let mut text = tf!(
            en,
            "{} au total : {} faite{} ({} %), {} en cours, {} à faire, {} bloquée{}.",
            "{} in total: {} done{} ({}%), {} in progress, {} to do, {} blocked{}.",
            taches(en, all.len() as i64),
            done,
            s_fr(en, done as i64),
            done * 100 / total,
            doing,
            todo,
            blocked,
            s_fr(en, blocked as i64)
        );
        let wk: i64 = week.iter().map(|w| w.1).sum();
        text += &tf!(
            en,
            " Cette semaine : {} terminée{}",
            " This week: {} done{}",
            taches(en, wk),
            s_fr(en, wk)
        );
        text += &if week.is_empty() {
            ".".into()
        } else {
            format!(
                " ({}).",
                week.iter()
                    .map(|w| format!("{} {}", first_name(&w.0), w.1))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        return reply(text, vec![]);
    }
    if has(&[
        "relais",
        "uefn est",
        "uefn connecte",
        "uefn link",
        "pop-link",
        "relay",
        "uefn is",
        "uefn connected",
    ]) && has(&[
        "connecte",
        "marche",
        "en ligne",
        "statut",
        "etat",
        "est",
        "connected",
        "online",
        "working",
        "status",
        "is",
    ]) {
        let s = crate::uefn::statut(&st, mid).await?;
        let text = match s {
            "online" => ls(
                en,
                "Le relais UEFN est en ligne : POP parle à UEFN.",
                "The UEFN relay is online: POP is talking to UEFN.",
            ),
            "offline" => ls(
                en,
                "Le relais UEFN est hors ligne : lancez pop-link.exe sur le PC où UEFN est ouvert.",
                "The UEFN relay is offline: run pop-link.exe on the PC where UEFN is open.",
            ),
            "unlinked" => ls(
                en,
                "Aucun projet UEFN n’est relié à cette map : reliez-le avec un code d’appairage (Organiseur → UEFN Link).",
                "No UEFN project is linked to this map: link it with a pairing code (Organizer → UEFN Link).",
            ),
            other => tf!(en, "Relais UEFN : {other}.", "UEFN relay: {other}."),
        };
        return reply(
            text,
            vec![
                json!({ "label": ls(en, "Ouvrir l’Organiseur", "Open the Organizer"), "link": "/organiseur" }),
            ],
        );
    }
    Ok(Json(json!({ "answer": null })))
}
