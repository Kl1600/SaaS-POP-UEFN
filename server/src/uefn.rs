//! UEFN Link : connexion du relais local, exécution des actions (journalisées, annulables, mises en attente
//! quand UEFN est fermé), dernier état connu, verrous Lore.
use crate::{
    access::{map_ctx, notify, MapCtx, Role},
    auth::{random_token, sha256_hex, AuthUser},
    catalog,
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::HeaderMap,
    response::Response,
    Json,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

// ---------- Connexions des relais ----------

struct RelayConn {
    tx: mpsc::UnboundedSender<String>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>>,
    uefn: String,
    project: String,
    conn_id: u64,
    tools: Value,
}

#[derive(Default)]
pub struct RelayHub {
    conns: Mutex<HashMap<i64, RelayConn>>,
    next: std::sync::atomic::AtomicU64,
}

impl RelayHub {
    pub fn status(&self, map_id: i64) -> &'static str {
        match self.conns.lock().unwrap().get(&map_id) {
            None => "offline",
            Some(c) if c.uefn == "connected" => "connected",
            Some(_) => "closed",
        }
    }
    fn tools(&self, map_id: i64) -> Value {
        self.conns
            .lock()
            .unwrap()
            .get(&map_id)
            .map(|c| c.tools.clone())
            .unwrap_or(Value::Null)
    }
    fn project(&self, map_id: i64) -> Option<String> {
        self.conns
            .lock()
            .unwrap()
            .get(&map_id)
            .map(|c| c.project.clone())
    }
}

/// Ce dont une action a besoin pour s'exécuter.
fn needs(action: &str) -> &'static str {
    match action {
        "write_file" | "delete_file" | "rename_file" | "lore_checkin" => "relay",
        _ => "uefn",
    }
}

fn ready(st: &AppState, map_id: i64, action: &str) -> bool {
    match (needs(action), st.relays.status(map_id)) {
        (_, "offline") => false,
        ("relay", _) => true,
        (_, s) => s == "connected",
    }
}

/// Envoie une commande au relais et attend sa réponse.
async fn relay_call(
    st: &AppState,
    map_id: i64,
    action: &str,
    args: &Value,
    secs: u64,
) -> Result<Value, AppError> {
    let id = random_token()[..16].to_string();
    let (tx, rx) = oneshot::channel();
    {
        let conns = st.relays.conns.lock().unwrap();
        let c = conns
            .get(&map_id)
            .ok_or_else(|| AppError::Conflict("Relais hors ligne.".into()))?;
        c.pending.lock().unwrap().insert(id.clone(), tx);
        let _ =
            c.tx.send(json!({ "t": "cmd", "id": id, "action": action, "args": args }).to_string());
    }
    match tokio::time::timeout(Duration::from_secs(secs), rx).await {
        Ok(Ok(v)) => {
            if v["ok"].as_bool() == Some(true) {
                Ok(v["data"].clone())
            } else {
                Err(AppError::Conflict(
                    v["error"]
                        .as_str()
                        .unwrap_or("Échec côté UEFN.")
                        .to_string(),
                ))
            }
        }
        _ => Err(AppError::Conflict(
            "UEFN ne répond pas (délai dépassé).".into(),
        )),
    }
}

// ---------- Appairage du relais ----------

pub async fn pair_code(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    let code: String = random_token()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(8)
        .collect::<String>();
    let code = format!("{:0<8}", code);
    sqlx::query("INSERT INTO relay_devices (map_id, user_id, token_hash, pair_code_hash, pair_expires) VALUES (?, ?, ?, ?, datetime('now', '+10 minutes'))")
        .bind(mid).bind(u.id).bind(format!("pending-{}", random_token())).bind(sha256_hex(&code))
        .execute(&st.db).await?;
    Ok(Json(json!({ "code": code, "expires_in": 600 })))
}

#[derive(Deserialize)]
pub struct PairIn {
    code: String,
    name: Option<String>,
}

/// Appelé par le relais avec le code affiché dans POP ; renvoie un jeton longue durée.
pub async fn pair(State(st): State<AppState>, Json(b): Json<PairIn>) -> ApiResult<Value> {
    let row: Option<(i64, i64)> = sqlx::query_as("SELECT id, map_id FROM relay_devices WHERE pair_code_hash = ? AND pair_expires > datetime('now')")
        .bind(sha256_hex(b.code.trim())).fetch_optional(&st.db).await?;
    let (id, mid) = row.ok_or_else(|| bad("Code invalide ou expiré."))?;
    let token = random_token();
    sqlx::query("UPDATE relay_devices SET token_hash = ?, pair_code_hash = NULL, pair_expires = NULL, name = ? WHERE id = ?")
        .bind(sha256_hex(&token)).bind(b.name.unwrap_or_else(|| "UEFN Link".into()).chars().take(60).collect::<String>()).bind(id)
        .execute(&st.db).await?;
    let (name,): (String,) = sqlx::query_as("SELECT name FROM maps WHERE id = ?")
        .bind(mid)
        .fetch_one(&st.db)
        .await?;
    Ok(Json(json!({ "token": token, "map_id": mid, "map": name })))
}

pub async fn devices(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT d.id, d.name, d.last_seen, u.name FROM relay_devices d JOIN users u ON u.id = d.user_id WHERE d.map_id = ? AND d.pair_code_hash IS NULL ORDER BY d.id DESC",
    ).bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, n, seen, who)| json!({"id": id, "name": n, "last_seen": seen, "owner": who}))
        .collect::<Vec<_>>())))
}

