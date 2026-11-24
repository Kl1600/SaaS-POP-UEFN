use crate::{
    access::{log_activity, map_ctx, notify, Role},
    auth::{random_token, AuthUser},
    error::{bad, check_len, ApiResult, AppError},
    tasks::{insert_task, NewTask},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub fn oid() -> String {
    random_token()[..12].to_string()
}

fn sticky(x: f64, y: f64, text: &str, shade: u8) -> Value {
    json!({ "id": oid(), "kind": "sticky", "x": x, "y": y, "w": 160, "h": 120, "text": text, "shade": shade, "votes": {} })
}
fn frame(x: f64, y: f64, w: f64, h: f64, title: &str) -> Value {
    json!({ "id": oid(), "kind": "frame", "x": x, "y": y, "w": w, "h": h, "text": title })
}

pub fn board_template(key: &str) -> Option<(&'static str, Vec<Value>)> {
    Some(match key {
        "brainstorm" => (
            "Brainstorm d’idées de map",
            vec![
                frame(0., 0., 900., 500., "Idées"),
                sticky(40., 60., "Idée 1", 0),
                sticky(240., 60., "Idée 2", 1),
            ],
        ),
        "plan" => (
            "Plan de map et parcours joueur",
            vec![
                frame(0., 0., 1000., 640., "Plan de la map"),
                json!({"id": oid(), "kind": "uefn", "shape": "spawn", "x": 60, "y": 80, "text": "Apparition"}),
            ],
        ),
        "loop" => (
            "Boucle de gameplay",
            vec![
                frame(0., 0., 300., 200., "Action"),
                frame(400., 0., 300., 200., "Récompense"),
                frame(200., 300., 300., 200., "Progression"),
            ],
        ),
        "journey" => (
            "Parcours du joueur",
            [
                "Découverte",
                "Première partie",
                "Accroche",
                "Retour",
                "Fidélité",
            ]
            .iter()
            .enumerate()
            .map(|(i, t)| frame(i as f64 * 260., 0., 240., 400., t))
            .collect(),
        ),
        "mockups" => (
            "Maquettes d’écrans",
            vec![
                frame(0., 0., 640., 360., "Écran 16:9"),
                frame(700., 0., 220., 440., "Mobile"),
            ],
        ),
        "retro" => (
            "Rétrospective",
            ["À garder", "À changer", "À essayer"]
                .iter()
                .enumerate()
                .map(|(i, t)| frame(i as f64 * 340., 0., 320., 480., t))
                .collect(),
        ),
        "roadmap" => (
            "Feuille de route par saison",
            ["Saison 1", "Saison 2", "Saison 3"]
                .iter()
                .enumerate()
                .map(|(i, t)| frame(i as f64 * 340., 0., 320., 480., t))
                .collect(),
        ),
        "matrix" => (
            "Matrice impact / effort",
            vec![
                frame(0., 0., 400., 300., "Impact fort, effort faible"),
                frame(420., 0., 400., 300., "Impact fort, effort fort"),
                frame(0., 320., 400., 300., "Impact faible, effort faible"),
                frame(420., 320., 400., 300., "Impact faible, effort fort"),
            ],
        ),
        _ => return None,
    })
}

fn to_map(list: Vec<Value>) -> Value {
    let mut m = Map::new();
    for o in list {
        m.insert(o["id"].as_str().unwrap().to_string(), o);
    }
    m.insert(
        "__meta".into(),
        json!({ "points": 3, "hidden": false, "timer_end": null }),
    );
    Value::Object(m)
}

pub async fn list(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, String)> = sqlx::query_as("SELECT id, title, updated_at FROM boards WHERE map_id = ? AND deleted_at IS NULL ORDER BY id").bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, t, at)| json!({"id": id, "title": t, "updated_at": at}))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct NewBoard {
    title: Option<String>,
    template: Option<String>,
}

