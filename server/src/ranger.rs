//! Maps « à ranger » (#143) : les maps de l'espace perso dont l'espace Creator n'est pas celui de la personne
//! (par exemple une équipe Creator). POP les propose à passer dans un studio, sans jamais le faire en silence.
use crate::{
    access::{studio_role, Role},
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    link::espace,
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Maps de l'espace perso à ranger, groupées par espace Creator.
pub async fn liste(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    let mien: Option<String> = sqlx::query_scalar("SELECT creator_space FROM users WHERE id = ?")
        .bind(u.id)
        .fetch_one(&st.db)
        .await?;
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT p.id, p.name, p.creator_space FROM maps p JOIN studios s ON s.id = p.studio_id JOIN memberships m ON m.studio_id = s.id
         WHERE m.user_id = ?1 AND s.personal = 1 AND p.creator_space IS NOT NULL AND p.creator_space <> COALESCE(?2, '')
           AND p.creator_space NOT IN (SELECT space FROM creator_space_solo WHERE user_id = ?1)
         ORDER BY p.creator_space, p.name",
    )
    .bind(u.id)
    .bind(&mien)
    .fetch_all(&st.db)
    .await?;
    let mut groupes: Vec<Value> = vec![];
    for (id, name, space) in rows {
        match groupes.iter_mut().find(|g| g["space"] == space.as_str()) {
            Some(g) => g["maps"]
                .as_array_mut()
                .unwrap()
                .push(json!({ "id": id, "name": name })),
            None => {
                // Studios de la personne déjà reliés à cet espace (pour « les passer dans un studio existant »).
                let relie: Option<i64> = sqlx::query_scalar("SELECT s.id FROM studios s JOIN memberships m ON m.studio_id = s.id WHERE m.user_id = ? AND s.creator_space = ? AND m.role IN ('owner', 'admin') LIMIT 1")
                    .bind(u.id).bind(&space).fetch_optional(&st.db).await?;
                groupes.push(json!({
                    "space": space,
                    "label": space.split('@').next().unwrap_or(&space),
                    "studio_id": relie,
                    "maps": [{ "id": id, "name": name }],
                }));
            }
        }
    }
    Ok(Json(json!({ "mine": mien, "groups": groupes })))
}

#[derive(Deserialize)]
pub struct EspaceIn {
    space: Option<String>,
}

/// « Laisser en solo » : ne plus proposer de ranger les maps de cet espace.
pub async fn laisser_solo(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<EspaceIn>,
) -> ApiResult<Value> {
    let space = espace(b.space.as_deref()).ok_or_else(|| bad("Espace Creator invalide."))?;
    sqlx::query("INSERT OR IGNORE INTO creator_space_solo (user_id, space) VALUES (?, ?)")
        .bind(u.id)
        .bind(&space)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Mon espace Creator (« Mes outils ») : vide pour l'oublier.
pub async fn mon_espace(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<EspaceIn>,
) -> ApiResult<Value> {
    let space = saisie(b.space.as_deref())?;
    sqlx::query("UPDATE users SET creator_space = ? WHERE id = ?")
        .bind(&space)
        .bind(u.id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "space": space })))
}

/// Espace Creator relié au studio : lu par tout membre, changé par un admin.
pub async fn espace_studio(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    let space: Option<String> =
        sqlx::query_scalar("SELECT creator_space FROM studios WHERE id = ?")
            .bind(sid)
            .fetch_one(&st.db)
            .await?;
    Ok(Json(json!({ "space": space })))
}

pub async fn changer_espace_studio(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<EspaceIn>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    if crate::studio::is_personal(&st, sid).await? {
        return Err(bad("Votre espace perso n'est relié à aucune équipe Creator : changez votre espace Creator dans « Mes outils »."));
    }
    let space = saisie(b.space.as_deref())?;
    sqlx::query("UPDATE studios SET creator_space = ? WHERE id = ?")
        .bind(&space)
        .bind(sid)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "space": space })))
}

/// Saisie d'un espace : vide → aucun ; « klmaps » → « klmaps@fortnite.com ».
fn saisie(s: Option<&str>) -> Result<Option<String>, AppError> {
    let s = s.unwrap_or("").trim();
    if s.is_empty() {
        return Ok(None);
    }
    let s = if s.contains('@') {
        s.to_string()
    } else {
        format!("{s}@fortnite.com")
    };
    espace(Some(&s))
        .map(Some)
        .ok_or_else(|| bad("Espace Creator invalide (ex. klmaps@fortnite.com)."))
}
