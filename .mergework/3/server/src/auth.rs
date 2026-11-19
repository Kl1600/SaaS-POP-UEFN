use crate::{
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
    Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

pub const COOKIE: &str = "pop_session";
const SESSION_DAYS: i64 = 30;

#[derive(Clone, Debug, Serialize)]
pub struct AuthUser {
    pub id: i64,
    pub email: String,
    pub name: String,
    /// Empreinte du jeton de session (jamais envoyée au navigateur).
    #[serde(skip)]
    pub sid: String,
}

pub fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

pub fn random_token() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

pub fn hash_password(pw: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("argon2: {e}")))
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar.get(COOKIE).map(|c| c.value().to_string()).ok_or(AppError::Unauthorized)?;
        let u = user_from_token(state, &token).await?.ok_or(AppError::Unauthorized)?;
        // Dernière activité (au plus une écriture toutes les 5 minutes par session).
        let _ = sqlx::query("UPDATE sessions SET last_seen = datetime('now') WHERE token_hash = ? AND (last_seen IS NULL OR last_seen < datetime('now', '-5 minutes'))").bind(&u.sid).execute(&state.db).await;
        Ok(u)
    }
}

pub async fn user_from_token(state: &AppState, token: &str) -> Result<Option<AuthUser>, AppError> {
    let row: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT u.id, u.email, u.name FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ? AND s.expires_at > datetime('now') AND u.disabled = 0
           AND COALESCE(s.last_seen, s.created_at, datetime('now')) > datetime('now', '-14 days')",
    )
    .bind(sha256_hex(token))
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|(id, email, name)| AuthUser { id, email, name, sid: sha256_hex(token) }))
}

/// Limite simple : 10 tentatives par minute et par adresse e-mail.
pub fn rate_limit(state: &AppState, key: &str) -> Result<(), AppError> {
    if hit(state, key, 10, Duration::from_secs(60)) { Ok(()) } else { Err(AppError::TooMany) }
}

/// Compte une tentative pour `key` : `false` au-delà de `max` tentatives dans la fenêtre `window`.
pub fn hit(state: &AppState, key: &str, max: u32, window: Duration) -> bool {
    let mut m = state.limiter.lock().unwrap();
    let now = Instant::now();
    let e = m.entry(key.to_lowercase()).or_insert((0, now));
    if now.duration_since(e.1) > window {
        *e = (0, now);
    }
    e.0 += 1;
    e.0 <= max
}

pub async fn open_session(state: &AppState, jar: CookieJar, user_id: i64, ip: &str, ua: &str, mfa: bool) -> Result<CookieJar, AppError> {
    let token = random_token();
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at, ip, user_agent, created_at, last_seen, mfa) VALUES (?, ?, datetime('now', ?), ?, ?, datetime('now'), datetime('now'), ?)")
        .bind(sha256_hex(&token))
        .bind(user_id)
        .bind(format!("+{SESSION_DAYS} days"))
        .bind(ip)
        .bind(ua.chars().take(300).collect::<String>())
        .bind(mfa as i64)
        .execute(&state.db)
        .await?;
    let cookie = Cookie::build((COOKIE, token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.secure_cookies)
        .path("/")
        .max_age(time::Duration::days(SESSION_DAYS))
        .build();
    Ok(jar.add(cookie))
}

#[derive(Deserialize)]
pub struct RegisterIn {
    email: String,
    name: String,
    password: String,
    invite: Option<String>,
    access: Option<String>,
}

