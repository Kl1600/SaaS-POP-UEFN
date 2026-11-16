//! Gestion du studio (renommer, supprimer, transférer, quitter), données personnelles (export, suppression)
//! et sauvegardes automatiques de la base.
use crate::{
    access::{log_activity, map_ctx, notify, studio_role, Role},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct NameIn {
    name: String,
}

pub async fn rename_studio(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>, Json(b): Json<NameIn>) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    check_len("Nom du studio", &b.name, 1, 80)?;
    sqlx::query("UPDATE studios SET name = ? WHERE id = ?").bind(b.name.trim()).bind(sid).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ConfirmIn {
    confirm: String,
}

/// Suppression définitive d'un studio : le propriétaire retape son nom.
pub async fn delete_studio(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>, Json(b): Json<ConfirmIn>) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? != Role::Owner {
        return Err(AppError::Forbidden);
    }
    let name: String = sqlx::query_scalar("SELECT name FROM studios WHERE id = ?").bind(sid).fetch_one(&st.db).await?;
    if b.confirm.trim() != name {
        return Err(bad("Le nom tapé ne correspond pas au nom du studio."));
    }
    let maps: Vec<i64> = sqlx::query_scalar("SELECT id FROM maps WHERE studio_id = ?").bind(sid).fetch_all(&st.db).await?;
    sqlx::query("DELETE FROM studios WHERE id = ?").bind(sid).execute(&st.db).await?;
    for m in maps {
        notify(&st, m, json!({ "t": "refresh", "what": "me" }));
    }
    Ok(Json(json!({ "ok": true })))
}

/// Suppression d'une map (propriétaire ou admin), sauf la dernière du studio.
pub async fn delete_map(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<ConfirmIn>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Admin)?;
    let (name,): (String,) = sqlx::query_as("SELECT name FROM maps WHERE id = ?").bind(mid).fetch_one(&st.db).await?;
    if b.confirm.trim() != name {
        return Err(bad("Le nom tapé ne correspond pas au nom de la map."));
    }
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM maps WHERE studio_id = ?").bind(ctx.studio_id).fetch_one(&st.db).await?;
    if n <= 1 {
        return Err(bad("C’est la dernière map du studio : créez-en une autre avant, ou supprimez le studio."));
    }
    sqlx::query("DELETE FROM maps WHERE id = ?").bind(mid).execute(&st.db).await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "me" }));
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct TransferIn {
    user_id: i64,
}

/// Le propriétaire passe la main ; il devient admin.
pub async fn transfer(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>, Json(b): Json<TransferIn>) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? != Role::Owner {
        return Err(AppError::Forbidden);
    }
    if b.user_id == u.id {
        return Err(bad("Vous êtes déjà propriétaire."));
    }
    let target: Option<String> = sqlx::query_scalar("SELECT role FROM memberships WHERE studio_id = ? AND user_id = ?").bind(sid).bind(b.user_id).fetch_optional(&st.db).await?;
    if target.is_none() {
        return Err(AppError::NotFound);
    }
    let mut tx = st.db.begin().await?;
    sqlx::query("UPDATE memberships SET role = 'admin' WHERE studio_id = ? AND user_id = ?").bind(sid).bind(u.id).execute(&mut *tx).await?;
    sqlx::query("UPDATE memberships SET role = 'owner' WHERE studio_id = ? AND user_id = ?").bind(sid).bind(b.user_id).execute(&mut *tx).await?;
    tx.commit().await?;
    let first_map: Option<i64> = sqlx::query_scalar("SELECT id FROM maps WHERE studio_id = ? ORDER BY id LIMIT 1").bind(sid).fetch_optional(&st.db).await?;
    if let Some(m) = first_map {
        crate::account::notify_user(&st, b.user_id, m, &format!("{} vous a transféré la propriété du studio", u.name), "/equipe").await;
        log_activity(&st, m, Some(u.id), "team", "Propriété du studio transférée", "").await?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn leave(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? == Role::Owner {
        return Err(bad("Le propriétaire ne peut pas quitter le studio : transférez d’abord la propriété, ou supprimez le studio."));
    }
    sqlx::query("DELETE FROM memberships WHERE studio_id = ? AND user_id = ?").bind(sid).bind(u.id).execute(&st.db).await?;
    sqlx::query("UPDATE tasks SET assignee_id = NULL WHERE assignee_id = ? AND map_id IN (SELECT id FROM maps WHERE studio_id = ?)").bind(u.id).bind(sid).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn cancel_invite(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>) -> ApiResult<Value> {
    let sid: i64 = sqlx::query_scalar("SELECT studio_id FROM invitations WHERE id = ? AND accepted_at IS NULL").bind(id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)?;
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    sqlx::query("DELETE FROM invitations WHERE id = ?").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------- Exports ----------

/// Toutes les lignes d'une table qui vérifient une condition, en JSON (colonnes lues dans le schéma).
async fn rows(st: &AppState, table: &str, cond: &str, binds: &[i64]) -> Result<Value, AppError> {
    let cols: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(&format!("PRAGMA table_info({table})")).fetch_all(&st.db).await?;
    let skip = ["password_hash", "token_hash", "pair_code_hash"];
    let obj = cols.iter().filter(|c| !skip.contains(&c.1.as_str())).map(|c| format!("'{0}', \"{0}\"", c.1)).collect::<Vec<_>>().join(", ");
    let sql = format!("SELECT COALESCE(json_group_array(json_object({obj})), '[]') FROM {table} WHERE {cond}");
    let mut q = sqlx::query_scalar::<_, String>(&sql);
    for b in binds {
        q = q.bind(*b);
    }
    let s = q.fetch_one(&st.db).await?;
    Ok(serde_json::from_str(&s).unwrap_or(json!([])))
}

fn download(name: &str, v: &Value) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8".to_string()), (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\""))],
        serde_json::to_string_pretty(v).unwrap_or_default(),
    )
        .into_response()
}

