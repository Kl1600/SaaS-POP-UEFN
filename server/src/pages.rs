use crate::{
    access::{log_activity, map_ctx, notify, MapCtx, Role},
    auth::{random_token, sha256_hex, AuthUser},
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

fn b(t: &str, text: &str) -> Value {
    json!({ "id": random_token()[..10].to_string(), "type": t, "text": text })
}

/// Modèles UEFN prêts à remplir.
pub fn template(key: &str) -> Option<(&'static str, &'static str, Vec<Value>)> {
    Some(match key {
        "gdd" => ("Document de conception", "D", vec![
            b("h1", "Pitch"), b("p", "En une phrase : qui joue, pour faire quoi, et pourquoi c’est fun."),
            b("h2", "Boucle de jeu"), b("bullet", "Action principale"), b("bullet", "Récompense"), b("bullet", "Progression"),
            b("h2", "Public visé"), b("p", ""), b("h2", "Références"), b("p", ""),
        ]),
        "rules" => ("Règles et conventions", "R", vec![
            b("h2", "Règles de la map"), b("p", "Objectif, conditions de victoire, équipes, durée d’une manche."),
            b("h2", "Tags Verse"), b("code", "# Exemple\nteam_police_tag := class(tag){}"),
            b("h2", "Noms des appareils"), b("bullet", "TRG_ pour les déclencheurs"), b("bullet", "BTN_ pour les boutons"), b("bullet", "SPN_ pour les points d’apparition"),
        ]),
        "economy" => ("Économie", "€", vec![
            b("h2", "Monnaies"), b("p", "Comment on gagne, comment on dépense."),
            b("callout", "Règle d’or : la première amélioration s’achète en moins de 4 minutes."),
            b("h2", "Prix"), json!({ "id": random_token()[..10].to_string(), "type": "table", "rows": [["Objet", "Prix", "Débloqué à"], ["", "", ""]] }),
        ]),
        "decisions" => ("Journal des décisions", "J", vec![
            json!({ "id": random_token()[..10].to_string(), "type": "table", "rows": [["Date", "Décision", "Pourquoi", "Alternatives écartées"], ["", "", "", ""]] }),
        ]),
        "patch" => ("Notes de patch", "N", vec![b("h2", "Nouveautés"), b("bullet", ""), b("h2", "Corrections"), b("bullet", ""), b("h2", "Problèmes connus"), b("bullet", "")]),
        "playtest" => ("Compte rendu de playtest", "P", vec![
            b("h2", "Contexte"), b("p", "Date, nombre de joueurs, version testée."),
            b("h2", "Ce qui marche"), b("bullet", ""), b("h2", "Ce qui bloque"), b("bullet", ""), b("h2", "Décisions"), b("todo", ""),
        ]),
        "postmortem" => ("Post-mortem", "M", vec![b("h2", "Objectifs"), b("p", ""), b("h2", "Ce qui a bien marché"), b("bullet", ""), b("h2", "Ce qu’on change"), b("bullet", ""), b("h2", "Chiffres"), b("p", "")]),
        "submission" => ("Checklist de soumission Epic", "✓", vec![
            b("todo", "Titre sans mention de V-Bucks ni d’XP"), b("todo", "Vignette fidèle au jeu"), b("todo", "Description du vrai gameplay"),
            b("todo", "Tags cohérents"), b("todo", "Classification d’âge remplie"), b("todo", "Notes de version"),
        ]),
        _ => return None,
    })
}

pub const TEMPLATE_KEYS: [&str; 8] = ["gdd", "rules", "economy", "decisions", "patch", "playtest", "postmortem", "submission"];

pub async fn templates() -> Json<Value> {
    Json(json!(TEMPLATE_KEYS.iter().map(|k| { let (t, i, _) = template(k).unwrap(); json!({"key": k, "title": t, "icon": i}) }).collect::<Vec<_>>()))
}

async fn page_ctx(st: &AppState, u: &AuthUser, pid: i64) -> Result<(MapCtx, Value), AppError> {
    let row: (i64, Option<i64>, String, String, String, String, i64, i64, Option<String>, String, Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT p.map_id, p.parent_id, p.title, p.icon, p.blocks, p.share, p.favorite, p.version, u.name, p.updated_at, p.created_by, p.share_token FROM pages p LEFT JOIN users u ON u.id = p.updated_by WHERE p.id = ?",
    )
    .bind(pid)
    .fetch_one(&st.db)
    .await?;
    let ctx = map_ctx(st, u, row.0).await?;
    if row.5 == "private" && row.10 != Some(u.id) {
        return Err(AppError::NotFound);
    }
    let blocks: Value = serde_json::from_str(&row.4).unwrap_or(json!([]));
    Ok((ctx, json!({
        "id": pid, "map_id": row.0, "parent_id": row.1, "title": row.2, "icon": row.3, "blocks": blocks, "share": row.5,
        "favorite": row.6 == 1, "version": row.7, "updated_by": row.8, "updated_at": row.9, "has_link": row.11.is_some()
    })))
}

pub async fn tree(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, Option<i64>, String, String, i64, String)> = sqlx::query_as(
        "SELECT id, parent_id, title, icon, favorite, share FROM pages WHERE map_id = ? AND (share <> 'private' OR created_by = ?) ORDER BY position, id",
    )
    .bind(mid)
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows.into_iter().map(|(id, parent, title, icon, fav, share)| json!({"id": id, "parent_id": parent, "title": title, "icon": icon, "favorite": fav == 1, "share": share})).collect::<Vec<_>>())))
}

