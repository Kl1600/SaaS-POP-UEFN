use crate::{
    access::{log_activity, map_ctx, studio_role, Role},
    auth::{random_token, sha256_hex, AuthUser},
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::FromRow;

pub const DEFAULT_CHECKS: &[(&str, &str, &str)] = &[
    (
        "title",
        "Titre",
        "Pas de mention de V-Bucks, d’XP ni de récompense trompeuse.",
    ),
    (
        "thumbnail",
        "Vignette",
        "Image fidèle au jeu, sans texte racoleur ni faux éléments d’interface.",
    ),
    (
        "description",
        "Description",
        "Décrit le vrai gameplay, sans promesse de gains.",
    ),
    (
        "tags",
        "Tags",
        "Tags pertinents, en nombre autorisé, cohérents avec le gameplay.",
    ),
    (
        "rating",
        "Classification d’âge",
        "Questionnaire de classification rempli et cohérent.",
    ),
    (
        "notes",
        "Notes de version",
        "Changements de la version résumés clairement.",
    ),
    (
        "memory",
        "Mémoire et performances",
        "Budget mémoire de l’île respecté sur toutes les plateformes.",
    ),
];

pub async fn seed_map_defaults(st: &AppState, map_id: i64) -> Result<(), AppError> {
    for (k, _, _) in DEFAULT_CHECKS {
        sqlx::query("INSERT OR IGNORE INTO publish_checks (map_id, key) VALUES (?, ?)")
            .bind(map_id)
            .bind(k)
            .execute(&st.db)
            .await?;
    }
    for (t, title, owner) in [
        (
            "10:00",
            "Vérifier que la version validée par Epic est la bonne",
            "",
        ),
        ("14:00", "Dernier playtest de contrôle (30 min)", ""),
        ("16:30", "Publier la version", ""),
        ("17:30", "Rappel aux testeurs inscrits", "Bot Discord"),
        ("18:00", "Ouverture et posts de lancement", ""),
        (
            "21:00",
            "Bilan : joueurs, durée moyenne, bugs",
            "Toute l’équipe",
        ),
    ] {
        sqlx::query("INSERT INTO launch_steps (map_id, time, title, owner) VALUES (?, ?, ?, ?)")
            .bind(map_id)
            .bind(t)
            .bind(title)
            .bind(owner)
            .execute(&st.db)
            .await?;
    }
    sqlx::query("INSERT INTO boards (map_id, title) VALUES (?, 'Plan de la map')")
        .bind(map_id)
        .execute(&st.db)
        .await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct NewStudio {
    name: String,
    /// Facultatif : un studio peut naître sans map.
    #[serde(default)]
    map_name: Option<String>,
    genre: Option<String>,
    release_date: Option<String>,
}

pub async fn create_studio(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<NewStudio>,
) -> ApiResult<Value> {
    check_len("Nom du studio", &b.name, 1, 60)?;
    let map_name = b.map_name.as_deref().map(str::trim).unwrap_or("");
    if !map_name.is_empty() {
        check_len("Nom de la map", map_name, 1, 60)?;
    }
    let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES (?) RETURNING id")
        .bind(b.name.trim())
        .fetch_one(&st.db)
        .await?;
    sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')")
        .bind(sid)
        .bind(u.id)
        .execute(&st.db)
        .await?;
    if map_name.is_empty() {
        return Ok(Json(json!({ "studio_id": sid, "map_id": null })));
    }
    let mid = insert_map(
        &st,
        sid,
        map_name,
        b.genre.as_deref().unwrap_or(""),
        b.release_date.as_deref(),
    )
    .await?;
    log_activity(
        &st,
        mid,
        Some(u.id),
        "map",
        "Map créée",
        &format!("{} a créé {}", u.name, map_name),
    )
    .await?;
    Ok(Json(json!({ "studio_id": sid, "map_id": mid })))
}

pub async fn insert_map(
    st: &AppState,
    sid: i64,
    name: &str,
    genre: &str,
    release: Option<&str>,
) -> Result<i64, AppError> {
    if let Some(d) = release {
        if !d.is_empty()
            && time::Date::parse(d, time::macros::format_description!("[year]-[month]-[day]"))
                .is_err()
        {
            return Err(bad("Date de sortie invalide (AAAA-MM-JJ)."));
        }
    }
    let mid: i64 = sqlx::query_scalar(
        "INSERT INTO maps (studio_id, name, genre, release_date) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(sid)
    .bind(name)
    .bind(genre)
    .bind(release.filter(|d| !d.is_empty()))
    .fetch_one(&st.db)
    .await?;
    seed_map_defaults(st, mid).await?;
    Ok(mid)
}

#[derive(Deserialize)]
pub struct NewMap {
    name: String,
    genre: Option<String>,
    release_date: Option<String>,
}

pub async fn create_map(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<NewMap>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    check_len("Nom de la map", &b.name, 1, 60)?;
    let mid = insert_map(
        &st,
        sid,
        b.name.trim(),
        b.genre.as_deref().unwrap_or(""),
        b.release_date.as_deref(),
    )
    .await?;
    Ok(Json(json!({ "map_id": mid })))
}

#[derive(Deserialize)]
pub struct MapPatch {
    name: Option<String>,
    genre: Option<String>,
    release_date: Option<String>,
    status: Option<String>,
}

pub async fn update_map(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<MapPatch>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Admin)?;
    if let Some(n) = &b.name {
        check_len("Nom", n, 1, 60)?;
        sqlx::query("UPDATE maps SET name = ? WHERE id = ?")
            .bind(n.trim())
            .bind(mid)
            .execute(&st.db)
            .await?;
    }
    if let Some(g) = &b.genre {
        sqlx::query("UPDATE maps SET genre = ? WHERE id = ?")
            .bind(g.trim())
            .bind(mid)
            .execute(&st.db)
            .await?;
    }
    if let Some(d) = &b.release_date {
        sqlx::query("UPDATE maps SET release_date = ? WHERE id = ?")
            .bind(d)
            .bind(mid)
            .execute(&st.db)
            .await?;
    }
    if let Some(s) = &b.status {
        sqlx::query("UPDATE maps SET status = ? WHERE id = ?")
            .bind(s)
            .bind(mid)
            .execute(&st.db)
            .await?;
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(FromRow, serde::Serialize)]
struct MemberRow {
    id: i64,
    name: String,
    email: String,
    role: String,
    specialty: String,
    portfolio: String,
    hours_per_week: i64,
}

pub async fn members(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    let my = studio_role(&st, &u, sid).await?;
    let rows: Vec<MemberRow> = sqlx::query_as(
        "SELECT u.id, u.name, u.email, m.role, m.specialty, m.portfolio, m.hours_per_week FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ? ORDER BY CASE m.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 WHEN 'member' THEN 2 WHEN 'freelance' THEN 3 ELSE 4 END, u.name",
    )
    .bind(sid)
    .fetch_all(&st.db)
    .await?;
    let invites: Vec<(i64, String, String, String)> = if my >= Role::Admin {
        sqlx::query_as("SELECT id, email, role, created_at FROM invitations WHERE studio_id = ? AND accepted_at IS NULL ORDER BY id DESC")
            .bind(sid)
            .fetch_all(&st.db)
            .await?
    } else {
        vec![]
    };
    Ok(Json(json!({
        "my_role": my.as_str(),
        "members": rows,
        "invitations": invites.into_iter().map(|(id, email, role, at)| json!({"id": id, "email": email, "role": role, "created_at": at})).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
pub struct MemberPatch {
    role: Option<String>,
    specialty: Option<String>,
    portfolio: Option<String>,
    hours_per_week: Option<i64>,
}

pub async fn update_member(
    State(st): State<AppState>,
    u: AuthUser,
    Path((sid, uid)): Path<(i64, i64)>,
    Json(b): Json<MemberPatch>,
) -> ApiResult<Value> {
    let my = studio_role(&st, &u, sid).await?;
    let target: (String,) =
        sqlx::query_as("SELECT role FROM memberships WHERE studio_id = ? AND user_id = ?")
            .bind(sid)
            .bind(uid)
            .fetch_one(&st.db)
            .await?;
    let self_edit = uid == u.id;
    if !self_edit && my < Role::Admin {
        return Err(AppError::Forbidden);
    }
    if let Some(r) = &b.role {
        let new = Role::parse(r).ok_or_else(|| bad("Rôle inconnu."))?;
        if my < Role::Admin
            || new == Role::Owner
            || target.0 == "owner"
            || (new == Role::Admin && my < Role::Owner)
        {
            return Err(AppError::Forbidden);
        }
        sqlx::query("UPDATE memberships SET role = ? WHERE studio_id = ? AND user_id = ?")
            .bind(new.as_str())
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await?;
    }
    if let Some(s) = &b.specialty {
        check_len("Spécialité", s, 0, 60)?;
        sqlx::query("UPDATE memberships SET specialty = ? WHERE studio_id = ? AND user_id = ?")
            .bind(s.trim())
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await?;
    }
    if let Some(p) = &b.portfolio {
        check_len("Portfolio", p, 0, 280)?;
        sqlx::query("UPDATE memberships SET portfolio = ? WHERE studio_id = ? AND user_id = ?")
            .bind(p.trim())
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await?;
    }
    if let Some(h) = b.hours_per_week {
        if !(0..=80).contains(&h) {
            return Err(bad("Heures par semaine entre 0 et 80."));
        }
        sqlx::query(
            "UPDATE memberships SET hours_per_week = ? WHERE studio_id = ? AND user_id = ?",
        )
        .bind(h)
        .bind(sid)
        .bind(uid)
        .execute(&st.db)
        .await?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove_member(
    State(st): State<AppState>,
    u: AuthUser,
    Path((sid, uid)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    let r = sqlx::query(
        "DELETE FROM memberships WHERE studio_id = ? AND user_id = ? AND role <> 'owner'",
    )
    .bind(sid)
    .bind(uid)
    .execute(&st.db)
    .await?;
    if r.rows_affected() == 0 {
        return Err(bad("Impossible de retirer ce membre."));
    }
    Ok(Json(json!({ "ok": true })))
}

/// Espace perso (maps faites seul) : pas d'invitations ni de gestion de studio.
pub async fn is_personal(st: &AppState, sid: i64) -> Result<bool, AppError> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT personal FROM studios WHERE id = ?")
            .bind(sid)
            .fetch_optional(&st.db)
            .await?
            .unwrap_or(0)
            == 1,
    )
}

/// Crée (si besoin) l'espace perso du compte et renvoie son id.
pub async fn ensure_personal(st: &AppState, uid: i64) -> Result<i64, AppError> {
    let found: Option<i64> = sqlx::query_scalar("SELECT s.id FROM studios s JOIN memberships m ON m.studio_id = s.id WHERE s.personal = 1 AND m.user_id = ? AND m.role = 'owner' LIMIT 1").bind(uid).fetch_optional(&st.db).await?;
    if let Some(id) = found {
        return Ok(id);
    }
    // Ancien studio créé automatiquement (« Studio de … », seul membre) : il devient l'espace perso.
    let auto: Option<i64> = sqlx::query_scalar(
        "SELECT s.id FROM studios s JOIN memberships m ON m.studio_id = s.id WHERE m.user_id = ? AND m.role = 'owner' AND s.name LIKE 'Studio de %'
           AND (SELECT COUNT(*) FROM memberships x WHERE x.studio_id = s.id) = 1 ORDER BY s.id LIMIT 1",
    ).bind(uid).fetch_optional(&st.db).await?;
    if let Some(id) = auto {
        sqlx::query("UPDATE studios SET personal = 1 WHERE id = ?")
            .bind(id)
            .execute(&st.db)
            .await?;
        return Ok(id);
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO studios (name, personal) VALUES ('Mes maps', 1) RETURNING id",
    )
    .fetch_one(&st.db)
    .await?;
    sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')")
        .bind(id)
        .bind(uid)
        .execute(&st.db)
        .await?;
    Ok(id)
}

#[derive(Deserialize)]
pub struct MoveIn {
    studio_id: Option<i64>,
    new_studio: Option<String>,
    /// Vrai : ne déplace rien, renvoie seulement ce que le déplacement changerait (pour la confirmation).
    #[serde(default)]
    preview: bool,
}

/// Passer une map dans un autre studio (ou dans un nouveau studio créé pour l'occasion), par exemple
/// pour travailler en équipe sur une map commencée seul, ou la reprendre seul. Il faut être admin des deux côtés.
/// Vers l'espace perso : toutes les tâches attribuées à quelqu'un d'autre reviennent à la personne
/// qui déplace la map, et les fichiers verrouillés par d'autres sont libérés (ils n'y ont plus accès).
pub async fn move_map(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<MoveIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Admin)?;
    let new_name = b
        .new_studio
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    if let Some(name) = new_name {
        check_len("Nom du studio", name, 1, 60)?;
    }
    if let (Some(sid), None) = (b.studio_id, new_name) {
        if studio_role(&st, &u, sid).await? < Role::Admin {
            return Err(AppError::Forbidden);
        }
        if sid == ctx.studio_id {
            return Ok(Json(
                json!({ "ok": true, "studio_id": sid, "reassigned": 0 }),
            ));
        }
    } else if new_name.is_none() {
        return Err(bad("Choisissez un studio."));
    }
    if !is_personal(&st, ctx.studio_id).await? {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM maps WHERE studio_id = ?")
            .bind(ctx.studio_id)
            .fetch_one(&st.db)
            .await?;
        if n <= 1 {
            return Err(bad(
                "C’est la dernière map du studio : elle ne peut pas le quitter.",
            ));
        }
    }
    let to_personal = match (b.studio_id, new_name) {
        (Some(sid), None) => is_personal(&st, sid).await?,
        _ => false,
    };
    let others: i64 = if to_personal {
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ? AND assignee_id IS NOT NULL AND assignee_id <> ?").bind(mid).bind(u.id).fetch_one(&st.db).await?
    } else {
        0
    };
    if b.preview {
        return Ok(Json(
            json!({ "ok": true, "preview": true, "to_personal": to_personal, "reassigned": others }),
        ));
    }

    // Tout ou rien : la map ne peut pas se retrouver à moitié déplacée.
    let mut tx = st.db.begin().await?;
    let target = match (b.studio_id, new_name) {
        (_, Some(name)) => {
            let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES (?) RETURNING id")
                .bind(name)
                .fetch_one(&mut *tx)
                .await?;
            sqlx::query(
                "INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'owner')",
            )
            .bind(sid)
            .bind(u.id)
            .execute(&mut *tx)
            .await?;
            sid
        }
        (Some(sid), None) => sid,
        _ => unreachable!(),
    };
    // Garde-fou si la map a été déplacée entre-temps par quelqu'un d'autre.
    let r = sqlx::query("UPDATE maps SET studio_id = ? WHERE id = ? AND studio_id = ?")
        .bind(target)
        .bind(mid)
        .bind(ctx.studio_id)
        .execute(&mut *tx)
        .await?;
    if r.rows_affected() == 0 {
        return Err(bad(
            "La map a changé de place entre-temps. Rechargez la page.",
        ));
    }
    let mut reassigned = 0;
    if to_personal {
        reassigned = sqlx::query("UPDATE tasks SET assignee_id = ? WHERE map_id = ? AND assignee_id IS NOT NULL AND assignee_id <> ?")
            .bind(u.id).bind(mid).bind(u.id).execute(&mut *tx).await?.rows_affected();
        sqlx::query(
            "DELETE FROM file_locks WHERE map_id = ? AND (user_id IS NULL OR user_id <> ?)",
        )
        .bind(mid)
        .bind(u.id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let (title, detail) = if to_personal {
        (
            "Map reprise en solo",
            if reassigned > 0 {
                format!("{} tâche(s) réattribuée(s) à {}", reassigned, u.name)
            } else {
                String::new()
            },
        )
    } else {
        ("Map passée dans un studio", String::new())
    };
    log_activity(&st, mid, Some(u.id), "team", title, &detail).await?;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "me" }));
    Ok(Json(
        json!({ "ok": true, "studio_id": target, "reassigned": reassigned }),
    ))
}

#[derive(Deserialize)]
pub struct InviteIn {
    email: String,
    role: String,
}

pub async fn invite(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<InviteIn>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    if is_personal(&st, sid).await? {
        return Err(bad("Vos maps perso sont pour travailler seul. Pour inviter quelqu’un, passez la map dans un studio (Équipe → Travailler en équipe)."));
    }
    let role = Role::parse(&b.role)
        .filter(|r| *r != Role::Owner)
        .ok_or_else(|| bad("Rôle invalide."))?;
    check_len("E-mail", &b.email, 3, 200)?;
    let token = random_token();
    sqlx::query("INSERT INTO invitations (studio_id, email, role, token_hash) VALUES (?, ?, ?, ?)")
        .bind(sid)
        .bind(b.email.trim().to_lowercase())
        .bind(role.as_str())
        .bind(sha256_hex(&token))
        .execute(&st.db)
        .await?;
    let mut mailed = false;
    if crate::mail::configured() {
        let sname: String = sqlx::query_scalar("SELECT name FROM studios WHERE id = ?")
            .bind(sid)
            .fetch_one(&st.db)
            .await?;
        let link = format!("{}/invitation/{token}", crate::mail::public_url());
        mailed = crate::mail::send(b.email.trim(), &format!("{} vous invite dans {sname} sur POP", u.name), &format!("Bonjour,\n\n{} vous invite à rejoindre le studio {sname} sur POP.\n\nRejoindre : {link}\n\nLe lien expire dans 7 jours.", u.name)).await.is_ok();
    }
    Ok(Json(
        json!({ "token": token, "link": format!("/invitation/{token}"), "mailed": mailed }),
    ))
}

pub async fn accept_invite_for(st: &AppState, user_id: i64, token: &str) -> Result<i64, AppError> {
    let row: Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT id, studio_id, role FROM invitations WHERE token_hash = ? AND accepted_at IS NULL AND created_at > datetime('now', '-7 days')",
    )
    .bind(sha256_hex(token))
    .fetch_optional(&st.db)
    .await?;
    let (iid, sid, role) = row.ok_or_else(|| bad("Invitation invalide ou expirée."))?;
    sqlx::query("INSERT OR IGNORE INTO memberships (studio_id, user_id, role) VALUES (?, ?, ?)")
        .bind(sid)
        .bind(user_id)
        .bind(role)
        .execute(&st.db)
        .await?;
    sqlx::query("UPDATE invitations SET accepted_at = datetime('now') WHERE id = ?")
        .bind(iid)
        .execute(&st.db)
        .await?;
    Ok(sid)
}

#[derive(Deserialize)]
pub struct AcceptIn {
    token: String,
}

pub async fn accept_invite(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<AcceptIn>,
) -> ApiResult<Value> {
    let sid = accept_invite_for(&st, u.id, &b.token).await?;
    Ok(Json(json!({ "studio_id": sid })))
}

pub async fn maps_summary(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    let maps: Vec<(i64, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, genre, status, release_date FROM maps WHERE studio_id = ? ORDER BY id",
    )
    .bind(sid)
    .fetch_all(&st.db)
    .await?;
    let members: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memberships WHERE studio_id = ?")
        .bind(sid)
        .fetch_one(&st.db)
        .await?;
    let mut out = vec![];
    for (id, name, genre, status, rd) in maps {
        let (total, done): (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE(SUM(status = 'done'), 0) FROM tasks WHERE map_id = ?",
        )
        .bind(id)
        .fetch_one(&st.db)
        .await?;
        let last: Option<(String, i64, i64)> = sqlx::query_as("SELECT month, engagement_cents, sales_cents FROM revenues WHERE map_id = ? ORDER BY month DESC LIMIT 1").bind(id).fetch_optional(&st.db).await?;
        out.push(json!({
            "id": id, "name": name, "genre": genre, "status": status, "release_date": rd, "members": members,
            "tasks": total, "done": done, "progress": if total > 0 { done * 100 / total } else { 0 },
            "last_month": last.as_ref().map(|l| l.0.clone()), "last_gains_cents": last.map(|l| l.1 + l.2).unwrap_or(0)
        }));
    }
    Ok(Json(json!(out)))
}

// ---------- Intégrations ----------

const SECRET_MASK: &str = "••••••••";

pub async fn integrations(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    let role = studio_role(&st, &u, sid).await?;
    let rows: Vec<(String, i64, String)> =
        sqlx::query_as("SELECT kind, enabled, config FROM integrations WHERE studio_id = ?")
            .bind(sid)
            .fetch_all(&st.db)
            .await?;
    let mut out = serde_json::Map::new();
    for kind in ["discord", "github", "lore", "apify", "youtube"] {
        let r = rows.iter().find(|r| r.0 == kind);
        let mut cfg: Value = r
            .map(|r| serde_json::from_str(&r.2).unwrap_or(json!({})))
            .unwrap_or(json!({}));
        if role < Role::Admin {
            cfg = json!({});
        }
        // Les secrets ne repartent jamais vers le navigateur.
        for k in ["bot_token", "token", "key"] {
            if cfg
                .get(k)
                .and_then(|v| v.as_str())
                .map(|v| !v.is_empty())
                .unwrap_or(false)
                && !(kind == "lore" && k == "token")
            {
                cfg[k] = json!(SECRET_MASK);
            }
        }
        out.insert(
            kind.into(),
            json!({ "enabled": r.map(|r| r.1 == 1).unwrap_or(false), "config": cfg }),
        );
    }
    Ok(Json(Value::Object(out)))
}

#[derive(Deserialize)]
pub struct IntegIn {
    enabled: bool,
    config: Option<Value>,
}

pub async fn set_integration(
    State(st): State<AppState>,
    u: AuthUser,
    Path((sid, kind)): Path<(i64, String)>,
    Json(b): Json<IntegIn>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    if !["discord", "github", "lore", "apify", "youtube"].contains(&kind.as_str()) {
        return Err(AppError::NotFound);
    }
    let mut cfg = b.config.unwrap_or(json!({}));
    if !cfg.is_object() {
        return Err(bad("Configuration invalide."));
    }
    // Secrets masqués côté navigateur : on garde la valeur enregistrée.
    let old: Option<String> =
        sqlx::query_scalar("SELECT config FROM integrations WHERE studio_id = ? AND kind = ?")
            .bind(sid)
            .bind(&kind)
            .fetch_optional(&st.db)
            .await?;
    let old: Value = old
        .and_then(|o| serde_json::from_str(&o).ok())
        .unwrap_or(json!({}));
    for k in ["bot_token", "token", "last_report", "secret", "key"] {
        let masked = cfg.get(k).and_then(|v| v.as_str()) == Some(SECRET_MASK)
            || (cfg.get(k).is_none() && old.get(k).is_some());
        if masked && !(kind == "lore" && k == "token") {
            cfg[k] = old.get(k).cloned().unwrap_or(json!(""));
        }
    }
    if kind == "github"
        && cfg
            .get("secret")
            .and_then(|t| t.as_str())
            .map(|t| t.is_empty())
            .unwrap_or(true)
    {
        cfg["secret"] = json!(random_token());
    }
    // Jeton du webhook Lore généré côté serveur.
    if kind == "lore" && b.enabled && cfg.get("token").and_then(|t| t.as_str()).is_none() {
        cfg["token"] = json!(random_token());
    }
    sqlx::query("INSERT INTO integrations (studio_id, kind, enabled, config) VALUES (?, ?, ?, ?) ON CONFLICT(studio_id, kind) DO UPDATE SET enabled = excluded.enabled, config = excluded.config")
        .bind(sid)
        .bind(&kind)
        .bind(b.enabled as i64)
        .bind(cfg.to_string())
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true, "config": cfg })))
}

#[derive(Deserialize)]
pub struct HookQ {
    token: String,
}

#[derive(Deserialize)]
pub struct LoreHook {
    map_id: i64,
    user_email: String,
    action: String, // checkin | lock | unlock
    message: Option<String>,
    files: Option<Vec<String>>,
}

/// Webhook appelé par un script de check-in Lore / UEFN. « #15 » relie le check-in à la tâche 15,
/// « fini #15 » la passe à Fait.
pub async fn lore_hook(
    State(st): State<AppState>,
    Path(sid): Path<i64>,
    Query(q): Query<HookQ>,
    Json(b): Json<LoreHook>,
) -> ApiResult<Value> {
    let cfg: Option<(i64, String)> = sqlx::query_as(
        "SELECT enabled, config FROM integrations WHERE studio_id = ? AND kind = 'lore'",
    )
    .bind(sid)
    .fetch_optional(&st.db)
    .await?;
    let (enabled, cfg) = cfg.ok_or(AppError::Unauthorized)?;
    let cfg: Value = serde_json::from_str(&cfg)?;
    let expected = cfg["token"].as_str().unwrap_or("");
    if enabled != 1 || expected.is_empty() || sha256_hex(expected) != sha256_hex(&q.token) {
        return Err(AppError::Unauthorized);
    }
    let map_ok: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM maps WHERE id = ? AND studio_id = ?")
            .bind(b.map_id)
            .bind(sid)
            .fetch_optional(&st.db)
            .await?;
    map_ok.ok_or(AppError::NotFound)?;
    let user: Option<(i64, String)> = sqlx::query_as("SELECT u.id, u.name FROM users u JOIN memberships m ON m.user_id = u.id AND m.studio_id = ? WHERE u.email = ?")
        .bind(sid)
        .bind(b.user_email.trim().to_lowercase())
        .fetch_optional(&st.db)
        .await?;
    let (uid, uname) = user.ok_or_else(|| bad("Membre inconnu."))?;
    let msg = b.message.unwrap_or_default();
    let num: Option<i64> = msg.split('#').nth(1).and_then(|s| {
        s.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()
    });
    let files = b.files.unwrap_or_default();
    match b.action.as_str() {
        "lock" => {
            for f in &files {
                sqlx::query("INSERT INTO file_locks (map_id, path, user_id, task_number) VALUES (?, ?, ?, ?) ON CONFLICT(map_id, path) DO UPDATE SET user_id = excluded.user_id, task_number = excluded.task_number, since = datetime('now')")
                    .bind(b.map_id).bind(f).bind(uid).bind(num).execute(&st.db).await?;
            }
        }
        "unlock" => {
            for f in &files {
                sqlx::query("DELETE FROM file_locks WHERE map_id = ? AND path = ?")
                    .bind(b.map_id)
                    .bind(f)
                    .execute(&st.db)
                    .await
                    .ok();
            }
        }
        _ => {
            for f in &files {
                sqlx::query("DELETE FROM file_locks WHERE map_id = ? AND path = ?")
                    .bind(b.map_id)
                    .bind(f)
                    .execute(&st.db)
                    .await
                    .ok();
            }
            if let Some(n) = num {
                let tid: Option<(i64,)> =
                    sqlx::query_as("SELECT id FROM tasks WHERE map_id = ? AND number = ?")
                        .bind(b.map_id)
                        .bind(n)
                        .fetch_optional(&st.db)
                        .await?;
                if let Some((tid,)) = tid {
                    sqlx::query("INSERT INTO task_events (task_id, user_id, what, via) VALUES (?, ?, ?, 'Lore / UEFN')")
                        .bind(tid).bind(uid).bind(format!("Check-in : {}", files.join(", "))).execute(&st.db).await?;
                    if msg.to_lowercase().contains("fini") {
                        // Le message de ce check-in Lore vaut check-in pour une tâche UEFN (#127).
                        match crate::tasks::set_status_checked(
                            &st,
                            b.map_id,
                            tid,
                            uid,
                            "done",
                            "Lore / UEFN",
                            Some(&msg),
                            None,
                        )
                        .await
                        {
                            // Message qui ne commence pas par #N : la tâche UEFN reste ouverte, le check-in est noté.
                            Err(AppError::Conflict(_) | AppError::BadRequest(_)) => {}
                            r => r?,
                        }
                    }
                }
            }
            log_activity(
                &st,
                b.map_id,
                Some(uid),
                "checkin",
                &format!("Check-in Lore de {uname}"),
                &msg.chars().take(140).collect::<String>(),
            )
            .await?;
        }
    }
    crate::access::notify(&st, b.map_id, json!({ "t": "refresh", "what": "tasks" }));
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn compte(st: &AppState) -> AuthUser {
        let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('s@exemple.com', 'Sam', 'x') RETURNING id")
            .fetch_one(&st.db).await.unwrap();
        AuthUser {
            id,
            email: "s@exemple.com".into(),
            name: "Sam".into(),
            sid: String::new(),
        }
    }
    fn studio(map: Option<&str>) -> Json<NewStudio> {
        Json(NewStudio {
            name: "POP Studio".into(),
            map_name: map.map(Into::into),
            genre: Some("Tycoon".into()),
            release_date: None,
        })
    }
    async fn maps(st: &AppState, sid: i64) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM maps WHERE studio_id = ?")
            .bind(sid)
            .fetch_one(&st.db)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn studio_avec_ou_sans_premiere_map() {
        let st = crate::test_state().await;
        let u = compte(&st).await;
        // Sans map (absente ou vide) : le studio existe, sans aucune map.
        for m in [None, Some("   ")] {
            let r = create_studio(State(st.clone()), u.clone(), studio(m))
                .await
                .unwrap()
                .0;
            assert!(r["map_id"].is_null());
            assert_eq!(maps(&st, r["studio_id"].as_i64().unwrap()).await, 0);
        }
        // Avec map : comme avant.
        let r = create_studio(State(st.clone()), u.clone(), studio(Some("VaultRush")))
            .await
            .unwrap()
            .0;
        assert!(r["map_id"].as_i64().is_some());
        assert_eq!(maps(&st, r["studio_id"].as_i64().unwrap()).await, 1);
        // Nom de map trop long : refusé.
        assert!(
            create_studio(State(st.clone()), u.clone(), studio(Some(&"x".repeat(61))))
                .await
                .is_err()
        );
    }
}
