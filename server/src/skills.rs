//! Compétences (#113) : consignes que les IA connectées lisent avant de travailler. Celles de POP sont dans
//! `server/skills/` (la « recette » de POP) ; celles du studio sont écrites par l'équipe (membre et plus).
use crate::{
    access::{studio_role, Role},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Compétences fournies par POP : (nom, titre, contenu).
pub const POP: &[(&str, &str, &str)] = &[
    (
        "verse",
        "Écrire du Verse pour UEFN avec POP",
        include_str!("../skills/verse.md"),
    ),
    (
        "appareils",
        "Placer et régler des appareils",
        include_str!("../skills/appareils.md"),
    ),
    (
        "session-test",
        "Tester en session et corriger en boucle",
        include_str!("../skills/session-test.md"),
    ),
    (
        "lore",
        "Archiver dans Lore",
        include_str!("../skills/lore.md"),
    ),
    (
        "rangement",
        "Ranger le niveau",
        include_str!("../skills/rangement.md"),
    ),
    (
        "miniature",
        "Faire la miniature de la map",
        include_str!("../skills/miniature.md"),
    ),
    (
        "studio3d",
        "Faire un modèle 3D avec Blender (3D Studio)",
        include_str!("../skills/studio3d.md"),
    ),
];

/// Liste des compétences (POP puis studio), sans le contenu.
pub async fn liste(st: &AppState, studio_id: i64) -> Result<Vec<Value>, AppError> {
    let mut out: Vec<Value> = POP
        .iter()
        .map(|(n, t, _)| json!({ "name": n, "title": t, "source": "pop" }))
        .collect();
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, name, title, updated_at FROM skills WHERE studio_id = ? ORDER BY name",
    )
    .bind(studio_id)
    .fetch_all(&st.db)
    .await?;
    out.extend(rows.into_iter().map(
        |r| json!({ "id": r.0, "name": r.1, "title": r.2, "source": "studio", "updated_at": r.3 }),
    ));
    Ok(out)
}

/// Une compétence par son nom : celle du studio si elle existe, sinon celle de POP.
pub async fn lire(st: &AppState, studio_id: i64, name: &str) -> Result<Value, AppError> {
    let r: Option<(i64, String, String)> =
        sqlx::query_as("SELECT id, title, content FROM skills WHERE studio_id = ? AND name = ?")
            .bind(studio_id)
            .bind(name)
            .fetch_optional(&st.db)
            .await?;
    if let Some((id, title, content)) = r {
        return Ok(
            json!({ "id": id, "name": name, "title": title, "content": content, "source": "studio" }),
        );
    }
    POP.iter()
        .find(|(n, _, _)| *n == name)
        .map(|(n, t, c)| json!({ "name": n, "title": t, "content": c, "source": "pop" }))
        .ok_or_else(|| bad(format!("Compétence inconnue : {name}. Voir list_skills.")))
}

pub async fn list(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    Ok(Json(json!(liste(&st, sid).await?)))
}

pub async fn get(
    State(st): State<AppState>,
    u: AuthUser,
    Path((sid, name)): Path<(i64, String)>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    Ok(Json(lire(&st, sid, &name).await?))
}

#[derive(Deserialize)]
pub struct SkillIn {
    name: String,
    title: String,
    content: String,
}

/// Ajoute ou remplace une compétence du studio (membre et plus).
pub async fn save(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<SkillIn>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let name = b.name.trim().to_lowercase();
    if name.is_empty()
        || name.len() > 40
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(bad(
            "Nom : lettres sans accent, chiffres et tirets, 40 au plus (ex. regles-du-studio).",
        ));
    }
    check_len("Titre", &b.title, 1, 120)?;
    check_len("Contenu", &b.content, 1, 20_000)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO skills (studio_id, name, title, content, author_id) VALUES (?, ?, ?, ?, ?) ON CONFLICT(studio_id, name) DO UPDATE SET title = excluded.title, content = excluded.content, author_id = excluded.author_id, updated_at = datetime('now') RETURNING id")
        .bind(sid).bind(&name).bind(b.title.trim()).bind(&b.content).bind(u.id).fetch_one(&st.db).await?;
    Ok(Json(json!({ "id": id, "name": name })))
}

pub async fn delete(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let sid: i64 = sqlx::query_scalar("SELECT studio_id FROM skills WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    sqlx::query("DELETE FROM skills WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn membre(st: &AppState, email: &str, studio: i64, role: &str) -> AuthUser {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
        )
        .bind(email)
        .bind(email)
        .fetch_one(&st.db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, ?)")
            .bind(studio)
            .bind(id)
            .bind(role)
            .execute(&st.db)
            .await
            .unwrap();
        AuthUser {
            id,
            email: email.into(),
            name: email.into(),
            sid: String::new(),
        }
    }

    #[tokio::test]
    async fn competences() {
        let st = crate::test_state().await;
        let s1: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('A') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let s2: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('B') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let m = membre(&st, "m@a.fr", s1, "member").await;
        let v = membre(&st, "v@a.fr", s1, "viewer").await;
        let o = membre(&st, "o@b.fr", s2, "owner").await;
        // Les compétences de POP sont là pour tous, avec leur contenu.
        let Json(l) = list(State(st.clone()), v.clone(), Path(s1)).await.unwrap();
        assert_eq!(l.as_array().unwrap().len(), POP.len());
        let Json(verse) = get(State(st.clone()), v.clone(), Path((s1, "verse".into())))
            .await
            .unwrap();
        assert!(verse["content"]
            .as_str()
            .unwrap()
            .contains("search_library"));
        // Un membre ajoute une compétence ; l'invité ne peut pas.
        let b = |n: &str| {
            Json(serde_json::from_value::<SkillIn>(json!({ "name": n, "title": "Règles du studio", "content": "Toujours nommer en français." })).unwrap())
        };
        save(
            State(st.clone()),
            m.clone(),
            Path(s1),
            b("regles-du-studio"),
        )
        .await
        .unwrap();
        assert!(matches!(
            save(State(st.clone()), v.clone(), Path(s1), b("x")).await,
            Err(AppError::Forbidden)
        ));
        assert!(
            save(State(st.clone()), m.clone(), Path(s1), b("Mauvais nom !"))
                .await
                .is_err()
        );
        let Json(l) = list(State(st.clone()), v.clone(), Path(s1)).await.unwrap();
        assert_eq!(l.as_array().unwrap().len(), POP.len() + 1);
        // Un autre studio ne la voit pas.
        assert!(get(
            State(st.clone()),
            o.clone(),
            Path((s2, "regles-du-studio".into()))
        )
        .await
        .is_err());
        assert!(list(State(st.clone()), o.clone(), Path(s1)).await.is_err());
    }
}