const MAP_TABLES: &[&str] = &[
    "tasks", "milestones", "activity", "pages", "boards", "comments", "revenues", "share_rules", "offers", "publish_checks", "launch_steps",
    "playtest_listings", "file_locks", "verse_files", "verse_versions", "uefn_actions", "approvals", "saved_filters", "uefn_state",
];

/// Export complet d'un studio (propriétaire ou admin) : sauvegarde lisible de tout son travail.
pub async fn export_studio(State(st): State<AppState>, u: AuthUser, Path(sid): Path<i64>) -> Result<Response, AppError> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    let mut out = serde_json::Map::new();
    out.insert("format".into(), json!("pop-export-1"));
    out.insert("exported_at".into(), json!(chrono::Utc::now().to_rfc3339()));
    out.insert("studio".into(), rows(&st, "studios", "id = ?", &[sid]).await?);
    out.insert("members".into(), json!(sqlx::query_as::<_, (i64, String, String, String)>("SELECT u.id, u.name, u.email, m.role FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ?").bind(sid).fetch_all(&st.db).await?.into_iter().map(|(id, n, e, r)| json!({ "id": id, "name": n, "email": e, "role": r })).collect::<Vec<_>>()));
    out.insert("maps".into(), rows(&st, "maps", "studio_id = ?", &[sid]).await?);
    for t in MAP_TABLES {
        out.insert((*t).into(), rows(&st, t, "map_id IN (SELECT id FROM maps WHERE studio_id = ?)", &[sid]).await?);
    }
    out.insert("task_deps".into(), rows(&st, "task_deps", "task_id IN (SELECT t.id FROM tasks t JOIN maps m ON m.id = t.map_id WHERE m.studio_id = ?)", &[sid]).await?);
    out.insert("subtasks".into(), rows(&st, "subtasks", "task_id IN (SELECT t.id FROM tasks t JOIN maps m ON m.id = t.map_id WHERE m.studio_id = ?)", &[sid]).await?);
    out.insert("page_versions".into(), rows(&st, "page_versions", "page_id IN (SELECT p.id FROM pages p JOIN maps m ON m.id = p.map_id WHERE m.studio_id = ?)", &[sid]).await?);
    let name: String = sqlx::query_scalar("SELECT name FROM studios WHERE id = ?").bind(sid).fetch_one(&st.db).await?;
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    Ok(download(&format!("pop-{safe}-{}.json", chrono::Utc::now().format("%Y-%m-%d")), &Value::Object(out)))
}

