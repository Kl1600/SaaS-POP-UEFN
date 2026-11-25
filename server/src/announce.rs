//! Annonce en bandeau pour tous les comptes connectés (#67) : maintenance, nouveauté. Sans e-mail ni notification.
use crate::auth::AuthUser;
use crate::error::{bad, ApiResult, AppError};
use crate::AppState;
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

/// Condition SQL de l'annonce active : non retirée, non échue.
const ACTIVE: &str = "removed_at IS NULL AND (ends_at IS NULL OR ends_at > datetime('now'))";

/// L'annonce à montrer à ce compte (null s'il l'a fermée ou s'il n'y en a pas). Utilisée par /api/me.
pub async fn for_user(st: &AppState, uid: i64) -> Result<Value, AppError> {
    let r: Option<(i64, String, String)> = sqlx::query_as(crate::sql!(
        "SELECT id, text, level FROM announcements WHERE {ACTIVE} AND id NOT IN (SELECT announcement_id FROM announcement_dismissals WHERE user_id = ?) ORDER BY id DESC LIMIT 1"
    ))
    .bind(uid)
    .fetch_optional(&st.db)
    .await?;
    // Seule la plus récente compte : une annonce plus ancienne fermée ne doit pas laisser revenir une plus ancienne encore.
    let last: Option<i64> = sqlx::query_scalar(crate::sql!(
        "SELECT MAX(id) FROM announcements WHERE {ACTIVE}"
    ))
    .fetch_one(&st.db)
    .await?;
    Ok(match r {
        Some((id, text, level)) if Some(id) == last => {
            json!({ "id": id, "text": text, "level": level })
        }
        _ => Value::Null,
    })
}

pub async fn dismiss(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    sqlx::query("INSERT OR IGNORE INTO announcement_dismissals (announcement_id, user_id) SELECT id, ? FROM announcements WHERE id = ?")
        .bind(u.id)
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Administration : l'annonce active, avec le nombre de comptes qui l'ont fermée.
pub async fn current(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    let r: Option<(i64, String, String, Option<String>, String, i64)> = sqlx::query_as(crate::sql!(
        "SELECT a.id, a.text, a.level, a.ends_at, a.created_at, (SELECT COUNT(*) FROM announcement_dismissals d WHERE d.announcement_id = a.id) FROM announcements a WHERE {ACTIVE} ORDER BY a.id DESC LIMIT 1"
    ))
    .fetch_optional(&st.db)
    .await?;
    Ok(Json(
        json!({ "announcement": r.map(|(id, text, level, ends_at, created_at, closed)| json!({ "id": id, "text": text, "level": level, "ends_at": ends_at, "created_at": created_at, "closed": closed })) }),
    ))
}

#[derive(Deserialize)]
pub struct AnnounceIn {
    text: String,
    level: String,
    /// Date de fin facultative, RFC 3339 (ex. 2026-11-22T18:00:00.000Z).
    ends_at: Option<String>,
}

/// Publie une annonce : elle remplace la précédente (une seule active à la fois).
pub async fn publish(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<AnnounceIn>,
) -> ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    let text = b.text.trim();
    if text.is_empty() || text.chars().count() > 300 {
        return Err(bad("Écrivez une annonce de 300 caractères au plus."));
    }
    if !["info", "important"].contains(&b.level.as_str()) {
        return Err(bad("Niveau inconnu."));
    }
    let ends_at = match b
        .ends_at
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        None => None,
        Some(s) => {
            let t = chrono::DateTime::parse_from_rfc3339(s)
                .map_err(|_| bad("Date de fin invalide."))?
                .with_timezone(&chrono::Utc);
            if t <= chrono::Utc::now() {
                return Err(bad("La date de fin est déjà passée."));
            }
            Some(t.format("%Y-%m-%d %H:%M:%S").to_string())
        }
    };
    sqlx::query(crate::sql!(
        "UPDATE announcements SET removed_at = datetime('now') WHERE {ACTIVE}"
    ))
    .execute(&st.db)
    .await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO announcements (text, level, ends_at, created_by) VALUES (?, ?, ?, ?) RETURNING id")
        .bind(text)
        .bind(&b.level)
        .bind(&ends_at)
        .bind(u.id)
        .fetch_one(&st.db)
        .await?;
    crate::security::audit(&st, Some(u.id), "annonce_publiée", text, "").await;
    Ok(Json(json!({ "id": id })))
}

pub async fn remove(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    sqlx::query(crate::sql!(
        "UPDATE announcements SET removed_at = datetime('now') WHERE {ACTIVE}"
    ))
    .execute(&st.db)
    .await?;
    crate::security::audit(&st, Some(u.id), "annonce_retirée", "", "").await;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn compte(st: &AppState, email: &str, admin: bool) -> AuthUser {
        let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, is_admin, email_verified_at) VALUES (?, 'x', 'x', ?, datetime('now')) RETURNING id")
            .bind(email).bind(admin as i64).fetch_one(&st.db).await.unwrap();
        let sid = format!("session-{id}");
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at, sudo_until) VALUES (?, ?, datetime('now', '+1 day'), datetime('now', '+10 minutes'))")
            .bind(&sid).bind(id).execute(&st.db).await.unwrap();
        AuthUser {
            id,
            email: email.into(),
            name: "x".into(),
            sid,
        }
    }
    fn ann(text: &str, ends_at: Option<&str>) -> Json<AnnounceIn> {
        Json(AnnounceIn {
            text: text.into(),
            level: "info".into(),
            ends_at: ends_at.map(Into::into),
        })
    }

    #[tokio::test]
    async fn publier_fermer_retirer() {
        let st = crate::test_state().await;
        let admin = compte(&st, "admin@exemple.com", true).await;
        let membre = compte(&st, "membre@exemple.com", false).await;
        assert!(matches!(
            publish(State(st.clone()), membre.clone(), ann("x", None)).await,
            Err(AppError::Forbidden)
        ));
        assert!(publish(
            State(st.clone()),
            admin.clone(),
            ann("Hier", Some("2020-01-01T00:00:00Z"))
        )
        .await
        .is_err());

        let id = publish(
            State(st.clone()),
            admin.clone(),
            ann("Maintenance ce soir", None),
        )
        .await
        .unwrap()
        .0["id"]
            .as_i64()
            .unwrap();
        assert_eq!(
            for_user(&st, membre.id).await.unwrap()["text"],
            "Maintenance ce soir"
        );
        dismiss(State(st.clone()), membre.clone(), Path(id))
            .await
            .unwrap();
        assert!(for_user(&st, membre.id).await.unwrap().is_null());
        assert!(!for_user(&st, admin.id).await.unwrap().is_null());

        // Une nouvelle annonce revient pour tous et remplace l'ancienne.
        publish(
            State(st.clone()),
            admin.clone(),
            ann("Nouveauté", Some("2099-01-01T00:00:00Z")),
        )
        .await
        .unwrap();
        assert_eq!(for_user(&st, membre.id).await.unwrap()["text"], "Nouveauté");
        remove(State(st.clone()), admin.clone()).await.unwrap();
        assert!(for_user(&st, membre.id).await.unwrap().is_null());
        assert!(for_user(&st, admin.id).await.unwrap().is_null());

        // Échue : plus personne ne la voit.
        publish(State(st.clone()), admin.clone(), ann("Courte", None))
            .await
            .unwrap();
        sqlx::query("UPDATE announcements SET ends_at = datetime('now', '-1 minute') WHERE removed_at IS NULL").execute(&st.db).await.unwrap();
        assert!(for_user(&st, membre.id).await.unwrap().is_null());
    }
}