pub async fn delete_device(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM relay_devices WHERE id = ?")
        .bind(id)
        .fetch_one(&st.db)
        .await?;
    map_ctx(&st, &u, mid).await?.require(Role::Admin)?;
    sqlx::query("DELETE FROM relay_devices WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------- WebSocket du relais ----------

#[derive(Deserialize)]
pub struct RelayQ {
    token: Option<String>,
}

pub async fn relay_ws(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<RelayQ>,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(String::from)
        .or(q.token)
        .ok_or(AppError::Unauthorized)?;
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, map_id FROM relay_devices WHERE token_hash = ? AND pair_code_hash IS NULL",
    )
    .bind(sha256_hex(&token))
    .fetch_optional(&st.db)
    .await?;
    let (dev, mid) = row.ok_or(AppError::Unauthorized)?;
    Ok(ws
        .max_message_size(32 * 1024 * 1024)
        .on_upgrade(move |s| relay_run(st, dev, mid, s)))
}

async fn relay_run(st: AppState, dev: i64, mid: i64, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let pending: Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>> = Default::default();
    let conn_id = st
        .relays
        .next
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    st.relays.conns.lock().unwrap().insert(
        mid,
        RelayConn {
            tx,
            pending: pending.clone(),
            uefn: "closed".into(),
            project: String::new(),
            conn_id,
            tools: Value::Null,
        },
    );
    let _ = sqlx::query("UPDATE relay_devices SET last_seen = datetime('now') WHERE id = ?")
        .bind(dev)
        .execute(&st.db)
        .await;
    broadcast_status(&st, mid);
    let writer = tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            if sink.send(Message::Text(m.into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(t) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&t) else {
            continue;
        };
        if let Err(e) = on_relay_msg(&st, mid, &pending, v).await {
            tracing::warn!("message du relais ignoré : {e:?}");
        }
    }
    writer.abort();
    {
        let mut conns = st.relays.conns.lock().unwrap();
        if conns.get(&mid).map(|c| c.conn_id) == Some(conn_id) {
            conns.remove(&mid);
        }
    }
    broadcast_status(&st, mid);
}

fn broadcast_status(st: &AppState, mid: i64) {
    notify(
        st,
        mid,
        json!({ "t": "uefn", "what": "status", "status": st.relays.status(mid), "project": st.relays.project(mid) }),
    );
}

async fn ensure_state(st: &AppState, mid: i64) -> Result<(), AppError> {
    sqlx::query("INSERT OR IGNORE INTO uefn_state (map_id) VALUES (?)")
        .bind(mid)
        .execute(&st.db)
        .await?;
    Ok(())
}

async fn on_relay_msg(
    st: &AppState,
    mid: i64,
    pending: &Arc<Mutex<HashMap<String, oneshot::Sender<Value>>>>,
    v: Value,
) -> Result<(), AppError> {
    ensure_state(st, mid).await?;
    match v["t"].as_str().unwrap_or("") {
        "result" => {
            if let Some(tx) = pending
                .lock()
                .unwrap()
                .remove(v["id"].as_str().unwrap_or(""))
            {
                let _ = tx.send(v);
            }
        }
        "tools" => {
            // Outils réellement exposés par le serveur MCP d'UEFN (affichés dans l'Organiseur).
            let tools: Vec<Value> = v["tools"].as_array().cloned().unwrap_or_default().into_iter().take(400).map(|t| json!({ "name": t["name"].as_str().unwrap_or("").chars().take(120).collect::<String>(), "description": t["description"].as_str().unwrap_or("").chars().take(300).collect::<String>() })).collect();
            if let Some(c) = st.relays.conns.lock().unwrap().get_mut(&mid) {
                c.tools = json!({ "url": v["url"].as_str().unwrap_or("").chars().take(200).collect::<String>(), "list": tools, "roles": v["roles"].clone() });
            }
            broadcast_status(st, mid);
        }
        "hello" | "status" => {
            let uefn = if v["uefn"] == "connected" {
                "connected"
            } else {
                "closed"
            };
            let was;
            {
                let mut conns = st.relays.conns.lock().unwrap();
                let Some(c) = conns.get_mut(&mid) else {
                    return Ok(());
                };
                was = c.uefn.clone();
                c.uefn = uefn.into();
                if let Some(p) = v["project"].as_str() {
                    c.project = p.chars().take(120).collect();
                }
            }
            if let Some(p) = v["project"].as_str() {
                sqlx::query("UPDATE uefn_state SET project = ? WHERE map_id = ?")
                    .bind(p)
                    .bind(mid)
                    .execute(&st.db)
                    .await?;
            }
            broadcast_status(st, mid);
            if was != uefn || v["t"] == "hello" {
                let st2 = st.clone();
                tokio::spawn(async move { flush_queue(&st2, mid).await });
            }
        }
        "files" => {
            // Synchronisation des fichiers .verse du disque vers POP (le disque fait foi).
            let files = v["files"].as_array().cloned().unwrap_or_default();
            let full = v["full"].as_bool().unwrap_or(false);
            let mut seen = vec![];
            for f in files.iter().take(2000) {
                let (Some(path), Some(content)) = (f["path"].as_str(), f["content"].as_str())
                else {
                    continue;
                };
                if !valid_path(path) || content.len() > 2_000_000 {
                    continue;
                }
                seen.push(path.to_string());
                let sha = sha256_hex(content);
                let cur: Option<(String,)> =
                    sqlx::query_as("SELECT sha FROM verse_files WHERE map_id = ? AND path = ?")
                        .bind(mid)
                        .bind(path)
                        .fetch_optional(&st.db)
                        .await?;
                if cur.map(|c| c.0) != Some(sha.clone()) {
                    sqlx::query("INSERT INTO verse_files (map_id, path, content, sha) VALUES (?, ?, ?, ?) ON CONFLICT(map_id, path) DO UPDATE SET content = excluded.content, sha = excluded.sha, updated_at = datetime('now')")
                        .bind(mid).bind(path).bind(content).bind(&sha).execute(&st.db).await?;
                    sqlx::query("INSERT INTO verse_versions (map_id, path, content, note) VALUES (?, ?, ?, 'Modifié dans UEFN ou sur le disque')").bind(mid).bind(path).bind(content).execute(&st.db).await?;
                    notify(
                        st,
                        mid,
                        json!({ "t": "verse", "what": "file", "path": path, "sha": sha, "source": "disk" }),
                    );
                }
            }
            if full {
                let existing: Vec<(String,)> =
                    sqlx::query_as("SELECT path FROM verse_files WHERE map_id = ?")
                        .bind(mid)
                        .fetch_all(&st.db)
                        .await?;
                for (p,) in existing {
                    if !seen.contains(&p) {
                        // Ne pas effacer un fichier dont l'écriture attend encore le relais.
                        let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uefn_actions WHERE map_id = ? AND status = 'queued' AND action = 'write_file' AND json_extract(args, '$.path') = ?").bind(mid).bind(&p).fetch_one(&st.db).await?;
                        if queued == 0 {
                            sqlx::query("DELETE FROM verse_files WHERE map_id = ? AND path = ?")
                                .bind(mid)
                                .bind(&p)
                                .execute(&st.db)
                                .await?;
                        }
                    }
                }
                notify(st, mid, json!({ "t": "verse", "what": "list" }));
            }
        }
        "digests" => {
            sqlx::query("UPDATE uefn_state SET symbols = ?, tags = ?, updated_at = datetime('now') WHERE map_id = ?")
                .bind(v["symbols"].to_string()).bind(v["tags"].to_string()).bind(mid).execute(&st.db).await?;
            notify(st, mid, json!({ "t": "verse", "what": "symbols" }));
        }
        "digests_tags" => {
            sqlx::query("UPDATE uefn_state SET tags = ? WHERE map_id = ?")
                .bind(v["tags"].to_string())
                .bind(mid)
                .execute(&st.db)
                .await?;
            notify(st, mid, json!({ "t": "verse", "what": "symbols" }));
        }
        "actors" => {
            let actors = keep_device_values(st, mid, &v["actors"], None).await?;
            sqlx::query(
                "UPDATE uefn_state SET actors = ?, updated_at = datetime('now') WHERE map_id = ?",
            )
            .bind(actors.to_string())
            .bind(mid)
            .execute(&st.db)
            .await?;
            notify(st, mid, json!({ "t": "uefn", "what": "actors" }));
        }
        "locks" => {
            sqlx::query("UPDATE uefn_state SET locks = ? WHERE map_id = ?")
                .bind(v["locks"].to_string())
                .bind(mid)
                .execute(&st.db)
                .await?;
            notify(st, mid, json!({ "t": "uefn", "what": "actors" }));
        }
        "topview" => {
            let img = v["image"].as_str().unwrap_or("");
            if img.len() < 12_000_000
                && (img.starts_with("data:image/png;base64,")
                    || img.starts_with("data:image/jpeg;base64,")
                    || img.starts_with("data:image/svg+xml;base64,"))
            {
                sqlx::query(
                    "UPDATE uefn_state SET topview = ?, topview_bounds = ? WHERE map_id = ?",
                )
                .bind(img)
                .bind(v["bounds"].to_string())
                .bind(mid)
                .execute(&st.db)
                .await?;
                notify(st, mid, json!({ "t": "uefn", "what": "topview" }));
            }
        }
        _ => {}
    }
    Ok(())
}

/// La liste d'acteurs envoyée par le relais ne contient pas les options des appareils (elles sont lues
/// une par une, à la sélection). On garde donc les valeurs déjà connues, et on y applique les options
/// que l'on vient de modifier (`patches`) : sans cela, l'annulation suivante n'aurait plus la valeur d'avant.
async fn keep_device_values(
    st: &AppState,
    mid: i64,
    actors: &Value,
    patches: Option<&Value>,
) -> Result<Value, AppError> {
    let Some(list) = actors.as_array() else {
        return Ok(actors.clone());
    };
    let old: Option<(String,)> = sqlx::query_as("SELECT actors FROM uefn_state WHERE map_id = ?")
        .bind(mid)
        .fetch_optional(&st.db)
        .await?;
    let old: Vec<Value> = old
        .and_then(|(s,)| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let known: HashMap<&str, &Value> = old
        .iter()
        .filter_map(|a| Some((a["id"].as_str()?, a)))
        .collect();
    let empty = |v: &Value| v.as_object().map(|m| m.is_empty()).unwrap_or(true);
    let mut out = list.clone();
    for a in out.iter_mut() {
        let id = a["id"].as_str().unwrap_or("").to_string();
        for field in ["options", "props"] {
            if empty(&a[field]) {
                if let Some(prev) = known
                    .get(id.as_str())
                    .map(|p| &p[field])
                    .filter(|p| !empty(p))
                {
                    a[field] = (*prev).clone();
                }
            }
            for p in patches
                .and_then(|p| p.as_array())
                .into_iter()
                .flatten()
                .filter(|p| p["id"].as_str() == Some(id.as_str()))
            {
                if let Some(m) = p["changes"][field].as_object() {
                    if !a[field].is_object() {
                        a[field] = json!({});
                    }
                    for (k, v) in m {
                        a[field][k] = v.clone();
                    }
                }
            }
        }
    }
    Ok(Value::Array(out))
}

pub fn valid_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() < 400
        && p.ends_with(".verse")
        && !p.contains("..")
        && !p.starts_with('/')
        && !p.contains('\\')
        && !p.contains(':')
        && !p.ends_with(".digest.verse")
}

// ---------- Dernier état connu ----------

#[derive(Clone)]
pub struct Snapshot {
    pub actors: Vec<Value>,
    pub locks: Vec<Value>,
}

pub async fn snapshot(st: &AppState, mid: i64) -> Result<Snapshot, AppError> {
    ensure_state(st, mid).await?;
    let (a, l): (String, String) =
        sqlx::query_as("SELECT actors, locks FROM uefn_state WHERE map_id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await?;
    let mut locks: Vec<Value> = serde_json::from_str(&l).unwrap_or_default();
    // Verrous déclarés par le webhook Lore (check-out d'un fichier par un membre).
    let lore: Vec<(String, Option<String>)> = sqlx::query_as("SELECT l.path, u.email FROM file_locks l LEFT JOIN users u ON u.id = l.user_id WHERE l.map_id = ?").bind(mid).fetch_all(&st.db).await?;
    for (path, email) in lore {
        locks.push(json!({ "target": path.replace('\\', "/"), "by": email.unwrap_or_else(|| "un membre".into()) }));
    }
    Ok(Snapshot {
        actors: serde_json::from_str(&a).unwrap_or_default(),
        locks,
    })
}

/// Un verrou couvre la cible exacte, ou un chemin dont l'un est la fin de l'autre (chemins Lore vs chemins du projet).
pub fn lock_matches(lock: &str, target: &str) -> bool {
    lock == target
        || (lock.contains('/')
            && target.contains('/')
            && (target.ends_with(&format!("/{lock}")) || lock.ends_with(&format!("/{target}"))))
}

pub fn locked_by<'a>(snap: &'a Snapshot, target: &str) -> Option<&'a str> {
    snap.locks
        .iter()
        .find(|l| {
            l["target"]
                .as_str()
                .map(|t| lock_matches(t, target))
                .unwrap_or(false)
        })
        .and_then(|l| l["by"].as_str())
}

