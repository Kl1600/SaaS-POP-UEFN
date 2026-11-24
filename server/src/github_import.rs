//! Import complet d'un dépôt GitHub dans une map (#128) : toutes les issues (ouvertes et fermées, sans les PR),
//! leurs étiquettes, personnes, jalons, liens « Blocked by » et commentaires. En une fois, par un admin du studio ;
//! relancer l'import ne crée aucun doublon (`tasks.github_issue`, `milestones.github_milestone`,
//! `comments.github_comment`). La synchronisation continue existante (`integrations::github_sync`) ne change pas.
use crate::{
    access::{studio_role, Role},
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use futures::{stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

/// Ce qu'on lit sur GitHub.
#[derive(Default)]
pub struct Depot {
    pub issues: Vec<Value>,
    pub milestones: Vec<Value>,
    pub comments: Vec<Value>,
    /// numéro d'issue → numéros des issues qui la bloquent
    pub blocked_by: HashMap<i64, Vec<i64>>,
}

fn ext(e: impl std::fmt::Display) -> AppError {
    bad(format!("GitHub : {e}"))
}

async fn pages(st: &AppState, url: &str, token: &str, max: u32) -> Result<Vec<Value>, AppError> {
    let mut out = vec![];
    for page in 1..=max {
        let sep = if url.contains('?') { '&' } else { '?' };
        let r = crate::integrations::gh(
            st,
            reqwest::Method::GET,
            format!("{url}{sep}per_page=100&page={page}"),
            token,
        )
        .send()
        .await
        .map_err(ext)?;
        if !r.status().is_success() {
            return Err(ext(format!(
                "{} — vérifiez le dépôt et le jeton (droit Issues en lecture).",
                r.status()
            )));
        }
        let batch: Vec<Value> = r.json().await.map_err(ext)?;
        let n = batch.len();
        out.extend(batch);
        if n < 100 {
            break;
        }
    }
    Ok(out)
}

/// Lit le dépôt : issues (sans PR), jalons, commentaires et liens « Blocked by » (8 requêtes à la fois).
pub async fn lire(st: &AppState, repo: &str, token: &str) -> Result<Depot, AppError> {
    let base = format!("https://api.github.com/repos/{repo}");
    let issues: Vec<Value> = pages(st, &format!("{base}/issues?state=all"), token, 50)
        .await?
        .into_iter()
        .filter(|i| i.get("pull_request").is_none())
        .collect();
    let milestones = pages(st, &format!("{base}/milestones?state=all"), token, 10).await?;
    let comments = pages(st, &format!("{base}/issues/comments"), token, 100).await?;
    let nums: Vec<i64> = issues.iter().filter_map(|i| i["number"].as_i64()).collect();
    let deps: Vec<(i64, Vec<i64>)> = stream::iter(nums)
        .map(|n| {
            let url = format!("{base}/issues/{n}/dependencies/blocked_by");
            async move {
                let r = crate::integrations::gh(st, reqwest::Method::GET, url, token)
                    .send()
                    .await
                    .ok()?;
                if !r.status().is_success() {
                    return None;
                }
                let v: Vec<Value> = r.json().await.ok()?;
                Some((
                    n,
                    v.iter()
                        .filter_map(|x| x["number"].as_i64())
                        .collect::<Vec<_>>(),
                ))
            }
        })
        .buffer_unordered(8)
        .filter_map(|x| async move { x })
        .collect()
        .await;
    Ok(Depot {
        issues,
        milestones,
        comments,
        blocked_by: deps.into_iter().filter(|d| !d.1.is_empty()).collect(),
    })
}

/// « 2026-11-21T10:00:00Z » → « 2026-11-21 10:00:00 » (format des dates de POP).
fn date_pop(s: &str) -> Option<String> {
    (s.len() >= 19).then(|| format!("{} {}", &s[..10], &s[11..19]))
}

/// Étiquettes GitHub → (étiquette de tâche, UEFN, statut, lot, étiquettes non reprises).
pub fn etiquettes(
    labels: &[String],
) -> (
    Option<String>,
    bool,
    Option<&'static str>,
    Option<String>,
    Vec<String>,
) {
    let (mut tag, mut uefn, mut statut, mut lot, mut autres) = (None, false, None, None, vec![]);
    for l in labels {
        let b = l.trim().to_lowercase();
        let court = b.split(':').next_back().unwrap_or(&b).trim().to_string();
        match court.as_str() {
            "code" | "carte" | "conception" | "test" if tag.is_none() => tag = Some(court.clone()),
            "uefn" => uefn = true,
            "en cours" => statut = Some("doing"),
            "bloqué" | "bloque" => statut = Some("blocked"),
            _ if b.starts_with("lot") => lot = Some(l.trim().to_string()),
            _ => autres.push(l.clone()),
        }
    }
    (tag, uefn, statut, lot, autres)
}

/// Importe le dépôt lu dans la map. `gens` : compte GitHub → membre du studio (ou personne).
pub async fn importer(
    st: &AppState,
    mid: i64,
    par: i64,
    d: &Depot,
    gens: &HashMap<String, Option<i64>>,
) -> Result<Value, AppError> {
    let mut tx = st.db.begin().await?;
    // Jalons : un jalon POP par jalon GitHub (une seule fois).
    let mut jalons: HashMap<i64, i64> = HashMap::new();
    let (mut jalons_crees, mut sans_date) = (0, vec![]);
    for m in &d.milestones {
        let Some(n) = m["number"].as_i64() else {
            continue;
        };
        let titre = m["title"]
            .as_str()
            .unwrap_or("Jalon")
            .chars()
            .take(120)
            .collect::<String>();
        let deja: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM milestones WHERE map_id = ? AND github_milestone = ?",
        )
        .bind(mid)
        .bind(n)
        .fetch_optional(&mut *tx)
        .await?;
        let id = match deja {
            Some(id) => id,
            None => {
                let date = m["due_on"]
                    .as_str()
                    .map(|s| s[..10.min(s.len())].to_string())
                    .unwrap_or_else(|| {
                        sans_date.push(titre.clone());
                        m["created_at"]
                            .as_str()
                            .map(|s| s[..10.min(s.len())].to_string())
                            .unwrap_or_default()
                    });
                jalons_crees += 1;
                sqlx::query_scalar("INSERT INTO milestones (map_id, title, date, kind, github_milestone, detail) VALUES (?, ?, ?, 'jalon', ?, ?) RETURNING id")
                    .bind(mid)
                    .bind(&titre)
                    .bind(&date)
                    .bind(n)
                    .bind(m["description"].as_str().unwrap_or("").chars().take(2000).collect::<String>())
                    .fetch_one(&mut *tx)
                    .await?
            }
        };
        jalons.insert(n, id);
    }
    // Tâches : une par issue, numéro de l'issue s'il est libre.
    let mut taches: HashMap<i64, i64> = HashMap::new(); // numéro d'issue → id de tâche
    let existantes: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT github_issue, id FROM tasks WHERE map_id = ? AND github_issue IS NOT NULL",
    )
    .bind(mid)
    .fetch_all(&mut *tx)
    .await?;
    taches.extend(existantes.iter().copied());
    let mut issues: Vec<&Value> = d.issues.iter().collect();
    issues.sort_by_key(|i| i["number"].as_i64());
    let (mut crees, mut fermees, mut renumerotees) = (0, 0, vec![]);
    let mut non_repris: BTreeMap<String, usize> = BTreeMap::new();
    for i in issues {
        let Some(n) = i["number"].as_i64() else {
            continue;
        };
        if taches.contains_key(&n) {
            continue;
        }
        let labels: Vec<String> = i["labels"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|l| l["name"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let (tag, uefn, statut_l, lot, autres) = etiquettes(&labels);
        let ferme = i["state"] == "closed";
        let statut = if ferme {
            "done"
        } else {
            statut_l.unwrap_or("todo")
        };
        let mut notes = vec![];
        if !autres.is_empty() {
            notes.push(format!("Étiquettes GitHub : {}", autres.join(", ")));
            for a in &autres {
                *non_repris.entry(format!("étiquette « {a} »")).or_default() += 1;
            }
        }
        let logins: Vec<String> = i["assignees"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x["login"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let assigne = logins.iter().find_map(|l| gens.get(l).copied().flatten());
        let sans: Vec<&String> = logins
            .iter()
            .filter(|l| gens.get(*l).copied().flatten().is_none())
            .collect();
        if !sans.is_empty() {
            notes.push(format!(
                "Assigné sur GitHub à : {}",
                sans.iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        notes.push(format!(
            "Importée de GitHub : issue #{n}{}",
            i["user"]["login"]
                .as_str()
                .map(|u| format!(", ouverte par {u}"))
                .unwrap_or_default()
        ));
        let corps = i["body"].as_str().unwrap_or("");
        let description: String = format!("{corps}\n\n---\n{}", notes.join("\n"))
            .trim()
            .chars()
            .take(20000)
            .collect();
        let libre: Option<i64> =
            sqlx::query_scalar("SELECT number FROM tasks WHERE map_id = ? AND number = ?")
                .bind(mid)
                .bind(n)
                .fetch_optional(&mut *tx)
                .await?;
        let numero = if libre.is_none() {
            n
        } else {
            let k: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(number), 0) + 1 FROM tasks WHERE map_id = ?",
            )
            .bind(mid)
            .fetch_one(&mut *tx)
            .await?;
            renumerotees.push(json!({ "issue": n, "task": k }));
            k
        };
        let titre: String = i["title"]
            .as_str()
            .unwrap_or("Sans titre")
            .chars()
            .take(140)
            .collect();
        let jalon = i["milestone"]["number"]
            .as_i64()
            .and_then(|m| jalons.get(&m).copied());
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO tasks (map_id, number, title, description, tag, status, assignee_id, lot, origin, uefn, done_at, github_issue, milestone_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'github', ?, ?, ?, ?, COALESCE(?, datetime('now'))) RETURNING id",
        )
        .bind(mid)
        .bind(numero)
        .bind(&titre)
        .bind(&description)
        .bind(tag.as_deref().unwrap_or("code"))
        .bind(statut)
        .bind(assigne)
        .bind(lot.unwrap_or_default())
        .bind(uefn as i64)
        .bind(if ferme { i["closed_at"].as_str().and_then(date_pop) } else { None })
        .bind(n)
        .bind(jalon)
        .bind(i["created_at"].as_str().and_then(date_pop))
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO task_events (task_id, user_id, what, via) VALUES (?, ?, ?, 'GitHub')",
        )
        .bind(id)
        .bind(par)
        .bind(format!("Importée de l'issue GitHub #{n}"))
        .execute(&mut *tx)
        .await?;
        taches.insert(n, id);
        crees += 1;
        if ferme {
            fermees += 1;
        }
    }
    // Dépendances « Blocked by ».
    let mut deps = 0;
    for (n, bloquants) in &d.blocked_by {
        let Some(t) = taches.get(n) else { continue };
        for b in bloquants {
            match taches.get(b) {
                Some(o) if o != t => {
                    deps += sqlx::query(
                        "INSERT OR IGNORE INTO task_deps (task_id, depends_on) VALUES (?, ?)",
                    )
                    .bind(t)
                    .bind(o)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();
                }
                _ => {
                    *non_repris
                        .entry(format!("dépendance vers #{b} (hors du dépôt)"))
                        .or_default() += 1
                }
            }
        }
    }
    // Commentaires, dans l'ordre, avec l'auteur GitHub et la date d'origine.
    let mut commentaires = 0;
    let mut cs: Vec<&Value> = d.comments.iter().collect();
    cs.sort_by(|a, b| a["created_at"].as_str().cmp(&b["created_at"].as_str()));
    for c in cs {
        let (Some(cid), Some(url)) = (c["id"].as_i64(), c["issue_url"].as_str()) else {
            continue;
        };
        let Some(n) = url.rsplit('/').next().and_then(|x| x.parse::<i64>().ok()) else {
            continue;
        };
        let Some(t) = taches.get(&n) else { continue };
        let deja: Option<i64> =
            sqlx::query_scalar("SELECT id FROM comments WHERE map_id = ? AND github_comment = ?")
                .bind(mid)
                .bind(cid)
                .fetch_optional(&mut *tx)
                .await?;
        if deja.is_some() {
            continue;
        }
        let login = c["user"]["login"].as_str().unwrap_or("?");
        let quand = c["created_at"].as_str().and_then(date_pop);
        let corps = format!(
            "**{login}** sur GitHub, le {} :\n\n{}",
            quand.as_deref().unwrap_or("?"),
            c["body"].as_str().unwrap_or("")
        );
        sqlx::query("INSERT INTO comments (map_id, target, user_id, body, github_comment, at) VALUES (?, ?, ?, ?, ?, COALESCE(?, datetime('now')))")
            .bind(mid)
            .bind(format!("task:{t}"))
            .bind(gens.get(login).copied().flatten())
            .bind(corps.chars().take(20000).collect::<String>())
            .bind(cid)
            .bind(quand)
            .execute(&mut *tx)
            .await?;
        commentaires += 1;
    }
    tx.commit().await?;
    for s in sans_date {
        *non_repris
            .entry(format!(
                "date du jalon « {s} » (pas d'échéance : date de création reprise)"
            ))
            .or_default() += 1;
    }
    crate::access::notify(st, mid, json!({ "t": "refresh", "what": "tasks" }));
    Ok(json!({
        "tasks": crees, "closed": fermees, "open": crees - fermees, "dependencies": deps, "milestones": jalons_crees,
        "comments": commentaires, "renumbered": renumerotees,
        "not_imported": non_repris.into_iter().map(|(k, n)| json!({ "what": k, "count": n })).collect::<Vec<_>>(),
    }))
}

#[derive(Deserialize)]
pub struct ImportIn {
    map: i64,
    /// compte GitHub → membre du studio (null : personne)
    #[serde(default)]
    people: HashMap<String, Option<i64>>,
}

/// Dépôt et jeton de l'intégration GitHub du studio, après vérification du rôle et de la map.
async fn depot_du_studio(
    st: &AppState,
    u: &AuthUser,
    sid: i64,
    mid: i64,
) -> Result<(String, String), AppError> {
    if studio_role(st, u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    let ok: Option<i64> = sqlx::query_scalar("SELECT id FROM maps WHERE id = ? AND studio_id = ?")
        .bind(mid)
        .bind(sid)
        .fetch_optional(&st.db)
        .await?;
    ok.ok_or(AppError::NotFound)?;
    crate::integrations::github_repo_token(st, sid).await
}

/// `POST /api/studios/{sid}/github/import/preview` : comptes GitHub à associer et nombres, avant l'import.
pub async fn apercu(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<ImportIn>,
) -> ApiResult<Value> {
    let (repo, token) = depot_du_studio(&st, &u, sid, b.map).await?;
    let issues: Vec<Value> = pages(
        &st,
        &format!("https://api.github.com/repos/{repo}/issues?state=all"),
        &token,
        50,
    )
    .await?
    .into_iter()
    .filter(|i| i.get("pull_request").is_none())
    .collect();
    let mut logins: Vec<String> = issues
        .iter()
        .flat_map(|i| i["assignees"].as_array().cloned().unwrap_or_default())
        .filter_map(|a| a["login"].as_str().map(String::from))
        .collect();
    logins.sort();
    logins.dedup();
    let fermees = issues.iter().filter(|i| i["state"] == "closed").count();
    let membres: Vec<(i64, String)> = sqlx::query_as("SELECT u.id, u.name FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ? ORDER BY u.name")
        .bind(sid)
        .fetch_all(&st.db)
        .await?;
    Ok(Json(json!({
        "repo": repo, "issues": issues.len(), "closed": fermees, "logins": logins,
        "members": membres.into_iter().map(|(id, n)| json!({ "id": id, "name": n })).collect::<Vec<_>>(),
    })))
}

/// `POST /api/studios/{sid}/github/import` : import complet, résumé à la fin.
pub async fn route_importer(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<ImportIn>,
) -> ApiResult<Value> {
    let (repo, token) = depot_du_studio(&st, &u, sid, b.map).await?;
    // Une personne associée doit être membre du studio.
    for uid in b.people.values().flatten() {
        let m: Option<i64> = sqlx::query_scalar(
            "SELECT user_id FROM memberships WHERE studio_id = ? AND user_id = ?",
        )
        .bind(sid)
        .bind(uid)
        .fetch_optional(&st.db)
        .await?;
        if m.is_none() {
            return Err(bad(
                "Associez les comptes GitHub à des membres du studio seulement.",
            ));
        }
    }
    let d = lire(&st, &repo, &token).await?;
    let r = importer(&st, b.map, u.id, &d, &b.people).await?;
    crate::access::log_activity(
        &st,
        b.map,
        Some(u.id),
        "task",
        "Dépôt GitHub importé",
        &format!(
            "{repo} : {} tâches, {} commentaires",
            r["tasks"], r["comments"]
        ),
    )
    .await?;
    Ok(Json(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etiquettes_github() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (tag, uefn, statut, lot, autres) = etiquettes(&l(&[
            "domaine:uefn",
            "carte",
            "en cours",
            "lot 2",
            "priorité:P1",
        ]));
        assert_eq!(
            (tag.as_deref(), uefn, statut, lot.as_deref()),
            (Some("carte"), true, Some("doing"), Some("lot 2"))
        );
        assert_eq!(autres, vec!["priorité:P1".to_string()]);
    }

    fn depot() -> Depot {
        let issue = |n: i64, state: &str, labels: &[&str], ms: Option<i64>, who: &[&str]| {
            json!({
                "number": n, "title": format!("Issue {n}"), "body": "Texte", "state": state,
                "labels": labels.iter().map(|l| json!({ "name": l })).collect::<Vec<_>>(),
                "milestone": ms.map(|m| json!({ "number": m })), "assignees": who.iter().map(|w| json!({ "login": w })).collect::<Vec<_>>(),
                "created_at": "2026-11-01T10:00:00Z", "closed_at": if state == "closed" { json!("2026-11-10T12:00:00Z") } else { Value::Null },
                "user": { "login": "benybens" },
            })
        };
        Depot {
            issues: vec![
                issue(1, "closed", &["code"], Some(7), &["benybens"]),
                issue(2, "open", &["test", "uefn"], Some(7), &["inconnu"]),
                issue(3, "open", &["bloqué", "type:bug"], None, &[]),
            ],
            milestones: vec![
                json!({ "number": 7, "title": "M1 UEFN réel validé", "due_on": "2026-12-01T08:00:00Z", "created_at": "2026-11-01T00:00:00Z" }),
            ],
            comments: vec![
                json!({ "id": 900, "issue_url": "https://api.github.com/repos/a/b/issues/2", "user": { "login": "benybens" }, "created_at": "2026-11-03T09:00:00Z", "body": "Deuxième" }),
                json!({ "id": 899, "issue_url": "https://api.github.com/repos/a/b/issues/2", "user": { "login": "kl" }, "created_at": "2026-11-02T09:00:00Z", "body": "Premier" }),
            ],
            blocked_by: HashMap::from([(2, vec![1]), (3, vec![2, 99])]),
        }
    }

    #[tokio::test]
    async fn import_complet_sans_doublon() {
        let st = crate::test_state().await;
        let q = |s: &'static str| sqlx::query_scalar::<_, i64>(s);
        let sid = q("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let uid = q("INSERT INTO users (email, name, password_hash) VALUES ('b@a.fr', 'Ben', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        // Le numéro 3 est déjà pris dans la map : l'issue 3 prend le numéro suivant.
        sqlx::query("INSERT INTO tasks (map_id, number, title) VALUES (?, 3, 'Déjà là')")
            .bind(mid)
            .execute(&st.db)
            .await
            .unwrap();
        let gens = HashMap::from([("benybens".to_string(), Some(uid))]);
        let r = importer(&st, mid, uid, &depot(), &gens).await.unwrap();
        assert_eq!(
            (
                r["tasks"].clone(),
                r["closed"].clone(),
                r["dependencies"].clone(),
                r["milestones"].clone(),
                r["comments"].clone()
            ),
            (json!(3), json!(1), json!(2), json!(1), json!(2)),
            "{r}"
        );
        assert_eq!(r["renumbered"][0], json!({ "issue": 3, "task": 4 }));
        assert!(
            r["not_imported"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["what"] == "dépendance vers #99 (hors du dépôt)"),
            "{r}"
        );
        let t: Vec<(i64, String, String, i64, Option<i64>, Option<String>, Option<i64>)> = sqlx::query_as(
            "SELECT number, status, tag, uefn, assignee_id, done_at, milestone_id FROM tasks WHERE map_id = ? AND github_issue IS NOT NULL ORDER BY github_issue",
        ).bind(mid).fetch_all(&st.db).await.unwrap();
        assert_eq!(
            (
                t[0].0,
                t[0].1.as_str(),
                t[0].2.as_str(),
                t[0].4,
                t[0].5.as_deref()
            ),
            (1, "done", "code", Some(uid), Some("2026-11-10 12:00:00"))
        );
        assert_eq!(
            (t[1].1.as_str(), t[1].2.as_str(), t[1].3),
            ("todo", "test", 1),
            "étiquette uefn → tâche UEFN"
        );
        assert!(t[0].6.is_some() && t[0].6 == t[1].6, "jalon relié");
        assert_eq!(t[2].1, "blocked");
        let desc: String = sqlx::query_scalar(
            "SELECT description FROM tasks WHERE map_id = ? AND github_issue = 2",
        )
        .bind(mid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        assert!(
            desc.contains("inconnu"),
            "personne non associée notée : {desc}"
        );
        let desc: String = sqlx::query_scalar(
            "SELECT description FROM tasks WHERE map_id = ? AND github_issue = 3",
        )
        .bind(mid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        assert!(
            desc.contains("type:bug"),
            "étiquette inconnue notée : {desc}"
        );
        // « Blocked by » : la tâche de l'issue 2 attend celle de l'issue 1 (faite) ; celle de l'issue 3 attend la 2.
        let id_de = |n: i64| {
            let st = st.clone();
            async move {
                sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM tasks WHERE map_id = ? AND github_issue = ?",
                )
                .bind(mid)
                .bind(n)
                .fetch_one(&st.db)
                .await
                .unwrap()
            }
        };
        let t3 = id_de(3).await;
        assert!(
            crate::tasks::set_status_internal(&st, mid, t3, uid, "doing", "Bouton")
                .await
                .is_err(),
            "bloquée tant que #2 n'est pas faite"
        );
        // Commentaires dans l'ordre d'origine, avec auteur et date.
        let c: Vec<String> =
            sqlx::query_scalar("SELECT body FROM comments WHERE map_id = ? ORDER BY at, id")
                .bind(mid)
                .fetch_all(&st.db)
                .await
                .unwrap();
        assert!(
            c[0].contains("**kl**") && c[0].contains("2026-11-02") && c[0].ends_with("Premier"),
            "{c:?}"
        );
        // Relancer : rien en double.
        let r = importer(&st, mid, uid, &depot(), &gens).await.unwrap();
        assert_eq!(
            (
                r["tasks"].clone(),
                r["dependencies"].clone(),
                r["comments"].clone(),
                r["milestones"].clone()
            ),
            (json!(0), json!(0), json!(0), json!(0)),
            "{r}"
        );
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(n, 4);
    }
}
