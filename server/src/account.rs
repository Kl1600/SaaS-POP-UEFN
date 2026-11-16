//! Compte : nom, mot de passe, mot de passe oublié, notifications, recherche globale.
use crate::{
    access::{map_ctx, studio_role, Role},
    auth::{hash_password, random_token, sha256_hex, AuthUser},
    error::{bad, check_len, ApiResult, AppError},
    mail, AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Crée une notification pour un membre et la pousse en direct.
pub async fn notify_user(st: &AppState, uid: i64, mid: i64, text: &str, link: &str) {
    if sqlx::query("INSERT INTO notifications (user_id, map_id, text, link) VALUES (?, ?, ?, ?)").bind(uid).bind(mid).bind(text.chars().take(300).collect::<String>()).bind(link).execute(&st.db).await.is_ok() {
        crate::access::notify(st, mid, json!({ "t": "notif", "uid": uid }));
    }
}

/// Mentions « @Prénom » ou « @Prénom Nom » dans un texte → membres du studio de la map.
pub async fn mentioned(st: &AppState, mid: i64, text: &str) -> Vec<i64> {
    let members: Vec<(i64, String)> = sqlx::query_as("SELECT u.id, u.name FROM memberships m JOIN users u ON u.id = m.user_id JOIN maps p ON p.studio_id = m.studio_id WHERE p.id = ?").bind(mid).fetch_all(&st.db).await.unwrap_or_default();
    let low = text.to_lowercase();
    members.into_iter().filter(|(_, n)| { let n = n.to_lowercase(); low.contains(&format!("@{n}")) || n.split_whitespace().next().map(|f| low.contains(&format!("@{f}"))).unwrap_or(false) }).map(|m| m.0).collect()
}

pub async fn notifications(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    let rows: Vec<(i64, Option<i64>, String, String, Option<String>, String)> = sqlx::query_as("SELECT id, map_id, text, link, read_at, at FROM notifications WHERE user_id = ? ORDER BY id DESC LIMIT 60").bind(u.id).fetch_all(&st.db).await?;
    let unread: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read_at IS NULL").bind(u.id).fetch_one(&st.db).await?;
    Ok(Json(json!({ "unread": unread, "items": rows.into_iter().map(|(id, mid, text, link, read, at)| json!({ "id": id, "map_id": mid, "text": text, "link": link, "read": read.is_some(), "at": at })).collect::<Vec<_>>() })))
}

pub async fn read_notifications(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    sqlx::query("UPDATE notifications SET read_at = datetime('now') WHERE user_id = ? AND read_at IS NULL").bind(u.id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ProfileIn {
    name: String,
}

pub async fn update_profile(State(st): State<AppState>, u: AuthUser, Json(b): Json<ProfileIn>) -> ApiResult<Value> {
    check_len("Nom", &b.name, 1, 80)?;
    sqlx::query("UPDATE users SET name = ? WHERE id = ?").bind(b.name.trim()).bind(u.id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PasswordIn {
    current: String,
    new: String,
}

pub async fn change_password(State(st): State<AppState>, u: AuthUser, Json(b): Json<PasswordIn>) -> ApiResult<Value> {
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    if !crate::auth::verify_password(&b.current, &hash) {
        return Err(bad("Mot de passe actuel incorrect."));
    }
    set_password(&st, u.id, &b.new).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn set_password(st: &AppState, uid: i64, pw: &str) -> Result<(), AppError> {
    if pw.chars().count() < 10 {
        return Err(bad("Le mot de passe doit faire au moins 10 caractères."));
    }
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?").bind(hash_password(pw)?).bind(uid).execute(&st.db).await?;
    // Déconnecte les autres appareils.
    sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(uid).execute(&st.db).await?;
    Ok(())
}

async fn new_reset(st: &AppState, uid: i64) -> Result<String, AppError> {
    let token = random_token();
    sqlx::query("INSERT INTO password_resets (token_hash, user_id, expires_at) VALUES (?, ?, datetime('now', '+2 hours'))").bind(sha256_hex(&token)).bind(uid).execute(&st.db).await?;
    Ok(format!("/reinitialiser/{token}"))
}

#[derive(Deserialize)]
pub struct ForgotIn {
    email: String,
}

/// Mot de passe oublié : e-mail avec lien (même réponse que l'adresse existe ou non).
pub async fn forgot(State(st): State<AppState>, Json(b): Json<ForgotIn>) -> ApiResult<Value> {
    crate::auth::rate_limit(&st, &format!("reset:{}", b.email))?;
    if !mail::configured() {
        return Ok(Json(json!({ "mail": false })));
    }
    let row: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE email = ?").bind(b.email.trim().to_lowercase()).fetch_optional(&st.db).await?;
    if let Some(uid) = row {
        let link = format!("{}{}", mail::public_url(), new_reset(&st, uid).await?);
        let to = b.email.trim().to_lowercase();
        tokio::spawn(async move {
            if let Err(e) = mail::send(&to, "POP · réinitialiser votre mot de passe", &format!("Bonjour,\n\nPour choisir un nouveau mot de passe, ouvrez ce lien (valable 2 heures) :\n{link}\n\nSi vous n'avez rien demandé, ignorez ce message.")).await {
                tracing::warn!("e-mail de réinitialisation : {e:#}");
            }
        });
    }
    Ok(Json(json!({ "mail": true })))
}

/// Un admin génère un lien de réinitialisation pour un membre (quand l'e-mail n'est pas configuré).
pub async fn admin_reset_link(State(st): State<AppState>, u: AuthUser, Path((sid, uid)): Path<(i64, i64)>) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    let target: Option<String> = sqlx::query_scalar("SELECT role FROM memberships WHERE studio_id = ? AND user_id = ?").bind(sid).bind(uid).fetch_optional(&st.db).await?;
    match target.as_deref() {
        None => return Err(AppError::NotFound),
        Some("owner") if studio_role(&st, &u, sid).await? != Role::Owner => return Err(AppError::Forbidden),
        _ => {}
    }
    Ok(Json(json!({ "link": new_reset(&st, uid).await? })))
}

#[derive(Deserialize)]
pub struct ResetIn {
    token: String,
    password: String,
}

pub async fn reset(State(st): State<AppState>, Json(b): Json<ResetIn>) -> ApiResult<Value> {
    let row: Option<i64> = sqlx::query_scalar("SELECT user_id FROM password_resets WHERE token_hash = ? AND used_at IS NULL AND expires_at > datetime('now')").bind(sha256_hex(&b.token)).fetch_optional(&st.db).await?;
    let uid = row.ok_or_else(|| bad("Lien expiré ou déjà utilisé."))?;
    set_password(&st, uid, &b.password).await?;
    sqlx::query("UPDATE password_resets SET used_at = datetime('now') WHERE token_hash = ?").bind(sha256_hex(&b.token)).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct SearchQ {
    q: String,
}

fn snippet(text: &str, q: &str) -> String {
    let low = text.to_lowercase();
    let Some(i) = low.find(q) else { return text.chars().take(90).collect() };
    let start = low[..i].char_indices().rev().nth(30).map(|x| x.0).unwrap_or(0);
    let s: String = text[start..].chars().take(110).collect();
    if start > 0 { format!("…{s}") } else { s }
}

/// Recherche globale dans la map : tâches, pages, fichiers Verse, acteurs, membres.
pub async fn search(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Query(q): Query<SearchQ>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let term = q.q.trim().to_lowercase();
    if term.chars().count() < 2 {
        return Ok(Json(json!([])));
    }
    let like = format!("%{}%", term.replace('%', "").replace('_', ""));
    let mut out = vec![];
    let num = term.trim_start_matches('#').parse::<i64>().ok();
    let tasks: Vec<(i64, i64, String, String)> = sqlx::query_as("SELECT id, number, title, status FROM tasks WHERE map_id = ? AND (lower(title) LIKE ? OR lower(description) LIKE ? OR number = ?) ORDER BY number = ? DESC, number DESC LIMIT 8").bind(mid).bind(&like).bind(&like).bind(num.unwrap_or(-1)).bind(num.unwrap_or(-1)).fetch_all(&st.db).await?;
    for (id, n, t, s) in tasks { out.push(json!({ "kind": "Tâche", "title": format!("#{n} {t}"), "detail": crate::tasks::status_label(&s), "link": format!("/taches?t={id}") })) }
    let pages: Vec<(i64, String, String, String, i64)> = sqlx::query_as("SELECT id, title, blocks, share, created_by FROM pages WHERE map_id = ? AND deleted_at IS NULL AND (lower(title) LIKE ? OR lower(blocks) LIKE ?) ORDER BY updated_at DESC LIMIT 20").bind(mid).bind(&like).bind(&like).fetch_all(&st.db).await?;
    for (id, t, blocks, share, by) in pages.into_iter().filter(|p| p.3 != "private" || p.4 == ctx.user.id).take(6) {
        let text: String = serde_json::from_str::<Value>(&blocks).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default().iter().filter_map(|b| b["text"].as_str().map(String::from)).find(|x| x.to_lowercase().contains(&term)).unwrap_or_default();
        let _ = share; let _ = by;
        out.push(json!({ "kind": "Page", "title": t, "detail": snippet(&text, &term), "link": format!("/pages/{id}") }));
    }
    let files: Vec<(String, String)> = sqlx::query_as("SELECT path, content FROM verse_files WHERE map_id = ? AND (lower(path) LIKE ? OR lower(content) LIKE ?) LIMIT 6").bind(mid).bind(&like).bind(&like).fetch_all(&st.db).await?;
    for (p, c) in files {
        let (line, text) = c.lines().enumerate().find(|(_, l)| l.to_lowercase().contains(&term)).map(|(i, l)| (i + 1, l.trim().to_string())).unwrap_or((1, String::new()));
        out.push(json!({ "kind": "Verse", "title": p.rsplit('/').next().unwrap_or(&p), "detail": if text.is_empty() { p.clone() } else { format!("ligne {line} : {}", snippet(&text, &term)) }, "link": format!("/verse?f={}&l={line}", urlencode(&p)) }));
    }
    let actors: Option<String> = sqlx::query_scalar("SELECT actors FROM uefn_state WHERE map_id = ?").bind(mid).fetch_optional(&st.db).await?;
    let actors: Vec<Value> = actors.and_then(|a| serde_json::from_str(&a).ok()).unwrap_or_default();
    for a in actors.iter().filter(|a| a["name"].as_str().map(|n| n.to_lowercase().contains(&term)).unwrap_or(false)).take(6) {
        out.push(json!({ "kind": "Acteur", "title": a["name"], "detail": a["folder"], "link": format!("/organiseur?q={}", urlencode(a["name"].as_str().unwrap_or(""))) }));
    }
    let people: Vec<(i64, String, String)> = sqlx::query_as("SELECT u.id, u.name, m.role FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ? AND lower(u.name) LIKE ? LIMIT 4").bind(ctx.studio_id).bind(&like).fetch_all(&st.db).await?;
    for (_, n, r) in people { out.push(json!({ "kind": "Membre", "title": n, "detail": r, "link": "/taches?vue=personnes" })) }
    Ok(Json(json!(out)))
}

fn urlencode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_./".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}
