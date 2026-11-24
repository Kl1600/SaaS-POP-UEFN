use crate::{
    access::{log_activity, map_ctx, notify, MapCtx, Role},
    auth::{random_token, sha256_hex, AuthUser},
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

fn b(t: &str, text: &str) -> Value {
    json!({ "id": random_token()[..10].to_string(), "type": t, "text": text })
}

/// Fiche d'une idée de map (onglet Idées → « Je me lance »), écrite à partir des chiffres de la niche.
pub fn idea_blocks(d: &Value) -> Value {
    let n = |k: &str| d[k].as_f64();
    let pct = |v: Option<f64>| {
        v.map(|x| format!("{} %", (x * 100.0).round()))
            .unwrap_or("—".into())
    };
    let num = |v: Option<f64>| {
        v.map(|x| format!("{}", x.round() as i64))
            .unwrap_or("—".into())
    };
    let mut v = vec![
        b("callout", &format!("Idée tirée de l’analyse du marché POP, le {}. Note d’opportunité : {}/100.", d["generated_day"].as_str().unwrap_or(""), num(n("score")))),
        b("h1", "Pourquoi cette niche"),
        b("bullet", &format!("{} maps jouées hier, {} joueurs au total ; une map typique en a {}.", num(n("islands")), num(n("players")), num(n("typical")))),
        b("bullet", &format!("Temps de jeu moyen : {} min (une map typique de Fortnite : {} min). Retour à J+1 : {}.", num(n("minutes")), num(d["eco"]["minutes"].as_f64()), pct(n("d1")))),
        b("bullet", &format!("Maps sorties depuis 30 jours : {}, dont {} au-dessus de 100 joueurs et {} au-dessus de 1 000.", num(n("newcomers")), num(n("success100")), num(n("success1000")))),
        b("bullet", &format!("Les 3 premières maps prennent {} % des joueurs.", num(n("top3_share")))),
        b("h2", "Chiffres à battre"),
        b("todo", &format!("Temps de jeu moyen d’au moins {} min", num(d["targets"]["minutes"].as_f64()))),
        b("todo", &format!("Retour à J+1 d’au moins {}", pct(d["targets"]["d1"].as_f64()))),
        b("h2", "Maps à étudier"),
    ];
    for m in d["study"].as_array().cloned().unwrap_or_default() {
        v.push(b(
            "bullet",
            &format!(
                "{} ({}) : {} joueurs hier",
                m["title"].as_str().unwrap_or(""),
                m["code"].as_str().unwrap_or(""),
                m["players"].as_i64().unwrap_or(0)
            ),
        ));
    }
    if let Some(m) = d["best_new"].as_object() {
        v.push(b(
            "bullet",
            &format!(
                "Meilleure nouvelle venue : {} ({}), jusqu’à {} joueurs",
                m["title"].as_str().unwrap_or(""),
                m["code"].as_str().unwrap_or(""),
                m["players"].as_i64().unwrap_or(0)
            ),
        ));
    }
    let tw: Vec<String> = d["twists"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|t| {
            format!(
                "{} ({} maps le font déjà)",
                t["fr"].as_str().unwrap_or(""),
                t["maps_together"].as_i64().unwrap_or(0)
            )
        })
        .collect();
    if !tw.is_empty() {
        v.push(b("h2", "Idées de twist"));
        for t in tw {
            v.push(b("bullet", &format!("Croiser avec : {t}")))
        }
    }
    v.push(b("h2", "Pièges à éviter"));
    let words: Vec<String> = d["title_words"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|w| w.as_str().map(String::from))
        .collect();
    if !words.is_empty() {
        v.push(b("bullet", &format!("Titres déjà très repris : {}. Un titre trop proche d’une map existante est moins montré dans Discover.", words.join(", "))))
    }
    v.push(b("bullet", "Vignette originale qui montre le vrai jeu : pas de fond uni ou en dégradé, pas d’armes collées, pas de badges d’Epic."));
    v.push(b(
        "bullet",
        &format!(
            "Genre à choisir dans le Creator Portal : {}.",
            d["genre"].as_str().unwrap_or("")
        ),
    ));
    v.push(b("h2", "Pitch"));
    v.push(b("p", "En une phrase : qui joue, pour faire quoi, et ce qui change par rapport aux maps à étudier."));
    json!(v)
}

/// Modèles UEFN prêts à remplir.
pub fn template(key: &str) -> Option<(&'static str, &'static str, Vec<Value>)> {
    Some(match key {
        "gdd" => (
            "Document de conception",
            "D",
            vec![
                b("h1", "Pitch"),
                b(
                    "p",
                    "En une phrase : qui joue, pour faire quoi, et pourquoi c’est fun.",
                ),
                b("h2", "Boucle de jeu"),
                b("bullet", "Action principale"),
                b("bullet", "Récompense"),
                b("bullet", "Progression"),
                b("h2", "Public visé"),
                b("p", ""),
                b("h2", "Références"),
                b("p", ""),
            ],
        ),
        "rules" => (
            "Règles et conventions",
            "R",
            vec![
                b("h2", "Règles de la map"),
                b(
                    "p",
                    "Objectif, conditions de victoire, équipes, durée d’une manche.",
                ),
                b("h2", "Tags Verse"),
                b("code", "# Exemple\nteam_police_tag := class(tag){}"),
                b("h2", "Noms des appareils"),
                b("bullet", "TRG_ pour les déclencheurs"),
                b("bullet", "BTN_ pour les boutons"),
                b("bullet", "SPN_ pour les points d’apparition"),
            ],
        ),
        "economy" => (
            "Économie",
            "€",
            vec![
                b("h2", "Monnaies"),
                b("p", "Comment on gagne, comment on dépense."),
                b(
                    "callout",
                    "Règle d’or : la première amélioration s’achète en moins de 4 minutes.",
                ),
                b("h2", "Prix"),
                json!({ "id": random_token()[..10].to_string(), "type": "table", "rows": [["Objet", "Prix", "Débloqué à"], ["", "", ""]] }),
            ],
        ),
        "decisions" => (
            "Journal des décisions",
            "J",
            vec![
                json!({ "id": random_token()[..10].to_string(), "type": "table", "rows": [["Date", "Décision", "Pourquoi", "Alternatives écartées"], ["", "", "", ""]] }),
            ],
        ),
        "patch" => (
            "Notes de patch",
            "N",
            vec![
                b("h2", "Nouveautés"),
                b("bullet", ""),
                b("h2", "Corrections"),
                b("bullet", ""),
                b("h2", "Problèmes connus"),
                b("bullet", ""),
            ],
        ),
        "playtest" => (
            "Compte rendu de playtest",
            "P",
            vec![
                b("h2", "Contexte"),
                b("p", "Date, nombre de joueurs, version testée."),
                b("h2", "Ce qui marche"),
                b("bullet", ""),
                b("h2", "Ce qui bloque"),
                b("bullet", ""),
                b("h2", "Décisions"),
                b("todo", ""),
            ],
        ),
        "postmortem" => (
            "Post-mortem",
            "M",
            vec![
                b("h2", "Objectifs"),
                b("p", ""),
                b("h2", "Ce qui a bien marché"),
                b("bullet", ""),
                b("h2", "Ce qu’on change"),
                b("bullet", ""),
                b("h2", "Chiffres"),
                b("p", ""),
            ],
        ),
        "submission" => (
            "Checklist de soumission Epic",
            "✓",
            vec![
                b("todo", "Titre sans mention de V-Bucks ni d’XP"),
                b("todo", "Vignette fidèle au jeu"),
                b("todo", "Description du vrai gameplay"),
                b("todo", "Tags cohérents"),
                b("todo", "Classification d’âge remplie"),
                b("todo", "Notes de version"),
            ],
        ),
        _ => return None,
    })
}

pub const TEMPLATE_KEYS: [&str; 8] = [
    "gdd",
    "rules",
    "economy",
    "decisions",
    "patch",
    "playtest",
    "postmortem",
    "submission",
];

pub async fn templates() -> Json<Value> {
    Json(json!(TEMPLATE_KEYS
        .iter()
        .map(|k| {
            let (t, i, _) = template(k).unwrap();
            json!({"key": k, "title": t, "icon": i})
        })
        .collect::<Vec<_>>()))
}

async fn page_ctx(st: &AppState, u: &AuthUser, pid: i64) -> Result<(MapCtx, Value), AppError> {
    let row: (i64, Option<i64>, String, String, String, String, i64, i64, Option<String>, String, Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT p.map_id, p.parent_id, p.title, p.icon, p.blocks, p.share, p.favorite, p.version, u.name, p.updated_at, p.created_by, p.share_token FROM pages p LEFT JOIN users u ON u.id = p.updated_by WHERE p.id = ?",
    )
    .bind(pid)
    .fetch_one(&st.db)
    .await?;
    let ctx = map_ctx(st, u, row.0).await?;
    if row.5 == "private" && row.10 != Some(u.id) {
        return Err(AppError::NotFound);
    }
    let blocks: Value = serde_json::from_str(&row.4).unwrap_or(json!([]));
    Ok((
        ctx,
        json!({
            "id": pid, "map_id": row.0, "parent_id": row.1, "title": row.2, "icon": row.3, "blocks": blocks, "share": row.5,
            "favorite": row.6 == 1, "version": row.7, "updated_by": row.8, "updated_at": row.9, "has_link": row.11.is_some()
        }),
    ))
}

pub async fn tree(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, Option<i64>, String, String, i64, String)> = sqlx::query_as(
        "SELECT id, parent_id, title, icon, favorite, share FROM pages WHERE map_id = ? AND deleted_at IS NULL AND (share <> 'private' OR created_by = ?) ORDER BY position, id",
    )
    .bind(mid)
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows.into_iter().map(|(id, parent, title, icon, fav, share)| json!({"id": id, "parent_id": parent, "title": title, "icon": icon, "favorite": fav == 1, "share": share})).collect::<Vec<_>>())))
}

