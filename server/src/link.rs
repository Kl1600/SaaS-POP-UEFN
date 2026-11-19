//! Liaison UEFN simplifiée : POP fournit un pop-link.exe personnalisé (adresse + code intégrés).
//! Au premier lancement, le relais envoie les projets UEFN trouvés sur le PC ; POP crée une map par projet.
use crate::{
    access::{studio_role, Role},
    auth::{random_token, sha256_hex, AuthUser},
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

fn origin(h: &HeaderMap) -> String {
    if let Ok(u) = std::env::var("POP_PUBLIC_URL") {
        return u.trim_end_matches('/').to_string();
    }
    let host = h
        .get("x-forwarded-host")
        .or(h.get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("127.0.0.1:8080");
    let proto = h
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    format!("{proto}://{host}")
}

/// pop-link.exe avec, à la fin du fichier, l'adresse de POP et un code d'appairage valable 24 h.
pub async fn download(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let web = std::env::var("POP_WEB_DIR").unwrap_or_else(|_| "../web/dist".into());
    let exe = std::fs::read(format!("{web}/downloads/pop-link.exe"))
        .map_err(|_| AppError::Conflict("pop-link.exe introuvable sur le serveur.".into()))?;
    let code = random_token()[..24].to_string();
    sqlx::query("INSERT INTO studio_pairings (code_hash, studio_id, user_id, expires_at) VALUES (?, ?, ?, datetime('now', '+1 day'))")
        .bind(sha256_hex(&code)).bind(sid).bind(u.id).execute(&st.db).await?;
    let mut out = exe;
    out.extend_from_slice(
        format!(
            "\n#POPLINK#{}#END#",
            json!({ "server": origin(&headers), "code": code })
        )
        .as_bytes(),
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"pop-link.exe\"".to_string(),
            ),
        ],
        out,
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct ProjectIn {
    name: String,
}

#[derive(Deserialize)]
pub struct PairStudioIn {
    code: String,
    device: Option<String>,
    projects: Vec<ProjectIn>,
}

/// Appelé par le relais : une map par projet UEFN (réutilise la map du même nom), un jeton par map.
pub async fn pair_studio(
    State(st): State<AppState>,
    Json(b): Json<PairStudioIn>,
) -> ApiResult<Value> {
    let row: Option<(i64, i64)> = sqlx::query_as("SELECT studio_id, user_id FROM studio_pairings WHERE code_hash = ? AND expires_at > datetime('now')").bind(sha256_hex(b.code.trim())).fetch_optional(&st.db).await?;
    let (sid, uid) =
        row.ok_or_else(|| bad("Lien expiré : retéléchargez pop-link.exe depuis POP."))?;
    if b.projects.is_empty() {
        return Err(bad(
            "Aucun projet UEFN trouvé sur ce PC (Documents\\Fortnite Projects).",
        ));
    }
    let device = b
        .device
        .unwrap_or_else(|| "PC UEFN".into())
        .chars()
        .take(60)
        .collect::<String>();
    let mut links = vec![];
    for p in b.projects.iter().take(40) {
        let name: String = p.name.trim().chars().take(80).collect();
        if name.is_empty() {
            continue;
        }
        let existing: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM maps WHERE studio_id = ? AND lower(name) = lower(?)",
        )
        .bind(sid)
        .bind(&name)
        .fetch_optional(&st.db)
        .await?;
        let mid = match existing {
            Some(m) => m,
            None => crate::studio::insert_map(&st, sid, &name, "", None).await?,
        };
        // Un seul relais actif par map et par PC : on remplace l'ancien jeton du même appareil.
        sqlx::query("DELETE FROM relay_devices WHERE map_id = ? AND user_id = ? AND name = ?")
            .bind(mid)
            .bind(uid)
            .bind(&device)
            .execute(&st.db)
            .await?;
        let token = random_token();
        sqlx::query(
            "INSERT INTO relay_devices (map_id, user_id, name, token_hash) VALUES (?, ?, ?, ?)",
        )
        .bind(mid)
        .bind(uid)
        .bind(&device)
        .bind(sha256_hex(&token))
        .execute(&st.db)
        .await?;
        links.push(json!({ "project": name, "map_id": mid, "token": token }));
    }
    sqlx::query("UPDATE studio_pairings SET used_at = datetime('now') WHERE code_hash = ?")
        .bind(sha256_hex(b.code.trim()))
        .execute(&st.db)
        .await?;
    if let Some(m) = links.first().and_then(|l| l["map_id"].as_i64()) {
        crate::access::notify(&st, m, json!({ "t": "refresh", "what": "me" }));
    }
    Ok(Json(json!({ "links": links })))
}

/// Pour l'écran d'accueil : le studio a-t-il des maps et des relais connectés ?
pub async fn studio_status(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    let maps: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM maps WHERE studio_id = ? ORDER BY id")
            .bind(sid)
            .fetch_all(&st.db)
            .await?;
    Ok(Json(
        json!({ "maps": maps.iter().map(|(id, n)| json!({ "id": id, "name": n, "status": st.relays.status(*id) })).collect::<Vec<_>>() }),
    ))
}

/// « Continuer sans UEFN » : une première map vide.
pub async fn skip(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<Value>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let name = b["name"]
        .as_str()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("Ma première map")
        .chars()
        .take(80)
        .collect::<String>();
    let mid = crate::studio::insert_map(&st, sid, &name, "", None).await?;
    Ok(Json(json!({ "map_id": mid })))
}
