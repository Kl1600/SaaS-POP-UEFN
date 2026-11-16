use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Erreur renvoyée par l'API. Les messages internes ne sont jamais exposés au client.
#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict(String),
    TooMany,
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (code, msg) = match self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Connexion requise.".into()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "Action non autorisée pour votre rôle.".into()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "Introuvable.".into()),
            AppError::Conflict(m) => (StatusCode::CONFLICT, m),
            AppError::TooMany => (StatusCode::TOO_MANY_REQUESTS, "Trop de tentatives, réessayez dans une minute.".into()),
            AppError::Internal(e) => {
                tracing::error!("erreur interne : {e:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Erreur interne.".into())
            }
        };
        (code, Json(json!({ "error": msg }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            other => AppError::Internal(other.into()),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(e.into())
    }
}

pub type ApiResult<T> = Result<Json<T>, AppError>;

pub fn bad(msg: impl Into<String>) -> AppError {
    AppError::BadRequest(msg.into())
}

/// Vérifie la longueur d'un texte saisi par l'utilisateur.
pub fn check_len(field: &str, v: &str, min: usize, max: usize) -> Result<(), AppError> {
    let n = v.trim().chars().count();
    if n < min || n > max {
        return Err(bad(format!("« {field} » doit faire entre {min} et {max} caractères.")));
    }
    Ok(())
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AppError {}