pub async fn get(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
) -> ApiResult<Value> {
    let (_, p) = page_ctx(&st, &u, pid).await?;
    Ok(Json(p))
}

#[derive(Deserialize)]
pub struct NewPage {
    title: Option<String>,
    parent_id: Option<i64>,
    template: Option<String>,
}

pub async fn create(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(bd): Json<NewPage>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    let (mut title, mut icon, mut blocks) =
        ("Sans titre".to_string(), String::new(), vec![b("p", "")]);
    if let Some(k) = &bd.template {
        let (t, i, bl) = template(k).ok_or_else(|| bad("Modèle inconnu."))?;
        title = t.into();
        icon = i.into();
        blocks = bl;
    }
    if let Some(t) = &bd.title {
        check_len("Titre", t, 1, 120)?;
        title = t.trim().into();
    }
    if let Some(p) = bd.parent_id {
        let ok: Option<(i64,)> = sqlx::query_as("SELECT id FROM pages WHERE id = ? AND map_id = ?")
            .bind(p)
            .bind(mid)
            .fetch_optional(&st.db)
            .await?;
        ok.ok_or_else(|| bad("Page parente introuvable."))?;
    }
    let content = json!(blocks).to_string();
    let id: i64 = sqlx::query_scalar("INSERT INTO pages (map_id, parent_id, title, icon, blocks, created_by, updated_by) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(mid).bind(bd.parent_id).bind(&title).bind(&icon).bind(&content).bind(u.id).bind(u.id)
        .fetch_one(&st.db).await?;
    snapshot(&st, id, 1, &title, &content, u.id).await?;
    log_activity(&st, mid, Some(u.id), "page", "Page créée", &title).await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "pages" }));
    Ok(Json(json!({ "id": id })))
}

