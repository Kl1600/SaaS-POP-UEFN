//! Activation des comptes créés par l'administrateur (#42).
//!
//! L'admin crée le compte sans mot de passe ; la personne reçoit un lien (7 jours, à usage unique, propre à son
//! compte), choisit son mot de passe et arrive connectée dans POP. Le lien arrive à son adresse : l'ouvrir vérifie
//! donc l'adresse (#11). Sans SMTP, l'admin reçoit le lien à copier. Tant que le compte n'est pas activé, la
//! connexion est refusée.
use crate::{
    auth::{hash_password, random_token, sha256_hex, AuthUser},
    error::{bad, ApiResult, AppError},
    mail, AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use serde_json::{json, Value};

pub const VALID_DAYS: i64 = 7;
pub const PENDING_MSG: &str = "Activez votre compte avec le lien reçu par e-mail.";

/// Nouveau lien d'activation pour `uid` : les liens précédents cessent de marcher. Envoie l'e-mail si le SMTP est
/// configuré. Renvoie `{ mailed, link? }` : le lien n'est donné à l'admin que sans SMTP.
pub async fn issue(st: &AppState, uid: i64, waitlist_id: Option<i64>) -> Result<Value, AppError> {
    let (email, name, username): (String, String, Option<String>) =
        sqlx::query_as("SELECT email, name, username FROM users WHERE id = ?")
            .bind(uid)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    // Un lien renvoyé garde la demande de liste d'attente liée au premier.
    let waitlist_id = match waitlist_id {
        Some(w) => Some(w),
        None => sqlx::query_scalar("SELECT waitlist_id FROM account_activations WHERE user_id = ? AND waitlist_id IS NOT NULL LIMIT 1")
            .bind(uid)
            .fetch_optional(&st.db)
            .await?,
    };
    sqlx::query("DELETE FROM account_activations WHERE user_id = ?")
        .bind(uid)
        .execute(&st.db)
        .await?;
    let token = random_token();
    sqlx::query("INSERT INTO account_activations (token_hash, user_id, waitlist_id, expires_at) VALUES (?, ?, ?, datetime('now', ?))")
        .bind(sha256_hex(&token))
        .bind(uid)
        .bind(waitlist_id)
        .bind(format!("+{VALID_DAYS} days"))
        .execute(&st.db)
        .await?;
    let link = format!("{}/activer?token={token}", mail::public_url());
    if !mail::configured() {
        tracing::warn!("SMTP non configuré : le lien d'activation de {email} est donné à l'administrateur, à transmettre à la main.");
        return Ok(json!({ "mailed": false, "link": link, "email": email }));
    }
    let first = name.split_whitespace().next().unwrap_or("").to_string();
    let login = username.unwrap_or_else(|| email.clone());
    let body = format!(
        "Bonjour {first},\n\nVotre compte POP est prêt. Votre identifiant : {login}\n\nPour l'activer et choisir votre mot de passe, ouvrez ce lien (valable {VALID_DAYS} jours, une seule fois) :\n{link}\n\nSi vous n'attendiez pas ce message, ignorez-le.\n\nÀ très vite,\nL’équipe POP"
    );
    let to = email.clone();
    tokio::spawn(async move {
        if let Err(e) = mail::send(&to, "POP · activez votre compte", &body).await {
            tracing::warn!("e-mail d'activation non envoyé à {to} : {e:#}");
        }
    });
    Ok(json!({ "mailed": true, "email": email }))
}

async fn pending_by_token(
    st: &AppState,
    token: &str,
) -> Result<(i64, String, String, Option<String>, Option<i64>), AppError> {
    sqlx::query_as(
        "SELECT u.id, u.email, u.name, u.username, a.waitlist_id FROM account_activations a JOIN users u ON u.id = a.user_id
         WHERE a.token_hash = ? AND a.used_at IS NULL AND a.expires_at > datetime('now') AND u.activation_pending = 1 AND u.disabled = 0",
    )
    .bind(sha256_hex(token.trim()))
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| bad("Lien d’activation invalide, déjà utilisé ou expiré : demandez un nouveau lien à l’administrateur."))
}

#[derive(Deserialize)]
pub struct TokenQ {
    token: String,
}

/// Page d'activation : rappelle l'identifiant. Pas besoin d'être connecté.
pub async fn check(
    State(st): State<AppState>,
    crate::security::ClientIp(ip): crate::security::ClientIp,
    Query(q): Query<TokenQ>,
) -> ApiResult<Value> {
    crate::auth::rate_limit(&st, &format!("ip:activation:{ip}"))?;
    let (_, email, name, username, _) = pending_by_token(&st, &q.token).await?;
    Ok(Json(
        json!({ "email": email, "name": name, "username": username }),
    ))
}

