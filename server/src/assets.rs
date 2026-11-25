//! Bibliothèque d'assets (#91) : FBX, images et sons de l'équipe POP et des studios, à ajouter aux projets UEFN.
//! Les fichiers vont dans un stockage objet compatible S3 (Cloudflare R2), jamais sur le disque du serveur : le
//! navigateur envoie le fichier directement au stockage avec un lien signé de 15 minutes, puis POP vérifie qu'il est
//! arrivé. Lecture par lien signé de 5 minutes. Les clés du stockage restent sur le serveur (variables d'environnement).
//!
//! Variables : `POP_S3_ENDPOINT` (ex. https://<compte>.r2.cloudflarestorage.com), `POP_S3_BUCKET`, `POP_S3_REGION`
//! (auto pour R2), `POP_S3_KEY_ID`, `POP_S3_SECRET`, `POP_ASSET_MAX_MB` (par fichier, 200 par défaut),
//! `POP_ASSET_STUDIO_MB` (par studio, 2000 par défaut).
use crate::{
    access::{studio_role, Role},
    auth::{random_token, AuthUser},
    error::{bad, check_len, ApiResult, AppError},
    s3::S3,
    AppState,
};
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub const TYPES: &[&str] = &[
    "fbx", "obj", "png", "jpg", "jpeg", "tga", "wav", "ogg", "mp3",
];

fn s3() -> Result<S3, AppError> {
    S3::from_env()
        .ok_or_else(|| bad("Le stockage des assets n’est pas encore configuré sur ce serveur POP."))
}

fn mo(var: &str, defaut: i64) -> i64 {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(defaut)
        * 1024
        * 1024
}

async fn est_admin(st: &AppState, u: &AuthUser) -> Result<bool, AppError> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT is_admin FROM users WHERE id = ?")
            .bind(u.id)
            .fetch_one(&st.db)
            .await?
            == 1,
    )
}

/// Assets visibles par un studio : les siens et ceux de l'équipe POP.
pub async fn list(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    let rows: Vec<(i64, Option<i64>, String, String, i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT a.id, a.studio_id, a.name, a.kind, a.size, a.origin, u.name, a.created_at FROM assets a LEFT JOIN users u ON u.id = a.author_id
         WHERE a.status = 'ready' AND (a.studio_id = ? OR a.studio_id IS NULL) ORDER BY a.origin, a.name",
    )
    .bind(sid)
    .fetch_all(&st.db)
    .await?;
    let utilise: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(size), 0) FROM assets WHERE studio_id = ?")
            .bind(sid)
            .fetch_one(&st.db)
            .await?;
    Ok(Json(json!({
        "assets": rows.into_iter().map(|r| json!({ "id": r.0, "pop": r.1.is_none(), "name": r.2, "kind": r.3, "size": r.4, "origin": r.5, "author": r.6, "created_at": r.7 })).collect::<Vec<_>>(),
        "quota": { "utilise": utilise, "max_studio": mo("POP_ASSET_STUDIO_MB", 2000), "max_fichier": mo("POP_ASSET_MAX_MB", 200) },
        "configure": S3::from_env().is_some(),
    })))
}

#[derive(Deserialize)]
pub struct NewAsset {
    name: String,
    size: i64,
    /// Déclaration de droits, obligatoire (ex. « Créé par notre équipe »).
    rights: String,
    /// true : asset de l'équipe POP, visible par tous (administrateurs du serveur seulement).
    #[serde(default)]
    pop: bool,
}