pub async fn get(State(st): State<AppState>, u: AuthUser, Path(pid): Path<i64>) -> ApiResult<Value> {
    let (_, p) = page_ctx(&st, &u, pid).await?;
    Ok(Json(p))
}

#[derive(Deserialize)]
pub struct NewPage {
    title: Option<String>,
    parent_id: Option<i64>,
    template: Option<String>,
}

pub async fn create(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(bd): Json<NewPage>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    let (mut title, mut icon, mut blocks) = ("Sans titre".to_string(), String::new(), vec![b("p", "")]);
    if let Some(k) = &bd.template {
        let (t, i, bl) = template(k).ok_or_else(|| bad("Modèle inconnu."))?;
        title = t.into();
        icon = i.into();
        blocks = bl;
    }
    if let Some(t) = &bd.title {
        check_len("Titre", t, 1, 120)?;
        title = t.trim().into();
    }
    if let Some(p) = bd.parent_id {
        let ok: Option<(i64,)> = sqlx::query_as("SELECT id FROM pages WHERE id = ? AND map_id = ?").bind(p).bind(mid).fetch_optional(&st.db).await?;
        ok.ok_or_else(|| bad("Page parente introuvable."))?;
    }
    let content = json!(blocks).to_string();
    let id: i64 = sqlx::query_scalar("INSERT INTO pages (map_id, parent_id, title, icon, blocks, created_by, updated_by) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(mid).bind(bd.parent_id).bind(&title).bind(&icon).bind(&content).bind(u.id).bind(u.id)
        .fetch_one(&st.db).await?;
    snapshot(&st, id, 1, &title, &content, u.id).await?;
    log_activity(&st, mid, Some(u.id), "page", "Page créée", &title).await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "pages" }));
    Ok(Json(json!({ "id": id })))
}