pub async fn snapshot(
    st: &AppState,
    pid: i64,
    version: i64,
    title: &str,
    blocks: &str,
    uid: i64,
) -> Result<(), AppError> {
    // L'empreinte SHA-256 horodatée sert de preuve d'antériorité.
    let digest = sha256_hex(&format!("{pid}|{version}|{title}|{blocks}"));
    sqlx::query("INSERT INTO page_versions (page_id, version, title, blocks, user_id, sha256) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(pid).bind(version).bind(title).bind(blocks).bind(uid).bind(digest)
        .execute(&st.db).await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct PagePatch {
    title: Option<String>,
    icon: Option<String>,
    blocks: Option<Value>,
    share: Option<String>,
    favorite: Option<bool>,
    parent_id: Option<Option<i64>>,
    client: Option<String>,
}

pub async fn update(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
    Json(bd): Json<PagePatch>,
) -> ApiResult<Value> {
    let (ctx, cur) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Freelance)?;
    let mut title = cur["title"].as_str().unwrap_or("").to_string();
    let mut content_changed = false;
    if let Some(t) = &bd.title {
        check_len("Titre", t, 0, 120)?;
        title = t.trim().to_string();
        content_changed = true;
    }
    if let Some(i) = &bd.icon {
        check_len("Icône", i, 0, 64)?;
        sqlx::query("UPDATE pages SET icon = ? WHERE id = ?")
            .bind(i)
            .bind(pid)
            .execute(&st.db)
            .await?;
    }
    let mut blocks = cur["blocks"].to_string();
    if let Some(bl) = &bd.blocks {
        if !bl.is_array() {
            return Err(bad("Blocs invalides."));
        }
        blocks = bl.to_string();
        if blocks.len() > 2_000_000 {
            return Err(bad("Page trop volumineuse."));
        }
        content_changed = true;
    }
    if let Some(s) = &bd.share {
        if !["private", "team", "link"].contains(&s.as_str()) {
            return Err(bad("Partage inconnu."));
        }
        if s == "private" && cur["share"] != "private" {
            sqlx::query("UPDATE pages SET created_by = ? WHERE id = ?")
                .bind(u.id)
                .bind(pid)
                .execute(&st.db)
                .await?;
        }
        let token = if s == "link" {
            Some(random_token())
        } else {
            None
        };
        sqlx::query("UPDATE pages SET share = ?, share_token = ? WHERE id = ?")
            .bind(s)
            .bind(token.as_ref().map(|t| sha256_hex(t)))
            .bind(pid)
            .execute(&st.db)
            .await?;
        if let Some(t) = token {
            return Ok(Json(json!({ "ok": true, "link": format!("/lien/{t}") })));
        }
    }
    if let Some(f) = bd.favorite {
        sqlx::query("UPDATE pages SET favorite = ? WHERE id = ?")
            .bind(f as i64)
            .bind(pid)
            .execute(&st.db)
            .await?;
    }
    if let Some(p) = bd.parent_id {
        if p == Some(pid) {
            return Err(bad("Une page ne peut pas être sa propre parente."));
        }
        sqlx::query("UPDATE pages SET parent_id = ? WHERE id = ?")
            .bind(p)
            .bind(pid)
            .execute(&st.db)
            .await?;
    }
    let mut version = cur["version"].as_i64().unwrap_or(1);
    if content_changed {
        version += 1;
        sqlx::query("UPDATE pages SET title = ?, blocks = ?, version = ?, updated_by = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(&title).bind(&blocks).bind(version).bind(u.id).bind(pid)
            .execute(&st.db).await?;
        // Une version conservée au plus toutes les 2 minutes par page.
        let recent: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM page_versions WHERE page_id = ? AND at > datetime('now', '-2 minutes')").bind(pid).fetch_one(&st.db).await?;
        if recent == 0 {
            snapshot(&st, pid, version, &title, &blocks, u.id).await?;
        }
    }
    notify(
        &st,
        ctx.map_id,
        json!({ "t": "page", "id": pid, "version": version, "by": u.id, "name": u.name, "client": bd.client }),
    );
    Ok(Json(json!({ "ok": true, "version": version })))
}

#[derive(Deserialize)]
pub struct DeleteQ {
    purge: Option<bool>,
}

/// Supprimer une page : elle va à la corbeille avec ses sous-pages ; « purge » l'efface définitivement.
pub async fn delete(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<DeleteQ>,
) -> ApiResult<Value> {
    let (ctx, p) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Member)?;
    if q.purge.unwrap_or(false) {
        sqlx::query("DELETE FROM pages WHERE id = ?")
            .bind(pid)
            .execute(&st.db)
            .await?;
    } else {
        sqlx::query("WITH RECURSIVE sub(id) AS (SELECT ? UNION ALL SELECT p.id FROM pages p JOIN sub ON p.parent_id = sub.id) UPDATE pages SET deleted_at = datetime('now') WHERE id IN (SELECT id FROM sub) AND deleted_at IS NULL")
            .bind(pid).execute(&st.db).await?;
        log_activity(
            &st,
            ctx.map_id,
            Some(u.id),
            "page",
            "Page mise à la corbeille",
            p["title"].as_str().unwrap_or(""),
        )
        .await?;
    }
    notify(&st, ctx.map_id, json!({ "t": "refresh", "what": "pages" }));
    Ok(Json(json!({ "ok": true })))
}

pub async fn trash(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let pages: Vec<(i64, String, String)> = sqlx::query_as("SELECT id, title, deleted_at FROM pages WHERE map_id = ? AND deleted_at IS NOT NULL AND (share <> 'private' OR created_by = ?) AND (parent_id IS NULL OR parent_id NOT IN (SELECT id FROM pages WHERE deleted_at IS NOT NULL)) ORDER BY deleted_at DESC").bind(mid).bind(u.id).fetch_all(&st.db).await?;
    let boards: Vec<(i64, String, String)> = sqlx::query_as("SELECT id, title, deleted_at FROM boards WHERE map_id = ? AND deleted_at IS NOT NULL ORDER BY deleted_at DESC").bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!({
        "pages": pages.into_iter().map(|(id, t, at)| json!({ "id": id, "title": t, "deleted_at": at })).collect::<Vec<_>>(),
        "boards": boards.into_iter().map(|(id, t, at)| json!({ "id": id, "title": t, "deleted_at": at })).collect::<Vec<_>>(),
    })))
}