#[derive(Deserialize)]
pub struct ActivateIn {
    token: String,
    password: String,
    #[serde(default)]
    accept_terms: bool,
}

/// La personne choisit son mot de passe : compte activé, adresse vérifiée, connectée.
pub async fn activate(
    State(st): State<AppState>,
    crate::security::ClientIp(ip): crate::security::ClientIp,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Json(b): Json<ActivateIn>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    crate::auth::rate_limit(&st, &format!("ip:activation:{ip}"))?;
    let (uid, email, name, username, waitlist_id) = pending_by_token(&st, &b.token).await?;
    if !b.accept_terms {
        return Err(bad("Acceptez les conditions d’utilisation et la politique de confidentialité pour continuer."));
    }
    crate::security::password_policy(&b.password, &email)?;
    // Le lien ne sert qu'une fois, même si deux onglets l'envoient en même temps.
    let used = sqlx::query("UPDATE account_activations SET used_at = datetime('now') WHERE token_hash = ? AND used_at IS NULL")
        .bind(sha256_hex(b.token.trim()))
        .execute(&st.db)
        .await?
        .rows_affected();
    if used == 0 {
        return Err(bad("Lien d’activation invalide, déjà utilisé ou expiré : demandez un nouveau lien à l’administrateur."));
    }
    sqlx::query("UPDATE users SET password_hash = ?, activation_pending = 0, must_change_password = 0, terms_at = datetime('now'), failed_logins = 0, locked_until = NULL WHERE id = ?")
        .bind(hash_password(&b.password)?)
        .bind(uid)
        .execute(&st.db)
        .await?;
    sqlx::query("DELETE FROM account_activations WHERE user_id = ?")
        .bind(uid)
        .execute(&st.db)
        .await?;
    crate::verify::mark_verified(&st, uid).await?;
    if let Some(w) = waitlist_id {
        crate::admin::mark_registered(&st, w, uid).await;
    }
    crate::security::audit(
        &st,
        Some(uid),
        "compte_activé",
        username.as_deref().unwrap_or(&email),
        &ip,
    )
    .await;
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let jar = crate::auth::open_session(&st, jar, uid, &ip, &ua, false).await?;
    Ok((jar, Json(json!({ "ok": true, "id": uid, "name": name }))))
}