#[derive(Deserialize)]
pub struct PropsQ {
    id: String,
}

/// Options réelles d'un acteur, lues dans UEFN à la demande (sélection dans l'Organiseur).
pub async fn props(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Query(q): Query<PropsQ>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    if st.relays.status(mid) != "connected" {
        return Err(AppError::Conflict(
            "UEFN n’est pas connecté : options indisponibles.".into(),
        ));
    }
    let snap = snapshot(&st, mid).await?;
    let mut a = snap
        .actors
        .iter()
        .find(|a| a["id"].as_str() == Some(q.id.as_str()))
        .cloned()
        .ok_or(AppError::NotFound)?;
    let r = relay_call(
        &st,
        mid,
        "actor_props",
        &json!({ "id": q.id, "name": a["name"] }),
        45,
    )
    .await?;
    let props = r["props"].clone();
    if a["type"] == "verse_device" {
        a["props"] = props.clone()
    } else {
        a["options"] = props.clone()
    }
    a["schema"] = Value::Null;
    crate::catalog::enrich(&mut a);
    // Mémorise les valeurs pour l'affichage hors ligne.
    let mut all = snap.actors.clone();
    if let Some(x) = all
        .iter_mut()
        .find(|x| x["id"].as_str() == Some(q.id.as_str()))
    {
        if a["type"] == "verse_device" {
            x["props"] = props.clone()
        } else {
            x["options"] = props.clone()
        }
        sqlx::query("UPDATE uefn_state SET actors = ? WHERE map_id = ?")
            .bind(json!(all).to_string())
            .bind(mid)
            .execute(&st.db)
            .await?;
    }
    Ok(Json(
        json!({ "values": props, "schema": a["schema"], "kind_label": a["kind_label"] }),
    ))
}

pub async fn state(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let snap = snapshot(&st, mid).await?;
    let (project, tags, updated, bounds, has_top): (String, String, String, Option<String>, i64) =
        sqlx::query_as("SELECT project, tags, updated_at, topview_bounds, topview IS NOT NULL FROM uefn_state WHERE map_id = ?").bind(mid).fetch_one(&st.db).await?;
    let actors: Vec<Value> = snap
        .actors
        .iter()
        .map(|a| {
            let mut a = a.clone();
            catalog::enrich(&mut a);
            let id = a["id"].as_str().unwrap_or("").to_string();
            if let Some(by) = locked_by(&snap, &id) {
                a["locked_by"] = json!(by);
            }
            a
        })
        .collect();
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM uefn_actions WHERE map_id = ? AND status = 'queued'",
    )
    .bind(mid)
    .fetch_one(&st.db)
    .await?;
    let unsaved: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT json_each.value) FROM uefn_actions ua, json_each(json_extract(ua.args, '$.ids')) WHERE ua.map_id = ? AND ua.status = 'done' AND ua.action LIKE 'actor_%' AND ua.id > COALESCE((SELECT MAX(id) FROM uefn_actions WHERE map_id = ? AND action = 'save_level' AND status = 'done'), 0)",
    ).bind(mid).bind(mid).fetch_one(&st.db).await.unwrap_or(0);
    let approvals: Vec<(i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT a.id, a.summary, u.name, a.at FROM approvals a LEFT JOIN users u ON u.id = a.user_id WHERE a.map_id = ? AND a.status = 'pending' ORDER BY a.id",
    ).bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!({
        "status": st.relays.status(mid), "project": project, "updated_at": updated, "tools": st.relays.tools(mid),
        "actors": actors, "tags": serde_json::from_str::<Value>(&tags).unwrap_or(json!([])),
        "pending": pending, "unsaved_actors": unsaved, "has_topview": has_top == 1,
        "topview_bounds": bounds.and_then(|b| serde_json::from_str::<Value>(&b).ok()),
        "approvals": approvals.into_iter().map(|(id, s, who, at)| json!({"id": id, "summary": s, "who": who, "at": at})).collect::<Vec<_>>(),
    })))
}

pub async fn topview(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let row: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT topview, topview_bounds FROM uefn_state WHERE map_id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await?;
    Ok(Json(
        json!({ "image": row.0, "bounds": row.1.and_then(|b| serde_json::from_str::<Value>(&b).ok()) }),
    ))
}