pub async fn restore_page(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
) -> ApiResult<Value> {
    let (ctx, _) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Member)?;
    sqlx::query("WITH RECURSIVE sub(id) AS (SELECT ? UNION ALL SELECT p.id FROM pages p JOIN sub ON p.parent_id = sub.id) UPDATE pages SET deleted_at = NULL WHERE id IN (SELECT id FROM sub)")
        .bind(pid).execute(&st.db).await?;
    // Une page restaurée dont le parent est encore à la corbeille remonte à la racine.
    sqlx::query("UPDATE pages SET parent_id = NULL WHERE id = ? AND parent_id IN (SELECT id FROM pages WHERE deleted_at IS NOT NULL)").bind(pid).execute(&st.db).await?;
    notify(&st, ctx.map_id, json!({ "t": "refresh", "what": "pages" }));
    Ok(Json(json!({ "ok": true })))
}

pub async fn duplicate(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
) -> ApiResult<Value> {
    let (ctx, p) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Freelance)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO pages (map_id, parent_id, title, icon, blocks, share, created_by, updated_by) SELECT map_id, parent_id, title || ' (copie)', icon, blocks, share, ?, ? FROM pages WHERE id = ? RETURNING id")
        .bind(u.id).bind(u.id).bind(pid).fetch_one(&st.db).await?;
    let _ = p;
    notify(&st, ctx.map_id, json!({ "t": "refresh", "what": "pages" }));
    Ok(Json(json!({ "id": id })))
}

pub async fn versions(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
) -> ApiResult<Value> {
    page_ctx(&st, &u, pid).await?;
    let rows: Vec<(i64, i64, Option<String>, String, String)> = sqlx::query_as(
        "SELECT v.id, v.version, u.name, v.at, v.sha256 FROM page_versions v LEFT JOIN users u ON u.id = v.user_id WHERE v.page_id = ? ORDER BY v.id DESC LIMIT 100",
    )
    .bind(pid)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows.into_iter().map(|(id, v, who, at, sha)| json!({"id": id, "version": v, "who": who, "at": at, "sha256": sha})).collect::<Vec<_>>())))
}

pub async fn restore(
    State(st): State<AppState>,
    u: AuthUser,
    Path((pid, vid)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    let (ctx, cur) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Member)?;
    let (title, blocks): (String, String) =
        sqlx::query_as("SELECT title, blocks FROM page_versions WHERE id = ? AND page_id = ?")
            .bind(vid)
            .bind(pid)
            .fetch_one(&st.db)
            .await?;
    let version = cur["version"].as_i64().unwrap_or(1) + 1;
    sqlx::query("UPDATE pages SET title = ?, blocks = ?, version = ?, updated_by = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(&title).bind(&blocks).bind(version).bind(u.id).bind(pid).execute(&st.db).await?;
    snapshot(&st, pid, version, &title, &blocks, u.id).await?;
    notify(
        &st,
        ctx.map_id,
        json!({ "t": "page", "id": pid, "version": version, "by": u.id, "name": u.name }),
    );
    Ok(Json(json!({ "ok": true, "version": version })))
}

/// Historique daté exportable (preuve d'antériorité).
pub async fn proof(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let (_, cur) = page_ctx(&st, &u, pid).await?;
    let rows: Vec<(i64, Option<String>, String, String)> = sqlx::query_as(
        "SELECT v.version, u.name, v.at, v.sha256 FROM page_versions v LEFT JOIN users u ON u.id = v.user_id WHERE v.page_id = ? ORDER BY v.id",
    )
    .bind(pid)
    .fetch_all(&st.db)
    .await?;
    let mut out = format!("Historique daté de la page « {} »\nHorodatage serveur (UTC). Empreinte = SHA-256 du contenu de chaque version.\n\n", cur["title"].as_str().unwrap_or(""));
    for (v, who, at, sha) in rows {
        out.push_str(&format!(
            "v{v}\t{at} UTC\t{}\t{sha}\n",
            who.unwrap_or_default()
        ));
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"historique.txt\"",
            ),
        ],
        out,
    ))
}