/// Administration → « Renvoyer le lien » : nouveau lien, l'ancien cesse de marcher.
pub async fn resend(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    let pending: Option<i64> =
        sqlx::query_scalar("SELECT activation_pending FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&st.db)
            .await?;
    match pending {
        None => return Err(AppError::NotFound),
        Some(0) => return Err(bad("Ce compte est déjà activé.")),
        _ => {}
    }
    let r = issue(&st, id, None).await?;
    let email = r["email"].as_str().unwrap_or("").to_string();
    crate::security::audit(&st, Some(u.id), "admin_lien_activation_renvoyé", &email, "").await;
    Ok(Json(r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthUser;

    const PW: &str = "Mon-secret-tout-neuf-2026";

    async fn admin(st: &AppState) -> AuthUser {
        let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, is_admin, email_verified_at) VALUES ('chef@exemple.com', 'Chef', 'x', 1, datetime('now')) RETURNING id")
            .fetch_one(&st.db).await.unwrap();
        let sid = format!("session-{id}");
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at, sudo_until) VALUES (?, ?, datetime('now', '+1 day'), datetime('now', '+10 minutes'))")
            .bind(&sid).bind(id).execute(&st.db).await.unwrap();
        AuthUser {
            id,
            email: "chef@exemple.com".into(),
            name: "Chef".into(),
            sid,
        }
    }
    async fn create(st: &AppState, a: &AuthUser, waitlist_id: Option<i64>) -> (i64, String) {
        let b = serde_json::from_value(json!({ "username": "nouveau", "name": "Nouveau Venu", "email": "nouveau@exemple.com", "plan": "pro", "waitlist_id": waitlist_id })).unwrap();
        let Json(r) = crate::admin::create_account(State(st.clone()), a.clone(), Json(b))
            .await
            .unwrap();
        assert!(
            r.get("password").is_none(),
            "aucun mot de passe dans la réponse : {r}"
        );
        let link = r["link"]
            .as_str()
            .expect("sans SMTP, le lien est donné à l'admin")
            .to_string();
        (
            r["id"].as_i64().unwrap(),
            link.split("token=").nth(1).unwrap().to_string(),
        )
    }
    async fn login(st: &AppState, who: &str, pw: &str) -> Result<Value, AppError> {
        let b = serde_json::from_value(json!({ "email": who, "password": pw })).unwrap();
        crate::auth::login(
            State(st.clone()),
            crate::security::ClientIp("203.0.113.7".into()),
            axum::http::HeaderMap::new(),
            CookieJar::new(),
            Json(b),
        )
        .await
        .map(|(_, Json(v))| v)
    }
    async fn act(st: &AppState, token: &str, pw: &str) -> Result<Value, AppError> {
        let b =
            serde_json::from_value(json!({ "token": token, "password": pw, "accept_terms": true }))
                .unwrap();
        activate(
            State(st.clone()),
            crate::security::ClientIp("203.0.113.7".into()),
            axum::http::HeaderMap::new(),
            CookieJar::new(),
            Json(b),
        )
        .await
        .map(|(_, Json(v))| v)
    }
    async fn col(st: &AppState, q: &str, id: i64) -> Option<String> {
        sqlx::query_scalar(q)
            .bind(id)
            .fetch_one(&st.db)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn creer_puis_activer() {
        let st = crate::test_state().await;
        let a = admin(&st).await;
        sqlx::query("INSERT INTO waitlist (id, email, name) VALUES (7, 'nouveau@exemple.com', 'Nouveau Venu')").execute(&st.db).await.unwrap();
        let (id, token) = create(&st, &a, Some(7)).await;
        assert_eq!(
            col(&st, "SELECT status FROM waitlist WHERE id = ?", 7)
                .await
                .as_deref(),
            Some("approved"),
            "pas « inscrit » à la création"
        );
        // Avant activation : connexion refusée avec le message prévu, compte non vérifié.
        let e = login(&st, "nouveau", "n-importe-quoi").await.unwrap_err();
        assert!(
            matches!(&e, AppError::Conflict(m) if m == PENDING_MSG),
            "{e:?}"
        );
        assert!(
            col(&st, "SELECT email_verified_at FROM users WHERE id = ?", id)
                .await
                .is_none()
        );
        let Json(c) = check(
            State(st.clone()),
            crate::security::ClientIp("203.0.113.7".into()),
            Query(TokenQ {
                token: token.clone(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(c["username"], "nouveau");
        // Activation : mot de passe choisi, adresse vérifiée, demande inscrite, connexion possible.
        act(&st, &token, PW).await.unwrap();
        assert!(
            col(&st, "SELECT email_verified_at FROM users WHERE id = ?", id)
                .await
                .is_some()
        );
        assert_eq!(
            col(&st, "SELECT status FROM waitlist WHERE id = ?", 7)
                .await
                .as_deref(),
            Some("registered")
        );
        login(&st, "nouveau", PW).await.unwrap();
        // Le lien ne sert qu'une fois.
        assert!(act(&st, &token, "Autre-secret-encore-2026").await.is_err());
        let actions: Vec<String> = sqlx::query_scalar("SELECT action FROM audit_log ORDER BY id")
            .fetch_all(&st.db)
            .await
            .unwrap();
        assert!(
            actions.contains(&"admin_compte_créé".to_string())
                && actions.contains(&"compte_activé".to_string()),
            "{actions:?}"
        );
    }

    #[tokio::test]
    async fn lien_renvoye_expire_et_propre_au_compte() {
        let st = crate::test_state().await;
        let a = admin(&st).await;
        let (id, old) = create(&st, &a, None).await;
        let Json(r) = resend(State(st.clone()), a.clone(), Path(id))
            .await
            .unwrap();
        let new = r["link"]
            .as_str()
            .unwrap()
            .split("token=")
            .nth(1)
            .unwrap()
            .to_string();
        assert!(
            act(&st, &old, PW).await.is_err(),
            "un lien renvoyé rend l'ancien inutilisable"
        );
        // Expiré après 7 jours.
        sqlx::query("UPDATE account_activations SET expires_at = datetime('now', '-1 minute') WHERE user_id = ?").bind(id).execute(&st.db).await.unwrap();
        assert!(act(&st, &new, PW).await.is_err(), "lien expiré refusé");
        // Un lien ne marche que pour son compte : un compte déjà actif ne s'active pas avec un lien d'un autre.
        let Json(r) = resend(State(st.clone()), a.clone(), Path(id))
            .await
            .unwrap();
        let fresh = r["link"]
            .as_str()
            .unwrap()
            .split("token=")
            .nth(1)
            .unwrap()
            .to_string();
        act(&st, &fresh, PW).await.unwrap();
        assert!(
            resend(State(st.clone()), a.clone(), Path(id))
                .await
                .is_err(),
            "déjà activé : plus de lien"
        );
        assert!(resend(State(st.clone()), a.clone(), Path(a.id))
            .await
            .is_err());
        let lien: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_log WHERE action = 'admin_lien_activation_renvoyé'",
        )
        .fetch_one(&st.db)
        .await
        .unwrap();
        assert_eq!(lien, 2);
    }
}