pub async fn create(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<NewBoard>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    let (mut title, objs) = match &b.template {
        Some(k) => {
            let (t, o) = board_template(k).ok_or_else(|| bad("Modèle inconnu."))?;
            (t.to_string(), o)
        }
        None => ("Nouveau tableau".to_string(), vec![]),
    };
    if let Some(t) = &b.title {
        check_len("Titre", t, 1, 80)?;
        title = t.trim().into();
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO boards (map_id, title, objects) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(mid)
    .bind(&title)
    .bind(to_map(objs).to_string())
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({ "id": id })))
}

pub async fn get(
    State(st): State<AppState>,
    u: AuthUser,
    Path(bid): Path<i64>,
) -> ApiResult<Value> {
    let (mid, title, objects): (i64, String, String) =
        sqlx::query_as("SELECT map_id, title, objects FROM boards WHERE id = ?")
            .bind(bid)
            .fetch_one(&st.db)
            .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    let mut objs: Value = serde_json::from_str(&objects).unwrap_or(json!({}));
    if objs.get("__meta").is_none() {
        objs["__meta"] = json!({ "points": 3, "hidden": false, "timer_end": null });
    }
    Ok(Json(
        json!({ "id": bid, "map_id": mid, "title": title, "objects": objs, "can_edit": ctx.can_edit().is_ok(), "me": u.id }),
    ))
}

#[derive(Deserialize)]
pub struct BoardPatch {
    title: Option<String>,
}

pub async fn update(
    State(st): State<AppState>,
    u: AuthUser,
    Path(bid): Path<i64>,
    Json(b): Json<BoardPatch>,
) -> ApiResult<Value> {
    let mid: i64 = sqlx::query_scalar("SELECT map_id FROM boards WHERE id = ?")
        .bind(bid)
        .fetch_one(&st.db)
        .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    if let Some(t) = &b.title {
        check_len("Titre", t, 1, 80)?;
        sqlx::query("UPDATE boards SET title = ? WHERE id = ?")
            .bind(t.trim())
            .bind(bid)
            .execute(&st.db)
            .await?;
    }
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "boards" }));
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct DeleteQ {
    purge: Option<bool>,
    restore: Option<bool>,
}

/// Corbeille des tableaux : supprimer, restaurer, ou effacer définitivement.
pub async fn delete(
    State(st): State<AppState>,
    u: AuthUser,
    Path(bid): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<DeleteQ>,
) -> ApiResult<Value> {
    let mid: i64 = sqlx::query_scalar("SELECT map_id FROM boards WHERE id = ?")
        .bind(bid)
        .fetch_one(&st.db)
        .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    if q.purge.unwrap_or(false) {
        sqlx::query("DELETE FROM boards WHERE id = ?")
            .bind(bid)
            .execute(&st.db)
            .await?;
    } else if q.restore.unwrap_or(false) {
        sqlx::query("UPDATE boards SET deleted_at = NULL WHERE id = ?")
            .bind(bid)
            .execute(&st.db)
            .await?;
    } else {
        sqlx::query("UPDATE boards SET deleted_at = datetime('now') WHERE id = ?")
            .bind(bid)
            .execute(&st.db)
            .await?;
    }
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "boards" }));
    Ok(Json(json!({ "ok": true })))
}

pub async fn duplicate(
    State(st): State<AppState>,
    u: AuthUser,
    Path(bid): Path<i64>,
) -> ApiResult<Value> {
    let mid: i64 = sqlx::query_scalar("SELECT map_id FROM boards WHERE id = ?")
        .bind(bid)
        .fetch_one(&st.db)
        .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO boards (map_id, title, objects) SELECT map_id, title || ' (copie)', objects FROM boards WHERE id = ? RETURNING id").bind(bid).fetch_one(&st.db).await?;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "boards" }));
    Ok(Json(json!({ "id": id })))
}

