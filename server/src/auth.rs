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
/// En HTTPS, le cookie de session porte le préfixe `__Host-` : le navigateur exige alors Secure, Path=/ et aucun domaine,
/// ce qui empêche un sous-domaine (ou un réseau non chiffré) de le poser ou de l'écraser.
pub const SECURE_COOKIE: &str = "__Host-pop_session";
pub fn cookie_name(secure: bool) -> &'static str {
    if secure {
        SECURE_COOKIE
    } else {
        COOKIE
    }
}
/// Jeton de session lu dans les cookies (nom sécurisé d'abord).
pub fn session_token(jar: &CookieJar) -> Option<String> {
    jar.get(SECURE_COOKIE)
        .or_else(|| jar.get(COOKIE))
        .map(|c| c.value().to_string())
}
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

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = session_token(&jar).ok_or(AppError::Unauthorized)?;
        let u = user_from_token(state, &token)
            .await?
            .ok_or(AppError::Unauthorized)?;
        // Essai terminé sans abonnement : seuls le choix d'un abonnement, l'export et la déconnexion restent ouverts.
        if crate::stripe::configured()
            && !crate::stripe::allowed_when_expired(parts.uri.path())
            && crate::stripe::access_state(state, u.id).await? == "expired"
        {
            return Err(AppError::Guard(
                "abonnement",
                "Votre essai est terminé : choisissez Starter ou Pro pour continuer.".into(),
            ));
        }
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
    Ok(row.map(|(id, email, name)| AuthUser {
        id,
        email,
        name,
        sid: sha256_hex(token),
    }))
}

/// Limite simple : 10 tentatives par minute et par adresse e-mail.
pub fn rate_limit(state: &AppState, key: &str) -> Result<(), AppError> {
    if hit(state, key, 10, Duration::from_secs(60)) {
        Ok(())
    } else {
        Err(AppError::TooMany)
    }
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

pub async fn open_session(
    state: &AppState,
    jar: CookieJar,
    user_id: i64,
    ip: &str,
    ua: &str,
    mfa: bool,
) -> Result<CookieJar, AppError> {
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
    let cookie = Cookie::build((cookie_name(state.secure_cookies), token))
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
    #[serde(default)]
    accept_terms: bool,
}

pub async fn register(
    State(st): State<AppState>,
    crate::security::ClientIp(ip): crate::security::ClientIp,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Json(b): Json<RegisterIn>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    if !b.accept_terms {
        return Err(bad("Acceptez les conditions d’utilisation et la politique de confidentialité pour continuer."));
    }
    rate_limit(&st, &b.email)?;
    rate_limit(&st, &format!("ip:reg:{ip}"))?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') || email.len() > 200 {
        return Err(bad("Adresse e-mail invalide."));
    }
    check_len("Nom", &b.name, 1, 80)?;
    crate::security::password_policy(&b.password, &email)?;
    // Inscriptions sur liste d'attente : il faut une invitation de studio ou un lien d'accès validé.
    let wid = crate::admin::signup_allowed(&st, &email, &ip, b.invite.as_deref(), b.access.as_deref()).await.map_err(|e| match e {
        AppError::Forbidden => AppError::Conflict("Les inscriptions se font sur liste d’attente : demandez l’accès à la bêta depuis la page d’accueil.".into()),
        other => other,
    })?;
    // Après le contrôle d'accès (#17) : sans invitation ni lien d'accès, on ne peut pas tester qui a un compte.
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE email = ?")
        .bind(&email)
        .fetch_optional(&st.db)
        .await?;
    if exists.is_some() {
        return Err(AppError::Conflict(
            "Un compte existe déjà avec cette adresse.".into(),
        ));
    }
    let hash = hash_password(&b.password)?;
    // Jamais administrateur selon l'adresse IP : un en-tête ou un proxy mal réglé suffirait à se faire passer pour
    // l'ordinateur du serveur. Les administrateurs sont nommés par `pop-server admin ajouter <e-mail>` ou POP_ADMIN_EMAILS.
    let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, terms_at, plan, plan_source, trial_until) VALUES (?, ?, ?, datetime('now'), 'starter', 'trial', datetime('now', '+7 days')) RETURNING id")
        .bind(&email)
        .bind(b.name.trim())
        .bind(hash)
        .fetch_one(&st.db)
        .await?;
    if let Some(w) = wid {
        crate::admin::mark_registered(&st, w, id).await;
    }
    if let Some(tok) = b.invite.filter(|t| !t.is_empty()) {
        crate::studio::accept_invite_for(&st, id, &tok).await?;
    }
    crate::security::audit(&st, Some(id), "compte_créé", &email, &ip).await;
    // Lien de vérification de l'adresse (sans SMTP : compte vérifié d'office, voir verify.rs).
    let mailed = crate::verify::start(&st, id, &email).await?;
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let jar = open_session(&st, jar, id, &ip, ua, false).await?;
    Ok((
        jar,
        Json(
            json!({ "id": id, "email": email, "name": b.name.trim(), "email_verified": !mailed, "verification_sent": mailed }),
        ),
    ))
}

#[derive(Deserialize)]
pub struct LoginIn {
    /// Identifiant ou adresse e-mail.
    #[serde(alias = "login")]
    email: String,
    password: String,
}

