//! Administration de la plateforme POP : liste d'attente de la bêta, utilisateurs, vue générale.
//! Réservé aux comptes `is_admin` (le premier compte créé, le compte de test « kl », ou POP_ADMIN_EMAILS).
use crate::{
    auth::{random_token, rate_limit, sha256_hex, AuthUser},
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn is_admin(st: &AppState, uid: i64) -> Result<bool, AppError> {
    let a: Option<(i64, String)> = sqlx::query_as("SELECT is_admin, email FROM users WHERE id = ?").bind(uid).fetch_optional(&st.db).await?;
    let Some((flag, email)) = a else { return Ok(false) };
    let env = std::env::var("POP_ADMIN_EMAILS").unwrap_or_default();
    Ok(flag == 1 || env.split(',').map(|e| e.trim().to_lowercase()).any(|e| !e.is_empty() && e == email.to_lowercase()))
}
async fn require(st: &AppState, u: &AuthUser) -> Result<(), AppError> {
    crate::security::require_admin(st, u).await
}
async fn log(st: &AppState, u: &AuthUser, action: &str, detail: &str) {
    crate::security::audit(st, Some(u.id), action, detail, "").await;
}

// ---------- Public : demande d'accès ----------

#[derive(Deserialize)]
pub struct WaitIn {
    email: String,
    name: String,
    studio: Option<String>,
    team: Option<String>,
    experience: Option<String>,
    links: Option<String>,
    message: Option<String>,
}

pub async fn join(State(st): State<AppState>, Json(b): Json<WaitIn>) -> ApiResult<Value> {
    rate_limit(&st, &format!("wait:{}", b.email))?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') || !email.contains('.') || email.len() > 200 { return Err(bad("Adresse e-mail invalide.")) }
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > 80 { return Err(bad("Indiquez votre nom (80 caractères au plus).")) }
    let cut = |v: &Option<String>, n: usize| v.as_deref().unwrap_or("").trim().chars().take(n).collect::<String>();
    let has_account: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?").bind(&email).fetch_one(&st.db).await?;
    if has_account > 0 { return Err(AppError::Conflict("Un compte existe déjà avec cette adresse : connectez-vous.".into())) }
    let existing: Option<String> = sqlx::query_scalar("SELECT status FROM waitlist WHERE email = ?").bind(&email).fetch_optional(&st.db).await?;
    if let Some(s) = existing {
        return Ok(Json(json!({ "ok": true, "already": true, "status": s })));
    }
    sqlx::query("INSERT INTO waitlist (email, name, studio, team, experience, links, message) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&email).bind(name).bind(cut(&b.studio, 80)).bind(cut(&b.team, 40)).bind(cut(&b.experience, 40)).bind(cut(&b.links, 400)).bind(cut(&b.message, 1500))
        .execute(&st.db).await?;
    // Prévenir les administrateurs (cloche dans POP, et e-mail si l'envoi est configuré).
    let admins: Vec<(i64, String)> = sqlx::query_as("SELECT id, email FROM users WHERE is_admin = 1 AND disabled = 0").fetch_all(&st.db).await?;
    for (id, mail) in &admins {
        let _ = sqlx::query("INSERT INTO notifications (user_id, map_id, text, link) VALUES (?, NULL, ?, '/admin')").bind(id).bind(format!("Nouvelle demande d’accès à la bêta : {name}")).execute(&st.db).await;
        if crate::mail::configured() && mail.contains('@') {
            let body = format!("{name} ({email}) demande l’accès à la bêta de POP.\n\nStudio : {}\nMessage : {}\n\nRépondre : {}/admin", cut(&b.studio, 80), cut(&b.message, 400), crate::mail::public_url());
            let m = mail.clone();
            tokio::spawn(async move { let _ = crate::mail::send(&m, "POP : nouvelle demande d’accès à la bêta", &body).await; });
        }
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct TokQ { token: String }

/// Vérifie un lien d'accès (page d'inscription).
pub async fn check_access(State(st): State<AppState>, Query(q): Query<TokQ>) -> ApiResult<Value> {
    let r: Option<(String, String)> = sqlx::query_as("SELECT email, name FROM waitlist WHERE token_hash = ? AND status = 'approved' AND decided_at > datetime('now', '-30 days')")
        .bind(sha256_hex(&q.token)).fetch_optional(&st.db).await?;
    let (email, name) = r.ok_or_else(|| bad("Lien d’accès invalide, déjà utilisé ou expiré (30 jours)."))?;
    Ok(Json(json!({ "email": email, "name": name })))
}

/// Autorise une inscription : premier compte, invitation d'un studio, lien d'accès validé, ou inscriptions ouvertes.
pub async fn signup_allowed(st: &AppState, email: &str, ip: &str, invite: Option<&str>, access: Option<&str>) -> Result<Option<i64>, AppError> {
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&st.db).await?;
    if std::env::var("POP_OPEN_SIGNUP").map(|v| v == "1").unwrap_or(false) { return Ok(None) }
    // Une installation vide n'accepte son premier compte que depuis l'ordinateur du serveur,
    // ou pour une adresse listée dans POP_ADMIN_EMAILS (sinon n'importe qui deviendrait administrateur).
    let env_admin = std::env::var("POP_ADMIN_EMAILS").unwrap_or_default().split(',').any(|e| !e.trim().is_empty() && e.trim().eq_ignore_ascii_case(email));
    if env_admin { return Ok(None) }
    if users == 0 && crate::security::is_loopback(ip) { return Ok(None) }
    if let Some(t) = invite.filter(|t| !t.is_empty()) {
        let ok: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invitations WHERE token_hash = ? AND accepted_at IS NULL AND created_at > datetime('now', '-7 days')").bind(sha256_hex(t)).fetch_one(&st.db).await?;
        if ok > 0 { return Ok(None) }
        return Err(bad("Invitation invalide ou expirée."));
    }
    if let Some(t) = access.filter(|t| !t.is_empty()) {
        let r: Option<(i64, String)> = sqlx::query_as("SELECT id, email FROM waitlist WHERE token_hash = ? AND status = 'approved' AND decided_at > datetime('now', '-30 days')").bind(sha256_hex(t)).fetch_optional(&st.db).await?;
        let (wid, wmail) = r.ok_or_else(|| bad("Lien d’accès invalide, déjà utilisé ou expiré."))?;
        if !wmail.eq_ignore_ascii_case(email) { return Err(bad("Ce lien d’accès est réservé à une autre adresse e-mail.")) }
        return Ok(Some(wid));
    }
    Err(AppError::Forbidden)
}

pub async fn mark_registered(st: &AppState, wid: i64, uid: i64) {
    let _ = sqlx::query("UPDATE waitlist SET status = 'registered', token_hash = NULL, registered_user = ? WHERE id = ?").bind(uid).bind(wid).execute(&st.db).await;
}

// ---------- Administration ----------

pub async fn overview(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    require(&st, &u).await?;
    let one = |q: &'static str| { let st = st.clone(); async move { sqlx::query_scalar::<_, i64>(q).fetch_one(&st.db).await.unwrap_or(0) } };
    let users = one("SELECT COUNT(*) FROM users").await;
    let users_7d = one("SELECT COUNT(*) FROM users WHERE created_at > datetime('now', '-7 days')").await;
    let active_7d = one("SELECT COUNT(*) FROM users WHERE last_seen > datetime('now', '-7 days')").await;
    let studios = one("SELECT COUNT(*) FROM studios").await;
    let maps = one("SELECT COUNT(*) FROM maps").await;
    let tasks = one("SELECT COUNT(*) FROM tasks").await;
    let tasks_done_7d = one("SELECT COUNT(*) FROM tasks WHERE done_at > datetime('now', '-7 days')").await;
    let pending = one("SELECT COUNT(*) FROM waitlist WHERE status = 'pending'").await;
    let approved = one("SELECT COUNT(*) FROM waitlist WHERE status = 'approved'").await;
    let registered = one("SELECT COUNT(*) FROM waitlist WHERE status = 'registered'").await;
    let tokens = one("SELECT COUNT(*) FROM api_tokens WHERE last_used > datetime('now', '-7 days')").await;
    let linked_maps: Vec<i64> = sqlx::query_scalar("SELECT id FROM maps").fetch_all(&st.db).await.unwrap_or_default();
    let relays = linked_maps.iter().filter(|m| st.relays.status(**m) != "offline").count();
    let signups: Vec<(String, i64)> = sqlx::query_as("SELECT date(created_at), COUNT(*) FROM waitlist WHERE created_at > datetime('now', '-30 days') GROUP BY 1 ORDER BY 1").fetch_all(&st.db).await?;
    Ok(Json(json!({
        "users": users, "users_7d": users_7d, "active_7d": active_7d, "studios": studios, "maps": maps, "tasks": tasks, "tasks_done_7d": tasks_done_7d,
        "waitlist": { "pending": pending, "approved": approved, "registered": registered }, "ai_tokens_active": tokens, "relays_online": relays,
        "mail": crate::mail::configured(), "open_signup": std::env::var("POP_OPEN_SIGNUP").map(|v| v == "1").unwrap_or(false),
        "requests_per_day": signups.into_iter().map(|(d, n)| json!({ "day": d, "count": n })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct ListQ { status: Option<String> }

pub async fn waitlist(State(st): State<AppState>, u: AuthUser, Query(q): Query<ListQ>) -> ApiResult<Value> {
    require(&st, &u).await?;
    let status = q.status.unwrap_or_else(|| "pending".into());
    let rows: Vec<(i64, String, String, String, String, String, String, String, String, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT w.id, w.email, w.name, w.studio, w.team, w.experience, w.links, w.message, w.status, w.note, w.created_at, w.decided_at, d.name
         FROM waitlist w LEFT JOIN users d ON d.id = w.decided_by WHERE (? = 'all' OR w.status = ?) ORDER BY w.created_at DESC LIMIT 500",
    ).bind(&status).bind(&status).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|r| json!({
        "id": r.0, "email": r.1, "name": r.2, "studio": r.3, "team": r.4, "experience": r.5, "links": r.6, "message": r.7, "status": r.8, "note": r.9,
        "created_at": r.10, "decided_at": r.11, "decided_by": r.12,
    })).collect::<Vec<_>>())))
}

async fn grant(st: &AppState, admin: &AuthUser, wid: i64) -> Result<Value, AppError> {
    let (email, name): (String, String) = sqlx::query_as("SELECT email, name FROM waitlist WHERE id = ?").bind(wid).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)?;
    log(st, admin, "admin_accès_accordé", &email).await;
    let token = random_token();
    sqlx::query("UPDATE waitlist SET status = 'approved', token_hash = ?, decided_at = datetime('now'), decided_by = ? WHERE id = ?").bind(sha256_hex(&token)).bind(admin.id).bind(wid).execute(&st.db).await?;
    let link = format!("{}/inscription?acces={token}", crate::mail::public_url());
    let mut mailed = false;
    if crate::mail::configured() {
        let first = name.split_whitespace().next().unwrap_or(&name);
        let body = format!("Bonjour {first},\n\nBonne nouvelle : votre accès à la bêta de POP est validé.\nCréez votre compte avec ce lien (valable 30 jours) :\n\n{link}\n\nÀ très vite,\nL’équipe POP");
        mailed = crate::mail::send(&email, "Votre accès à la bêta de POP", &body).await.is_ok();
    }
    Ok(json!({ "link": link, "mailed": mailed, "email": email }))
}

pub async fn approve(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>) -> ApiResult<Value> {
    require(&st, &u).await?;
    Ok(Json(grant(&st, &u, id).await?))
}

pub async fn refuse(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>) -> ApiResult<Value> {
    require(&st, &u).await?;
    log(&st, &u, "admin_demande_refusée", &id.to_string()).await;
    sqlx::query("UPDATE waitlist SET status = 'refused', token_hash = NULL, decided_at = datetime('now'), decided_by = ? WHERE id = ? AND status != 'registered'").bind(u.id).bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn reopen(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>) -> ApiResult<Value> {
    require(&st, &u).await?;
    sqlx::query("UPDATE waitlist SET status = 'pending', token_hash = NULL, decided_at = NULL, decided_by = NULL WHERE id = ? AND status != 'registered'").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>) -> ApiResult<Value> {
    require(&st, &u).await?;
    log(&st, &u, "admin_demande_supprimée", &id.to_string()).await;
    sqlx::query("DELETE FROM waitlist WHERE id = ?").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct NoteIn { note: String }
pub async fn note(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>, Json(b): Json<NoteIn>) -> ApiResult<Value> {
    require(&st, &u).await?;
    sqlx::query("UPDATE waitlist SET note = ? WHERE id = ?").bind(b.note.chars().take(1000).collect::<String>()).bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct InviteIn { email: String, name: String }
/// Donner un accès directement, sans demande préalable.
pub async fn invite(State(st): State<AppState>, u: AuthUser, Json(b): Json<InviteIn>) -> ApiResult<Value> {
    require(&st, &u).await?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') { return Err(bad("Adresse e-mail invalide.")) }
    let has: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?").bind(&email).fetch_one(&st.db).await?;
    if has > 0 { return Err(AppError::Conflict("Cette personne a déjà un compte.".into())) }
    let name = if b.name.trim().is_empty() { email.split('@').next().unwrap_or("").to_string() } else { b.name.trim().chars().take(80).collect() };
    let id: i64 = sqlx::query_scalar("INSERT INTO waitlist (email, name, message) VALUES (?, ?, 'Invité par un administrateur') ON CONFLICT(email) DO UPDATE SET name = excluded.name RETURNING id")
        .bind(&email).bind(&name).fetch_one(&st.db).await?;
    Ok(Json(grant(&st, &u, id).await?))
}

pub async fn users(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    require(&st, &u).await?;
    let rows: Vec<(i64, String, String, String, Option<String>, i64, i64, i64, i64, Option<String>, i64, String)> = sqlx::query_as(
        "SELECT u.id, u.email, u.name, u.created_at, u.last_seen, u.is_admin, u.disabled,
                (SELECT COUNT(*) FROM memberships m WHERE m.user_id = u.id),
                (SELECT COUNT(*) FROM tasks t WHERE t.assignee_id = u.id AND t.status = 'done'), u.username, u.must_change_password, u.plan
         FROM users u ORDER BY u.created_at DESC LIMIT 2000",
    ).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|r| json!({ "id": r.0, "email": r.1, "name": r.2, "created_at": r.3, "last_seen": r.4, "is_admin": r.5 == 1, "disabled": r.6 == 1, "studios": r.7, "tasks_done": r.8, "username": r.9, "must_change": r.10 == 1, "plan": r.11, "me": r.0 == u.id })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct UserPatch { is_admin: Option<bool>, disabled: Option<bool> }
pub async fn update_user(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>, Json(b): Json<UserPatch>) -> ApiResult<Value> {
    require(&st, &u).await?;
    if id == u.id { return Err(bad("Vous ne pouvez pas modifier votre propre compte ici.")) }
    log(&st, &u, "admin_compte_modifié", &format!("utilisateur {id} : admin={:?} désactivé={:?}", b.is_admin, b.disabled)).await;
    if let Some(a) = b.is_admin { sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?").bind(a as i64).bind(id).execute(&st.db).await?; }
    if let Some(d) = b.disabled {
        sqlx::query("UPDATE users SET disabled = ? WHERE id = ?").bind(d as i64).bind(id).execute(&st.db).await?;
        if d { sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(id).execute(&st.db).await?; }
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PlanIn { plan: String }
/// Change l'abonnement d'un compte (bêta : pas encore de paiement).
pub async fn set_plan(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>, Json(b): Json<PlanIn>) -> ApiResult<Value> {
    require(&st, &u).await?;
    if !crate::billing::PLANS.contains(&b.plan.as_str()) { return Err(bad("Plan inconnu.")) }
    let n = sqlx::query("UPDATE users SET plan = ? WHERE id = ?").bind(&b.plan).bind(id).execute(&st.db).await?.rows_affected();
    if n == 0 { return Err(AppError::NotFound) }
    log(&st, &u, "admin_plan_modifié", &format!("utilisateur {id} : {}", b.plan)).await;
    Ok(Json(json!({ "ok": true })))
}

// ---------- Comptes créés par l'administrateur (bêta) ----------

/// Mot de passe provisoire lisible, sans caractères ambigus (ex. « k7mq-x2pa-9hrt »).
fn temp_password() -> String {
    use rand::Rng;
    const A: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut r = rand::rngs::OsRng;
    (0..3).map(|_| (0..4).map(|_| A[r.gen_range(0..A.len())] as char).collect::<String>()).collect::<Vec<_>>().join("-")
}

/// Identifiant : 3 à 32 caractères, lettres sans accent, chiffres, point, tiret, tiret bas.
fn check_username(raw: &str) -> Result<String, AppError> {
    let u = raw.trim().to_lowercase();
    let n = u.chars().count();
    if !(3..=32).contains(&n) { return Err(bad("L’identifiant doit faire entre 3 et 32 caractères.")) }
    if !u.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_')) {
        return Err(bad("L’identifiant ne peut contenir que des lettres sans accent, des chiffres, des points et des tirets."))
    }
    Ok(u)
}

#[derive(Deserialize)]
pub struct AccountIn {
    username: String,
    name: String,
    email: String,
    password: Option<String>,
    waitlist_id: Option<i64>,
    plan: Option<String>,
}

/// Crée le compte d'un bêta-testeur. Le mot de passe (provisoire) n'est montré qu'une fois ;
/// la personne devra le changer à sa première connexion.
pub async fn create_account(State(st): State<AppState>, u: AuthUser, Json(b): Json<AccountIn>) -> ApiResult<Value> {
    require(&st, &u).await?;
    let username = check_username(&b.username)?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') || email.len() > 200 { return Err(bad("Adresse e-mail invalide.")) }
    let name: String = if b.name.trim().is_empty() { username.clone() } else { b.name.trim().chars().take(80).collect() };
    let taken: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = ?1 OR email = ?1").bind(&username).fetch_one(&st.db).await?;
    if taken > 0 { return Err(AppError::Conflict("Cet identifiant est déjà pris.".into())) }
    let has: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?").bind(&email).fetch_one(&st.db).await?;
    if has > 0 { return Err(AppError::Conflict("Un compte existe déjà avec cette adresse e-mail.".into())) }
    let password = match b.password.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => { crate::security::password_policy(p, &email)?; p.to_string() }
        None => temp_password(),
    };
    let plan = b.plan.as_deref().unwrap_or("studio");
    if !crate::billing::PLANS.contains(&plan) { return Err(bad("Plan inconnu.")) }
    let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, username, must_change_password, plan) VALUES (?, ?, ?, ?, 1, ?) RETURNING id")
        .bind(&email).bind(&name).bind(crate::auth::hash_password(&password)?).bind(&username).bind(plan).fetch_one(&st.db).await?;
    // Relie la demande de la liste d'attente (celle choisie, sinon celle de la même adresse).
    let wid: Option<i64> = match b.waitlist_id {
        Some(w) => Some(w),
        None => sqlx::query_scalar("SELECT id FROM waitlist WHERE email = ?").bind(&email).fetch_optional(&st.db).await?,
    };
    if let Some(w) = wid {
        sqlx::query("UPDATE waitlist SET decided_at = COALESCE(decided_at, datetime('now')), decided_by = COALESCE(decided_by, ?) WHERE id = ?").bind(u.id).bind(w).execute(&st.db).await?;
        mark_registered(&st, w, id).await;
    }
    log(&st, &u, "admin_compte_créé", &format!("{username} ({email})")).await;
    Ok(Json(json!({ "id": id, "username": username, "name": name, "email": email, "password": password, "url": crate::mail::public_url() + "/connexion" })))
}

/// Nouveau mot de passe provisoire pour un utilisateur (ses sessions sont fermées).
pub async fn reset_password(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>) -> ApiResult<Value> {
    require(&st, &u).await?;
    if id == u.id { return Err(bad("Changez votre propre mot de passe dans Paramètres.")) }
    let (email, username): (String, Option<String>) = sqlx::query_as("SELECT email, username FROM users WHERE id = ?").bind(id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)?;
    let password = temp_password();
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 1, failed_logins = 0, locked_until = NULL WHERE id = ?").bind(crate::auth::hash_password(&password)?).bind(id).execute(&st.db).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(id).execute(&st.db).await?;
    log(&st, &u, "admin_mot_de_passe_réinitialisé", &username.clone().unwrap_or_else(|| email.clone())).await;
    Ok(Json(json!({ "username": username.unwrap_or(email), "password": password, "url": crate::mail::public_url() + "/connexion" })))
}
