//! Sécurité des comptes : double authentification (TOTP, RFC 6238), codes de secours, adresse IP du client,
//! journal d'audit, sessions actives, mode « confirmation récente » pour l'administration.
use crate::{
    auth::{random_token, sha256_hex, verify_password, AuthUser},
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{ConnectInfo, FromRequestParts, Path, State},
    http::request::Parts,
    Json,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;

// ---------- Adresse IP du client ----------

/// IP du client. Derrière un proxy de confiance (Cloudflare, Caddy…), mettre POP_TRUST_PROXY=1 pour lire X-Forwarded-For.
pub struct ClientIp(pub String);
impl FromRequestParts<AppState> for ClientIp {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        Ok(ClientIp(ip_of(parts)))
    }
}
pub fn ip_of(parts: &Parts) -> String {
    if std::env::var("POP_TRUST_PROXY").map(|v| v == "1").unwrap_or(false) {
        let h = parts.headers.get("cf-connecting-ip").or(parts.headers.get("x-real-ip")).and_then(|v| v.to_str().ok()).map(String::from)
            .or_else(|| parts.headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|v| v.split(',').next()).map(|v| v.trim().to_string()));
        if let Some(ip) = h.filter(|i| !i.is_empty() && i.len() < 64) { return ip }
    }
    parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip().to_string()).unwrap_or_default()
}
pub fn is_loopback(ip: &str) -> bool {
    ip.parse::<std::net::IpAddr>().map(|a| a.is_loopback()).unwrap_or(false)
}

// ---------- Journal d'audit ----------

pub async fn audit(st: &AppState, uid: Option<i64>, action: &str, detail: &str, ip: &str) {
    let _ = sqlx::query("INSERT INTO audit_log (user_id, action, detail, ip) VALUES (?, ?, ?, ?)").bind(uid).bind(action).bind(detail.chars().take(500).collect::<String>()).bind(ip).execute(&st.db).await;
}

// ---------- TOTP ----------

fn totp_at(secret: &[u8], counter: u64) -> u32 {
    let mut mac = Hmac::<sha1::Sha1>::new_from_slice(secret).expect("clé HMAC");
    mac.update(&counter.to_be_bytes());
    let h = mac.finalize().into_bytes();
    let o = (h[19] & 0x0f) as usize;
    let bin = ((h[o] as u32 & 0x7f) << 24) | ((h[o + 1] as u32) << 16) | ((h[o + 2] as u32) << 8) | h[o + 3] as u32;
    bin % 1_000_000
}
/// Vérifie un code à 6 chiffres, avec une tolérance de ±30 s pour les horloges décalées.
pub fn totp_ok(secret_b32: &str, code: &str) -> bool {
    let code: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    if code.len() != 6 { return false }
    let Ok(secret) = data_encoding::BASE32_NOPAD.decode(secret_b32.as_bytes()) else { return false };
    let Ok(want) = code.parse::<u32>() else { return false };
    let now = chrono::Utc::now().timestamp() as u64 / 30;
    (now.saturating_sub(1)..=now + 1).any(|c| totp_at(&secret, c) == want)
}
fn new_secret() -> String {
    use rand::RngCore;
    let mut b = [0u8; 20];
    rand::thread_rng().fill_bytes(&mut b);
    data_encoding::BASE32_NOPAD.encode(&b)
}

/// Vérifie un code de double authentification ou un code de secours (consommé s'il est utilisé).
pub async fn check_second_factor(st: &AppState, uid: i64, code: &str) -> Result<bool, AppError> {
    let (secret, recovery): (Option<String>, String) = sqlx::query_as("SELECT totp_secret, recovery FROM users WHERE id = ? AND totp_enabled = 1").bind(uid).fetch_optional(&st.db).await?.unwrap_or((None, "[]".into()));
    if let Some(s) = &secret { if totp_ok(s, code) { return Ok(true) } }
    let norm: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    if norm.len() >= 10 {
        let mut list: Vec<String> = serde_json::from_str(&recovery).unwrap_or_default();
        let h = sha256_hex(&norm);
        if let Some(i) = list.iter().position(|x| *x == h) {
            list.remove(i);
            sqlx::query("UPDATE users SET recovery = ? WHERE id = ?").bind(json!(list).to_string()).bind(uid).execute(&st.db).await?;
            return Ok(true);
        }
    }
    Ok(false)
}

// ---------- Réglages de sécurité du compte ----------