/// Connexion par identifiant (ou e-mail) et mot de passe. Verrouillage 15 minutes après 5 échecs,
/// limite par adresse IP, compte de test réservé à cet ordinateur.
pub async fn login(
    State(st): State<AppState>,
    crate::security::ClientIp(ip): crate::security::ClientIp,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Json(b): Json<LoginIn>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    rate_limit(&st, &b.email)?;
    rate_limit(&st, &format!("ip:{ip}"))?;
    let email = b.email.trim().to_lowercase();
    if email == "kl" && !crate::security::is_loopback(&ip) {
        return Err(bad("Identifiant ou mot de passe incorrect."));
    }
    let row: Option<(i64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT id, name, password_hash, COALESCE(locked_until > datetime('now'), 0), activation_pending FROM users WHERE (email = ?1 OR username = ?1) AND disabled = 0",
    ).bind(&email).fetch_optional(&st.db).await?;
    // Même message et même coût que l'adresse existe ou non.
    let (id, name, hash, locked, pending) = match row {
        Some(r) => r,
        None => {
            let _ = verify_password(&b.password, &st.dummy_hash);
            return Err(bad("Identifiant ou mot de passe incorrect."));
        }
    };
    // Compte créé par l'admin, pas encore activé : il n'a pas de mot de passe connu (#42).
    if pending == 1 {
        let _ = verify_password(&b.password, &st.dummy_hash);
        return Err(AppError::Conflict(crate::activation::PENDING_MSG.into()));
    }
    if locked == 1 {
        let _ = verify_password(&b.password, &st.dummy_hash);
        return Err(AppError::Conflict("Trop d’essais : ce compte est bloqué 15 minutes. Utilisez « Mot de passe oublié » si besoin.".into()));
    }
    if !verify_password(&b.password, &hash) {
        let n: i64 = sqlx::query_scalar("UPDATE users SET failed_logins = failed_logins + 1 WHERE id = ? RETURNING failed_logins").bind(id).fetch_one(&st.db).await?;
        if n >= 5 {
            sqlx::query("UPDATE users SET failed_logins = 0, locked_until = datetime('now', '+15 minutes') WHERE id = ?").bind(id).execute(&st.db).await?;
            crate::security::audit(&st, Some(id), "compte_bloqué", "5 mots de passe faux", &ip)
                .await;
        } else {
            crate::security::audit(&st, Some(id), "connexion_échouée", "", &ip).await;
        }
        return Err(bad("Identifiant ou mot de passe incorrect."));
    }
    sqlx::query("UPDATE users SET failed_logins = 0, locked_until = NULL WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    crate::security::audit(&st, Some(id), "connexion", "", &ip).await;
    let jar = open_session(&st, jar, id, &ip, &ua, false).await?;
    Ok((jar, Json(json!({ "id": id, "name": name }))))
}

pub async fn logout(
    State(st): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, Json<Value>), AppError> {
    if let Some(c) = session_token(&jar) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(sha256_hex(&c))
            .execute(&st.db)
            .await?;
    }
    let jar = jar
        .remove(Cookie::build(COOKIE).path("/").build())
        .remove(Cookie::build(SECURE_COOKIE).path("/").secure(true).build());
    Ok((jar, Json(json!({ "ok": true }))))
}

pub async fn me(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    // Chaque compte a d'office son espace perso (maps faites seul) : pas de formulaire à l'arrivée.
    crate::studio::ensure_personal(&st, u.id).await?;
    let (credits, username, must_change): (i64, Option<String>, i64) =
        sqlx::query_as("SELECT credits, username, must_change_password FROM users WHERE id = ?")
            .bind(u.id)
            .fetch_one(&st.db)
            .await?;
    let _ = sqlx::query("UPDATE users SET last_seen = datetime('now') WHERE id = ?")
        .bind(u.id)
        .execute(&st.db)
        .await;
    let staff = crate::admin::staff(&st, u.id).await?;
    let admin = staff == crate::admin::Staff::Admin;
    let moderator = staff == crate::admin::Staff::Moderator;
    let pending: i64 = if staff > crate::admin::Staff::None {
        sqlx::query_scalar("SELECT COUNT(*) FROM waitlist WHERE status = 'pending'")
            .fetch_one(&st.db)
            .await?
    } else {
        0
    };
    let studios: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT s.id, s.name, m.role, s.personal FROM memberships m JOIN studios s ON s.id = m.studio_id WHERE m.user_id = ? ORDER BY s.personal DESC, s.id",
    )
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    let mut out = vec![];
    for (sid, sname, role, personal) in studios {
        let maps: Vec<(i64, String, String, Option<String>, String)> =
            sqlx::query_as("SELECT id, name, genre, release_date, status FROM maps WHERE studio_id = ? ORDER BY id")
                .bind(sid)
                .fetch_all(&st.db)
                .await?;
        let analyse = crate::billing::studio_analyse(&st, sid).await?;
        out.push(json!({
            "id": sid, "name": sname, "role": role, "personal": personal == 1, "analyse": analyse,
            "maps": maps.into_iter().map(|(id, name, genre, rd, status)| json!({"id": id, "name": name, "genre": genre, "release_date": rd, "status": status})).collect::<Vec<_>>()
        }));
    }
    let email_verified = crate::verify::is_verified(&st, u.id).await?;
    Ok(Json(
        json!({ "user": u, "credits": credits, "studios": out, "admin": admin, "moderator": moderator, "admin_pending": pending, "username": username, "must_change_password": must_change == 1, "email_verified": email_verified, "announcement": crate::announce::for_user(&st, u.id).await?,
        "plan": crate::billing::user_plan(&st, u.id).await?, "trial_until": crate::billing::trial_until(&st, u.id).await?, "billing_state": crate::stripe::access_state(&st, u.id).await?, "ai_credits": sqlx::query_scalar::<_, i64>("SELECT ai_credits FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?, "analyse": crate::billing::user_analyse(&st, u.id).await? }),
    ))
}