fn to_markdown(title: &str, blocks: &Value) -> String {
    let mut md = format!("# {title}\n\n");
    for bl in blocks.as_array().cloned().unwrap_or_default() {
        let t = bl["text"].as_str().unwrap_or("");
        match bl["type"].as_str().unwrap_or("p") {
            "h1" => md.push_str(&format!("# {t}\n\n")),
            "h2" => md.push_str(&format!("## {t}\n\n")),
            "h3" => md.push_str(&format!("### {t}\n\n")),
            "bullet" => md.push_str(&format!("- {t}\n")),
            "number" => md.push_str(&format!("1. {t}\n")),
            "todo" => md.push_str(&format!(
                "- [{}] {t}\n",
                if bl["checked"].as_bool().unwrap_or(false) {
                    "x"
                } else {
                    " "
                }
            )),
            "quote" => md.push_str(&format!("> {t}\n\n")),
            "callout" => md.push_str(&format!("> **Note :** {t}\n\n")),
            "code" => md.push_str(&format!(
                "```{}\n{t}\n```\n\n",
                bl["lang"].as_str().unwrap_or("verse")
            )),
            "divider" => md.push_str("---\n\n"),
            "toggle" => md.push_str(&format!(
                "<details><summary>{t}</summary>\n\n{}\n\n</details>\n\n",
                bl["body"].as_str().unwrap_or("")
            )),
            "table" => {
                if let Some(rows) = bl["rows"].as_array() {
                    for (i, r) in rows.iter().enumerate() {
                        let cells: Vec<String> = r
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .iter()
                            .map(|c| c.as_str().unwrap_or("").replace('|', "\\|"))
                            .collect();
                        md.push_str(&format!("| {} |\n", cells.join(" | ")));
                        if i == 0 {
                            md.push_str(&format!("|{}|\n", vec![" --- "; cells.len()].join("|")));
                        }
                    }
                    md.push('\n');
                }
            }
            _ => md.push_str(&format!("{t}\n\n")),
        }
    }
    md
}

/// Markdown → blocs de page (#147) : titres, listes, cases à cocher, citations, code, séparateurs, tableaux et
/// paragraphes. Le contraire de `to_markdown`, pour les IA qui écrivent la documentation.
pub fn from_markdown(md: &str) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    let mut lines = md.lines().peekable();
    let mut para: Vec<String> = vec![];
    // Ligne qui suit un élément de liste ou une citation sans ligne vide : elle le continue (texte replié).
    let mut suite = false;
    let flush = |para: &mut Vec<String>, out: &mut Vec<Value>| {
        if !para.is_empty() {
            out.push(b("p", &para.join(" ")));
            para.clear();
        }
    };
    while let Some(l) = lines.next() {
        let s = l.trim_end();
        let t = s.trim_start();
        if t.is_empty() {
            flush(&mut para, &mut out);
            suite = false;
            continue;
        }
        if let Some(lang) = t.strip_prefix("```") {
            flush(&mut para, &mut out);
            suite = false;
            let mut code = vec![];
            for c in lines.by_ref() {
                if c.trim_start().starts_with("```") {
                    break;
                }
                code.push(c.to_string());
            }
            let mut bl = b("code", &code.join("\n"));
            bl["lang"] = json!(lang.trim());
            out.push(bl);
            continue;
        }
        if t.starts_with('|') {
            flush(&mut para, &mut out);
            suite = false;
            let mut rows = vec![];
            let mut cur = Some(t.to_string());
            while let Some(r) = cur {
                let cells: Vec<String> = r
                    .trim()
                    .trim_matches('|')
                    .split('|')
                    .map(|c| c.trim().replace("\\|", "|"))
                    .collect();
                if !cells
                    .iter()
                    .all(|c| !c.is_empty() && c.chars().all(|x| matches!(x, '-' | ':' | ' ')))
                {
                    rows.push(json!(cells));
                }
                cur = lines
                    .next_if(|n| n.trim_start().starts_with('|'))
                    .map(|n| n.trim().to_string());
            }
            out.push(
                json!({ "id": random_token()[..10].to_string(), "type": "table", "rows": rows }),
            );
            continue;
        }
        let (ty, rest) = if let Some(r) = ["###### ", "##### ", "#### ", "### "]
            .iter()
            .find_map(|p| t.strip_prefix(p))
        {
            ("h3", r)
        } else if let Some(r) = t.strip_prefix("## ") {
            ("h2", r)
        } else if let Some(r) = t.strip_prefix("# ") {
            ("h1", r)
        } else if let Some(r) = t.strip_prefix("- [ ] ").or(t.strip_prefix("* [ ] ")) {
            ("todo", r)
        } else if let Some(r) = t
            .strip_prefix("- [x] ")
            .or(t.strip_prefix("- [X] "))
            .or(t.strip_prefix("* [x] "))
        {
            flush(&mut para, &mut out);
            let mut bl = b("todo", r);
            bl["checked"] = json!(true);
            out.push(bl);
            suite = true;
            continue;
        } else if let Some(r) = t.strip_prefix("- ").or(t.strip_prefix("* ")) {
            ("bullet", r)
        } else if let Some(r) = t
            .split_once(". ")
            .filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            .map(|(_, r)| r)
        {
            ("number", r)
        } else if let Some(r) = t.strip_prefix("> ").or(t.strip_prefix(">")) {
            ("quote", r)
        } else if t == "---" || t == "***" {
            ("divider", "")
        } else {
            if suite && para.is_empty() {
                if let Some(dernier) = out.last_mut() {
                    let texte = format!("{} {t}", dernier["text"].as_str().unwrap_or(""));
                    dernier["text"] = json!(texte.trim());
                    continue;
                }
            }
            para.push(t.to_string());
            continue;
        };
        flush(&mut para, &mut out);
        suite = matches!(ty, "bullet" | "number" | "todo" | "quote");
        out.push(b(ty, rest));
    }
    flush(&mut para, &mut out);
    if out.is_empty() {
        out.push(b("p", ""));
    }
    out
}

#[derive(Deserialize)]
pub struct ImportFile {
    path: String,
    content: String,
}

#[derive(Deserialize)]
pub struct ImportIn {
    files: Vec<ImportFile>,
    parent_id: Option<i64>,
}

/// Fichiers Markdown importés en une fois (#129) : 300 fichiers, 1 Mo chacun, 10 Mo en tout.
const IMPORT_MAX: (usize, usize, usize) = (300, 1_000_000, 10_000_000);

/// Chemin propre d'un fichier importé : séparateurs `/`, sans `.` ni `..`.
fn chemin_propre(p: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for s in p.replace('\\', "/").split('/') {
        match s.trim() {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            x => out.push(x.to_string()),
        }
    }
    out
}

