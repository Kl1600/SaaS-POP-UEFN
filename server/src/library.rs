//! Bibliothèque Verse du studio (#85) : modules prêts (économie, équipes, minuteur, HUD, sauvegarde…) partagés par
//! l'équipe. Les humains et les IA connectées (MCP : `search_library`, `get_library_module`) la consultent avant
//! d'écrire du neuf. Lecture : tout membre du studio, invité compris ; ajout, modification, suppression : membre et plus.
use crate::{
    access::{studio_role, Role},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub const CATEGORIES: &[&str] = &[
    "economie",
    "equipes",
    "minuteur",
    "hud",
    "sauvegarde",
    "interface",
    "jeu",
    "outils",
    "autre",
];
const MAX_CONTENU: usize = 200_000;

#[derive(Deserialize, Default)]
pub struct SearchQ {
    pub q: Option<String>,
    pub category: Option<String>,
}

/// Modules d'un studio (sans le code), filtrés par texte (nom, description) et catégorie.
pub async fn search(st: &AppState, studio_id: i64, q: &SearchQ) -> Result<Vec<Value>, AppError> {
    let texte = format!("%{}%", q.q.as_deref().unwrap_or("").trim().to_lowercase());
    let cat = q.category.as_deref().unwrap_or("").trim().to_string();
    let rows: Vec<(i64, String, String, String, String, i64, Option<String>, String)> = sqlx::query_as(
        "SELECT v.id, v.name, v.category, v.description, v.devices, v.version, u.name, v.updated_at FROM verse_modules v LEFT JOIN users u ON u.id = v.author_id
         WHERE v.studio_id = ? AND (lower(v.name) LIKE ? OR lower(v.description) LIKE ?) AND (? = '' OR v.category = ?)
         ORDER BY v.category, v.name LIMIT 200",
    )
    .bind(studio_id).bind(&texte).bind(&texte).bind(&cat).bind(&cat)
    .fetch_all(&st.db).await?;
    Ok(rows
        .into_iter()
        .map(|r| json!({ "id": r.0, "name": r.1, "category": r.2, "description": r.3, "devices": serde_json::from_str::<Value>(&r.4).unwrap_or(json!([])), "version": r.5, "author": r.6, "updated_at": r.7 }))
        .collect())
}

/// Un module complet (avec le code), s'il appartient au studio donné.
pub async fn module(st: &AppState, studio_id: i64, id: i64) -> Result<Value, AppError> {
    let r: Option<(i64, String, String, String, String, String, i64, Option<String>, String)> = sqlx::query_as(
        "SELECT v.id, v.name, v.category, v.description, v.content, v.devices, v.version, u.name, v.updated_at FROM verse_modules v LEFT JOIN users u ON u.id = v.author_id WHERE v.id = ? AND v.studio_id = ?",
    )
    .bind(id).bind(studio_id).fetch_optional(&st.db).await?;
    let r = r.ok_or(AppError::NotFound)?;
    Ok(
        json!({ "id": r.0, "name": r.1, "category": r.2, "description": r.3, "content": r.4, "devices": serde_json::from_str::<Value>(&r.5).unwrap_or(json!([])), "version": r.6, "author": r.7, "updated_at": r.8 }),
    )
}

async fn studio_of(st: &AppState, id: i64) -> Result<i64, AppError> {
    sqlx::query_scalar("SELECT studio_id FROM verse_modules WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn list(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Query(q): Query<SearchQ>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    Ok(Json(json!(search(&st, sid, &q).await?)))
}

pub async fn get(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>) -> ApiResult<Value> {
    let sid = studio_of(&st, id).await?;
    studio_role(&st, &u, sid).await?;
    Ok(Json(module(&st, sid, id).await?))
}

#[derive(Deserialize)]
pub struct ModuleIn {
    name: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    description: String,
    content: String,
    #[serde(default)]
    devices: Vec<String>,
}

fn valider(b: &ModuleIn) -> Result<String, AppError> {
    check_len("Nom", &b.name, 1, 80)?;
    check_len("Description", &b.description, 0, 1000)?;
    if b.content.trim().is_empty() {
        return Err(bad("Le code du module est vide."));
    }
    if b.content.len() > MAX_CONTENU {
        return Err(bad("Module trop long (200 000 caractères au plus)."));
    }
    if b.devices.len() > 50 || b.devices.iter().any(|d| d.chars().count() > 120) {
        return Err(bad("Liste d'appareils requis trop longue."));
    }
    let cat = b.category.as_deref().unwrap_or("autre");
    if !CATEGORIES.contains(&cat) {
        return Err(bad(format!(
            "Catégorie inconnue : {cat}. Catégories : {}.",
            CATEGORIES.join(", ")
        )));
    }
    Ok(cat.to_string())
}

fn doublon(e: sqlx::Error) -> AppError {
    match &e {
        sqlx::Error::Database(d) if d.message().contains("UNIQUE") => {
            bad("Un module de ce nom existe déjà dans la bibliothèque du studio.")
        }
        _ => e.into(),
    }
}

pub async fn create(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<ModuleIn>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let cat = valider(&b)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO verse_modules (studio_id, name, category, description, content, devices, author_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(sid).bind(b.name.trim()).bind(&cat).bind(b.description.trim()).bind(&b.content).bind(json!(b.devices).to_string()).bind(u.id)
        .fetch_one(&st.db).await.map_err(doublon)?;
    Ok(Json(json!({ "id": id })))
}

pub async fn update(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<ModuleIn>,
) -> ApiResult<Value> {
    let sid = studio_of(&st, id).await?;
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let cat = valider(&b)?;
    let version: i64 = sqlx::query_scalar("UPDATE verse_modules SET name = ?, category = ?, description = ?, content = ?, devices = ?, author_id = ?, version = version + 1, updated_at = datetime('now') WHERE id = ? RETURNING version")
        .bind(b.name.trim()).bind(&cat).bind(b.description.trim()).bind(&b.content).bind(json!(b.devices).to_string()).bind(u.id).bind(id)
        .fetch_one(&st.db).await.map_err(doublon)?;
    Ok(Json(json!({ "id": id, "version": version })))
}

pub async fn delete(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let sid = studio_of(&st, id).await?;
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    sqlx::query("DELETE FROM verse_modules WHERE id = ?")
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
    async fn studio(st: &AppState, nom: &str) -> i64 {
        sqlx::query_scalar("INSERT INTO studios (name) VALUES (?) RETURNING id")
            .bind(nom)
            .fetch_one(&st.db)
            .await
            .unwrap()
    }
    fn module_in(nom: &str) -> Json<ModuleIn> {
        Json(serde_json::from_value(json!({ "name": nom, "category": "minuteur", "description": "Compte à rebours de manche", "content": "minuteur := class(creative_device):\n    OnBegin<override>()<suspends>:void = {}", "devices": ["timer_device"] })).unwrap())
    }

    #[tokio::test]
    async fn droits_et_recherche() {
        let st = crate::test_state().await;
        let (s1, s2) = (studio(&st, "Studio A").await, studio(&st, "Studio B").await);
        let m = membre(&st, "m@a.fr", s1, "member").await;
        let invite = membre(&st, "v@a.fr", s1, "viewer").await;
        let autre = membre(&st, "o@b.fr", s2, "owner").await;

        // Un membre ajoute ; un invité ne peut pas.
        let Json(r) = create(
            State(st.clone()),
            m.clone(),
            Path(s1),
            module_in("Minuteur de manche"),
        )
        .await
        .unwrap();
        let id = r["id"].as_i64().unwrap();
        assert!(matches!(
            create(
                State(st.clone()),
                invite.clone(),
                Path(s1),
                module_in("Autre")
            )
            .await,
            Err(AppError::Forbidden)
        ));
        assert!(matches!(
            update(State(st.clone()), invite.clone(), Path(id), module_in("X")).await,
            Err(AppError::Forbidden)
        ));

        // L'invité lit et cherche.
        let Json(l) = list(
            State(st.clone()),
            invite.clone(),
            Path(s1),
            Query(SearchQ {
                q: Some("rebours".into()),
                category: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(l.as_array().unwrap().len(), 1);
        assert!(
            l[0].get("content").is_none(),
            "la liste ne renvoie pas le code"
        );
        let Json(g) = get(State(st.clone()), invite.clone(), Path(id))
            .await
            .unwrap();
        assert!(g["content"].as_str().unwrap().contains("creative_device"));
        assert_eq!(g["devices"][0], "timer_device");

        // Un autre studio ne voit rien.
        assert!(list(
            State(st.clone()),
            autre.clone(),
            Path(s1),
            Query(SearchQ::default())
        )
        .await
        .is_err());
        assert!(get(State(st.clone()), autre.clone(), Path(id))
            .await
            .is_err());
        let Json(vide) = list(
            State(st.clone()),
            autre.clone(),
            Path(s2),
            Query(SearchQ::default()),
        )
        .await
        .unwrap();
        assert_eq!(vide.as_array().unwrap().len(), 0);

        // Modification : la version monte ; nom en double refusé ; catégorie inconnue refusée.
        let Json(u) = update(
            State(st.clone()),
            m.clone(),
            Path(id),
            module_in("Minuteur de manche"),
        )
        .await
        .unwrap();
        assert_eq!(u["version"], 2);
        assert!(create(
            State(st.clone()),
            m.clone(),
            Path(s1),
            module_in("Minuteur de manche")
        )
        .await
        .is_err());
        let Json(mauvais) = module_in("Y");
        let mauvais = ModuleIn {
            category: Some("violet".into()),
            ..mauvais
        };
        assert!(
            create(State(st.clone()), m.clone(), Path(s1), Json(mauvais))
                .await
                .is_err()
        );

        // Filtre par catégorie, puis suppression.
        let Json(c) = list(
            State(st.clone()),
            m.clone(),
            Path(s1),
            Query(SearchQ {
                q: None,
                category: Some("hud".into()),
            }),
        )
        .await
        .unwrap();
        assert_eq!(c.as_array().unwrap().len(), 0);
        delete(State(st.clone()), m.clone(), Path(id))
            .await
            .unwrap();
        assert!(get(State(st.clone()), m.clone(), Path(id)).await.is_err());
    }
}