/// Prépare l'envoi : contrôles (type, taille, quota, droits), fiche « en cours », lien signé d'envoi (PUT, 15 min).
pub async fn create(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<NewAsset>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    if b.pop && !est_admin(&st, &u).await? {
        return Err(AppError::Forbidden);
    }
    check_len("Nom", &b.name, 1, 120)?;
    check_len("Déclaration de droits", &b.rights, 3, 300)?;
    let nom: String = b
        .name
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || "._- ".contains(*c))
        .collect();
    let kind = nom.rsplit('.').next().unwrap_or("").to_lowercase();
    if !nom.contains('.') || !TYPES.contains(&kind.as_str()) {
        return Err(bad(format!(
            "Type de fichier non accepté. Types : {}.",
            TYPES.join(", ")
        )));
    }
    let max = mo("POP_ASSET_MAX_MB", 200);
    if b.size <= 0 || b.size > max {
        return Err(bad(format!(
            "Fichier trop gros : {} Mo au plus.",
            max / 1024 / 1024
        )));
    }
    if !b.pop {
        let utilise: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(size), 0) FROM assets WHERE studio_id = ?")
                .bind(sid)
                .fetch_one(&st.db)
                .await?;
        let quota = mo("POP_ASSET_STUDIO_MB", 2000);
        if utilise + b.size > quota {
            return Err(bad(format!(
                "Espace du studio plein : {} Mo sur {} Mo. Supprimez des assets ou demandez plus d’espace.",
                utilise / 1024 / 1024,
                quota / 1024 / 1024
            )));
        }
    }
    let s3 = s3()?;
    let key = format!(
        "{}/{}/{}",
        if b.pop {
            "pop".to_string()
        } else {
            format!("studios/{sid}")
        },
        &random_token()[..20],
        nom.replace(' ', "_")
    );
    let id: i64 = sqlx::query_scalar("INSERT INTO assets (studio_id, name, kind, size, object_key, origin, rights, author_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(if b.pop { None } else { Some(sid) }).bind(&nom).bind(&kind).bind(b.size).bind(&key)
        .bind(if b.pop { "pop" } else { "studio" }).bind(b.rights.trim()).bind(u.id)
        .fetch_one(&st.db).await?;
    Ok(Json(
        json!({ "id": id, "upload_url": s3.presign("PUT", &key, 900, chrono::Utc::now()) }),
    ))
}

/// Fiche d'un asset et contrôle d'accès : (studio, clé, nom, statut, taille déclarée).
async fn fiche(
    st: &AppState,
    u: &AuthUser,
    id: i64,
    min: Role,
) -> Result<(Option<i64>, String, String, String, i64), AppError> {
    let r: (Option<i64>, String, String, String, i64) =
        sqlx::query_as("SELECT studio_id, object_key, name, status, size FROM assets WHERE id = ?")
            .bind(id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    match r.0 {
        Some(sid) => {
            if studio_role(st, u, sid).await? < min {
                return Err(AppError::Forbidden);
            }
        }
        // Asset de l'équipe POP : lisible par tous, modifiable par les administrateurs.
        None if min > Role::Viewer && !est_admin(st, u).await? => return Err(AppError::Forbidden),
        None => {}
    }
    Ok(r)
}

/// Après l'envoi : POP vérifie que le fichier est bien dans le stockage, avec une taille permise.
pub async fn confirm(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (_, key, _, _, declaree) = fiche(&st, &u, id, Role::Member).await?;
    let url = s3()?.presign("HEAD", &key, 60, chrono::Utc::now());
    let r = st
        .http
        .head(url)
        .send()
        .await
        .map_err(|_| bad("Stockage des assets injoignable."))?;
    if !r.status().is_success() {
        return Err(bad(
            "Le fichier n’est pas arrivé dans le stockage : renvoyez-le.",
        ));
    }
    let taille = r.content_length().unwrap_or(0) as i64;
    if taille == 0 || taille > mo("POP_ASSET_MAX_MB", 200) || taille > declaree * 2 {
        let _ = st
            .http
            .delete(s3()?.presign("DELETE", &key, 60, chrono::Utc::now()))
            .send()
            .await;
        sqlx::query("DELETE FROM assets WHERE id = ?")
            .bind(id)
            .execute(&st.db)
            .await?;
        return Err(bad("Taille du fichier refusée : l’asset a été retiré."));
    }
    sqlx::query("UPDATE assets SET status = 'ready', size = ? WHERE id = ?")
        .bind(taille)
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "id": id, "size": taille })))
}

/// Téléchargement : redirection vers un lien signé de 5 minutes.
pub async fn download(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let (_, key, _, status, _) = fiche(&st, &u, id, Role::Viewer).await?;
    if status != "ready" {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::temporary(&s3()?.presign("GET", &key, 300, chrono::Utc::now())).into_response())
}