/// Réécrit les liens vers d'autres fichiers importés (`[CARTE](CARTE.md)`, `(../agents/web.md#x)`) en liens vers
/// leurs pages POP. Les autres liens ne bougent pas.
fn relier_liens(texte: &str, dossier: &[String], pages: &HashMap<String, i64>, mid: i64) -> String {
    let re = regex::Regex::new(r"\]\(([^)\s]+?\.md)(#[^)\s]*)?\)").unwrap();
    re.replace_all(texte, |c: &regex::Captures| {
        let cible = &c[1];
        if cible.contains("://") {
            return c[0].to_string();
        }
        let mut p: Vec<String> = dossier.to_vec();
        p.push(cible.to_string());
        let cle = chemin_propre(&p.join("/")).join("/").to_lowercase();
        match pages.get(&cle) {
            Some(id) => format!("](/map/{mid}/pages/{id})"),
            None => c[0].to_string(),
        }
    })
    .into_owned()
}

/// Import de fichiers Markdown en pages (#129) : un fichier = une page ; un dossier = une page parente ; les liens
/// entre fichiers importés deviennent des liens vers les pages POP ; pages partagées avec l'équipe.
pub async fn import_md(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(entree): Json<ImportIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    let (max_n, max_un, max_tout) = IMPORT_MAX;
    if entree.files.is_empty() {
        return Err(bad("Aucun fichier Markdown à importer."));
    }
    if entree.files.len() > max_n {
        return Err(bad(format!(
            "{} fichiers : {max_n} au plus par import.",
            entree.files.len()
        )));
    }
    if entree.files.iter().map(|f| f.content.len()).sum::<usize>() > max_tout {
        return Err(bad(
            "Plus de 10 Mo de Markdown : importez le dossier en plusieurs fois.",
        ));
    }
    for f in &entree.files {
        let nom = f.path.rsplit(['/', '\\']).next().unwrap_or(&f.path);
        let ext = nom
            .rsplit_once('.')
            .map(|(_, e)| e.to_lowercase())
            .unwrap_or_default();
        if !["md", "markdown"].contains(&ext.as_str()) {
            return Err(bad(format!(
                "« {nom} » n’est pas un fichier Markdown (.md) : rien n’a été importé."
            )));
        }
        if f.content.len() > max_un {
            return Err(bad(format!(
                "« {nom} » dépasse 1 Mo : rien n’a été importé."
            )));
        }
        if f.content.contains('\0') {
            return Err(bad(format!(
                "« {nom} » n’est pas du texte : rien n’a été importé."
            )));
        }
    }
    if let Some(p) = entree.parent_id {
        let ok: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM pages WHERE id = ? AND map_id = ? AND deleted_at IS NULL",
        )
        .bind(p)
        .bind(mid)
        .fetch_optional(&st.db)
        .await?;
        ok.ok_or_else(|| bad("Page parente introuvable."))?;
    }
    let mut fichiers: Vec<(Vec<String>, &str)> = entree
        .files
        .iter()
        .map(|f| (chemin_propre(&f.path), f.content.as_str()))
        .filter(|(c, _)| !c.is_empty())
        .collect();
    fichiers.sort_by(|a, b| a.0.cmp(&b.0));
    let mut tx = st.db.begin().await?;
    // 1. Une page par dossier (une seule fois), puis une page vide par fichier : les liens ont besoin de tous les ids.
    let mut dossiers: Vec<(String, i64)> = vec![];
    let mut pages: HashMap<String, i64> = HashMap::new();
    let mut ids = vec![];
    for (chemin, _) in &fichiers {
        let mut parent = entree.parent_id;
        for i in 0..chemin.len() - 1 {
            let cle = chemin[..=i].join("/").to_lowercase();
            let id = match dossiers.iter().find(|d| d.0 == cle) {
                Some(d) => d.1,
                None => {
                    let titre: String = chemin[i].chars().take(120).collect();
                    let id: i64 = sqlx::query_scalar("INSERT INTO pages (map_id, parent_id, title, icon, blocks, share, created_by, updated_by) VALUES (?, ?, ?, '', ?, 'team', ?, ?) RETURNING id")
                        .bind(mid).bind(parent).bind(&titre).bind(json!([b("p", "")]).to_string()).bind(u.id).bind(u.id)
                        .fetch_one(&mut *tx).await?;
                    dossiers.push((cle, id));
                    id
                }
            };
            parent = Some(id);
        }
        let nom = chemin.last().map(String::as_str).unwrap_or("");
        let titre: String = nom
            .rsplit_once('.')
            .map(|(n, _)| n)
            .unwrap_or(nom)
            .chars()
            .take(120)
            .collect();
        let id: i64 = sqlx::query_scalar("INSERT INTO pages (map_id, parent_id, title, icon, blocks, share, created_by, updated_by) VALUES (?, ?, ?, '', '[]', 'team', ?, ?) RETURNING id")
            .bind(mid).bind(parent).bind(&titre).bind(u.id).bind(u.id)
            .fetch_one(&mut *tx).await?;
        pages.insert(chemin.join("/").to_lowercase(), id);
        ids.push(id);
    }
    // 2. Contenu : liens entre fichiers réécrits ; un « # titre » en tête devient le titre de la page.
    for ((chemin, contenu), id) in fichiers.iter().zip(&ids) {
        let md = relier_liens(contenu, &chemin[..chemin.len() - 1], &pages, mid);
        let mut blocks = from_markdown(&md);
        let mut titre: Option<String> = None;
        if blocks.len() > 1 && blocks[0]["type"] == "h1" {
            titre = blocks[0]["text"]
                .as_str()
                .map(|s| s.chars().take(120).collect());
            blocks.remove(0);
        }
        sqlx::query("UPDATE pages SET blocks = ?, title = COALESCE(?, title) WHERE id = ?")
            .bind(json!(blocks).to_string())
            .bind(&titre)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    for id in dossiers.iter().map(|d| &d.1).chain(ids.iter()) {
        let (t, bl): (String, String) =
            sqlx::query_as("SELECT title, blocks FROM pages WHERE id = ?")
                .bind(id)
                .fetch_one(&st.db)
                .await?;
        snapshot(&st, *id, 1, &t, &bl, u.id).await?;
    }
    log_activity(
        &st,
        mid,
        Some(u.id),
        "page",
        "Pages importées",
        &format!("{} fichier(s) Markdown", ids.len()),
    )
    .await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "pages" }));
    let premiere = dossiers.first().map(|d| d.1).or(ids.first().copied());
    Ok(Json(
        json!({ "pages": ids.len(), "folders": dossiers.len(), "first": premiere }),
    ))
}

