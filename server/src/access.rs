use crate::{auth::AuthUser, error::AppError, AppState};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Viewer = 0,
    Freelance = 1,
    Member = 2,
    Admin = 3,
    Owner = 4,
}

impl Role {
    pub fn parse(s: &str) -> Option<Role> {
        Some(match s {
            "viewer" => Role::Viewer,
            "freelance" => Role::Freelance,
            "member" => Role::Member,
            "admin" => Role::Admin,
            "owner" => Role::Owner,
            _ => return None,
        })
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Freelance => "freelance",
            Role::Member => "member",
            Role::Admin => "admin",
            Role::Owner => "owner",
        }
    }
}

pub struct MapCtx {
    pub map_id: i64,
    pub studio_id: i64,
    pub role: Role,
    pub user: AuthUser,
}

impl MapCtx {
    pub fn require(&self, min: Role) -> Result<(), AppError> {
        if self.role >= min {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
    pub fn can_edit(&self) -> Result<(), AppError> {
        self.require(Role::Freelance)
    }
}

/// Vérifie que l'utilisateur appartient au studio de la map. Répond « introuvable » sinon,
/// pour ne pas révéler l'existence de la map.
pub async fn map_ctx(st: &AppState, user: &AuthUser, map_id: i64) -> Result<MapCtx, AppError> {
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT m.studio_id, ms.role FROM maps m JOIN memberships ms ON ms.studio_id = m.studio_id AND ms.user_id = ? WHERE m.id = ?",
    )
    .bind(user.id)
    .bind(map_id)
    .fetch_optional(&st.db)
    .await?;
    let (studio_id, role) = row.ok_or(AppError::NotFound)?;
    Ok(MapCtx { map_id, studio_id, role: Role::parse(&role).unwrap_or(Role::Viewer), user: user.clone() })
}

pub async fn studio_role(st: &AppState, user: &AuthUser, studio_id: i64) -> Result<Role, AppError> {
    let r: Option<(String,)> = sqlx::query_as("SELECT role FROM memberships WHERE studio_id = ? AND user_id = ?")
        .bind(studio_id)
        .bind(user.id)
        .fetch_optional(&st.db)
        .await?;
    r.and_then(|(r,)| Role::parse(&r)).ok_or(AppError::NotFound)
}

/// Ajoute une ligne au fil d'activité et prévient les membres connectés.
pub async fn log_activity(st: &AppState, map_id: i64, user_id: Option<i64>, kind: &str, title: &str, detail: &str) -> Result<(), AppError> {
    sqlx::query("INSERT INTO activity (map_id, user_id, kind, title, detail) VALUES (?, ?, ?, ?, ?)")
        .bind(map_id)
        .bind(user_id)
        .bind(kind)
        .bind(title)
        .bind(detail)
        .execute(&st.db)
        .await?;
    st.hub.send(map_id, json!({ "t": "activity", "kind": kind, "title": title, "detail": detail }));
    Ok(())
}

pub fn notify(st: &AppState, map_id: i64, msg: Value) {
    st.hub.send(map_id, msg);
}
