//! Abonnements : qui a accès à Analyse.
//! - La catégorie Analyse (marché, maps qui marchent, démarrages, tendances vidéo) suit le plan du compte.
//! - L'onglet « Marché » d'un studio suit le plan du créateur (propriétaire) du studio : ses membres y ont
//!   accès seulement dans ce studio.
use crate::{auth::AuthUser, error::AppError, AppState};

pub const PLANS: &[&str] = &["createur", "studio", "studio_plus"];

pub fn analyse_ok(plan: &str) -> bool { plan == "studio" || plan == "studio_plus" }

pub async fn user_plan(st: &AppState, uid: i64) -> Result<String, AppError> {
    Ok(sqlx::query_scalar("SELECT plan FROM users WHERE id = ?").bind(uid).fetch_optional(&st.db).await?.unwrap_or_else(|| "createur".into()))
}

pub async fn user_analyse(st: &AppState, uid: i64) -> Result<bool, AppError> { Ok(analyse_ok(&user_plan(st, uid).await?)) }

/// Le créateur du studio a-t-il un plan qui comprend Analyse ?
pub async fn studio_analyse(st: &AppState, sid: i64) -> Result<bool, AppError> {
    let p: Option<String> = sqlx::query_scalar("SELECT u.plan FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ? AND m.role = 'owner' ORDER BY m.user_id LIMIT 1")
        .bind(sid).fetch_optional(&st.db).await?;
    Ok(p.map(|p| analyse_ok(&p)).unwrap_or(false))
}

fn refused() -> AppError { AppError::Guard("plan", "Analyse est comprise dans le plan Studio.".into()) }

/// Toute la catégorie Analyse : plan du compte.
pub async fn require_analyse(st: &AppState, u: &AuthUser) -> Result<(), AppError> {
    if user_analyse(st, u.id).await? { Ok(()) } else { Err(refused()) }
}

/// Chiffres du marché (genres, fiche d'une île) : plan du compte, ou membre d'un studio dont le créateur a le plan.
pub async fn require_market(st: &AppState, u: &AuthUser) -> Result<(), AppError> {
    if user_analyse(st, u.id).await? { return Ok(()) }
    let ok: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM memberships me JOIN studios s ON s.id = me.studio_id AND s.personal = 0
         JOIN memberships o ON o.studio_id = s.id AND o.role = 'owner' JOIN users ou ON ou.id = o.user_id
         WHERE me.user_id = ? AND ou.plan IN ('studio', 'studio_plus')",
    ).bind(u.id).fetch_one(&st.db).await?;
    if ok > 0 { Ok(()) } else { Err(refused()) }
}

/// Tendances vidéo d'un studio (ou de l'espace perso) : plan du créateur de ce studio.
pub async fn require_studio(st: &AppState, sid: i64) -> Result<(), AppError> {
    if studio_analyse(st, sid).await? { Ok(()) } else { Err(refused()) }
}