async fn snapshot(st: &AppState, pid: i64, version: i64, title: &str, blocks: &str, uid: i64) -> Result<(), AppError> {
    // L'empreinte SHA-256 horodatée sert de preuve d'antériorité.
    let digest = sha256_hex(&format!("{pid}|{version}|{title}|{blocks}"));
    sqlx::query("INSERT INTO page_versions (page_id, version, title, blocks, user_id, sha256) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(pid).bind(version).bind(title).bind(blocks).bind(uid).bind(digest)
        .execute(&st.db).await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct PagePatch {
    title: Option<String>,
    icon: Option<String>,
    blocks: Option<Value>,
    share: Option<String>,
    favorite: Option<bool>,
    parent_id: Option<Option<i64>>,
    client: Option<String>,
}

pub async fn update(State(st): State<AppState>, u: AuthUser, Path(pid): Path<i64>, Json(bd): Json<PagePatch>) -> ApiResult<Value> {
    let (ctx, cur) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Freelance)?;
    let mut title = cur["title"].as_str().unwrap_or("").to_string();
    let mut content_changed = false;
    if let Some(t) = &bd.title {
        check_len("Titre", t, 0, 120)?;
        title = t.trim().to_string();
        content_changed = true;
    }
    if let Some(i) = &bd.icon {
        check_len("Icône", i, 0, 4)?;
        sqlx::query("UPDATE pages SET icon = ? WHERE id = ?").bind(i).bind(pid).execute(&st.db).await?;
    }
    let mut blocks = cur["blocks"].to_string();
    if let Some(bl) = &bd.blocks {
        if !bl.is_array() {
            return Err(bad("Blocs invalides."));
        }
        blocks = bl.to_string();
        if blocks.len() > 2_000_000 {
            return Err(bad("Page trop volumineuse."));
        }
        content_changed = true;
    }
    if let Some(s) = &bd.share {
        if !["private", "team", "link"].contains(&s.as_str()) {
            return Err(bad("Partage inconnu."));
        }
        if s == "private" && cur["share"] != "private" {
            sqlx::query("UPDATE pages SET created_by = ? WHERE id = ?").bind(u.id).bind(pid).execute(&st.db).await?;
        }
        let token = if s == "link" { Some(random_token()) } else { None };
        sqlx::query("UPDATE pages SET share = ?, share_token = ? WHERE id = ?").bind(s).bind(token.as_ref().map(|t| sha256_hex(t))).bind(pid).execute(&st.db).await?;
        if let Some(t) = token {
            return Ok(Json(json!({ "ok": true, "link": format!("/lien/{t}") })));
        }
    }
    if let Some(f) = bd.favorite {
        sqlx::query("UPDATE pages SET favorite = ? WHERE id = ?").bind(f as i64).bind(pid).execute(&st.db).await?;
    }
    if let Some(p) = bd.parent_id {
        if p == Some(pid) {
            return Err(bad("Une page ne peut pas être sa propre parente."));
        }
        sqlx::query("UPDATE pages SET parent_id = ? WHERE id = ?").bind(p).bind(pid).execute(&st.db).await?;
    }
    let mut version = cur["version"].as_i64().unwrap_or(1);
    if content_changed {
        version += 1;
        sqlx::query("UPDATE pages SET title = ?, blocks = ?, version = ?, updated_by = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(&title).bind(&blocks).bind(version).bind(u.id).bind(pid)
            .execute(&st.db).await?;
        // Une version conservée au plus toutes les 2 minutes par page.
        let recent: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM page_versions WHERE page_id = ? AND at > datetime('now', '-2 minutes')").bind(pid).fetch_one(&st.db).await?;
        if recent == 0 {
            snapshot(&st, pid, version, &title, &blocks, u.id).await?;
        }
    }
    notify(&st, ctx.map_id, json!({ "t": "page", "id": pid, "version": version, "by": u.id, "name": u.name, "client": bd.client }));
    Ok(Json(json!({ "ok": true, "version": version })))
}

pub async fn delete(State(st): State<AppState>, u: AuthUser, Path(pid): Path<i64>) -> ApiResult<Value> {
    let (ctx, _) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Member)?;
    sqlx::query("DELETE FROM pages WHERE id = ?").bind(pid).execute(&st.db).await?;
    notify(&st, ctx.map_id, json!({ "t": "refresh", "what": "pages" }));
    Ok(Json(json!({ "ok": true })))
}

pub async fn versions(State(st): State<AppState>, u: AuthUser, Path(pid): Path<i64>) -> ApiResult<Value> {
    page_ctx(&st, &u, pid).await?;
    let rows: Vec<(i64, i64, Option<String>, String, String)> = sqlx::query_as(
        "SELECT v.id, v.version, u.name, v.at, v.sha256 FROM page_versions v LEFT JOIN users u ON u.id = v.user_id WHERE v.page_id = ? ORDER BY v.id DESC LIMIT 100",
    )
    .bind(pid)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows.into_iter().map(|(id, v, who, at, sha)| json!({"id": id, "version": v, "who": who, "at": at, "sha256": sha})).collect::<Vec<_>>())))
}