pub async fn delete(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (_, key, _, _, _) = fiche(&st, &u, id, Role::Member).await?;
    let r = st
        .http
        .delete(s3()?.presign("DELETE", &key, 60, chrono::Utc::now()))
        .send()
        .await;
    if !matches!(&r, Ok(x) if x.status().is_success() || x.status().as_u16() == 404) {
        return Err(bad(
            "Le stockage n’a pas confirmé la suppression : réessayez.",
        ));
    }
    sqlx::query("DELETE FROM assets WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn monde() -> (AppState, i64, AuthUser, AuthUser, AuthUser) {
        let st = crate::test_state().await;
        let s1: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('A') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let s2: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('B') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mut us = vec![];
        for (e, s, r) in [
            ("m@a.fr", s1, "member"),
            ("v@a.fr", s1, "viewer"),
            ("o@b.fr", s2, "owner"),
        ] {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
            )
            .bind(e)
            .bind(e)
            .fetch_one(&st.db)
            .await
            .unwrap();
            sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, ?)")
                .bind(s)
                .bind(id)
                .bind(r)
                .execute(&st.db)
                .await
                .unwrap();
            us.push(AuthUser {
                id,
                email: e.into(),
                name: e.into(),
                sid: String::new(),
            });
        }
        (st, s1, us[0].clone(), us[1].clone(), us[2].clone())
    }

    fn envoi(name: &str, size: i64, rights: &str) -> Json<NewAsset> {
        Json(
            serde_json::from_value(json!({ "name": name, "size": size, "rights": rights }))
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn controles_et_droits() {
        std::env::set_var("POP_S3_ENDPOINT", "https://compte.r2.cloudflarestorage.com");
        std::env::set_var("POP_S3_BUCKET", "pop-assets");
        std::env::set_var("POP_S3_KEY_ID", "cle");
        std::env::set_var("POP_S3_SECRET", "secret");
        std::env::set_var("POP_ASSET_MAX_MB", "10");
        std::env::set_var("POP_ASSET_STUDIO_MB", "15");
        let (st, s1, m, v, o) = monde().await;
        let mb = 1024 * 1024;
        // Refus : invité, sans droits déclarés, mauvais type, trop gros.
        assert!(matches!(
            create(
                State(st.clone()),
                v.clone(),
                Path(s1),
                envoi("arbre.fbx", mb, "Créé par nous")
            )
            .await,
            Err(AppError::Forbidden)
        ));
        assert!(create(
            State(st.clone()),
            m.clone(),
            Path(s1),
            envoi("arbre.fbx", mb, "")
        )
        .await
        .is_err());
        assert!(create(
            State(st.clone()),
            m.clone(),
            Path(s1),
            envoi("outil.exe", mb, "Créé par nous")
        )
        .await
        .is_err());
        assert!(create(
            State(st.clone()),
            m.clone(),
            Path(s1),
            envoi("gros.fbx", 11 * mb, "Créé par nous")
        )
        .await
        .is_err());
        // Envoi accepté : lien signé vers R2 (par chemin), jamais vers le serveur.
        let Json(r) = create(
            State(st.clone()),
            m.clone(),
            Path(s1),
            envoi("arbre.fbx", 9 * mb, "Créé par notre équipe"),
        )
        .await
        .unwrap();
        let url = r["upload_url"].as_str().unwrap();
        assert!(
            url.starts_with("https://compte.r2.cloudflarestorage.com/pop-assets/studios/"),
            "{url}"
        );
        assert!(url.contains("X-Amz-Signature=") && !url.contains("secret"));
        // Quota du studio : 9 Mo déjà réservés sur 15.
        assert!(create(
            State(st.clone()),
            m.clone(),
            Path(s1),
            envoi("son.wav", 7 * mb, "Créé par nous")
        )
        .await
        .is_err());
        // Asset de l'équipe POP : réservé aux administrateurs du serveur.
        let pop = Json(
            serde_json::from_value::<NewAsset>(
                json!({ "name": "rocher.fbx", "size": mb, "rights": "Équipe POP", "pop": true }),
            )
            .unwrap(),
        );
        assert!(matches!(
            create(State(st.clone()), m.clone(), Path(s1), pop).await,
            Err(AppError::Forbidden)
        ));
        // Un autre studio ne voit pas la liste ni la fiche.
        assert!(list(State(st.clone()), o.clone(), Path(s1)).await.is_err());
        let id = r["id"].as_i64().unwrap();
        assert!(download(State(st.clone()), o.clone(), Path(id))
            .await
            .is_err());
        // Tant que l'envoi n'est pas confirmé, l'asset n'est pas listé.
        let Json(l) = list(State(st.clone()), v.clone(), Path(s1)).await.unwrap();
        assert_eq!(l["assets"].as_array().unwrap().len(), 0);
        sqlx::query("UPDATE assets SET status = 'ready' WHERE id = ?")
            .bind(id)
            .execute(&st.db)
            .await
            .unwrap();
        let Json(l) = list(State(st.clone()), v.clone(), Path(s1)).await.unwrap();
        assert_eq!(l["assets"][0]["name"], "arbre.fbx");
        // Lecture par l'invité : redirection vers un lien signé.
        let rep = download(State(st.clone()), v.clone(), Path(id))
            .await
            .unwrap();
        assert_eq!(rep.status(), axum::http::StatusCode::TEMPORARY_REDIRECT);
    }
}