// ---------- Exécution des actions ----------

pub struct Origin<'a> {
    pub ctx: &'a MapCtx,
    pub source: &'a str,
    /// Agent IA (jeton MCP) qui demande l'action, pour le journal par agent (#93).
    pub agent: Option<i64>,
    pub task: Option<i64>,
}

fn role_for(action: &str) -> Role {
    match action {
        "actor_delete" | "delete_file" | "asset_delete" => Role::Member,
        "refresh_actors" | "render_topview" | "session_output" => Role::Viewer,
        _ => Role::Freelance,
    }
}

fn short(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "—".into(),
        Value::Array(a) => a.iter().map(short).collect::<Vec<_>>().join(", "),
        other => other.to_string(),
    }
}

/// Prépare une action : contrôle des droits et des verrous, état « avant », action inverse pour l'annulation.
async fn prepare(
    st: &AppState,
    o: &Origin<'_>,
    action: &str,
    args: &mut Value,
) -> Result<(String, Value, Option<Value>), AppError> {
    o.ctx.require(role_for(action))?;
    let mid = o.ctx.map_id;
    let snap = snapshot(st, mid).await?;
    let me = &o.ctx.user.email;
    let by_id = |id: &str| {
        snap.actors
            .iter()
            .find(|a| a["id"].as_str() == Some(id))
            .cloned()
    };
    let check_lock = |target: &str| -> Result<(), AppError> {
        if let Some(by) = locked_by(&snap, target) {
            if !by.eq_ignore_ascii_case(me) {
                return Err(AppError::Conflict(format!(
                    "« {target} » est verrouillé dans Lore par {by}."
                )));
            }
        }
        Ok(())
    };
    match action {
        "actor_update" => {
            // Modification d'un ou plusieurs acteurs ; la même valeur s'applique à tous.
            let ids: Vec<String> = args["ids"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect();
            if ids.is_empty() || ids.len() > 500 {
                return Err(bad("Sélection vide ou trop grande."));
            }
            let ch = args["changes"].clone();
            let mut patches = vec![];
            let mut undo = vec![];
            let mut lines = vec![];
            for id in &ids {
                check_lock(id)?;
                let a = by_id(id).ok_or_else(|| bad(format!("Acteur introuvable : {id}")))?;
                let name = a["name"].as_str().unwrap_or(id).to_string();
                let mut p = json!({});
                let mut u = json!({});
                for k in ["name", "folder", "hidden"] {
                    if !ch[k].is_null() {
                        if k == "name" {
                            check_len("Nom", ch[k].as_str().unwrap_or(""), 1, 120)?;
                        }
                        p[k] = ch[k].clone();
                        u[k] = a[k].clone();
                        lines.push(format!(
                            "{name} · {k} : {} → {}",
                            short(&a[k]),
                            short(&ch[k])
                        ));
                    }
                }
                if let Some(t) = ch["transform"].as_object() {
                    let mut nt = a["transform"].clone();
                    for (k, v) in t {
                        if !v
                            .as_array()
                            .map(|x| x.len() == 3 && x.iter().all(|n| n.is_number()))
                            .unwrap_or(false)
                        {
                            return Err(bad("Transformation invalide."));
                        }
                        nt[k] = v.clone();
                    }
                    p["transform"] = nt.clone();
                    u["transform"] = a["transform"].clone();
                    lines.push(format!(
                        "{name} · position {} → {}",
                        short(&a["transform"]["location"]),
                        short(&nt["location"])
                    ));
                }
                for (field, label) in [("options", "option"), ("props", "propriété")] {
                    if let Some(m) = ch[field].as_object() {
                        let mut prev = json!({});
                        for (k, v) in m {
                            if field == "props" {
                                check_prop_type(k, &a[field][k], v)?;
                            }
                            prev[k] = a[field][k].clone();
                            lines.push(format!(
                                "{name} · {label} {k} : {} → {}",
                                short(&a[field][k]),
                                short(v)
                            ));
                        }
                        p[field] = ch[field].clone();
                        u[field] = prev;
                    }
                }
                if ch["tags_add"].is_array() || ch["tags_remove"].is_array() {
                    let mut tags: Vec<String> = a["tags"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect();
                    let before = tags.clone();
                    for t in ch["tags_add"].as_array().cloned().unwrap_or_default() {
                        if let Some(t) = t.as_str() {
                            if !tags.iter().any(|x| x == t) {
                                tags.push(t.to_string());
                            }
                        }
                    }
                    let rm: Vec<String> = ch["tags_remove"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect();
                    tags.retain(|t| !rm.contains(t));
                    p["tags"] = json!(tags);
                    u["tags"] = json!(before);
                    lines.push(format!(
                        "{name} · tags : {} → {}",
                        before.join(", "),
                        tags.join(", ")
                    ));
                }
                if ch["events"].is_array() {
                    p["events"] = ch["events"].clone();
                    u["events"] = a["events"].clone();
                    lines.push(format!("{name} · liens d’événements modifiés"));
                }
                patches.push(json!({ "id": id, "changes": p }));
                undo.push(json!({ "id": id, "changes": u }));
            }
            *args = json!({ "ids": ids, "patches": patches });
            let summary = if lines.len() > 3 {
                format!(
                    "{} · et {} autres changements",
                    lines[..3].join(" ; "),
                    lines.len() - 3
                )
            } else {
                lines.join(" ; ")
            };
            Ok((
                summary,
                json!(undo),
                Some(
                    json!({ "action": "actor_update_raw", "args": { "ids": ids, "patches": undo } }),
                ),
            ))
        }
        "actor_spawn" => {
            // Poser un appareil (#81) : depuis un asset du Content Browser, ou sur le modèle d'un acteur du niveau.
            let asset = args["asset"].as_str().unwrap_or("").trim().to_string();
            let like = args["like"].as_str().unwrap_or("").trim().to_string();
            let modele = if like.is_empty() {
                if !asset.starts_with('/') || asset.len() > 400 {
                    return Err(bad("Indiquez l’asset de l’appareil (chemin du Content Browser, ex. /Game/…) ou un appareil modèle du niveau."));
                }
                asset.rsplit('/').next().unwrap_or(&asset).to_string()
            } else {
                let a = by_id(&like)
                    .ok_or_else(|| bad(format!("Appareil modèle introuvable : {like}")))?;
                a["name"].as_str().unwrap_or(&like).to_string()
            };
            let nom = args["name"].as_str().unwrap_or("").trim().to_string();
            check_len("Nom", &nom, 0, 120)?;
            if let Some(f) = args["folder"].as_str().filter(|f| !f.is_empty()) {
                check_len("Dossier", f, 1, 200)?;
                check_lock(f)?;
            }
            for (k, n) in [("location", 3), ("rotation", 3)] {
                if !args[k].is_null()
                    && !args[k]
                        .as_array()
                        .map(|v| v.len() == n && v.iter().all(|x| x.is_number()))
                        .unwrap_or(false)
                {
                    return Err(bad(format!("{k} : trois nombres attendus.")));
                }
            }
            let quoi = if nom.is_empty() {
                modele
            } else {
                format!("{nom} ({modele})")
            };
            Ok((format!("Pose : {quoi}"), Value::Null, None))
        }

        "actor_organize" => {
            // Rangement du niveau (#84) : nom et dossier propres à chaque acteur, en une action annulable.
            let patches = args["patches"].as_array().cloned().unwrap_or_default();
            if patches.is_empty() || patches.len() > 5000 {
                return Err(bad("Rien à ranger (5 000 acteurs au plus)."));
            }
            let mut before = vec![];
            let mut undo = vec![];
            let mut propres = vec![];
            for p in &patches {
                let id = p["id"].as_str().unwrap_or("");
                check_lock(id)?;
                let a = by_id(id).ok_or_else(|| bad(format!("Acteur introuvable : {id}")))?;
                let mut c = json!({});
                let mut old = json!({});
                if let Some(n) = p["changes"]["name"].as_str() {
                    check_len("Nom", n, 1, 120)?;
                    c["name"] = json!(n.trim());
                    old["name"] = a["name"].clone();
                }
                if let Some(f) = p["changes"]["folder"].as_str() {
                    check_len("Dossier", f, 0, 200)?;
                    c["folder"] = json!(f.trim());
                    old["folder"] = a["folder"].clone();
                }
                propres.push(json!({ "id": id, "changes": c }));
                undo.push(json!({ "id": id, "changes": old }));
                before.push(a);
            }
            *args = json!({ "patches": propres });
            Ok((
                format!("Rangement du niveau : {} acteur(s)", propres.len()),
                json!(before),
                Some(
                    json!({ "action": "actor_update_raw", "args": { "ids": propres.iter().map(|p| p["id"].clone()).collect::<Vec<_>>(), "patches": undo } }),
                ),
            ))
        }
        "asset_import" => {
            // Import d'un fichier dans le projet UEFN (#90) : fichier du PC ou adresse du serveur POP.
            const EXT: &[&str] = &[
                "fbx", "obj", "png", "jpg", "jpeg", "tga", "wav", "ogg", "mp3",
            ];
            let src = args["local_path"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or(args["url"].as_str().filter(|s| !s.is_empty()))
                .ok_or_else(|| {
                    bad("Indiquez le fichier à importer (chemin sur le PC ou adresse POP).")
                })?
                .to_string();
            let nom = args["filename"]
                .as_str()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| src.rsplit(['/', '\\']).next().unwrap_or(&src))
                .to_string();
            let ext = nom.rsplit('.').next().unwrap_or("").to_lowercase();
            if !nom.contains('.') || !EXT.contains(&ext.as_str()) {
                return Err(bad(format!(
                    "Type de fichier non importé ({nom}) : {}.",
                    EXT.join(", ")
                )));
            }
            let dossier = args["folder"].as_str().unwrap_or("").trim().to_string();
            if !dossier.is_empty() {
                if !dossier.starts_with('/') || dossier.len() > 200 || dossier.contains("..") {
                    return Err(bad(
                        "Dossier du Content Browser invalide (ex. /MonProjet/Assets).",
                    ));
                }
                check_lock(&dossier)?;
            }
            let vers = if dossier.is_empty() {
                "dossier POP du projet".to_string()
            } else {
                dossier
            };
            Ok((format!("Import : {nom} → {vers}"), Value::Null, None))
        }
        "asset_delete" => {
            let paths: Vec<String> = args["paths"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect();
            if paths.is_empty() {
                return Err(bad("Aucun asset."));
            }
            for p in &paths {
                check_lock(p)?;
            }
            Ok((
                format!("Retrait d’assets : {}", paths.join(", ")),
                Value::Null,
                None,
            ))
        }
        "actor_duplicate" | "actor_delete" => {
            let ids: Vec<String> = args["ids"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect();
            let mut before = vec![];
            for id in &ids {
                check_lock(id)?;
                before.push(by_id(id).ok_or_else(|| bad(format!("Acteur introuvable : {id}")))?);
            }
            let names = before
                .iter()
                .map(|a| a["name"].as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
                .join(", ");
            if action == "actor_delete" {
                Ok((
                    format!("Suppression : {names}"),
                    json!(before),
                    Some(
                        json!({ "action": "actor_restore", "args": { "actors": before, "ids": ids } }),
                    ),
                ))
            } else {
                Ok((format!("Duplication : {names}"), json!(before), None))
            }
        }
        "write_file" | "delete_file" | "rename_file" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            if !valid_path(&path) {
                return Err(bad("Chemin de fichier Verse invalide."));
            }
            check_lock(&path)?;
            let cur: Option<(String,)> =
                sqlx::query_as("SELECT content FROM verse_files WHERE map_id = ? AND path = ?")
                    .bind(mid)
                    .bind(&path)
                    .fetch_optional(&st.db)
                    .await?;
            match action {
                "write_file" => {
                    let content = args["content"]
                        .as_str()
                        .ok_or_else(|| bad("Contenu manquant."))?;
                    if content.len() > 2_000_000 {
                        return Err(bad("Fichier trop gros."));
                    }
                    let undo = match &cur {
                        Some((old,)) => {
                            json!({ "action": "write_file", "args": { "path": path, "content": old } })
                        }
                        None => json!({ "action": "delete_file", "args": { "path": path } }),
                    };
                    Ok((
                        format!(
                            "{} {path}",
                            if cur.is_some() {
                                "Écriture"
                            } else {
                                "Création"
                            }
                        ),
                        json!(cur.map(|c| c.0)),
                        Some(undo),
                    ))
                }
                "delete_file" => {
                    let (old,) = cur.ok_or(AppError::NotFound)?;
                    Ok((
                        format!("Suppression {path}"),
                        json!(old),
                        Some(
                            json!({ "action": "write_file", "args": { "path": path, "content": old } }),
                        ),
                    ))
                }
                _ => {
                    let to = args["to"].as_str().unwrap_or("").to_string();
                    if !valid_path(&to) {
                        return Err(bad("Nouveau nom invalide."));
                    }
                    cur.ok_or(AppError::NotFound)?;
                    Ok((
                        format!("Renommage {path} → {to}"),
                        Value::Null,
                        Some(
                            json!({ "action": "rename_file", "args": { "path": to, "to": path } }),
                        ),
                    ))
                }
            }
        }
        "actor_restore" | "actor_update_raw" => Ok(("Annulation".into(), Value::Null, None)),
        "compile" => Ok(("Compilation Verse".into(), Value::Null, None)),
        "push_session" => Ok((
            "Envoi des changements en session de test".into(),
            Value::Null,
            None,
        )),
        "start_session" => Ok(("Démarrage de la session de test".into(), Value::Null, None)),
        "stop_session" => Ok(("Arrêt de la session de test".into(), Value::Null, None)),
        "session_output" => Ok((
            "Lecture de la sortie de la session".into(),
            Value::Null,
            None,
        )),
        "save_level" => Ok(("Enregistrement du niveau".into(), Value::Null, None)),
        "lore_checkin" => {
            check_len("Message", args["message"].as_str().unwrap_or(""), 1, 300)?;
            Ok((
                format!(
                    "Archivage Lore : {}",
                    args["message"].as_str().unwrap_or("")
                ),
                Value::Null,
                None,
            ))
        }
        "refresh_actors" => Ok(("Actualisation des acteurs".into(), Value::Null, None)),
        "render_topview" => Ok(("Rendu de la vue du dessus".into(), Value::Null, None)),
        _ => Err(bad("Action inconnue.")),
    }
}

/// Exécute (ou met en attente) une action UEFN et la journalise.
pub async fn exec(
    st: &AppState,
    o: Origin<'_>,
    action: &str,
    mut args: Value,
) -> Result<Value, AppError> {
    let (summary, before, undo) = prepare(st, &o, action, &mut args).await?;
    let mid = o.ctx.map_id;
    let journal = !matches!(
        action,
        "refresh_actors" | "render_topview" | "session_output"
    );
    // La session de test ne se met pas en attente : sans UEFN, rien n'est envoyé (#82).
    if matches!(action, "start_session" | "stop_session") && !ready(st, mid, action) {
        return Err(AppError::Conflict(
            "UEFN n’est pas connecté : ouvrez le projet dans UEFN et démarrez son serveur MCP, puis réessayez.".into(),
        ));
    }
    let aid: Option<i64> = if journal {
        Some(sqlx::query_scalar("INSERT INTO uefn_actions (map_id, user_id, source, action, args, summary, before, undo, task_number, status, agent_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'queued', ?) RETURNING id")
            .bind(mid).bind(o.ctx.user.id).bind(o.source).bind(action).bind(args.to_string()).bind(&summary).bind(before.to_string())
            .bind(undo.map(|u| u.to_string())).bind(o.task).bind(o.agent).fetch_one(&st.db).await?)
    } else {
        None
    };
    // Fichiers Verse : le dernier état connu suit immédiatement, même en attente du relais.
    apply_file_cache(st, mid, action, &args, o.ctx.user.id, o.task, &summary).await?;
    if !ready(st, mid, action) {
        if !journal {
            return Err(AppError::Conflict(
                "UEFN n’est pas connecté : lecture seule sur le dernier état connu.".into(),
            ));
        }
        notify(st, mid, json!({ "t": "uefn", "what": "journal" }));
        return Ok(
            json!({ "queued": true, "action_id": aid, "message": "UEFN n’est pas disponible : modification mise en attente, elle partira à l’ouverture." }),
        );
    }
    let r = run_action(st, mid, aid, action, &args).await;
    if matches!(&r, Ok(v) if v["retry"].as_bool() == Some(true)) {
        let st2 = st.clone();
        tokio::spawn(async move { flush_queue(&st2, mid).await });
    }
    r
}

async fn run_action(
    st: &AppState,
    mid: i64,
    aid: Option<i64>,
    action: &str,
    args: &Value,
) -> Result<Value, AppError> {
    if let Some(id) = aid {
        sqlx::query("UPDATE uefn_actions SET status = 'sent' WHERE id = ?")
            .bind(id)
            .execute(&st.db)
            .await?;
    }
    let secs = if matches!(
        action,
        "compile"
            | "push_session"
            | "render_topview"
            | "lore_checkin"
            | "stop_session"
            | "asset_import"
    ) {
        120
    } else if action == "start_session" {
        // Démarrer une session peut prendre plusieurs minutes (chargement du niveau).
        300
    } else {
        30
    };
    let res = relay_call(st, mid, action, args, secs).await;
    match &res {
        Ok(data) => {
            if let Some(actors) = data.get("actors").filter(|a| a.is_array()) {
                let patches = if matches!(
                    action,
                    "actor_update" | "actor_update_raw" | "actor_organize"
                ) {
                    Some(&args["patches"])
                } else {
                    None
                };
                let actors = keep_device_values(st, mid, actors, patches).await?;
                sqlx::query("UPDATE uefn_state SET actors = ?, updated_at = datetime('now') WHERE map_id = ?").bind(actors.to_string()).bind(mid).execute(&st.db).await?;
                notify(st, mid, json!({ "t": "uefn", "what": "actors" }));
            }
            if let Some(id) = aid {
                // Une duplication ou une pose s'annule en supprimant les acteurs créés.
                let mut extra_undo = None;
                // Un import s'annule en retirant les assets créés (refusé par UEFN s'ils sont utilisés).
                if action == "asset_import" {
                    if let Some(p) = data.get("imported") {
                        extra_undo = Some(
                            json!({ "action": "asset_delete", "args": { "paths": p } }).to_string(),
                        );
                    }
                }
                if action == "actor_duplicate" || action == "actor_spawn" {
                    if let Some(ids) = data.get("created_ids") {
                        extra_undo = Some(
                            json!({ "action": "actor_delete", "args": { "ids": ids } }).to_string(),
                        );
                    }
                }
                // Une suppression s'annule en recréant les acteurs depuis la sauvegarde faite par le relais.
                if action == "actor_delete" {
                    if let Some(saved) = data.get("saved").and_then(|s| s.as_str()) {
                        let cur: Option<(Option<String>,)> =
                            sqlx::query_as("SELECT undo FROM uefn_actions WHERE id = ?")
                                .bind(id)
                                .fetch_optional(&st.db)
                                .await?;
                        if let Some(mut u) = cur
                            .and_then(|(u,)| u)
                            .and_then(|u| serde_json::from_str::<Value>(&u).ok())
                        {
                            u["args"]["saved"] = json!(saved);
                            extra_undo = Some(u.to_string());
                        }
                    }
                }
                let mut short_data = data.clone();
                if let Some(o) = short_data.as_object_mut() {
                    o.remove("actors");
                    o.remove("saved");
                }
                sqlx::query("UPDATE uefn_actions SET status = 'done', after = ?, undo = COALESCE(?, undo) WHERE id = ?").bind(short_data.to_string()).bind(extra_undo).bind(id).execute(&st.db).await?;
            }
        }
        // L'archivage n'a besoin que du relais quand il passe par `lore_cmd` ; sinon il passe par UEFN. Relais
        // en ligne mais UEFN fermé : il reste en attente et part à la réouverture, comme les autres actions (#4).
        Err(AppError::Conflict(m)) if action == "lore_checkin" && m.contains("UEFN est fermé") => {
            if let Some(id) = aid {
                sqlx::query("UPDATE uefn_actions SET status = 'queued' WHERE id = ?")
                    .bind(id)
                    .execute(&st.db)
                    .await?;
            }
            notify(st, mid, json!({ "t": "uefn", "what": "journal" }));
            return Ok(
                json!({ "queued": true, "action_id": aid, "message": "UEFN est fermé : archivage mis en attente, il partira à la réouverture d’UEFN." }),
            );
        }
        // À la réouverture, le serveur MCP d'UEFN répond avant que le projet soit chargé (vérifié le 18/11/2026) :
        // l'action reste en attente et `flush_queue` la réessaie un peu plus tard (#4).
        Err(AppError::Conflict(m))
            if m.contains("no project is currently open") || m.contains("Mcp-Session-Id") =>
        {
            if let Some(id) = aid {
                sqlx::query("UPDATE uefn_actions SET status = 'queued' WHERE id = ?")
                    .bind(id)
                    .execute(&st.db)
                    .await?;
            }
            notify(st, mid, json!({ "t": "uefn", "what": "journal" }));
            return Ok(
                json!({ "queued": true, "retry": true, "action_id": aid, "message": "UEFN ouvre encore le projet : action mise en attente, elle repartira dans quelques secondes." }),
            );
        }
        Err(e) => {
            if let Some(id) = aid {
                sqlx::query("UPDATE uefn_actions SET status = 'failed', error = ? WHERE id = ?")
                    .bind(format!("{e}"))
                    .bind(id)
                    .execute(&st.db)
                    .await?;
            }
        }
    }
    notify(st, mid, json!({ "t": "uefn", "what": "journal" }));
    // Le texte brut d'UEFN reste dans le journal ; l'interface reçoit un message en français (#6).
    let res = res.map_err(|e| match e {
        AppError::Conflict(m) if matches!(action, "actor_update" | "actor_update_raw") => {
            AppError::Conflict(french_option_error(&m))
        }
        AppError::Conflict(m)
            if action == "lore_checkin" && m.to_lowercase().contains("nothing to check in") =>
        {
            AppError::Conflict("Rien à archiver : aucune modification en attente dans Lore.".into())
        }
        e => e,
    });
    res.map(|mut d| {
        if let Some(o) = d.as_object_mut() {
            o.remove("actors");
            o.remove("saved");
        }
        d
    })
}

/// Un réglage @editable garde son type : un entier reste entier, un booléen reste un booléen, un texte reste un
/// texte. UEFN arrondirait 2,5 en 2 sans rien dire (vérifié le 18/11/2026) : POP refuse avant d'envoyer.
/// Une valeur actuelle inconnue (null, référence) ne bloque rien : UEFN tranche.
fn check_prop_type(k: &str, cur: &Value, new: &Value) -> Result<(), AppError> {
    let int = |v: &Value| v.is_i64() || v.is_u64();
    let expected = if int(cur) {
        Some(("un nombre entier", int(new)))
    } else if cur.is_f64() {
        Some(("un nombre", new.is_number()))
    } else if cur.is_boolean() {
        Some(("oui ou non", new.is_boolean()))
    } else if cur.is_string() {
        Some(("un texte", new.is_string()))
    } else {
        None
    };
    match expected {
        Some((what, false)) => Err(bad(format!(
            "« {k} » attend {what} : la valeur {} est refusée, rien n’a changé.",
            short(new)
        ))),
        _ => Ok(()),
    }
}

/// Traduit un refus d'UEFN sur une option ou un réglage (« option bouton : Reference to a device… ») en français.
fn french_option_error(m: &str) -> String {
    let Some(rest) = m.strip_prefix("option ") else {
        return m.to_string();
    };
    let key = rest.split(" : ").next().unwrap_or("").trim();
    let why = if m.contains("Reference to a device") {
        "il faut une référence à un appareil ou à un accessoire posé dans le niveau"
    } else if m.contains("AllowedValues") || m.contains("not accept") || m.contains("rejecting") {
        "cette valeur ne fait pas partie des valeurs acceptées"
    } else {
        "valeur invalide"
    };
    format!(
        "UEFN a refusé la valeur de « {key} » : {why}. Rien n’a changé (détail dans le journal)."
    )
}

async fn apply_file_cache(
    st: &AppState,
    mid: i64,
    action: &str,
    args: &Value,
    uid: i64,
    task: Option<i64>,
    note: &str,
) -> Result<(), AppError> {
    let path = args["path"].as_str().unwrap_or("");
    match action {
        "write_file" => {
            let content = args["content"].as_str().unwrap_or("");
            sqlx::query("INSERT INTO verse_files (map_id, path, content, sha) VALUES (?, ?, ?, ?) ON CONFLICT(map_id, path) DO UPDATE SET content = excluded.content, sha = excluded.sha, updated_at = datetime('now')")
                .bind(mid).bind(path).bind(content).bind(sha256_hex(content)).execute(&st.db).await?;
            sqlx::query("INSERT INTO verse_versions (map_id, path, content, user_id, task_number, note) VALUES (?, ?, ?, ?, ?, ?)").bind(mid).bind(path).bind(content).bind(uid).bind(task).bind(note).execute(&st.db).await?;
            notify(
                st,
                mid,
                json!({ "t": "verse", "what": "file", "path": path, "sha": sha256_hex(content), "by": uid }),
            );
        }
        "delete_file" => {
            sqlx::query("DELETE FROM verse_files WHERE map_id = ? AND path = ?")
                .bind(mid)
                .bind(path)
                .execute(&st.db)
                .await?;
            notify(st, mid, json!({ "t": "verse", "what": "list" }));
        }
        "rename_file" => {
            let to = args["to"].as_str().unwrap_or("");
            sqlx::query("UPDATE verse_files SET path = ? WHERE map_id = ? AND path = ?")
                .bind(to)
                .bind(mid)
                .bind(path)
                .execute(&st.db)
                .await?;
            sqlx::query("UPDATE verse_versions SET path = ? WHERE map_id = ? AND path = ?")
                .bind(to)
                .bind(mid)
                .bind(path)
                .execute(&st.db)
                .await?;
            notify(st, mid, json!({ "t": "verse", "what": "list" }));
        }
        _ => {}
    }
    Ok(())
}

/// Envoie les actions en attente dès que le relais ou UEFN redevient disponible.
pub async fn flush_queue(st: &AppState, mid: i64) {
    // UEFN peut répondre avant d'avoir chargé le projet (2 min 30 pour un petit projet, mesuré le 18/11/2026) :
    // on réessaie toutes les 20 s, 10 minutes au plus (#4).
    for _ in 0..30 {
        let mut retry = false;
        {
            let _g = st.board_lock.lock().await;
            let rows: Vec<(i64, String, String)> = match sqlx::query_as("SELECT id, action, args FROM uefn_actions WHERE map_id = ? AND status = 'queued' ORDER BY id").bind(mid).fetch_all(&st.db).await {
                Ok(r) => r,
                Err(_) => return,
            };
            for (id, action, args) in rows {
                if !ready(st, mid, &action) {
                    continue;
                }
                let args: Value = serde_json::from_str(&args).unwrap_or(json!({}));
                if let Ok(v) = run_action(st, mid, Some(id), &action, &args).await {
                    if v["retry"].as_bool() == Some(true) {
                        retry = true;
                        break;
                    }
                }
            }
        }
        if !retry {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
    }
}

// ---------- Routes de l'Organiseur ----------

#[derive(Deserialize)]
pub struct ActionIn {
    action: String,
    #[serde(default)]
    args: Value,
    task: Option<i64>,
}

pub async fn action(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<ActionIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    if b.action == "actor_restore" || b.action == "actor_update_raw" {
        return Err(AppError::Forbidden);
    }
    Ok(Json(
        exec(
            &st,
            Origin {
                ctx: &ctx,
                source: "web",
                agent: None,
                task: b.task,
            },
            &b.action,
            b.args,
        )
        .await?,
    ))
}

#[derive(Deserialize, Default)]
pub struct JournalQ {
    /// Seulement les actions de cet agent IA (#93).
    agent: Option<i64>,
}

pub async fn journal(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Query(q): Query<JournalQ>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, Option<String>, String, String, String, Option<i64>, String, Option<String>, String, i64, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, u.name, a.source, a.action, a.summary, a.task_number, a.status, a.error, a.at, a.undo IS NOT NULL, a.agent_id, t.name FROM uefn_actions a LEFT JOIN users u ON u.id = a.user_id LEFT JOIN api_tokens t ON t.id = a.agent_id WHERE a.map_id = ? AND (? IS NULL OR a.agent_id = ?) ORDER BY a.id DESC LIMIT 200",
    ).bind(mid).bind(q.agent).bind(q.agent).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|r| json!({
        "id": r.0, "who": r.1, "source": r.2, "action": r.3, "summary": r.4, "task": r.5, "status": r.6, "error": r.7, "at": r.8, "undoable": r.9 == 1 && (r.6 == "done" || r.6 == "queued"), "agent": r.10, "agent_name": r.11
    })).collect::<Vec<_>>())))
}

/// Annule une action : envoie l'action inverse (ou retire une action encore en attente).
pub async fn undo(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (mid, status, undo, uid): (i64, String, Option<String>, Option<i64>) =
        sqlx::query_as("SELECT map_id, status, undo, user_id FROM uefn_actions WHERE id = ?")
            .bind(id)
            .fetch_one(&st.db)
            .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    if uid != Some(u.id) && ctx.role < Role::Admin {
        return Err(AppError::Conflict(
            "Seul l’auteur ou un admin peut annuler cette action.".into(),
        ));
    }
    if status == "queued" {
        sqlx::query("UPDATE uefn_actions SET status = 'undone' WHERE id = ?")
            .bind(id)
            .execute(&st.db)
            .await?;
        notify(&st, mid, json!({ "t": "uefn", "what": "journal" }));
        return Ok(Json(json!({ "ok": true, "removed": true })));
    }
    let u2: Value =
        serde_json::from_str(&undo.ok_or_else(|| bad("Cette action ne s’annule pas."))?)?;
    if status != "done" {
        return Err(bad("Seule une action réussie s’annule."));
    }
    // Une interface POP déjà posée dans le niveau ne se retire pas : le niveau ne compilerait plus (#115).
    if u2["action"] == "delete_file" {
        if let Some(acteur) = crate::widgets::utilise(
            u2["args"]["path"].as_str().unwrap_or(""),
            &snapshot(&st, mid).await?.actors,
        ) {
            return Err(AppError::Conflict(format!(
                "Interface encore utilisée par l’acteur « {acteur} » : retirez-le du niveau avant d’annuler."
            )));
        }
    }
    // Réserve l'annulation avant de l'envoyer : deux Ctrl+Z rapprochés n'annulent pas deux fois la même action (#6).
    let claimed =
        sqlx::query("UPDATE uefn_actions SET status = 'undone' WHERE id = ? AND status = 'done'")
            .bind(id)
            .execute(&st.db)
            .await?
            .rows_affected();
    if claimed == 0 {
        return Err(bad("Cette action vient déjà d’être annulée."));
    }
    let r = match exec(
        &st,
        Origin {
            ctx: &ctx,
            source: "undo",
            agent: None,
            task: None,
        },
        u2["action"].as_str().unwrap_or(""),
        u2["args"].clone(),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            sqlx::query("UPDATE uefn_actions SET status = 'done' WHERE id = ?")
                .bind(id)
                .execute(&st.db)
                .await?;
            return Err(e);
        }
    };
    notify(&st, mid, json!({ "t": "uefn", "what": "journal" }));
    Ok(Json(r))
}

