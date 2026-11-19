//! Sécurité des comptes : adresse IP du client, journal d'audit, sessions actives,
//! mode « confirmation récente » (mot de passe) pour l'administration.
//! La double authentification a été retirée pendant la bêta (comptes créés par l'administrateur).
use crate::{
    auth::{verify_password, AuthUser},
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{ConnectInfo, FromRequestParts, Path, State},
    http::request::Parts,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;

// ---------- Adresse IP du client ----------

/// Où lire l'adresse IP du client (`POP_TRUST_PROXY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyMode {
    /// Absent ou `0` : l'adresse de la connexion. Ne jamais lire d'en-tête, le client les choisit.
    Direct,
    /// `1` : derrière un proxy (Caddy, Nginx…) : le **dernier** élément de `X-Forwarded-For`, celui que pose le
    /// proxy. Les éléments précédents viennent du client et peuvent être faux.
    Forwarded,
    /// `cloudflare` : l'en-tête `cf-connecting-ip`, posé par Cloudflare. Seulement si seul Cloudflare peut joindre le serveur.
    Cloudflare,
}

pub fn proxy_mode() -> ProxyMode {
    match std::env::var("POP_TRUST_PROXY")
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .as_str()
    {
        "1" | "true" | "oui" => ProxyMode::Forwarded,
        "cloudflare" => ProxyMode::Cloudflare,
        _ => ProxyMode::Direct,
    }
}

/// IP du client, d'après la connexion et le mode de proxy déclaré. Une valeur d'en-tête qui n'est pas une
/// adresse IP est ignorée (on retombe sur l'adresse de la connexion).
pub fn client_ip(
    headers: &axum::http::HeaderMap,
    peer: Option<std::net::IpAddr>,
    mode: ProxyMode,
) -> String {
    let parse = |s: &str| s.trim().parse::<std::net::IpAddr>().ok();
    let from_header = match mode {
        ProxyMode::Direct => None,
        // Plusieurs en-têtes X-Forwarded-For peuvent arriver : on les met bout à bout et on garde le dernier élément.
        ProxyMode::Forwarded => headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
            .last()
            .and_then(parse),
        ProxyMode::Cloudflare => headers
            .get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(parse),
    };
    from_header
        .or(peer)
        .map(|a| a.to_string())
        .unwrap_or_default()
}

/// IP du client (voir `client_ip` et `POP_TRUST_PROXY` dans le README).
pub struct ClientIp(pub String);
impl FromRequestParts<AppState> for ClientIp {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, Self::Rejection> {
        Ok(ClientIp(ip_of(parts)))
    }
}
pub fn ip_of(parts: &Parts) -> String {
    let peer = parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    client_ip(&parts.headers, peer, proxy_mode())
}
pub fn is_loopback(ip: &str) -> bool {
    ip.parse::<std::net::IpAddr>()
        .map(|a| a.is_loopback())
        .unwrap_or(false)
}

// ---------- Journal d'audit ----------

pub async fn audit(st: &AppState, uid: Option<i64>, action: &str, detail: &str, ip: &str) {
    let _ = sqlx::query("INSERT INTO audit_log (user_id, action, detail, ip) VALUES (?, ?, ?, ?)")
        .bind(uid)
        .bind(action)
        .bind(detail.chars().take(500).collect::<String>())
        .bind(ip)
        .execute(&st.db)
        .await;
}

// ---------- Réglages de sécurité du compte ----------

