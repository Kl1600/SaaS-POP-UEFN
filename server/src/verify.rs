//! Vérification des adresses e-mail (#11).
//!
//! Une adresse est vérifiée quand la personne ouvre un lien reçu à cette adresse : lien de vérification (24 h, à usage
//! unique) ou lien de réinitialisation du mot de passe. Une invitation ou un lien d'accès ne suffisent pas : un admin
//! peut les copier et les transmettre par un autre canal. Sans SMTP configuré (POP en local), le compte est vérifié
//! d'office et un avertissement part dans les journaux. Seul un compte vérifié peut être administrateur.
use crate::{
    auth::{random_token, sha256_hex, AuthUser},
    error::{bad, ApiResult, AppError},
    mail, AppState,
};
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

const VALID_HOURS: i64 = 24;
/// Délai minimal entre deux envois du lien à la même personne.
const RESEND_SECONDS: i64 = 120;

pub async fn is_verified(st: &AppState, uid: i64) -> Result<bool, AppError> {
    let v: Option<Option<String>> =
        sqlx::query_scalar("SELECT email_verified_at FROM users WHERE id = ?")
            .bind(uid)
            .fetch_optional(&st.db)
            .await?;
    Ok(matches!(v, Some(Some(_))))
}

pub async fn mark_verified(st: &AppState, uid: i64) -> Result<(), AppError> {
    sqlx::query("UPDATE users SET email_verified_at = COALESCE(email_verified_at, datetime('now')) WHERE id = ?").bind(uid).execute(&st.db).await?;
    Ok(())
}

/// Après une inscription : envoie le lien de vérification. Renvoie `true` si un e-mail est parti.
/// Sans SMTP, vérifie le compte d'office (comportement d'avant #11) et le signale dans les journaux.
pub async fn start(st: &AppState, uid: i64, email: &str) -> Result<bool, AppError> {
    if !mail::configured() {
        tracing::warn!("SMTP non configuré : l'adresse {email} est considérée comme vérifiée sans e-mail. En production, configurez POP_SMTP_* pour vérifier les adresses (et protéger le rôle d'administrateur).");
        mark_verified(st, uid).await?;
        return Ok(false);
    }
    send_link(st, uid, email).await?;
    Ok(true)
}

async fn send_link(st: &AppState, uid: i64, email: &str) -> Result<(), AppError> {
    let token = random_token();
    sqlx::query("INSERT INTO email_verifications (token_hash, user_id, expires_at) VALUES (?, ?, datetime('now', ?))")
        .bind(sha256_hex(&token))
        .bind(uid)
        .bind(format!("+{VALID_HOURS} hours"))
        .execute(&st.db)
        .await?;
    let link = format!("{}/verifier-email/{token}", mail::public_url());
    let to = email.to_string();
    let body = format!("Bonjour,\n\nPour confirmer votre adresse e-mail sur POP, ouvrez ce lien (valable {VALID_HOURS} heures) :\n{link}\n\nSi vous n'avez pas créé de compte sur POP, ignorez ce message.\n\nL’équipe POP");
    tokio::spawn(async move {
        if let Err(e) = mail::send(&to, "POP · confirmez votre adresse e-mail", &body).await {
            tracing::warn!("e-mail de vérification non envoyé à {to} : {e:#}");
        }
    });
    Ok(())
}

#[derive(Deserialize)]
pub struct VerifyIn {
    token: String,
}

/// Ouvre le lien reçu par e-mail : marque l'adresse comme vérifiée. Pas besoin d'être connecté.
pub async fn verify(
    State(st): State<AppState>,
    crate::security::ClientIp(ip): crate::security::ClientIp,
    Json(b): Json<VerifyIn>,
) -> ApiResult<Value> {
    crate::auth::rate_limit(&st, &format!("ip:verify:{ip}"))?;
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT v.user_id, u.email FROM email_verifications v JOIN users u ON u.id = v.user_id
         WHERE v.token_hash = ? AND v.used_at IS NULL AND v.expires_at > datetime('now')",
    )
    .bind(sha256_hex(b.token.trim()))
    .fetch_optional(&st.db)
    .await?;
    let (uid, email) = row.ok_or_else(|| bad("Lien de vérification invalide, déjà utilisé ou expiré : demandez-en un nouveau depuis POP."))?;
    sqlx::query("UPDATE email_verifications SET used_at = datetime('now') WHERE token_hash = ?")
        .bind(sha256_hex(b.token.trim()))
        .execute(&st.db)
        .await?;
    mark_verified(&st, uid).await?;
    crate::security::audit(&st, Some(uid), "email_vérifié", &email, &ip).await;
    Ok(Json(json!({ "ok": true, "email": email })))
}

/// Renvoie le lien de vérification à la personne connectée.
pub async fn resend(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    if is_verified(&st, u.id).await? {
        return Ok(Json(json!({ "ok": true, "already": true })));
    }
    if !mail::configured() {
        return Err(bad(
            "L’envoi d’e-mails n’est pas configuré sur ce serveur : demandez à l’administrateur.",
        ));
    }
    let recent: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM email_verifications WHERE user_id = ? AND created_at > datetime('now', ?)")
        .bind(u.id)
        .bind(format!("-{RESEND_SECONDS} seconds"))
        .fetch_one(&st.db)
        .await?;
    if recent > 0 {
        return Err(bad("Un lien vient de vous être envoyé : regardez vos e-mails (et les indésirables), ou attendez deux minutes avant d’en demander un autre."));
    }
    send_link(&st, u.id, &u.email).await?;
    Ok(Json(json!({ "ok": true, "email": u.email })))
}