/// Page en Markdown, pour les IA (#147).
pub fn page_markdown(p: &Value) -> String {
    to_markdown(p["title"].as_str().unwrap_or(""), &p["blocks"])
}

pub async fn export_md(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let (_, p) = page_ctx(&st, &u, pid).await?;
    let md = to_markdown(p["title"].as_str().unwrap_or(""), &p["blocks"]);
    Ok((
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"page.md\"",
            ),
        ],
        md,
    ))
}

/// Lecture publique d'une page partagée par lien.
pub async fn public_page(
    State(st): State<AppState>,
    Path(token): Path<String>,
) -> ApiResult<Value> {
    let row: (String, String, String) = sqlx::query_as(
        "SELECT title, icon, blocks FROM pages WHERE share = 'link' AND share_token = ?",
    )
    .bind(sha256_hex(&token))
    .fetch_one(&st.db)
    .await?;
    Ok(Json(
        json!({ "title": row.0, "icon": row.1, "blocks": serde_json::from_str::<Value>(&row.2).unwrap_or(json!([])) }),
    ))
}

pub async fn page_comments(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
) -> ApiResult<Value> {
    let (ctx, _) = page_ctx(&st, &u, pid).await?;
    let rows: Vec<(i64, String, Option<String>, String, i64, String)> = sqlx::query_as(
        "SELECT c.id, c.target, u.name, c.body, c.resolved, c.at FROM comments c LEFT JOIN users u ON u.id = c.user_id WHERE c.map_id = ? AND c.target LIKE ? ORDER BY c.id",
    )
    .bind(ctx.map_id)
    .bind(format!("page:{pid}:%"))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows.into_iter().map(|(id, target, who, body, res, at)| json!({
        "id": id, "block": target.splitn(3, ':').nth(2).unwrap_or(""), "who": who, "body": body, "resolved": res == 1, "at": at
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct PageCommentIn {
    block: String,
    body: String,
}

pub async fn add_page_comment(
    State(st): State<AppState>,
    u: AuthUser,
    Path(pid): Path<i64>,
    Json(bd): Json<PageCommentIn>,
) -> ApiResult<Value> {
    let (ctx, p) = page_ctx(&st, &u, pid).await?;
    ctx.require(Role::Viewer)?;
    check_len("Commentaire", &bd.body, 1, 4000)?;
    check_len("Bloc", &bd.block, 1, 40)?;
    sqlx::query("INSERT INTO comments (map_id, target, user_id, body) VALUES (?, ?, ?, ?)")
        .bind(ctx.map_id)
        .bind(format!("page:{pid}:{}", bd.block))
        .bind(u.id)
        .bind(bd.body.trim())
        .execute(&st.db)
        .await?;
    log_activity(
        &st,
        ctx.map_id,
        Some(u.id),
        "comment",
        &format!("Commentaire sur « {} »", p["title"].as_str().unwrap_or("")),
        &bd.body.chars().take(120).collect::<String>(),
    )
    .await?;
    notify(&st, ctx.map_id, json!({ "t": "comments", "page": pid }));
    let mut to = crate::account::mentioned(&st, ctx.map_id, &bd.body).await;
    if let Some(c) = p["created_by"].as_i64() {
        to.push(c)
    }
    to.sort();
    to.dedup();
    for uid in to.into_iter().filter(|x| *x != u.id) {
        crate::account::notify_user(
            &st,
            uid,
            ctx.map_id,
            &format!(
                "{} a commenté « {} » : {}",
                u.name,
                p["title"].as_str().unwrap_or(""),
                bd.body.chars().take(100).collect::<String>()
            ),
            &format!("/pages/{pid}"),
        )
        .await;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn resolve_comment(
    State(st): State<AppState>,
    u: AuthUser,
    Path(cid): Path<i64>,
) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM comments WHERE id = ?")
        .bind(cid)
        .fetch_one(&st.db)
        .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    sqlx::query("UPDATE comments SET resolved = 1 - resolved WHERE id = ?")
        .bind(cid)
        .execute(&st.db)
        .await?;
    notify(&st, mid, json!({ "t": "comments" }));
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests_import {
    use super::*;

    async fn monde() -> (AppState, AuthUser, i64) {
        let st = crate::test_state().await;
        let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 = sqlx::query_scalar(
            "INSERT INTO maps (studio_id, name) VALUES (?, 'VaultRush') RETURNING id",
        )
        .bind(sid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        let uid: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('a@a.fr', 'Yanis', 'x') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        let u = AuthUser {
            id: uid,
            email: "a@a.fr".into(),
            name: "Yanis".into(),
            sid: String::new(),
        };
        (st, u, mid)
    }

    async fn importer(
        st: &AppState,
        u: &AuthUser,
        mid: i64,
        files: Value,
    ) -> Result<Value, AppError> {
        let b = serde_json::from_value(json!({ "files": files })).unwrap();
        import_md(State(st.clone()), u.clone(), Path(mid), Json(b))
            .await
            .map(|Json(v)| v)
    }

    const TOUT: &str = "# Carte\n\nIntro **gras**, *italique*, `code` et [lien](https://pop.example/x).\n\n## Zones\n- Banque\n- Toits\n\n1. Entrer\n2. Voler\n\n- [ ] Tester\n- [x] Poser\n\n> Attention aux gardes\n\n```verse\nPrint(\"Bonjour\")\n```\n\n| Zone | Joueurs |\n| --- | --- |\n| Banque | 4 |\n\n---\n\nVoir [les agents](agents/web.md) et [l'état](ETAT.md#haut).\n";

    /// #129 : dossier avec sous-dossier, liens entre fichiers, titres, partage équipe, aller-retour Markdown.
    #[tokio::test]
    async fn importer_un_dossier() {
        let (st, u, mid) = monde().await;
        let r = importer(&st, &u, mid, json!([
            { "path": "KB/CARTE.md", "content": TOUT },
            { "path": "KB/ETAT.md", "content": "État du projet." },
            { "path": "KB/agents/web.md", "content": "# Domaine web\n\nRetour à la [carte](../CARTE.md)." },
        ])).await.unwrap();
        assert_eq!(
            (r["pages"].as_i64(), r["folders"].as_i64()),
            (Some(3), Some(2)),
            "{r}"
        );
        let rows: Vec<(i64, Option<i64>, String, String, String)> = sqlx::query_as(
            "SELECT id, parent_id, title, share, blocks FROM pages WHERE map_id = ? ORDER BY id",
        )
        .bind(mid)
        .fetch_all(&st.db)
        .await
        .unwrap();
        let id = |t: &str| rows.iter().find(|r| r.2 == t).map(|r| r.0).unwrap();
        let parent = |t: &str| rows.iter().find(|r| r.2 == t).and_then(|r| r.1);
        // Arborescence : KB > (Carte, ETAT, agents > Domaine web) ; titres repris du « # » en tête.
        assert_eq!(parent("Carte"), Some(id("KB")));
        assert_eq!(parent("agents"), Some(id("KB")));
        assert_eq!(parent("Domaine web"), Some(id("agents")));
        assert!(rows.iter().all(|r| r.3 == "team"));
        // Liens entre fichiers → pages POP ; liens externes inchangés.
        let carte = &rows.iter().find(|r| r.2 == "Carte").unwrap().4;
        assert!(
            carte.contains(&format!(
                "[les agents](/map/{mid}/pages/{})",
                id("Domaine web")
            )),
            "{carte}"
        );
        assert!(
            carte.contains(&format!("[l'état](/map/{mid}/pages/{})", id("ETAT"))),
            "{carte}"
        );
        assert!(carte.contains("[lien](https://pop.example/x)"));
        let web = &rows.iter().find(|r| r.2 == "Domaine web").unwrap().4;
        assert!(
            web.contains(&format!("(/map/{mid}/pages/{})", id("Carte"))),
            "{web}"
        );
        // Chaque élément devient le bon bloc.
        let bl: Vec<Value> = serde_json::from_str(carte).unwrap();
        let types: Vec<&str> = bl.iter().map(|b| b["type"].as_str().unwrap()).collect();
        for t in [
            "p", "h2", "bullet", "number", "todo", "quote", "code", "table", "divider",
        ] {
            assert!(types.contains(&t), "{t} absent : {types:?}");
        }
        assert!(bl
            .iter()
            .any(|b| b["type"] == "todo" && b["checked"] == true));
        assert_eq!(
            bl.iter().find(|b| b["type"] == "table").unwrap()["rows"],
            json!([["Zone", "Joueurs"], ["Banque", "4"]])
        );
        // Un élément de liste replié sur deux lignes reste un seul élément.
        let l = from_markdown("- Adresse : x,\n  suite de l'adresse.\nAutre ligne\n\nParagraphe.");
        assert_eq!(l.len(), 2, "{l:?}");
        assert_eq!(l[0]["text"], "Adresse : x, suite de l'adresse. Autre ligne");
        // Aller-retour : l'export redonne le même Markdown, à la mise en forme près.
        let export = to_markdown("Carte", &json!(bl));
        let norm = |s: &str| {
            s.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace("| --- | --- |", "")
                .replace("|--- | --- |", "")
        };
        for morceau in [
            "**gras**",
            "*italique*",
            "`code`",
            "- Banque",
            "1. Voler",
            "- [x] Poser",
            "> Attention aux gardes",
            "```verse",
            "| Banque | 4 |",
            "---",
        ] {
            assert!(
                norm(&export).contains(morceau),
                "{morceau} absent de l'export :\n{export}"
            );
        }
        // Une version est gardée pour chaque page.
        let v: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM page_versions")
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(v, 5);
    }

    /// #129, critère 6 : fichier qui n'est pas du Markdown ou trop gros : refus clair, rien d'importé.
    #[tokio::test]
    async fn refus_clairs() {
        let (st, u, mid) = monde().await;
        for (files, attendu) in [
            (
                json!([{ "path": "notes.txt", "content": "x" }]),
                "n’est pas un fichier Markdown",
            ),
            (
                json!([{ "path": "a.md", "content": "x".repeat(1_000_001) }]),
                "dépasse 1 Mo",
            ),
            (
                json!([{ "path": "b.md", "content": "a\u{0}b" }]),
                "n’est pas du texte",
            ),
            (json!([]), "Aucun fichier"),
        ] {
            let e = importer(&st, &u, mid, files).await.unwrap_err();
            assert!(format!("{e}").contains(attendu), "{e}");
        }
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages")
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(n, 0);
    }
}
