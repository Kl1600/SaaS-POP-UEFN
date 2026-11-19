//! Administration de la plateforme POP : liste d'attente de la bêta, utilisateurs, vue générale.
//! Réservé aux comptes `is_admin` (nommés par `pop-server admin ajouter <e-mail>`, le compte de test « kl »),
//! ou dont l'adresse est dans POP_ADMIN_EMAILS.
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

/// Administrateur de la plateforme : nommé (`is_admin`) ou listé dans POP_ADMIN_EMAILS, et **adresse vérifiée** (#11) :
/// sans cela, n'importe qui pourrait s'inscrire avec l'adresse d'un administrateur.
pub async fn is_admin(st: &AppState, uid: i64) -> Result<bool, AppError> {
    let a: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT is_admin, email, email_verified_at IS NOT NULL FROM users WHERE id = ?",
    )
    .bind(uid)
    .fetch_optional(&st.db)
    .await?;
    let Some((flag, email, verified)) = a else {
        return Ok(false);
    };
    if verified == 0 {
        return Ok(false);
    }
    let env = std::env::var("POP_ADMIN_EMAILS").unwrap_or_default();
    Ok(flag == 1
        || env
            .split(',')
            .map(|e| e.trim().to_lowercase())
            .any(|e| !e.is_empty() && e == email.to_lowercase()))
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
    /// Champ piège, caché aux humains : rempli, la demande vient d'un robot.
    #[serde(default)]
    pop_hp: String,
}

/// Demandes d'accès acceptées par adresse IP et par heure.
const WAIT_PER_IP_HOUR: u32 = 5;

pub async fn join(
    State(st): State<AppState>,
    crate::security::ClientIp(ip): crate::security::ClientIp,
    Json(b): Json<WaitIn>,
) -> ApiResult<Value> {
    if !crate::auth::hit(
        &st,
        &format!("wait:ip:{ip}"),
        WAIT_PER_IP_HOUR,
        std::time::Duration::from_secs(3600),
    ) {
        return Err(AppError::TooManyMsg(
            "Trop de demandes d’accès depuis votre connexion : réessayez dans une heure.".into(),
        ));
    }
    // Un robot reçoit la même réponse qu'un humain, mais rien n'est enregistré.
    if !b.pop_hp.trim().is_empty() {
        return Ok(Json(json!({ "ok": true })));
    }
    rate_limit(&st, &format!("wait:{}", b.email))?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') || !email.contains('.') || email.len() > 200 {
        return Err(bad("Adresse e-mail invalide."));
    }
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(bad("Indiquez votre nom (80 caractères au plus)."));
    }
    let cut = |v: &Option<String>, n: usize| {
        v.as_deref()
            .unwrap_or("")
            .trim()
            .chars()
            .take(n)
            .collect::<String>()
    };
    let has_account: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&st.db)
        .await?;
    if has_account > 0 {
        return Err(AppError::Conflict(
            "Un compte existe déjà avec cette adresse : connectez-vous.".into(),
        ));
    }
    let existing: Option<String> =
        sqlx::query_scalar("SELECT status FROM waitlist WHERE email = ?")
            .bind(&email)
            .fetch_optional(&st.db)
            .await?;
    if let Some(s) = existing {
        return Ok(Json(json!({ "ok": true, "already": true, "status": s })));
    }
    sqlx::query("INSERT INTO waitlist (email, name, studio, team, experience, links, message) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&email).bind(name).bind(cut(&b.studio, 80)).bind(cut(&b.team, 40)).bind(cut(&b.experience, 40)).bind(cut(&b.links, 400)).bind(cut(&b.message, 1500))
        .execute(&st.db).await?;
    // Les administrateurs sont prévenus par le résumé horaire (`digest_loop`).
    Ok(Json(json!({ "ok": true })))
}

/// Demandes pas encore signalées, si la plus ancienne attend depuis une heure : au plus un résumé par heure.
/// Les marque comme signalées et renvoie (nom, e-mail, studio) de chacune.
pub async fn take_digest(db: &sqlx::SqlitePool) -> Result<Vec<(String, String, String)>, AppError> {
    let due: Option<i64> = sqlx::query_scalar("SELECT MAX(id) FROM waitlist WHERE admin_notified_at IS NULL HAVING MIN(created_at) <= datetime('now', '-1 hour')")
        .fetch_optional(db).await?.flatten();
    let Some(last) = due else { return Ok(vec![]) };
    let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT name, email, studio FROM waitlist WHERE admin_notified_at IS NULL AND id <= ? ORDER BY id")
        .bind(last).fetch_all(db).await?;
    sqlx::query("UPDATE waitlist SET admin_notified_at = datetime('now') WHERE admin_notified_at IS NULL AND id <= ?").bind(last).execute(db).await?;
    Ok(rows)
}