pub async fn register(State(st): State<AppState>, crate::security::ClientIp(ip): crate::security::ClientIp, headers: axum::http::HeaderMap, jar: CookieJar, Json(b): Json<RegisterIn>) -> Result<(CookieJar, Json<Value>), AppError> {
    rate_limit(&st, &b.email)?;
    rate_limit(&st, &format!("ip:reg:{ip}"))?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') || email.len() > 200 {
        return Err(bad("Adresse e-mail invalide."));
    }
    check_len("Nom", &b.name, 1, 80)?;
    crate::security::password_policy(&b.password, &email)?;
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE email = ?").bind(&email).fetch_optional(&st.db).await?;
    if exists.is_some() {
        return Err(AppError::Conflict("Un compte existe déjà avec cette adresse.".into()));
    }
    // Inscriptions sur liste d'attente : il faut une invitation de studio ou un lien d'accès validé.
    let wid = crate::admin::signup_allowed(&st, &email, &ip, b.invite.as_deref(), b.access.as_deref()).await.map_err(|e| match e {
        AppError::Forbidden => AppError::Conflict("Les inscriptions se font sur liste d’attente : demandez l’accès à la bêta depuis la page d’accueil.".into()),
        other => other,
    })?;
    let hash = hash_password(&b.password)?;
    // Jamais administrateur selon l'adresse IP : un en-tête ou un proxy mal réglé suffirait à se faire passer pour
    // l'ordinateur du serveur. Les administrateurs sont nommés par `pop-server admin ajouter <e-mail>` ou POP_ADMIN_EMAILS.
    let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES (?, ?, ?) RETURNING id")
        .bind(&email)
        .bind(b.name.trim())
        .bind(hash)
        .fetch_one(&st.db)
        .await?;
    if let Some(w) = wid { crate::admin::mark_registered(&st, w, id).await; }
    if let Some(tok) = b.invite.filter(|t| !t.is_empty()) {
        crate::studio::accept_invite_for(&st, id, &tok).await?;
    }
    crate::security::audit(&st, Some(id), "compte_créé", &email, &ip).await;
    let ua = headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("");
    let jar = open_session(&st, jar, id, &ip, ua, false).await?;
    Ok((jar, Json(json!({ "id": id, "email": email, "name": b.name.trim() }))))
}

#[derive(Deserialize)]
pub struct LoginIn {
    email: String,
    password: String,
}

/// Connexion. Verrouillage 15 minutes après 5 échecs, limite par adresse IP, compte de test réservé à cet ordinateur,
/// et second facteur (code à 6 chiffres) si la double authentification est activée.
pub async fn login(State(st): State<AppState>, crate::security::ClientIp(ip): crate::security::ClientIp, headers: axum::http::HeaderMap, jar: CookieJar, Json(b): Json<LoginIn>) -> Result<(CookieJar, Json<Value>), AppError> {
    rate_limit(&st, &b.email)?;
    rate_limit(&st, &format!("ip:{ip}"))?;
    let email = b.email.trim().to_lowercase();
    if email == "kl" && !crate::security::is_loopback(&ip) {
        return Err(bad("E-mail ou mot de passe incorrect."));
    }
    let row: Option<(i64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT id, name, password_hash, totp_enabled, COALESCE(locked_until > datetime('now'), 0) FROM users WHERE email = ? AND disabled = 0",
    ).bind(&email).fetch_optional(&st.db).await?;
    // Même message et même coût que l'adresse existe ou non.
    let (id, name, hash, totp, locked) = match row {
        Some(r) => r,
        None => {
            let _ = verify_password(&b.password, &st.dummy_hash);
            return Err(bad("E-mail ou mot de passe incorrect."));
        }
    };
    if locked == 1 {
        let _ = verify_password(&b.password, &st.dummy_hash);
        return Err(AppError::Conflict("Trop d’essais : ce compte est bloqué 15 minutes. Utilisez « Mot de passe oublié » si besoin.".into()));
    }
    if !verify_password(&b.password, &hash) {
        let n: i64 = sqlx::query_scalar("UPDATE users SET failed_logins = failed_logins + 1 WHERE id = ? RETURNING failed_logins").bind(id).fetch_one(&st.db).await?;
        if n >= 5 {
            sqlx::query("UPDATE users SET failed_logins = 0, locked_until = datetime('now', '+15 minutes') WHERE id = ?").bind(id).execute(&st.db).await?;
            crate::security::audit(&st, Some(id), "compte_bloqué", "5 mots de passe faux", &ip).await;
        } else {
            crate::security::audit(&st, Some(id), "connexion_échouée", "", &ip).await;
        }
        return Err(bad("E-mail ou mot de passe incorrect."));
    }
    sqlx::query("UPDATE users SET failed_logins = 0, locked_until = NULL WHERE id = ?").bind(id).execute(&st.db).await?;
    let ua = headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    if totp == 1 {
        let ticket = random_token();
        st.mfa_tickets.lock().unwrap().insert(sha256_hex(&ticket), (id, Instant::now(), 0));
        return Ok((jar, Json(json!({ "mfa": true, "ticket": ticket }))));
    }
    crate::security::audit(&st, Some(id), "connexion", "", &ip).await;
    let jar = open_session(&st, jar, id, &ip, &ua, false).await?;
    Ok((jar, Json(json!({ "id": id, "name": name }))))
}

