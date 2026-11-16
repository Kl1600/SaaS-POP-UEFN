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
        user_from_token(state, &token).await?.ok_or(AppError::Unauthorized)
    }
}

pub async fn user_from_token(state: &AppState, token: &str) -> Result<Option<AuthUser>, AppError> {
    let row: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT u.id, u.email, u.name FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ? AND s.expires_at > datetime('now')",
    )
    .bind(sha256_hex(token))
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|(id, email, name)| AuthUser { id, email, name }))
}

/// Limite simple : 10 tentatives par minute et par adresse e-mail.
pub fn rate_limit(state: &AppState, key: &str) -> Result<(), AppError> {
    let mut m = state.limiter.lock().unwrap();
    let now = Instant::now();
    let e = m.entry(key.to_lowercase()).or_insert((0, now));
    if now.duration_since(e.1) > Duration::from_secs(60) {
        *e = (0, now);
    }
    e.0 += 1;
    if e.0 > 10 {
        return Err(AppError::TooMany);
    }
    Ok(())
}

async fn open_session(state: &AppState, jar: CookieJar, user_id: i64) -> Result<CookieJar, AppError> {
    let token = random_token();
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES (?, ?, datetime('now', ?))")
        .bind(sha256_hex(&token))
        .bind(user_id)
        .bind(format!("+{SESSION_DAYS} days"))
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
}

pub async fn register(State(st): State<AppState>, jar: CookieJar, Json(b): Json<RegisterIn>) -> Result<(CookieJar, Json<Value>), AppError> {
    rate_limit(&st, &b.email)?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') || email.len() > 200 {
        return Err(bad("Adresse e-mail invalide."));
    }
    check_len("Nom", &b.name, 1, 80)?;
    if b.password.chars().count() < 10 {
        return Err(bad("Le mot de passe doit faire au moins 10 caractères."));
    }
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE email = ?").bind(&email).fetch_optional(&st.db).await?;
    if exists.is_some() {
        return Err(AppError::Conflict("Un compte existe déjà avec cette adresse.".into()));
    }
    let hash = hash_password(&b.password)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES (?, ?, ?) RETURNING id")
        .bind(&email)
        .bind(b.name.trim())
        .bind(hash)
        .fetch_one(&st.db)
        .await?;
    if let Some(tok) = b.invite.filter(|t| !t.is_empty()) {
        crate::studio::accept_invite_for(&st, id, &tok).await?;
    }
    let jar = open_session(&st, jar, id).await?;
    Ok((jar, Json(json!({ "id": id, "email": email, "name": b.name.trim() }))))
}

#[derive(Deserialize)]
pub struct LoginIn {
    email: String,
    password: String,
}

pub async fn login(State(st): State<AppState>, jar: CookieJar, Json(b): Json<LoginIn>) -> Result<(CookieJar, Json<Value>), AppError> {
    rate_limit(&st, &b.email)?;
    let row: Option<(i64, String, String)> = sqlx::query_as("SELECT id, name, password_hash FROM users WHERE email = ?")
        .bind(b.email.trim().to_lowercase())
        .fetch_optional(&st.db)
        .await?;
    // Même message et même coût que l'adresse existe ou non.
    let (id, name, hash) = match row {
        Some(r) => r,
        None => {
            let _ = verify_password(&b.password, &st.dummy_hash);
            return Err(bad("E-mail ou mot de passe incorrect."));
        }
    };
    if !verify_password(&b.password, &hash) {
        return Err(bad("E-mail ou mot de passe incorrect."));
    }
    let jar = open_session(&st, jar, id).await?;
    Ok((jar, Json(json!({ "id": id, "name": name }))))
}

pub async fn logout(State(st): State<AppState>, jar: CookieJar) -> Result<(CookieJar, Json<Value>), AppError> {
    if let Some(c) = jar.get(COOKIE) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?").bind(sha256_hex(c.value())).execute(&st.db).await?;
    }
    Ok((jar.remove(Cookie::build(COOKIE).path("/").build()), Json(json!({ "ok": true }))))
}

pub async fn me(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    let credits: i64 = sqlx::query_scalar("SELECT credits FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
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
    Ok(Json(json!({ "user": u, "credits": credits, "studios": out })))
}