async fn load(st: &AppState, bid: i64, map_id: i64) -> Result<Map<String, Value>, AppError> {
    let (mid, objects): (i64, String) =
        sqlx::query_as("SELECT map_id, objects FROM boards WHERE id = ?")
            .bind(bid)
            .fetch_one(&st.db)
            .await?;
    if mid != map_id {
        return Err(AppError::NotFound);
    }
    Ok(serde_json::from_str::<Value>(&objects)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default())
}

async fn save(st: &AppState, bid: i64, objs: &Map<String, Value>) -> Result<(), AppError> {
    let s = Value::Object(objs.clone()).to_string();
    if s.len() > 5_000_000 {
        return Err(bad("Tableau trop volumineux."));
    }
    sqlx::query("UPDATE boards SET objects = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(s)
        .bind(bid)
        .execute(&st.db)
        .await?;
    Ok(())
}

pub const KINDS: [&str; 10] = [
    "artifact",
    "sticky",
    "text",
    "shape",
    "frame",
    "connector",
    "stroke",
    "uefn",
    "image",
    "comment",
];

/// Lien d'artefact Claude accepté dans un tableau (#207) : seulement https://claude.ai/…
/// (claude.ai interdit l'affichage dans une iframe d'un autre site : POP en montre une carte).
pub fn artifact_url_ok(u: &str) -> bool {
    let re = regex::Regex::new(
        r"^https://claude\.ai/(artifact|public/artifacts|share)/[A-Za-z0-9_-]{6,80}/?$",
    )
    .unwrap();
    re.is_match(u)
}

/// Applique une opération reçue par WebSocket (ou par le MCP, #147) puis la diffuse aux membres ; rend le message
/// diffusé (objets gardés, identifiants retirés).
pub async fn apply_op(
    st: &AppState,
    u: &AuthUser,
    map_id: i64,
    v: &Value,
) -> Result<Value, AppError> {
    let bid = v["board"].as_i64().ok_or_else(|| bad("board"))?;
    let _guard = st.board_lock.lock().await;
    let mut objs = load(st, bid, map_id).await?;
    let op = v["op"].as_str().unwrap_or("");
    let mut out = json!({ "t": "board", "board": bid, "op": op, "by": u.id });
    match op {
        "upsert" => {
            let list = v["objs"].as_array().cloned().unwrap_or_default();
            let mut saved = vec![];
            for mut o in list.into_iter().take(200) {
                let id = o["id"].as_str().unwrap_or("").to_string();
                if id.is_empty()
                    || id.len() > 40
                    || id == "__meta"
                    || !KINDS.contains(&o["kind"].as_str().unwrap_or(""))
                {
                    continue;
                }
                if let Some(t) = o["text"].as_str() {
                    if t.chars().count() > 2000 {
                        continue;
                    }
                }
                if o["kind"] == "artifact" && !artifact_url_ok(o["url"].as_str().unwrap_or("")) {
                    continue;
                }
                if o["html"]
                    .as_str()
                    .map(|h| h.len() > 40_000)
                    .unwrap_or(false)
                {
                    continue;
                }
                // Les votes, l'auteur et le lien vers la tâche sont gérés par le serveur.
                match objs.get(&id) {
                    Some(prev) => {
                        for k in ["votes", "author", "author_id", "task_id", "task_number"] {
                            o[k] = prev[k].clone();
                        }
                    }
                    None => {
                        o["author"] = json!(u.name);
                        o["author_id"] = json!(u.id);
                        o["votes"] = json!({});
                    }
                }
                objs.insert(id, o.clone());
                saved.push(o);
            }
            out["objs"] = json!(saved);
        }
        "delete" => {
            let ids: Vec<String> = v["ids"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect();
            for id in &ids {
                if id != "__meta" {
                    objs.remove(id);
                }
            }
            out["ids"] = json!(ids);
        }
        "vote" => {
            let id = v["id"].as_str().unwrap_or("").to_string();
            let delta = v["delta"].as_i64().unwrap_or(1).signum();
            let points = objs
                .get("__meta")
                .and_then(|m| m["points"].as_i64())
                .unwrap_or(3);
            let key = u.id.to_string();
            let used: i64 = objs
                .values()
                .map(|o| o["votes"][&key].as_i64().unwrap_or(0))
                .sum();
            let o = objs.get_mut(&id).ok_or(AppError::NotFound)?;
            let mine = o["votes"][&key].as_i64().unwrap_or(0);
            if delta > 0 && used >= points {
                return Err(bad("Plus de points de vote."));
            }
            if !o["votes"].is_object() {
                o["votes"] = json!({});
            }
            o["votes"][&key] = json!((mine + delta).max(0));
            out["objs"] = json!([o.clone()]);
        }
        "meta" => {
            let mut m = objs
                .get("__meta")
                .cloned()
                .unwrap_or(json!({ "points": 3, "hidden": false, "timer_end": null }));
            if let Some(h) = v["hidden"].as_bool() {
                m["hidden"] = json!(h);
            }
            if let Some(p) = v["points"].as_i64() {
                m["points"] = json!(p.clamp(1, 20));
            }
            if v.get("timer_end").is_some() {
                m["timer_end"] = v["timer_end"].clone();
            }
            if v["reset_votes"].as_bool() == Some(true) {
                for (_, o) in objs.iter_mut() {
                    if o.get("votes").is_some() {
                        o["votes"] = json!({});
                    }
                }
            }
            objs.insert("__meta".into(), m.clone());
            out["meta"] = m;
            if v["reset_votes"].as_bool() == Some(true) {
                out["reload"] = json!(true);
            }
        }
        _ => return Err(bad("op inconnue")),
    }
    save(st, bid, &objs).await?;
    notify(st, map_id, out.clone());
    Ok(out)
}

#[derive(Deserialize)]
pub struct ConvertIn {
    ids: Vec<String>,
    assignee_id: Option<i64>,
    tag: Option<String>,
}

/// Post-it sélectionnés → tâches, avec un lien conservé dans les deux sens.
pub async fn convert(
    State(st): State<AppState>,
    u: AuthUser,
    Path(bid): Path<i64>,
    Json(b): Json<ConvertIn>,
) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM boards WHERE id = ?")
        .bind(bid)
        .fetch_one(&st.db)
        .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    let _guard = st.board_lock.lock().await;
    let mut objs = load(&st, bid, mid).await?;
    let mut created = vec![];
    for id in b.ids.iter().take(100) {
        let Some(o) = objs.get(id).cloned() else {
            continue;
        };
        if o["kind"] != "sticky" || o["task_id"].is_i64() {
            continue;
        }
        let title: String = o["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .chars()
            .take(140)
            .collect();
        if title.is_empty() {
            continue;
        }
        let (tid, num) = insert_task(
            &st,
            &ctx,
            &NewTask {
                title: title.clone(),
                tag: b.tag.clone(),
                description: Some(format!(
                    "Créée depuis un post-it du tableau blanc (vote : {}).",
                    o["votes"]
                        .as_object()
                        .map(|v| v.values().filter_map(|x| x.as_i64()).sum::<i64>())
                        .unwrap_or(0)
                )),
                assignee_id: b.assignee_id,
                deps: None,
                planned_day: None,
                lot: Some("Idées du tableau".into()),
                status: None,
                ai: None,
                origin: Some(format!("board:{bid}:{id}")),
                uefn: None,
            },
        )
        .await?;
        let e = objs.get_mut(id).unwrap();
        e["task_id"] = json!(tid);
        e["task_number"] = json!(num);
        created.push(json!({ "id": tid, "number": num, "obj": e.clone() }));
    }
    save(&st, bid, &objs).await?;
    if !created.is_empty() {
        log_activity(
            &st,
            mid,
            Some(u.id),
            "board",
            &format!("{} post-it convertis en tâches", created.len()),
            "Depuis le tableau blanc",
        )
        .await?;
    }
    notify(
        &st,
        mid,
        json!({ "t": "board", "board": bid, "op": "upsert", "by": u.id, "objs": created.iter().map(|c| c["obj"].clone()).collect::<Vec<_>>() }),
    );
    notify(&st, mid, json!({ "t": "refresh", "what": "tasks" }));
    Ok(Json(json!({ "created": created })))
}

#[derive(Deserialize)]
pub struct FrameIn {
    frame_id: String,
}

/// Cadre → page structurée (titre du cadre puis un point par post-it qu'il contient).
pub async fn frame_to_page(
    State(st): State<AppState>,
    u: AuthUser,
    Path(bid): Path<i64>,
    Json(b): Json<FrameIn>,
) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM boards WHERE id = ?")
        .bind(bid)
        .fetch_one(&st.db)
        .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    let objs = load(&st, bid, mid).await?;
    let f = objs
        .get(&b.frame_id)
        .filter(|f| f["kind"] == "frame")
        .ok_or(AppError::NotFound)?;
    let (fx, fy, fw, fh) = (
        f["x"].as_f64().unwrap_or(0.),
        f["y"].as_f64().unwrap_or(0.),
        f["w"].as_f64().unwrap_or(0.),
        f["h"].as_f64().unwrap_or(0.),
    );
    let title = f["text"].as_str().unwrap_or("Cadre").to_string();
    let mut inside: Vec<&Value> = objs
        .values()
        .filter(|o| {
            (o["kind"] == "sticky" || o["kind"] == "text") && {
                let cx = o["x"].as_f64().unwrap_or(0.) + o["w"].as_f64().unwrap_or(0.) / 2.;
                let cy = o["y"].as_f64().unwrap_or(0.) + o["h"].as_f64().unwrap_or(0.) / 2.;
                cx >= fx && cx <= fx + fw && cy >= fy && cy <= fy + fh
            }
        })
        .collect();
    inside.sort_by(|a, b| {
        a["y"]
            .as_f64()
            .partial_cmp(&b["y"].as_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut blocks = vec![
        json!({ "id": oid(), "type": "p", "text": "Page créée depuis un cadre du tableau blanc." }),
    ];
    for o in inside {
        blocks.push(
            json!({ "id": oid(), "type": "bullet", "text": o["text"].as_str().unwrap_or("") }),
        );
    }
    let content = json!(blocks).to_string();
    let pid: i64 = sqlx::query_scalar("INSERT INTO pages (map_id, title, icon, blocks, created_by, updated_by) VALUES (?, ?, '◫', ?, ?, ?) RETURNING id")
        .bind(mid).bind(&title).bind(&content).bind(u.id).bind(u.id).fetch_one(&st.db).await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "pages" }));
    Ok(Json(json!({ "page_id": pid })))
}

#[cfg(test)]
mod tests_artefact {
    use super::artifact_url_ok;

    #[test]
    fn seuls_les_liens_claude_ai_passent() {
        assert!(artifact_url_ok(
            "https://claude.ai/artifact/WiiU6vVRT3cMJFNtQGo87B"
        ));
        assert!(artifact_url_ok(
            "https://claude.ai/public/artifacts/3f2a9c1e-1234-4abc-9def-001122334455"
        ));
        for u in [
            "http://claude.ai/artifact/WiiU6vVRT3cMJFNtQGo87B",
            "https://claude.ai.evil.com/artifact/WiiU6vVRT3cMJFNtQGo87B",
            "https://evil.com/?https://claude.ai/artifact/abcdefgh",
            "https://claude.ai/artifact/abc def",
            "javascript:alert(1)",
            "",
        ] {
            assert!(!artifact_url_ok(u), "{u}");
        }
    }
}