/// Export RGPD : tout ce que POP sait de la personne connectée.
pub async fn export_me(State(st): State<AppState>, u: AuthUser) -> Result<Response, AppError> {
    let mut out = serde_json::Map::new();
    out.insert("compte".into(), rows(&st, "users", "id = ?", &[u.id]).await?);
    out.insert("studios".into(), rows(&st, "memberships", "user_id = ?", &[u.id]).await?);
    out.insert("taches_assignees".into(), rows(&st, "tasks", "assignee_id = ?", &[u.id]).await?);
    out.insert("commentaires".into(), rows(&st, "comments", "user_id = ?", &[u.id]).await?);
    out.insert("pages_creees".into(), rows(&st, "pages", "created_by = ?", &[u.id]).await?);
    out.insert("activite".into(), rows(&st, "activity", "user_id = ?", &[u.id]).await?);
    out.insert("notifications".into(), rows(&st, "notifications", "user_id = ?", &[u.id]).await?);
    out.insert("actions_uefn".into(), rows(&st, "uefn_actions", "user_id = ?", &[u.id]).await?);
    out.insert("retours_playtest".into(), rows(&st, "playtest_feedback", "tester_id = ?", &[u.id]).await?);
    out.insert("relais_uefn".into(), rows(&st, "relay_devices", "user_id = ?", &[u.id]).await?);
    out.insert("jetons_ia".into(), rows(&st, "api_tokens", "user_id = ?", &[u.id]).await?);
    out.insert("sessions".into(), rows(&st, "sessions", "user_id = ?", &[u.id]).await?);
    Ok(download("pop-mes-donnees.json", &Value::Object(out)))
}

#[derive(Deserialize)]
pub struct DeleteMeIn {
    password: String,
}

/// Suppression du compte. Un propriétaire doit d'abord transférer ses studios partagés ; ses studios
/// sans autre membre sont supprimés avec le compte.
pub async fn delete_me(State(st): State<AppState>, u: AuthUser, Json(b): Json<DeleteMeIn>) -> ApiResult<Value> {
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    if !crate::auth::verify_password(&b.password, &hash) {
        return Err(bad("Mot de passe incorrect."));
    }
    let owned: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT s.id, s.name, (SELECT COUNT(*) FROM memberships x WHERE x.studio_id = s.id) FROM memberships m JOIN studios s ON s.id = m.studio_id WHERE m.user_id = ? AND m.role = 'owner'",
    )
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    let shared: Vec<String> = owned.iter().filter(|o| o.2 > 1).map(|o| o.1.clone()).collect();
    if !shared.is_empty() {
        return Err(bad(format!("Transférez d’abord la propriété de : {} (Équipe et intégrations → Membres).", shared.join(", "))));
    }
    let mut tx = st.db.begin().await?;
    for (sid, _, _) in &owned {
        sqlx::query("DELETE FROM studios WHERE id = ?").bind(sid).execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM users WHERE id = ?").bind(u.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------- Sauvegardes ----------

/// Copie cohérente de la base chaque jour (VACUUM INTO), 14 jours gardés, dans « backups/ » à côté de la base.
pub async fn backup_loop(st: AppState, db_url: String) {
    let path = db_url.trim_start_matches("sqlite://").trim_start_matches("sqlite:").to_string();
    let dir = std::path::Path::new(&path).parent().map(|p| p.join("backups")).unwrap_or_else(|| "backups".into());
    let dir = if dir.as_os_str().is_empty() || dir == std::path::Path::new("backups") { std::path::PathBuf::from("backups") } else { dir };
    loop {
        let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let file = dir.join(format!("pop-{day}.db"));
        if !file.exists() && std::fs::create_dir_all(&dir).is_ok() {
            match sqlx::query(&format!("VACUUM INTO '{}'", file.to_string_lossy().replace('\'', "''"))).execute(&st.db).await {
                Ok(_) => tracing::info!("sauvegarde : {}", file.display()),
                Err(e) => tracing::warn!("sauvegarde impossible : {e}"),
            }
            if let Ok(rd) = std::fs::read_dir(&dir) {
                let mut olds: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.file_name().map(|n| n.to_string_lossy().starts_with("pop-")).unwrap_or(false)).collect();
                olds.sort();
                while olds.len() > 14 {
                    let _ = std::fs::remove_file(olds.remove(0));
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
