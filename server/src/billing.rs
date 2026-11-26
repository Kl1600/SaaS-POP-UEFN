//! Abonnements : qui a accès à Analyse.
//! Plans : Starter et Pro (Pro = Analyse). Essai de Pro de 7 jours pour les nouveaux comptes.
//! - La catégorie Analyse (marché, maps qui marchent, démarrages, tendances vidéo) suit le plan du compte.
//! - L'onglet « Marché » d'un studio suit le plan du créateur (propriétaire) du studio : ses membres y ont
//!   accès seulement dans ce studio.
use crate::{auth::AuthUser, error::AppError, AppState};

/// Deux abonnements : Starter (espace de travail) et Pro (+ Analyse et onglet Discover).
pub const PLANS: &[&str] = &["starter", "pro"];
/// Condition SQL « le compte a Analyse » (plan Pro ou essai en cours), sur la table users aliasée `u`.
pub const SQL_PRO: &str = "((u.plan = 'pro' AND (u.plan_source <> 'stripe' OR u.sub_status IN ('active', 'trialing', 'past_due'))) OR u.trial_until > datetime('now'))";

pub async fn user_plan(st: &AppState, uid: i64) -> Result<String, AppError> {
    Ok(sqlx::query_scalar("SELECT plan FROM users WHERE id = ?")
        .bind(uid)
        .fetch_optional(&st.db)
        .await?
        .unwrap_or_else(|| "starter".into()))
}

/// Fin de l'essai de Pro, s'il est en cours.
pub async fn trial_until(st: &AppState, uid: i64) -> Result<Option<String>, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT trial_until FROM users WHERE id = ? AND trial_until > datetime('now')",
    )
    .bind(uid)
    .fetch_optional(&st.db)
    .await?
    .flatten())
}

pub async fn user_analyse(st: &AppState, uid: i64) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar::<_, i64>(crate::sql!(
        "SELECT COUNT(*) FROM users u WHERE u.id = ? AND {SQL_PRO}"
    ))
    .bind(uid)
    .fetch_one(&st.db)
    .await?
        > 0)
}

/// Le créateur du studio a-t-il un plan qui comprend Analyse ?
pub async fn studio_analyse(st: &AppState, sid: i64) -> Result<bool, AppError> {
    let n: i64 = sqlx::query_scalar(crate::sql!("SELECT COUNT(*) FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ? AND m.role = 'owner' AND {SQL_PRO}"))
        .bind(sid).fetch_one(&st.db).await?;
    Ok(n > 0)
}

fn refused() -> AppError {
    AppError::Guard("plan", "Analyse est comprise dans le plan Pro.".into())
}

/// Toute la catégorie Analyse : plan du compte.
pub async fn require_analyse(st: &AppState, u: &AuthUser) -> Result<(), AppError> {
    if user_analyse(st, u.id).await? {
        Ok(())
    } else {
        Err(refused())
    }
}

/// Chiffres du marché (genres, fiche d'une île) : plan du compte, ou membre d'un studio dont le créateur a le plan.
pub async fn require_market(st: &AppState, u: &AuthUser) -> Result<(), AppError> {
    if user_analyse(st, u.id).await? {
        return Ok(());
    }
    let ok: i64 = sqlx::query_scalar(crate::sql!(
        "SELECT COUNT(*) FROM memberships me JOIN studios s ON s.id = me.studio_id AND s.personal = 0
         JOIN memberships o ON o.studio_id = s.id AND o.role = 'owner' JOIN users u ON u.id = o.user_id
         WHERE me.user_id = ? AND {SQL_PRO}",
    )).bind(u.id).fetch_one(&st.db).await?;
    if ok > 0 {
        Ok(())
    } else {
        Err(refused())
    }
}

/// Tendances vidéo d'un studio (ou de l'espace perso) : plan du créateur de ce studio.
pub async fn require_studio(st: &AppState, sid: i64) -> Result<(), AppError> {
    if studio_analyse(st, sid).await? {
        Ok(())
    } else {
        Err(refused())
    }
}

/// Écrans de construction de Forge (#102) : réservés à l'abonnement Pro (essai compris) et aux administrateurs de
/// la plateforme. Le relais de Forge et les IA connectées par MCP au site n'en dépendent pas (#404).
pub async fn has_forge(st: &AppState, uid: i64) -> Result<bool, AppError> {
    Ok(user_analyse(st, uid).await? || crate::admin::is_admin(st, uid).await?)
}

/// Refus en français des routes propres à Forge pour un compte sans Pro.
pub async fn require_forge(st: &AppState, uid: i64) -> Result<(), AppError> {
    if has_forge(st, uid).await? {
        Ok(())
    } else {
        Err(AppError::Guard("forge_pro", "POP Forge est inclus dans l'abonnement Pro (essai compris). Passez à Pro depuis POP : Paramètres → Abonnement.".into()))
    }
}

/// `GET /api/forge/download` (#80) : dernière version publiée de l'installateur de Forge (fichier construit par le
/// workflow « relais Windows » dans web/public/downloads). Ouvert à tout compte (#404) : sans Pro, Forge sert de
/// relais UEFN ; ses écrans de construction restent réservés au Pro.
pub async fn forge_download(
    _u: crate::auth::AuthUser,
) -> crate::error::ApiResult<serde_json::Value> {
    let dir = std::path::PathBuf::from(
        std::env::var("POP_WEB_DIR").unwrap_or_else(|_| "../web/dist".into()),
    )
    .join("downloads");
    let info: serde_json::Value = std::fs::read_to_string(dir.join("pop-forge.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let dispo = dir.join("pop-forge-setup.exe").is_file();
    Ok(axum::Json(serde_json::json!({
        "available": dispo,
        "version": info["version"],
        "date": info["date"],
        "url": if dispo { serde_json::json!("/downloads/pop-forge-setup.exe") } else { serde_json::Value::Null },
        "size": std::fs::metadata(dir.join("pop-forge-setup.exe")).map(|m| m.len()).unwrap_or(0),
    })))
}