pub async fn restore(State(st): State<AppState>, u: AuthUser, Path((pid, vid)): Path<(i64, i64)>) -> ApiResult<Value> {
    let (ctx, cur) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Member)?;
    let (title, blocks): (String, String) = sqlx::query_as("SELECT title, blocks FROM page_versions WHERE id = ? AND page_id = ?").bind(vid).bind(pid).fetch_one(&st.db).await?;
    let version = cur["version"].as_i64().unwrap_or(1) + 1;
    sqlx::query("UPDATE pages SET title = ?, blocks = ?, version = ?, updated_by = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(&title).bind(&blocks).bind(version).bind(u.id).bind(pid).execute(&st.db).await?;
    snapshot(&st, pid, version, &title, &blocks, u.id).await?;
    notify(&st, ctx.map_id, json!({ "t": "page", "id": pid, "version": version, "by": u.id, "name": u.name }));
    Ok(Json(json!({ "ok": true, "version": version })))
}

/// Historique daté exportable (preuve d'antériorité).
pub async fn proof(State(st): State<AppState>, u: AuthUser, Path(pid): Path<i64>) -> Result<impl IntoResponse, AppError> {
    let (_, cur) = page_ctx(&st, &u, pid).await?;
    let rows: Vec<(i64, Option<String>, String, String)> = sqlx::query_as(
        "SELECT v.version, u.name, v.at, v.sha256 FROM page_versions v LEFT JOIN users u ON u.id = v.user_id WHERE v.page_id = ? ORDER BY v.id",
    )
    .bind(pid)
    .fetch_all(&st.db)
    .await?;
    let mut out = format!("Historique daté de la page « {} »\nHorodatage serveur (UTC). Empreinte = SHA-256 du contenu de chaque version.\n\n", cur["title"].as_str().unwrap_or(""));
    for (v, who, at, sha) in rows {
        out.push_str(&format!("v{v}\t{at} UTC\t{}\t{sha}\n", who.unwrap_or_default()));
    }
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8"), (header::CONTENT_DISPOSITION, "attachment; filename=\"historique.txt\"")], out))
}

fn to_markdown(title: &str, blocks: &Value) -> String {
    let mut md = format!("# {title}\n\n");
    for bl in blocks.as_array().cloned().unwrap_or_default() {
        let t = bl["text"].as_str().unwrap_or("");
        match bl["type"].as_str().unwrap_or("p") {
            "h1" => md.push_str(&format!("# {t}\n\n")),
            "h2" => md.push_str(&format!("## {t}\n\n")),
            "h3" => md.push_str(&format!("### {t}\n\n")),
            "bullet" => md.push_str(&format!("- {t}\n")),
            "number" => md.push_str(&format!("1. {t}\n")),
            "todo" => md.push_str(&format!("- [{}] {t}\n", if bl["checked"].as_bool().unwrap_or(false) { "x" } else { " " })),
            "quote" => md.push_str(&format!("> {t}\n\n")),
            "callout" => md.push_str(&format!("> **Note :** {t}\n\n")),
            "code" => md.push_str(&format!("```{}\n{t}\n```\n\n", bl["lang"].as_str().unwrap_or("verse"))),
            "divider" => md.push_str("---\n\n"),
            "toggle" => md.push_str(&format!("<details><summary>{t}</summary>\n\n{}\n\n</details>\n\n", bl["body"].as_str().unwrap_or(""))),
            "table" => {
                if let Some(rows) = bl["rows"].as_array() {
                    for (i, r) in rows.iter().enumerate() {
                        let cells: Vec<String> = r.as_array().cloned().unwrap_or_default().iter().map(|c| c.as_str().unwrap_or("").replace('|', "\\|")).collect();
                        md.push_str(&format!("| {} |\n", cells.join(" | ")));
                        if i == 0 {
                            md.push_str(&format!("|{}|\n", vec![" --- "; cells.len()].join("|")));
                        }
                    }
                    md.push('\n');
                }
            }
            _ => md.push_str(&format!("{t}\n\n")),
        }
    }
    md
}