/// Envoie le résumé des nouvelles demandes d'accès : une notification et un e-mail par admin.
pub async fn send_digest(st: &AppState) -> Result<usize, AppError> {
    let rows = take_digest(&st.db).await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let n = rows.len();
    let title = if n == 1 {
        "1 nouvelle demande d’accès à la bêta".to_string()
    } else {
        format!("{n} nouvelles demandes d’accès à la bêta")
    };
    let mut body = format!(
        "{title} de POP :

"
    );
    for (name, email, studio) in rows.iter().take(50) {
        body += &format!(
            "- {name} ({email}){}
",
            if studio.is_empty() {
                String::new()
            } else {
                format!(", studio {studio}")
            }
        );
    }
    if n > 50 {
        body += &format!(
            "… et {} autres.
",
            n - 50
        )
    }
    body += &format!(
        "
Répondre : {}/admin",
        crate::mail::public_url()
    );
    let admins: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, email FROM users WHERE is_admin = 1 AND disabled = 0")
            .fetch_all(&st.db)
            .await?;
    for (id, mail) in &admins {
        let _ = sqlx::query(
            "INSERT INTO notifications (user_id, map_id, text, link) VALUES (?, NULL, ?, '/admin')",
        )
        .bind(id)
        .bind(&title)
        .execute(&st.db)
        .await;
        if crate::mail::configured() && mail.contains('@') {
            if let Err(e) = crate::mail::send(mail, &format!("POP : {title}"), &body).await {
                tracing::warn!("résumé de la liste d’attente non envoyé à un admin : {e}")
            }
        }
    }
    tracing::info!(
        "résumé de la liste d’attente : {n} demande(s), {} admin(s)",
        admins.len()
    );
    Ok(n)
}

/// Toutes les minutes : envoie le résumé des demandes d'accès quand il est dû.
pub async fn digest_loop(st: AppState) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        if let Err(e) = send_digest(&st).await {
            tracing::warn!("résumé de la liste d’attente : {e:?}")
        }
    }
}

#[derive(Deserialize)]
pub struct TokQ {
    token: String,
}

/// Vérifie un lien d'accès (page d'inscription).
pub async fn check_access(State(st): State<AppState>, Query(q): Query<TokQ>) -> ApiResult<Value> {
    let r: Option<(String, String)> = sqlx::query_as("SELECT email, name FROM waitlist WHERE token_hash = ? AND status = 'approved' AND decided_at > datetime('now', '-30 days')")
        .bind(sha256_hex(&q.token)).fetch_optional(&st.db).await?;
    let (email, name) =
        r.ok_or_else(|| bad("Lien d’accès invalide, déjà utilisé ou expiré (30 jours)."))?;
    Ok(Json(json!({ "email": email, "name": name })))
}