// ---------- Validation humaine (suppressions demandées par une IA) ----------

pub async fn decide(
    State(st): State<AppState>,
    u: AuthUser,
    Path((id, verdict)): Path<(i64, String)>,
) -> ApiResult<Value> {
    let (mid, action, args, status): (i64, String, String, String) =
        sqlx::query_as("SELECT map_id, action, args, status FROM approvals WHERE id = ?")
            .bind(id)
            .fetch_one(&st.db)
            .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    if status != "pending" {
        return Err(bad("Déjà traité."));
    }
    let ok = verdict == "approve";
    sqlx::query("UPDATE approvals SET status = ?, decided_by = ? WHERE id = ?")
        .bind(if ok { "approved" } else { "refused" })
        .bind(u.id)
        .bind(id)
        .execute(&st.db)
        .await?;
    let r = if ok {
        exec(
            &st,
            Origin {
                ctx: &ctx,
                source: "mcp",
                agent: None,
                task: None,
            },
            &action,
            serde_json::from_str(&args)?,
        )
        .await?
    } else {
        json!({ "refused": true })
    };
    notify(&st, mid, json!({ "t": "uefn", "what": "actors" }));
    Ok(Json(r))
}

// ---------- Filtres enregistrés ----------

pub async fn filters(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, name, query FROM saved_filters WHERE map_id = ? ORDER BY name")
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, n, q)| json!({"id": id, "name": n, "query": q}))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct FilterIn {
    name: String,
    query: String,
}