pub async fn overview(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    let rows: Vec<(String, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT token_hash, ip, user_agent, created_at, last_seen FROM sessions WHERE user_id = ? AND expires_at > datetime('now') ORDER BY COALESCE(last_seen, created_at) DESC",
    ).bind(u.id).fetch_all(&st.db).await?;
    let audit: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT action, detail, ip, at FROM audit_log WHERE user_id = ? ORDER BY id DESC LIMIT 20",
    )
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "admin": crate::admin::is_admin(&st, u.id).await?,
        "sessions": rows.into_iter().map(|r| json!({ "id": &r.0[..16], "ip": r.1, "device": device(&r.2), "created_at": r.3, "last_seen": r.4, "current": r.0 == u.sid })).collect::<Vec<_>>(),
        "events": audit.into_iter().map(|a| json!({ "action": a.0, "detail": a.1, "ip": a.2, "at": a.3 })).collect::<Vec<_>>(),
    })))
}
fn device(ua: &str) -> String {
    let os = ["Windows", "Mac OS", "iPhone", "iPad", "Android", "Linux"]
        .iter()
        .find(|o| ua.contains(*o))
        .map(|o| if *o == "Mac OS" { "Mac" } else { o })
        .unwrap_or("Appareil inconnu");
    let br = if ua.contains("Edg/") {
        "Edge"
    } else if ua.contains("Chrome/") {
        "Chrome"
    } else if ua.contains("Firefox/") {
        "Firefox"
    } else if ua.contains("Safari/") {
        "Safari"
    } else {
        ""
    };
    if br.is_empty() {
        os.to_string()
    } else {
        format!("{br} · {os}")
    }
}

pub async fn revoke_session(
    State(st): State<AppState>,
    u: AuthUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    if id.len() != 16 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::NotFound);
    }
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND substr(token_hash, 1, 16) = ? AND token_hash != ?").bind(u.id).bind(&id).bind(&u.sid).execute(&st.db).await?;
    audit(&st, Some(u.id), "session_fermée", &id, &ip).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn revoke_others(
    State(st): State<AppState>,
    u: AuthUser,
    ClientIp(ip): ClientIp,
) -> ApiResult<Value> {
    let n = sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash != ?")
        .bind(u.id)
        .bind(&u.sid)
        .execute(&st.db)
        .await?
        .rows_affected();
    audit(
        &st,
        Some(u.id),
        "sessions_fermées",
        &format!("{n} autre(s) appareil(s)"),
        &ip,
    )
    .await;
    Ok(Json(json!({ "closed": n })))
}

// ---------- Administration : confirmation récente du mot de passe ----------