pub async fn overview(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    let (enabled, recovery): (i64, String) = sqlx::query_as("SELECT totp_enabled, recovery FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    let rows: Vec<(String, String, String, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT token_hash, ip, user_agent, created_at, last_seen, mfa FROM sessions WHERE user_id = ? AND expires_at > datetime('now') ORDER BY COALESCE(last_seen, created_at) DESC",
    ).bind(u.id).fetch_all(&st.db).await?;
    let recovery_left = serde_json::from_str::<Vec<String>>(&recovery).map(|v| v.len()).unwrap_or(0);
    let audit: Vec<(String, String, String, String)> = sqlx::query_as("SELECT action, detail, ip, at FROM audit_log WHERE user_id = ? ORDER BY id DESC LIMIT 20").bind(u.id).fetch_all(&st.db).await?;
    Ok(Json(json!({
        "totp": enabled == 1, "recovery_left": recovery_left, "admin": crate::admin::is_admin(&st, u.id).await?,
        "sessions": rows.into_iter().map(|r| json!({ "id": &r.0[..16], "ip": r.1, "device": device(&r.2), "created_at": r.3, "last_seen": r.4, "mfa": r.5 == 1, "current": r.0 == u.sid })).collect::<Vec<_>>(),
        "events": audit.into_iter().map(|a| json!({ "action": a.0, "detail": a.1, "ip": a.2, "at": a.3 })).collect::<Vec<_>>(),
    })))
}
fn device(ua: &str) -> String {
    let os = ["Windows", "Mac OS", "iPhone", "iPad", "Android", "Linux"].iter().find(|o| ua.contains(*o)).map(|o| if *o == "Mac OS" { "Mac" } else { o }).unwrap_or("Appareil inconnu");
    let br = if ua.contains("Edg/") { "Edge" } else if ua.contains("Chrome/") { "Chrome" } else if ua.contains("Firefox/") { "Firefox" } else if ua.contains("Safari/") { "Safari" } else { "" };
    if br.is_empty() { os.to_string() } else { format!("{br} · {os}") }
}