pub async fn add_filter(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<FilterIn>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?.require(Role::Freelance)?;
    check_len("Nom", &b.name, 1, 60)?;
    check_len("Filtre", &b.query, 1, 300)?;
    sqlx::query("INSERT INTO saved_filters (map_id, user_id, name, query) VALUES (?, ?, ?, ?)")
        .bind(mid)
        .bind(u.id)
        .bind(b.name.trim())
        .bind(b.query.trim())
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_filter(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM saved_filters WHERE id = ?")
        .bind(id)
        .fetch_one(&st.db)
        .await?;
    map_ctx(&st, &u, mid).await?.require(Role::Freelance)?;
    sqlx::query("DELETE FROM saved_filters WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_reglage_garde_son_type() {
        assert!(check_prop_type("nombre", &json!(3), &json!(7)).is_ok());
        assert!(check_prop_type("nombre", &json!(3), &json!(2.5)).is_err());
        assert!(check_prop_type("nombre", &json!(3), &json!("abc")).is_err());
        assert!(check_prop_type("vitesse", &json!(1.5), &json!(2)).is_ok());
        assert!(check_prop_type("actif", &json!(true), &json!(false)).is_ok());
        assert!(check_prop_type("actif", &json!(true), &json!("oui")).is_err());
        assert!(check_prop_type("texte", &json!("a"), &json!("b")).is_ok());
        assert!(check_prop_type("texte", &json!("a"), &json!(3)).is_err());
        // Valeur actuelle inconnue (référence vide) : UEFN tranche.
        assert!(check_prop_type("bouton", &Value::Null, &json!("x")).is_ok());
    }

    #[test]
    fn refus_uefn_en_francais() {
        let m = "option bouton : Reference to a device or prop placed in the level: {\"refPath\": \"<actor path>\"}, or null to clear. Got: \"x\" ; Device '/a' has no option named 'bouton'.";
        let f = french_option_error(m);
        assert!(
            f.starts_with("UEFN a refusé la valeur de « bouton » : il faut une référence"),
            "{f}"
        );
        assert_eq!(
            french_option_error("Relais hors ligne."),
            "Relais hors ligne."
        );
    }
}

#[cfg(test)]
mod tests_rangement {
    use super::*;

    /// Rangement du niveau (#84) : une action, annulable ; un acteur verrouillé par un autre est refusé.
    #[tokio::test]
    async fn ranger() {
        let st = crate::test_state().await;
        let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let uid: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('moi@a.fr', 'Moi', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        let u = AuthUser {
            id: uid,
            email: "moi@a.fr".into(),
            name: "Moi".into(),
            sid: String::new(),
        };
        snapshot(&st, mid).await.unwrap();
        let acteurs = json!([
            { "id": "L.A", "name": "Button2", "type": "device", "class": "Button_C", "folder": "", "tags": [], "options": {}, "props": {} },
            { "id": "L.B", "name": "Arbre", "type": "prop", "class": "BP_Arbre_C", "folder": "Decor", "tags": [], "options": {}, "props": {} },
        ]);
        sqlx::query("UPDATE uefn_state SET actors = ?, locks = ? WHERE map_id = ?")
            .bind(acteurs.to_string())
            .bind(json!([{ "target": "L.B", "by": "autre@a.fr" }]).to_string())
            .bind(mid)
            .execute(&st.db)
            .await
            .unwrap();
        let ctx = map_ctx(&st, &u, mid).await.unwrap();
        let o = || Origin {
            ctx: &ctx,
            source: "web",
            task: None,
            agent: None,
        };
        // L'acteur verrouillé par un autre fait refuser le rangement.
        let p = json!({ "patches": [{ "id": "L.B", "changes": { "name": "Arbre_1", "folder": "Accessoires/Arbre" } }] });
        assert!(matches!(
            exec(&st, o(), "actor_organize", p).await,
            Err(AppError::Conflict(_))
        ));
        // Rangement valide : une seule action journalisée, avec son annulation (ancien nom et dossier).
        let p = json!({ "patches": [{ "id": "L.A", "changes": { "name": "Bouton_1", "folder": "Appareils/Bouton" } }] });
        exec(&st, o(), "actor_organize", p).await.unwrap();
        let (resume, undo): (String, String) = sqlx::query_as(
            "SELECT summary, undo FROM uefn_actions WHERE action = 'actor_organize'",
        )
        .fetch_one(&st.db)
        .await
        .unwrap();
        assert_eq!(resume, "Rangement du niveau : 1 acteur(s)");
        let undo: Value = serde_json::from_str(&undo).unwrap();
        assert_eq!(undo["action"], "actor_update_raw");
        assert_eq!(undo["args"]["patches"][0]["changes"]["name"], "Button2");
        assert_eq!(undo["args"]["patches"][0]["changes"]["folder"], "");
    }
}