/// Garde des routes d'administration. Refuse avec le code `sudo` (confirmer son mot de passe, valable 30 minutes),
/// que l'interface sait traiter.
pub async fn require_admin(st: &AppState, u: &AuthUser) -> Result<(), AppError> {
    if !crate::admin::is_admin(st, u.id).await? {
        return Err(AppError::Forbidden);
    }
    let sudo: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sudo_until > datetime('now'), 0) FROM sessions WHERE token_hash = ?",
    )
    .bind(&u.sid)
    .fetch_optional(&st.db)
    .await?
    .unwrap_or(0);
    if sudo == 0 {
        return Err(AppError::Guard(
            "sudo",
            "Confirmez votre mot de passe pour accéder à l’administration (valable 30 minutes)."
                .into(),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct UnlockIn {
    password: String,
}

pub async fn unlock_admin(
    State(st): State<AppState>,
    u: AuthUser,
    ClientIp(ip): ClientIp,
    Json(b): Json<UnlockIn>,
) -> ApiResult<Value> {
    crate::auth::rate_limit(&st, &format!("sudo:{}", u.id))?;
    if !crate::admin::is_admin(&st, u.id).await? {
        return Err(AppError::Forbidden);
    }
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(u.id)
        .fetch_one(&st.db)
        .await?;
    if !verify_password(&b.password, &hash) {
        audit(&st, Some(u.id), "admin_confirmation_échouée", "", &ip).await;
        return Err(bad("Mot de passe incorrect."));
    }
    sqlx::query(
        "UPDATE sessions SET sudo_until = datetime('now', '+30 minutes') WHERE token_hash = ?",
    )
    .bind(&u.sid)
    .execute(&st.db)
    .await?;
    audit(&st, Some(u.id), "admin_déverrouillé", "30 minutes", &ip).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn lock_admin(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    sqlx::query("UPDATE sessions SET sudo_until = NULL WHERE token_hash = ?")
        .bind(&u.sid)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn admin_audit(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    require_admin(&st, &u).await?;
    let rows: Vec<(String, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT a.action, a.detail, a.ip, a.at, u.name FROM audit_log a LEFT JOIN users u ON u.id = a.user_id ORDER BY a.id DESC LIMIT 300",
    ).fetch_all(&st.db).await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|r| json!({ "action": r.0, "detail": r.1, "ip": r.2, "at": r.3, "who": r.4 }))
        .collect::<Vec<_>>())))
}

#[cfg(test)]
mod tests {
    use super::{client_ip, ProxyMode};
    use axum::http::HeaderMap;
    use std::net::IpAddr;

    fn h(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.append(*k, v.parse().unwrap());
        }
        m
    }
    const PEER: &str = "203.0.113.9";
    fn peer() -> Option<IpAddr> {
        Some(PEER.parse().unwrap())
    }

    #[test]
    fn direct_ignore_les_en_tetes() {
        let hd = h(&[
            ("x-forwarded-for", "127.0.0.1"),
            ("cf-connecting-ip", "127.0.0.1"),
            ("x-real-ip", "127.0.0.1"),
        ]);
        assert_eq!(client_ip(&hd, peer(), ProxyMode::Direct), PEER);
    }

    #[test]
    fn proxy_garde_le_dernier_element() {
        // Le client a mis 127.0.0.1 en tête ; le proxy a ajouté la vraie adresse à la fin.
        let hd = h(&[("x-forwarded-for", "127.0.0.1, 198.51.100.7")]);
        assert_eq!(client_ip(&hd, peer(), ProxyMode::Forwarded), "198.51.100.7");
        // Deux en-têtes séparés : le dernier gagne aussi.
        let hd = h(&[
            ("x-forwarded-for", "127.0.0.1"),
            ("x-forwarded-for", "198.51.100.8"),
        ]);
        assert_eq!(client_ip(&hd, peer(), ProxyMode::Forwarded), "198.51.100.8");
    }

    #[test]
    fn proxy_sans_en_tete_ou_valeur_invalide() {
        assert_eq!(client_ip(&h(&[]), peer(), ProxyMode::Forwarded), PEER);
        assert_eq!(
            client_ip(
                &h(&[("x-forwarded-for", "pas-une-ip")]),
                peer(),
                ProxyMode::Forwarded
            ),
            PEER
        );
        // x-real-ip et cf-connecting-ip ne comptent pas en mode proxy générique.
        assert_eq!(
            client_ip(
                &h(&[
                    ("x-real-ip", "127.0.0.1"),
                    ("cf-connecting-ip", "127.0.0.1")
                ]),
                peer(),
                ProxyMode::Forwarded
            ),
            PEER
        );
    }

    #[test]
    fn cloudflare() {
        let hd = h(&[
            ("cf-connecting-ip", "2001:db8::1"),
            ("x-forwarded-for", "127.0.0.1"),
        ]);
        assert_eq!(client_ip(&hd, peer(), ProxyMode::Cloudflare), "2001:db8::1");
        assert_eq!(
            client_ip(
                &h(&[("x-forwarded-for", "127.0.0.1")]),
                peer(),
                ProxyMode::Cloudflare
            ),
            PEER
        );
    }
}

/// Mots de passe refusés : trop courts, trop communs, ou contenant l'identifiant.
pub fn password_policy(pw: &str, email: &str) -> Result<(), AppError> {
    let n = pw.chars().count();
    if n < 10 {
        return Err(bad("Le mot de passe doit faire au moins 10 caractères."));
    }
    if n > 200 {
        return Err(bad("Mot de passe trop long."));
    }
    let low = pw.to_lowercase();
    const COMMON: &[&str] = &[
        "password",
        "motdepasse",
        "123456",
        "azerty",
        "qwerty",
        "fortnite",
        "1234567890",
        "0123456789",
        "abcdef",
        "soleil",
        "bonjour",
        "iloveyou",
        "admin",
    ];
    if COMMON.iter().any(|c| low.contains(c)) && n < 16 {
        return Err(bad("Ce mot de passe est trop courant : ajoutez des mots ou des chiffres moins prévisibles."));
    }
    let first = low.chars().next().unwrap_or(' ');
    if low.chars().all(|c| c == first) {
        return Err(bad("Ce mot de passe est trop simple."));
    }
    let local = email.split('@').next().unwrap_or("").to_lowercase();
    if local.len() >= 3 && low.contains(&local) {
        return Err(bad(
            "Le mot de passe ne doit pas contenir votre adresse e-mail.",
        ));
    }
    Ok(())
}