pub async fn setup_2fa(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    let enabled: i64 = sqlx::query_scalar("SELECT totp_enabled FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    if enabled == 1 { return Err(bad("La double authentification est déjà activée.")) }
    let secret = new_secret();
    sqlx::query("UPDATE users SET totp_secret = ? WHERE id = ?").bind(&secret).bind(u.id).execute(&st.db).await?;
    let label = format!("POP:{}", u.email).replace(' ', "%20");
    Ok(Json(json!({ "secret": secret, "uri": format!("otpauth://totp/{label}?secret={secret}&issuer=POP&algorithm=SHA1&digits=6&period=30") })))
}

#[derive(Deserialize)]
pub struct CodeIn { code: String, password: Option<String> }

pub async fn enable_2fa(State(st): State<AppState>, u: AuthUser, ClientIp(ip): ClientIp, Json(b): Json<CodeIn>) -> ApiResult<Value> {
    let secret: Option<String> = sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = ? AND totp_enabled = 0").bind(u.id).fetch_one(&st.db).await?;
    let secret = secret.ok_or_else(|| bad("Recommencez l’activation."))?;
    if !totp_ok(&secret, &b.code) { return Err(bad("Code incorrect : vérifiez l’heure de votre téléphone et réessayez.")) }
    // 10 codes de secours, montrés une seule fois.
    let codes: Vec<String> = (0..10).map(|_| { let t = random_token(); format!("{}-{}", &t[..5], &t[5..10]) }).collect();
    let hashes: Vec<String> = codes.iter().map(|c| sha256_hex(&c.replace('-', ""))).collect();
    sqlx::query("UPDATE users SET totp_enabled = 1, recovery = ? WHERE id = ?").bind(json!(hashes).to_string()).bind(u.id).execute(&st.db).await?;
    sqlx::query("UPDATE sessions SET mfa = 1 WHERE token_hash = ?").bind(&u.sid).execute(&st.db).await?;
    // Les autres appareils devront se reconnecter avec le code.
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash != ?").bind(u.id).bind(&u.sid).execute(&st.db).await?;
    audit(&st, Some(u.id), "2fa_activée", "", &ip).await;
    Ok(Json(json!({ "recovery": codes })))
}

pub async fn disable_2fa(State(st): State<AppState>, u: AuthUser, ClientIp(ip): ClientIp, Json(b): Json<CodeIn>) -> ApiResult<Value> {
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    if !verify_password(b.password.as_deref().unwrap_or(""), &hash) { return Err(bad("Mot de passe incorrect.")) }
    if !check_second_factor(&st, u.id, &b.code).await? { return Err(bad("Code incorrect.")) }
    sqlx::query("UPDATE users SET totp_enabled = 0, totp_secret = NULL, recovery = '[]' WHERE id = ?").bind(u.id).execute(&st.db).await?;
    sqlx::query("UPDATE sessions SET mfa = 0, sudo_until = NULL WHERE user_id = ?").bind(u.id).execute(&st.db).await?;
    audit(&st, Some(u.id), "2fa_désactivée", "", &ip).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn revoke_session(State(st): State<AppState>, u: AuthUser, ClientIp(ip): ClientIp, Path(id): Path<String>) -> ApiResult<Value> {
    if id.len() != 16 || !id.chars().all(|c| c.is_ascii_hexdigit()) { return Err(AppError::NotFound) }
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND substr(token_hash, 1, 16) = ? AND token_hash != ?").bind(u.id).bind(&id).bind(&u.sid).execute(&st.db).await?;
    audit(&st, Some(u.id), "session_fermée", &id, &ip).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn revoke_others(State(st): State<AppState>, u: AuthUser, ClientIp(ip): ClientIp) -> ApiResult<Value> {
    let n = sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash != ?").bind(u.id).bind(&u.sid).execute(&st.db).await?.rows_affected();
    audit(&st, Some(u.id), "sessions_fermées", &format!("{n} autre(s) appareil(s)"), &ip).await;
    Ok(Json(json!({ "closed": n })))
}

// ---------- Administration : double authentification obligatoire + confirmation récente ----------

/// Garde des routes d'administration. Refuse avec un code que l'interface sait traiter :
/// `mfa_setup` (activer la double authentification), `mfa_session` (se reconnecter avec le code), `sudo` (confirmer son identité).
pub async fn require_admin(st: &AppState, u: &AuthUser) -> Result<(), AppError> {
    if !crate::admin::is_admin(st, u.id).await? { return Err(AppError::Forbidden) }
    let enabled: i64 = sqlx::query_scalar("SELECT totp_enabled FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    if enabled == 0 { return Err(AppError::Guard("mfa_setup", "L’administration exige la double authentification : activez-la dans Paramètres → Sécurité.".into())) }
    let (mfa, sudo): (i64, i64) = sqlx::query_as("SELECT mfa, COALESCE(sudo_until > datetime('now'), 0) FROM sessions WHERE token_hash = ?").bind(&u.sid).fetch_optional(&st.db).await?.unwrap_or((0, 0));
    if mfa == 0 { return Err(AppError::Guard("mfa_session", "Reconnectez-vous avec votre code de double authentification pour accéder à l’administration.".into())) }
    if sudo == 0 { return Err(AppError::Guard("sudo", "Confirmez votre identité pour accéder à l’administration (valable 30 minutes).".into())) }
    Ok(())
}

#[derive(Deserialize)]
pub struct UnlockIn { password: String, code: String }

pub async fn unlock_admin(State(st): State<AppState>, u: AuthUser, ClientIp(ip): ClientIp, Json(b): Json<UnlockIn>) -> ApiResult<Value> {
    crate::auth::rate_limit(&st, &format!("sudo:{}", u.id))?;
    if !crate::admin::is_admin(&st, u.id).await? { return Err(AppError::Forbidden) }
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    if !verify_password(&b.password, &hash) || !check_second_factor(&st, u.id, &b.code).await? {
        audit(&st, Some(u.id), "admin_confirmation_échouée", "", &ip).await;
        return Err(bad("Mot de passe ou code incorrect."));
    }
    sqlx::query("UPDATE sessions SET sudo_until = datetime('now', '+30 minutes'), mfa = 1 WHERE token_hash = ?").bind(&u.sid).execute(&st.db).await?;
    audit(&st, Some(u.id), "admin_déverrouillé", "30 minutes", &ip).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn lock_admin(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    sqlx::query("UPDATE sessions SET sudo_until = NULL WHERE token_hash = ?").bind(&u.sid).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn admin_audit(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    require_admin(&st, &u).await?;
    let rows: Vec<(String, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT a.action, a.detail, a.ip, a.at, u.name FROM audit_log a LEFT JOIN users u ON u.id = a.user_id ORDER BY a.id DESC LIMIT 300",
    ).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|r| json!({ "action": r.0, "detail": r.1, "ip": r.2, "at": r.3, "who": r.4 })).collect::<Vec<_>>())))
}

/// Mots de passe refusés : trop courts, trop communs, ou contenant l'identifiant.
pub fn password_policy(pw: &str, email: &str) -> Result<(), AppError> {
    let n = pw.chars().count();
    if n < 10 { return Err(bad("Le mot de passe doit faire au moins 10 caractères.")) }
    if n > 200 { return Err(bad("Mot de passe trop long.")) }
    let low = pw.to_lowercase();
    const COMMON: &[&str] = &["password", "motdepasse", "123456", "azerty", "qwerty", "fortnite", "1234567890", "0123456789", "abcdef", "soleil", "bonjour", "iloveyou", "admin"];
    if COMMON.iter().any(|c| low.contains(c)) && n < 16 { return Err(bad("Ce mot de passe est trop courant : ajoutez des mots ou des chiffres moins prévisibles.")) }
    let first = low.chars().next().unwrap_or(' ');
    if low.chars().all(|c| c == first) { return Err(bad("Ce mot de passe est trop simple.")) }
    let local = email.split('@').next().unwrap_or("").to_lowercase();
    if local.len() >= 3 && low.contains(&local) { return Err(bad("Le mot de passe ne doit pas contenir votre adresse e-mail.")) }
    Ok(())
}