/// Autorise une inscription : premier compte, invitation d'un studio, lien d'accès validé, ou inscriptions ouvertes.
pub async fn signup_allowed(
    st: &AppState,
    email: &str,
    ip: &str,
    invite: Option<&str>,
    access: Option<&str>,
) -> Result<Option<i64>, AppError> {
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&st.db)
        .await?;
    if std::env::var("POP_OPEN_SIGNUP")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return Ok(None);
    }
    // Une installation vide accepte son premier compte depuis l'ordinateur du serveur (sans droit d'administration :
    // voir `pop-server admin`), ou pour une adresse listée dans POP_ADMIN_EMAILS.
    let env_admin = std::env::var("POP_ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .any(|e| !e.trim().is_empty() && e.trim().eq_ignore_ascii_case(email));
    if env_admin {
        return Ok(None);
    }
    if users == 0 && crate::security::is_loopback(ip) {
        return Ok(None);
    }
    if let Some(t) = invite.filter(|t| !t.is_empty()) {
        let ok: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invitations WHERE token_hash = ? AND accepted_at IS NULL AND created_at > datetime('now', '-7 days')").bind(sha256_hex(t)).fetch_one(&st.db).await?;
        if ok > 0 {
            return Ok(None);
        }
        return Err(bad("Invitation invalide ou expirée."));
    }
    if let Some(t) = access.filter(|t| !t.is_empty()) {
        let r: Option<(i64, String)> = sqlx::query_as("SELECT id, email FROM waitlist WHERE token_hash = ? AND status = 'approved' AND decided_at > datetime('now', '-30 days')").bind(sha256_hex(t)).fetch_optional(&st.db).await?;
        let (wid, wmail) =
            r.ok_or_else(|| bad("Lien d’accès invalide, déjà utilisé ou expiré."))?;
        if !wmail.eq_ignore_ascii_case(email) {
            return Err(bad(
                "Ce lien d’accès est réservé à une autre adresse e-mail.",
            ));
        }
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
    let one = |q: &'static str| {
        let st = st.clone();
        async move {
            sqlx::query_scalar::<_, i64>(q)
                .fetch_one(&st.db)
                .await
                .unwrap_or(0)
        }
    };
    let users = one("SELECT COUNT(*) FROM users").await;
    let users_7d =
        one("SELECT COUNT(*) FROM users WHERE created_at > datetime('now', '-7 days')").await;
    let active_7d =
        one("SELECT COUNT(*) FROM users WHERE last_seen > datetime('now', '-7 days')").await;
    let studios = one("SELECT COUNT(*) FROM studios").await;
    let maps = one("SELECT COUNT(*) FROM maps").await;
    let tasks = one("SELECT COUNT(*) FROM tasks").await;
    let tasks_done_7d =
        one("SELECT COUNT(*) FROM tasks WHERE done_at > datetime('now', '-7 days')").await;
    let pending = one("SELECT COUNT(*) FROM waitlist WHERE status = 'pending'").await;
    let approved = one("SELECT COUNT(*) FROM waitlist WHERE status = 'approved'").await;
    let registered = one("SELECT COUNT(*) FROM waitlist WHERE status = 'registered'").await;
    let tokens =
        one("SELECT COUNT(*) FROM api_tokens WHERE last_used > datetime('now', '-7 days')").await;
    let linked_maps: Vec<i64> = sqlx::query_scalar("SELECT id FROM maps")
        .fetch_all(&st.db)
        .await
        .unwrap_or_default();
    let relays = linked_maps
        .iter()
        .filter(|m| st.relays.status(**m) != "offline")
        .count();
    let signups: Vec<(String, i64)> = sqlx::query_as("SELECT date(created_at), COUNT(*) FROM waitlist WHERE created_at > datetime('now', '-30 days') GROUP BY 1 ORDER BY 1").fetch_all(&st.db).await?;
    Ok(Json(json!({
        "users": users, "users_7d": users_7d, "active_7d": active_7d, "studios": studios, "maps": maps, "tasks": tasks, "tasks_done_7d": tasks_done_7d,
        "waitlist": { "pending": pending, "approved": approved, "registered": registered }, "ai_tokens_active": tokens, "relays_online": relays,
        "mail": crate::mail::configured(), "open_signup": std::env::var("POP_OPEN_SIGNUP").map(|v| v == "1").unwrap_or(false),
        "requests_per_day": signups.into_iter().map(|(d, n)| json!({ "day": d, "count": n })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct ListQ {
    status: Option<String>,
}

pub async fn waitlist(
    State(st): State<AppState>,
    u: AuthUser,
    Query(q): Query<ListQ>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    let status = q.status.unwrap_or_else(|| "pending".into());
    let rows: Vec<(i64, String, String, String, String, String, String, String, String, String, String, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT w.id, w.email, w.name, w.studio, w.team, w.experience, w.links, w.message, w.status, w.note, w.created_at, w.decided_at, d.name,
                EXISTS (SELECT 1 FROM account_activations a JOIN users au ON au.id = a.user_id WHERE a.waitlist_id = w.id AND au.activation_pending = 1)
         FROM waitlist w LEFT JOIN users d ON d.id = w.decided_by WHERE (? = 'all' OR w.status = ?) ORDER BY w.created_at DESC LIMIT 500",
    ).bind(&status).bind(&status).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|r| json!({
        "id": r.0, "email": r.1, "name": r.2, "studio": r.3, "team": r.4, "experience": r.5, "links": r.6, "message": r.7, "status": r.8, "note": r.9,
        "created_at": r.10, "decided_at": r.11, "decided_by": r.12, "activation_pending": r.13 == 1,
    })).collect::<Vec<_>>())))
}

async fn grant(st: &AppState, admin: &AuthUser, wid: i64) -> Result<Value, AppError> {
    let (email, name): (String, String) =
        sqlx::query_as("SELECT email, name FROM waitlist WHERE id = ?")
            .bind(wid)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    log(st, admin, "admin_accès_accordé", &email).await;
    let token = random_token();
    sqlx::query("UPDATE waitlist SET status = 'approved', token_hash = ?, decided_at = datetime('now'), decided_by = ? WHERE id = ?").bind(sha256_hex(&token)).bind(admin.id).bind(wid).execute(&st.db).await?;
    let link = format!("{}/inscription?acces={token}", crate::mail::public_url());
    let mut mailed = false;
    if crate::mail::configured() {
        let first = name.split_whitespace().next().unwrap_or(&name);
        let body = format!("Bonjour {first},\n\nBonne nouvelle : votre accès à la bêta de POP est validé.\nCréez votre compte avec ce lien (valable 30 jours) :\n\n{link}\n\nÀ très vite,\nL’équipe POP");
        mailed = crate::mail::send(&email, "Votre accès à la bêta de POP", &body)
            .await
            .is_ok();
    }
    Ok(json!({ "link": link, "mailed": mailed, "email": email }))
}

pub async fn approve(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    Ok(Json(grant(&st, &u, id).await?))
}

pub async fn refuse(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    log(&st, &u, "admin_demande_refusée", &id.to_string()).await;
    sqlx::query("UPDATE waitlist SET status = 'refused', token_hash = NULL, decided_at = datetime('now'), decided_by = ? WHERE id = ? AND status != 'registered'").bind(u.id).bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn reopen(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    sqlx::query("UPDATE waitlist SET status = 'pending', token_hash = NULL, decided_at = NULL, decided_by = NULL WHERE id = ? AND status != 'registered'").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    log(&st, &u, "admin_demande_supprimée", &id.to_string()).await;
    sqlx::query("DELETE FROM waitlist WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct NoteIn {
    note: String,
}
pub async fn note(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<NoteIn>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    sqlx::query("UPDATE waitlist SET note = ? WHERE id = ?")
        .bind(b.note.chars().take(1000).collect::<String>())
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct InviteIn {
    email: String,
    name: String,
}
/// Donner un accès directement, sans demande préalable.
pub async fn invite(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<InviteIn>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(bad("Adresse e-mail invalide."));
    }
    let has: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&st.db)
        .await?;
    if has > 0 {
        return Err(AppError::Conflict(
            "Cette personne a déjà un compte.".into(),
        ));
    }
    let name = if b.name.trim().is_empty() {
        email.split('@').next().unwrap_or("").to_string()
    } else {
        b.name.trim().chars().take(80).collect()
    };
    let id: i64 = sqlx::query_scalar("INSERT INTO waitlist (email, name, message) VALUES (?, ?, 'Invité par un administrateur') ON CONFLICT(email) DO UPDATE SET name = excluded.name RETURNING id")
        .bind(&email).bind(&name).fetch_one(&st.db).await?;
    Ok(Json(grant(&st, &u, id).await?))
}

pub async fn users(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    require(&st, &u).await?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(i64, String, String, String, Option<String>, i64, i64, i64, i64, Option<String>, i64, String, Option<String>, i64, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.email, u.name, u.created_at, u.last_seen, u.is_admin, u.disabled,
                (SELECT COUNT(*) FROM memberships m WHERE m.user_id = u.id),
                (SELECT COUNT(*) FROM tasks t WHERE t.assignee_id = u.id AND t.status = 'done'), u.username, u.must_change_password, u.plan,
                CASE WHEN u.locked_until > datetime('now') THEN u.locked_until END,
                u.activation_pending, (SELECT MAX(a.created_at) FROM account_activations a WHERE a.user_id = u.id)
         FROM users u ORDER BY u.created_at DESC LIMIT 2000",
    ).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|r| json!({
        "id": r.0, "email": r.1, "name": r.2, "created_at": r.3, "last_seen": r.4, "is_admin": r.5 == 1, "disabled": r.6 == 1, "studios": r.7, "tasks_done": r.8,
        "username": r.9, "must_change": r.10 == 1, "plan": r.11, "locked_until": r.12, "me": r.0 == u.id,
        "activation_pending": r.13 == 1, "activation_sent_at": r.14,
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct UserPatch {
    is_admin: Option<bool>,
    disabled: Option<bool>,
}
pub async fn update_user(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<UserPatch>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    if id == u.id {
        return Err(bad("Vous ne pouvez pas modifier votre propre compte ici."));
    }
    log(
        &st,
        &u,
        "admin_compte_modifié",
        &format!(
            "utilisateur {id} : admin={:?} désactivé={:?}",
            b.is_admin, b.disabled
        ),
    )
    .await;
    if let Some(a) = b.is_admin {
        sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
            .bind(a as i64)
            .bind(id)
            .execute(&st.db)
            .await?;
    }
    if let Some(d) = b.disabled {
        sqlx::query("UPDATE users SET disabled = ? WHERE id = ?")
            .bind(d as i64)
            .bind(id)
            .execute(&st.db)
            .await?;
        if d {
            // Un compte désactivé perd ses sessions et ses jetons IA (#13).
            sqlx::query("DELETE FROM sessions WHERE user_id = ?")
                .bind(id)
                .execute(&st.db)
                .await?;
            sqlx::query("DELETE FROM api_tokens WHERE user_id = ?")
                .bind(id)
                .execute(&st.db)
                .await?;
        }
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PlanIn {
    plan: String,
}
/// Change l'abonnement d'un compte (bêta : pas encore de paiement).
pub async fn set_plan(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<PlanIn>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    if !crate::billing::PLANS.contains(&b.plan.as_str()) {
        return Err(bad("Plan inconnu."));
    }
    let n = sqlx::query("UPDATE users SET plan = ? WHERE id = ?")
        .bind(&b.plan)
        .bind(id)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    log(
        &st,
        &u,
        "admin_plan_modifié",
        &format!("utilisateur {id} : {}", b.plan),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

// ---------- Comptes créés par l'administrateur (bêta) ----------

/// Mot de passe provisoire lisible, sans caractères ambigus (ex. « k7mq-x2pa-9hrt »).
fn temp_password() -> String {
    use rand::Rng;
    const A: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut r = rand::rngs::OsRng;
    (0..3)
        .map(|_| {
            (0..4)
                .map(|_| A[r.gen_range(0..A.len())] as char)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Identifiant : 3 à 32 caractères, lettres sans accent, chiffres, point, tiret, tiret bas.
fn check_username(raw: &str) -> Result<String, AppError> {
    let u = raw.trim().to_lowercase();
    let n = u.chars().count();
    if !(3..=32).contains(&n) {
        return Err(bad("L’identifiant doit faire entre 3 et 32 caractères."));
    }
    if !u
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
    {
        return Err(bad("L’identifiant ne peut contenir que des lettres sans accent, des chiffres, des points et des tirets."));
    }
    Ok(u)
}

#[derive(Deserialize)]
pub struct AccountIn {
    username: String,
    name: String,
    email: String,
    waitlist_id: Option<i64>,
    plan: Option<String>,
}

/// Crée le compte d'un bêta-testeur, sans mot de passe (#42) : la personne reçoit un lien d'activation par e-mail
/// et choisit elle-même son mot de passe. Sans SMTP, l'admin reçoit le lien à copier.
pub async fn create_account(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<AccountIn>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    let username = check_username(&b.username)?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') || email.len() > 200 {
        return Err(bad("Adresse e-mail invalide."));
    }
    let name: String = if b.name.trim().is_empty() {
        username.clone()
    } else {
        b.name.trim().chars().take(80).collect()
    };
    let taken: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = ?1 OR email = ?1")
            .bind(&username)
            .fetch_one(&st.db)
            .await?;
    if taken > 0 {
        return Err(AppError::Conflict("Cet identifiant est déjà pris.".into()));
    }
    let has: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&st.db)
        .await?;
    if has > 0 {
        return Err(AppError::Conflict(
            "Un compte existe déjà avec cette adresse e-mail.".into(),
        ));
    }
    let plan = b.plan.as_deref().unwrap_or("pro");
    if !crate::billing::PLANS.contains(&plan) {
        return Err(bad("Plan inconnu."));
    }
    // Mot de passe inconnu de tous (aléatoire) : le compte ne s'ouvre qu'avec le lien d'activation.
    let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, username, must_change_password, activation_pending, plan) VALUES (?, ?, ?, ?, 0, 1, ?) RETURNING id")
        .bind(&email).bind(&name).bind(crate::auth::hash_password(&crate::auth::random_token())?).bind(&username).bind(plan).fetch_one(&st.db).await?;
    // Relie la demande de la liste d'attente (celle choisie, sinon celle de la même adresse).
    let wid: Option<i64> = match b.waitlist_id {
        Some(w) => Some(w),
        None => {
            sqlx::query_scalar("SELECT id FROM waitlist WHERE email = ?")
                .bind(&email)
                .fetch_optional(&st.db)
                .await?
        }
    };
    if let Some(w) = wid {
        // Demande acceptée ; elle passe à « inscrit » à l'activation, pas à la création.
        sqlx::query("UPDATE waitlist SET status = 'approved', token_hash = NULL, decided_at = COALESCE(decided_at, datetime('now')), decided_by = COALESCE(decided_by, ?) WHERE id = ?").bind(u.id).bind(w).execute(&st.db).await?;
    }
    let sent = crate::activation::issue(&st, id, wid).await?;
    log(
        &st,
        &u,
        "admin_compte_créé",
        &format!("{username} ({email})"),
    )
    .await;
    Ok(Json(
        json!({ "id": id, "username": username, "name": name, "email": email, "mailed": sent["mailed"], "link": sent.get("link") }),
    ))
}

/// Nouveau mot de passe provisoire pour un utilisateur (ses sessions sont fermées).
pub async fn reset_password(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    if id == u.id {
        return Err(bad("Changez votre propre mot de passe dans Paramètres."));
    }
    let (email, username): (String, Option<String>) =
        sqlx::query_as("SELECT email, username FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    let password = temp_password();
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 1, failed_logins = 0, locked_until = NULL WHERE id = ?").bind(crate::auth::hash_password(&password)?).bind(id).execute(&st.db).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    log(
        &st,
        &u,
        "admin_mot_de_passe_réinitialisé",
        &username.clone().unwrap_or_else(|| email.clone()),
    )
    .await;
    Ok(Json(
        json!({ "username": username.unwrap_or(email), "password": password, "url": crate::mail::public_url() + "/connexion" }),
    ))
}

// ---------- Dépannage d'un compte (#38) ----------

/// Compte visé par une action de dépannage : verrou admin, jamais son propre compte. Renvoie (e-mail, nom).
async fn target(st: &AppState, u: &AuthUser, id: i64) -> Result<(String, String), AppError> {
    require(st, u).await?;
    if id == u.id {
        return Err(bad(
            "Vous ne pouvez pas faire cette action sur votre propre compte.",
        ));
    }
    sqlx::query_as("SELECT email, name FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)
}

/// Lien de nouveau mot de passe pour n'importe quel compte (2 heures, une seule fois) : envoyé par e-mail si le
/// SMTP est configuré, toujours renvoyé pour être copié.
pub async fn user_reset_link(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (email, name) = target(&st, &u, id).await?;
    let link = format!(
        "{}{}",
        crate::mail::public_url(),
        crate::account::new_reset(&st, id).await?
    );
    log(&st, &u, "admin_lien_mot_de_passe", &email).await;
    let mut mailed = false;
    if crate::mail::configured() {
        let first = name.split_whitespace().next().unwrap_or(&name);
        let body = format!("Bonjour {first},\n\nUn administrateur de POP vous envoie un lien pour choisir un nouveau mot de passe (valable 2 heures, une seule fois) :\n\n{link}\n\nSi vous n’avez rien demandé, ignorez ce message.\n\nL’équipe POP");
        mailed = crate::mail::send(&email, "POP · choisir un nouveau mot de passe", &body)
            .await
            .is_ok();
    }
    Ok(Json(
        json!({ "link": link, "mailed": mailed, "email": email }),
    ))
}

/// Lève le blocage après trop de mots de passe faux.
pub async fn unlock_user(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (email, _) = target(&st, &u, id).await?;
    sqlx::query("UPDATE users SET failed_logins = 0, locked_until = NULL WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    log(&st, &u, "admin_compte_débloqué", &email).await;
    Ok(Json(json!({ "ok": true })))
}

fn plan_json(p: &crate::manage::DeletionPlan) -> Value {
    json!({
        "delete": p.delete.iter().map(|d| d.1.clone()).collect::<Vec<_>>(),
        "transfer": p.transfer.iter().map(|t| json!({ "studio": t.1, "to": t.3 })).collect::<Vec<_>>(),
    })
}

/// Ce que deviendront les studios du compte s'il est supprimé (affiché avant la confirmation).
pub async fn deletion_preview(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (email, name) = target(&st, &u, id).await?;
    let plan = crate::manage::deletion_plan(&st.db, id).await?;
    let mut v = plan_json(&plan);
    v["email"] = json!(email);
    v["name"] = json!(name);
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct DeleteUserIn {
    confirm: String,
}

/// Supprime un compte : l'administrateur retape l'adresse e-mail. Les studios partagés dont il était seul
/// propriétaire sont transférés, les autres supprimés (même logique que la suppression par la personne).
pub async fn delete_user(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<DeleteUserIn>,
) -> ApiResult<Value> {
    let (email, _) = target(&st, &u, id).await?;
    if !b.confirm.trim().eq_ignore_ascii_case(&email) {
        return Err(bad("L’adresse tapée ne correspond pas à celle du compte."));
    }
    let plan = crate::manage::deletion_plan(&st.db, id).await?;
    let names = |v: Vec<String>| {
        if v.is_empty() {
            "aucun".to_string()
        } else {
            v.join(", ")
        }
    };
    let detail = format!(
        "{email} · studios supprimés : {} · transférés : {}",
        names(plan.delete.iter().map(|d| d.1.clone()).collect()),
        names(
            plan.transfer
                .iter()
                .map(|t| format!("{} à {}", t.1, t.3))
                .collect()
        ),
    );
    crate::manage::remove_account(&st, id, &plan).await?;
    log(&st, &u, "admin_compte_supprimé", &detail).await;
    Ok(Json(plan_json(&plan)))
}

#[cfg(test)]
mod tests {
    use super::take_digest;

    async fn db() -> sqlx::SqlitePool {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        db
    }
    async fn ask(db: &sqlx::SqlitePool, i: usize, age: &str) {
        sqlx::query(
            "INSERT INTO waitlist (email, name, created_at) VALUES (?, ?, datetime('now', ?))",
        )
        .bind(format!("p{i}@exemple.com"))
        .bind(format!("P{i}"))
        .bind(age)
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn vingt_demandes_un_seul_resume() {
        let db = db().await;
        for i in 0..20 {
            ask(&db, i, &format!("-{} minutes", 59 - i)).await
        }
        assert!(
            take_digest(&db).await.unwrap().is_empty(),
            "la plus ancienne n'attend pas encore depuis une heure"
        );
        sqlx::query("UPDATE waitlist SET created_at = datetime(created_at, '-2 minutes')")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(
            take_digest(&db).await.unwrap().len(),
            20,
            "un résumé couvre les 20 demandes"
        );
        assert!(
            take_digest(&db).await.unwrap().is_empty(),
            "rien n'est signalé deux fois"
        );
        ask(&db, 99, "-1 minutes").await;
        assert!(
            take_digest(&db).await.unwrap().is_empty(),
            "la demande suivante attend le prochain résumé"
        );
    }
}

#[cfg(test)]
mod depannage_tests {
    use super::*;
    use crate::{auth::hash_password, error::AppError};
    use axum::{
        extract::{Path, State},
        http::HeaderMap,
        Json,
    };
    use axum_extra::extract::cookie::CookieJar;

    const PW: &str = "Ancien-secret-du-compte-42";
    const NEW_PW: &str = "Nouveau-secret-tout-neuf-77";

    async fn user(st: &AppState, email: &str, admin: bool) -> AuthUser {
        // Adresse vérifiée : sans elle, un admin n'a pas ses droits (#11).
        let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, is_admin, email_verified_at) VALUES (?, ?, ?, ?, datetime('now')) RETURNING id")
            .bind(email).bind(email.split('@').next().unwrap()).bind(hash_password(PW).unwrap()).bind(admin as i64)
            .fetch_one(&st.db).await.unwrap();
        let sid = format!("session-{id}");
        // Session d'admin déverrouillée (mot de passe confirmé il y a moins de 30 minutes).
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at, sudo_until) VALUES (?, ?, datetime('now', '+1 day'), datetime('now', '+10 minutes'))")
            .bind(&sid).bind(id).execute(&st.db).await.unwrap();
        AuthUser {
            id,
            email: email.into(),
            name: email.into(),
            sid,
        }
    }
    async fn login(st: &AppState, email: &str, pw: &str) -> Result<Value, AppError> {
        let b = serde_json::from_value(json!({ "email": email, "password": pw })).unwrap();
        crate::auth::login(
            State(st.clone()),
            crate::security::ClientIp("203.0.113.9".into()),
            HeaderMap::new(),
            CookieJar::new(),
            Json(b),
        )
        .await
        .map(|(_, Json(v))| v)
    }
    async fn audited(st: &AppState, action: &str, by: i64) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_log WHERE action = ? AND user_id = ?",
        )
        .bind(action)
        .bind(by)
        .fetch_one(&st.db)
        .await
        .unwrap()
            > 0
    }
    /// Les quatre actions de dépannage, sur le compte `id`, par `u`.
    async fn all(st: &AppState, u: &AuthUser, id: i64) -> Vec<Result<Value, AppError>> {
        let s = || State(st.clone());
        vec![
            user_reset_link(s(), u.clone(), Path(id)).await.map(|j| j.0),
            unlock_user(s(), u.clone(), Path(id)).await.map(|j| j.0),
            deletion_preview(s(), u.clone(), Path(id))
                .await
                .map(|j| j.0),
            delete_user(
                s(),
                u.clone(),
                Path(id),
                Json(DeleteUserIn {
                    confirm: "x@exemple.com".into(),
                }),
            )
            .await
            .map(|j| j.0),
        ]
    }

    #[tokio::test]
    async fn droits_et_propre_compte() {
        let st = crate::test_state().await;
        let admin = user(&st, "admin@exemple.com", true).await;
        let member = user(&st, "x@exemple.com", false).await;
        for r in all(&st, &member, admin.id).await {
            assert!(
                matches!(r, Err(AppError::Forbidden)),
                "un non-admin est refusé : {r:?}"
            )
        }
        // Admin sans le verrou (confirmation récente expirée).
        sqlx::query("UPDATE sessions SET sudo_until = NULL WHERE token_hash = ?")
            .bind(&admin.sid)
            .execute(&st.db)
            .await
            .unwrap();
        for r in all(&st, &admin, member.id).await {
            assert!(
                matches!(r, Err(AppError::Guard("sudo", _))),
                "le verrou admin est exigé : {r:?}"
            )
        }
        sqlx::query(
            "UPDATE sessions SET sudo_until = datetime('now', '+10 minutes') WHERE token_hash = ?",
        )
        .bind(&admin.sid)
        .execute(&st.db)
        .await
        .unwrap();
        for r in all(&st, &admin, admin.id).await {
            assert!(
                matches!(r, Err(AppError::BadRequest(_))),
                "son propre compte est refusé : {r:?}"
            )
        }
        assert!(matches!(
            unlock_user(State(st.clone()), admin.clone(), Path(9999)).await,
            Err(AppError::NotFound)
        ));
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(n, 2, "rien n'a été supprimé");
    }

    #[tokio::test]
    async fn lien_de_nouveau_mot_de_passe_hors_studio_et_blocage_leve() {
        let st = crate::test_state().await;
        let admin = user(&st, "admin@exemple.com", true).await;
        let m = user(&st, "membre@exemple.com", false).await;
        // Le membre est dans un studio dont l'admin ne fait pas partie, et il est bloqué.
        sqlx::query("INSERT INTO studios (id, name) VALUES (1, 'Studio du membre')")
            .execute(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (1, ?, 'owner')")
            .bind(m.id)
            .execute(&st.db)
            .await
            .unwrap();
        sqlx::query("UPDATE users SET locked_until = datetime('now', '+15 minutes') WHERE id = ?")
            .bind(m.id)
            .execute(&st.db)
            .await
            .unwrap();
        assert!(
            matches!(
                login(&st, "membre@exemple.com", PW).await,
                Err(AppError::Conflict(_))
            ),
            "compte bloqué"
        );
        let r = user_reset_link(State(st.clone()), admin.clone(), Path(m.id))
            .await
            .unwrap()
            .0;
        let link = r["link"].as_str().unwrap();
        assert!(link.contains("/reinitialiser/"));
        assert!(audited(&st, "admin_lien_mot_de_passe", admin.id).await);
        let token = link.rsplit('/').next().unwrap();
        let b = serde_json::from_value(json!({ "token": token, "password": NEW_PW })).unwrap();
        let _ = crate::account::reset(State(st.clone()), Json(b))
            .await
            .unwrap();
        // Le blocage est levé : connexion immédiate avec le nouveau mot de passe.
        login(&st, "membre@exemple.com", NEW_PW).await.unwrap();
        // Le lien ne sert qu'une fois.
        let b = serde_json::from_value(
            json!({ "token": token, "password": "Encore-un-autre-secret-88" }),
        )
        .unwrap();
        assert!(crate::account::reset(State(st.clone()), Json(b))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn debloquer_apres_cinq_essais() {
        let st = crate::test_state().await;
        let admin = user(&st, "admin@exemple.com", true).await;
        let m = user(&st, "membre@exemple.com", false).await;
        for _ in 0..5 {
            let _ = login(&st, "membre@exemple.com", "Mauvais-secret-000").await;
        }
        assert!(
            matches!(
                login(&st, "membre@exemple.com", PW).await,
                Err(AppError::Conflict(_))
            ),
            "bloqué après 5 essais"
        );
        let list = users(State(st.clone()), admin.clone()).await.unwrap().0;
        let row = list
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == m.id)
            .unwrap()
            .clone();
        assert!(
            row["locked_until"].is_string(),
            "la liste affiche le blocage"
        );
        let _ = unlock_user(State(st.clone()), admin.clone(), Path(m.id))
            .await
            .unwrap();
        assert!(audited(&st, "admin_compte_débloqué", admin.id).await);
        login(&st, "membre@exemple.com", PW).await.unwrap();
        let list = users(State(st.clone()), admin.clone()).await.unwrap().0;
        assert!(list
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == m.id)
            .unwrap()["locked_until"]
            .is_null());
    }

    #[tokio::test]
    async fn supprimer_un_compte_transfere_ou_supprime_ses_studios() {
        let st = crate::test_state().await;
        let admin = user(&st, "admin@exemple.com", true).await;
        let m = user(&st, "membre@exemple.com", false).await;
        let other = user(&st, "autre@exemple.com", false).await;
        sqlx::query("INSERT INTO studios (id, name) VALUES (1, 'Seul'), (2, 'Partagé')")
            .execute(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (1, ?, 'owner'), (2, ?, 'owner'), (2, ?, 'member')")
            .bind(m.id).bind(m.id).bind(other.id).execute(&st.db).await.unwrap();
        let p = deletion_preview(State(st.clone()), admin.clone(), Path(m.id))
            .await
            .unwrap()
            .0;
        assert_eq!(p["delete"], json!(["Seul"]));
        assert_eq!(
            p["transfer"],
            json!([{ "studio": "Partagé", "to": "autre" }])
        );
        let wrong = delete_user(
            State(st.clone()),
            admin.clone(),
            Path(m.id),
            Json(DeleteUserIn {
                confirm: "autre@exemple.com".into(),
            }),
        )
        .await;
        assert!(
            matches!(wrong, Err(AppError::BadRequest(_))),
            "adresse à retaper"
        );
        let _ = delete_user(
            State(st.clone()),
            admin.clone(),
            Path(m.id),
            Json(DeleteUserIn {
                confirm: " Membre@Exemple.com ".into(),
            }),
        )
        .await
        .unwrap();
        assert!(
            matches!(
                login(&st, "membre@exemple.com", PW).await,
                Err(AppError::BadRequest(_))
            ),
            "plus de connexion possible"
        );
        let studios: Vec<String> = sqlx::query_scalar("SELECT name FROM studios ORDER BY id")
            .fetch_all(&st.db)
            .await
            .unwrap();
        assert_eq!(studios, vec!["Partagé".to_string()]);
        let role: String =
            sqlx::query_scalar("SELECT role FROM memberships WHERE studio_id = 2 AND user_id = ?")
                .bind(other.id)
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(role, "owner");
        assert!(audited(&st, "admin_compte_supprimé", admin.id).await);
    }
}