#[derive(Deserialize)]
pub struct MfaIn {
    ticket: String,
    code: String,
}

/// Deuxième étape de connexion : code de l'application d'authentification ou code de secours.
pub async fn login_mfa(State(st): State<AppState>, crate::security::ClientIp(ip): crate::security::ClientIp, headers: axum::http::HeaderMap, jar: CookieJar, Json(b): Json<MfaIn>) -> Result<(CookieJar, Json<Value>), AppError> {
    rate_limit(&st, &format!("ip:{ip}"))?;
    let key = sha256_hex(&b.ticket);
    let uid = {
        let mut m = st.mfa_tickets.lock().unwrap();
        m.retain(|_, v| v.1.elapsed() < Duration::from_secs(300));
        match m.get_mut(&key) {
            Some(v) if v.2 < 5 => { v.2 += 1; v.0 }
            Some(_) => { m.remove(&key); return Err(bad("Trop d’essais : reconnectez-vous.")) }
            None => return Err(bad("Session de connexion expirée : reconnectez-vous.")),
        }
    };
    if !crate::security::check_second_factor(&st, uid, &b.code).await? {
        crate::security::audit(&st, Some(uid), "code_2fa_faux", "", &ip).await;
        return Err(bad("Code incorrect."));
    }
    st.mfa_tickets.lock().unwrap().remove(&key);
    let ua = headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("");
    crate::security::audit(&st, Some(uid), "connexion", "avec double authentification", &ip).await;
    let jar = open_session(&st, jar, uid, &ip, ua, true).await?;
    Ok((jar, Json(json!({ "id": uid }))))
}

pub async fn logout(State(st): State<AppState>, jar: CookieJar) -> Result<(CookieJar, Json<Value>), AppError> {
    if let Some(c) = jar.get(COOKIE) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?").bind(sha256_hex(c.value())).execute(&st.db).await?;
    }
    Ok((jar.remove(Cookie::build(COOKIE).path("/").build()), Json(json!({ "ok": true }))))
}

pub async fn me(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    // Chaque compte a d'office son studio : pas de formulaire à l'arrivée.
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memberships WHERE user_id = ?").bind(u.id).fetch_one(&st.db).await?;
    if n == 0 {
        let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES (?) RETURNING id").bind(format!("Studio de {}", u.name.split_whitespace().next().unwrap_or(&u.name))).fetch_one(&st.db).await?;
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')").bind(sid).bind(u.id).execute(&st.db).await?;
    }
    let credits: i64 = sqlx::query_scalar("SELECT credits FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    let _ = sqlx::query("UPDATE users SET last_seen = datetime('now') WHERE id = ?").bind(u.id).execute(&st.db).await;
    let admin = crate::admin::is_admin(&st, u.id).await?;
    let pending: i64 = if admin { sqlx::query_scalar("SELECT COUNT(*) FROM waitlist WHERE status = 'pending'").fetch_one(&st.db).await? } else { 0 };
    let studios: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT s.id, s.name, m.role FROM memberships m JOIN studios s ON s.id = m.studio_id WHERE m.user_id = ? ORDER BY s.id",
    )
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    let mut out = vec![];
    for (sid, sname, role) in studios {
        let maps: Vec<(i64, String, String, Option<String>, String)> =
            sqlx::query_as("SELECT id, name, genre, release_date, status FROM maps WHERE studio_id = ? ORDER BY id")
                .bind(sid)
                .fetch_all(&st.db)
                .await?;
        out.push(json!({
            "id": sid, "name": sname, "role": role,
            "maps": maps.into_iter().map(|(id, name, genre, rd, status)| json!({"id": id, "name": name, "genre": genre, "release_date": rd, "status": status})).collect::<Vec<_>>()
        }));
    }
    Ok(Json(json!({ "user": u, "credits": credits, "studios": out, "admin": admin, "admin_pending": pending })))
}