pub async fn export_md(State(st): State<AppState>, u: AuthUser, Path(pid): Path<i64>) -> Result<impl IntoResponse, AppError> {
    let (_, p) = page_ctx(&st, &u, pid).await?;
    let md = to_markdown(p["title"].as_str().unwrap_or(""), &p["blocks"]);
    Ok(([(header::CONTENT_TYPE, "text/markdown; charset=utf-8"), (header::CONTENT_DISPOSITION, "attachment; filename=\"page.md\"")], md))
}

/// Lecture publique d'une page partagée par lien.
pub async fn public_page(State(st): State<AppState>, Path(token): Path<String>) -> ApiResult<Value> {
    let row: (String, String, String) = sqlx::query_as("SELECT title, icon, blocks FROM pages WHERE share = 'link' AND share_token = ?")
        .bind(sha256_hex(&token))
        .fetch_one(&st.db)
        .await?;
    Ok(Json(json!({ "title": row.0, "icon": row.1, "blocks": serde_json::from_str::<Value>(&row.2).unwrap_or(json!([])) })))
}

pub async fn page_comments(State(st): State<AppState>, u: AuthUser, Path(pid): Path<i64>) -> ApiResult<Value> {
    let (ctx, _) = page_ctx(&st, &u, pid).await?;
    let rows: Vec<(i64, String, Option<String>, String, i64, String)> = sqlx::query_as(
        "SELECT c.id, c.target, u.name, c.body, c.resolved, c.at FROM comments c LEFT JOIN users u ON u.id = c.user_id WHERE c.map_id = ? AND c.target LIKE ? ORDER BY c.id",
    )
    .bind(ctx.map_id)
    .bind(format!("page:{pid}:%"))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows.into_iter().map(|(id, target, who, body, res, at)| json!({
        "id": id, "block": target.splitn(3, ':').nth(2).unwrap_or(""), "who": who, "body": body, "resolved": res == 1, "at": at
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct PageCommentIn {
    block: String,
    body: String,
}

pub async fn add_page_comment(State(st): State<AppState>, u: AuthUser, Path(pid): Path<i64>, Json(bd): Json<PageCommentIn>) -> ApiResult<Value> {
    let (ctx, p) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Viewer)?;
    check_len("Commentaire", &bd.body, 1, 4000)?;
    check_len("Bloc", &bd.block, 1, 40)?;
    sqlx::query("INSERT INTO comments (map_id, target, user_id, body) VALUES (?, ?, ?, ?)")
        .bind(ctx.map_id).bind(format!("page:{pid}:{}", bd.block)).bind(u.id).bind(bd.body.trim())
        .execute(&st.db).await?;
    log_activity(&st, ctx.map_id, Some(u.id), "comment", &format!("Commentaire sur « {} »", p["title"].as_str().unwrap_or("")), &bd.body.chars().take(120).collect::<String>()).await?;
    notify(&st, ctx.map_id, json!({ "t": "comments", "page": pid }));
    let mut to = crate::account::mentioned(&st, ctx.map_id, &bd.body).await;
    if let Some(c) = p["created_by"].as_i64() { to.push(c) }
    to.sort(); to.dedup();
    for uid in to.into_iter().filter(|x| *x != u.id) {
        crate::account::notify_user(&st, uid, ctx.map_id, &format!("{} a commenté « {} » : {}", u.name, p["title"].as_str().unwrap_or(""), bd.body.chars().take(100).collect::<String>()), &format!("/pages/{pid}")).await;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn resolve_comment(State(st): State<AppState>, u: AuthUser, Path(cid): Path<i64>) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM comments WHERE id = ?").bind(cid).fetch_one(&st.db).await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    sqlx::query("UPDATE comments SET resolved = 1 - resolved WHERE id = ?").bind(cid).execute(&st.db).await?;
    notify(&st, mid, json!({ "t": "comments" }));
    Ok(Json(json!({ "ok": true })))
}
