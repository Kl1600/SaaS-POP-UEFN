//! Temps réel : un canal par map (présence, curseurs, opérations du tableau blanc, rafraîchissements).
use crate::{access::map_ctx, auth::AuthUser, error::AppError, AppState};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::HeaderMap,
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};
use tokio::sync::broadcast;

#[derive(Clone)]
struct Presence {
    uid: i64,
    name: String,
    place: String,
}

#[derive(Default)]
pub struct Hub {
    chans: Mutex<HashMap<i64, broadcast::Sender<String>>>,
    presence: Mutex<HashMap<i64, HashMap<u64, Presence>>>,
    next: AtomicU64,
}

impl Hub {
    fn chan(&self, map_id: i64) -> broadcast::Sender<String> {
        let mut c = self.chans.lock().unwrap();
        c.entry(map_id).or_insert_with(|| broadcast::channel(256).0).clone()
    }
    pub fn send(&self, map_id: i64, msg: Value) {
        let _ = self.chan(map_id).send(msg.to_string());
    }
    fn presence_list(&self, map_id: i64) -> Value {
        let p = self.presence.lock().unwrap();
        let mut seen = HashMap::new();
        if let Some(m) = p.get(&map_id) {
            for e in m.values() {
                seen.insert(e.uid, json!({ "uid": e.uid, "name": e.name, "place": e.place }));
            }
        }
        json!({ "t": "presence", "users": seen.into_values().collect::<Vec<_>>() })
    }
    fn set_presence(&self, map_id: i64, conn: u64, p: Option<Presence>) {
        let mut all = self.presence.lock().unwrap();
        let m = all.entry(map_id).or_default();
        match p {
            Some(p) => {
                m.insert(conn, p);
            }
            None => {
                m.remove(&conn);
            }
        }
    }
}

/// Refuse les connexions WebSocket venant d'un autre site (protection CSWSH).
fn same_origin(h: &HeaderMap) -> bool {
    let host = h.get("host").and_then(|v| v.to_str().ok()).unwrap_or("");
    match h.get("origin").and_then(|v| v.to_str().ok()) {
        None => true,
        Some(o) => {
            let o = o.trim_start_matches("http://").trim_start_matches("https://");
            o == host || o.starts_with("localhost:") || o.starts_with("127.0.0.1:")
        }
    }
}

pub async fn ws_handler(
    State(st): State<AppState>,
    user: AuthUser,
    Path(map_id): Path<i64>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    if !same_origin(&headers) {
        return Err(AppError::Forbidden);
    }
    let ctx = map_ctx(&st, &user, map_id).await?;
    let can_edit = ctx.can_edit().is_ok();
    Ok(ws.on_upgrade(move |socket| run(st, user, map_id, can_edit, socket)))
}

async fn run(st: AppState, user: AuthUser, map_id: i64, can_edit: bool, socket: WebSocket) {
    let conn = st.hub.next.fetch_add(1, Ordering::Relaxed);
    let mut rx = st.hub.chan(map_id).subscribe();
    let (mut tx, mut inbound) = socket.split();
    st.hub.set_presence(map_id, conn, Some(Presence { uid: user.id, name: user.name.clone(), place: String::new() }));
    st.hub.send(map_id, st.hub.presence_list(map_id));

    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Ok(text) => { if tx.send(Message::Text(text.into())).await.is_err() { break; } }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            incoming = inbound.next() => match incoming {
                Some(Ok(Message::Text(t))) => {
                    if t.len() > 1_500_000 { continue; }
                    let Ok(v) = serde_json::from_str::<Value>(&t) else { continue };
                    handle(&st, &user, map_id, conn, can_edit, v).await;
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            }
        }
    }
    st.hub.set_presence(map_id, conn, None);
    st.hub.send(map_id, st.hub.presence_list(map_id));
}

async fn handle(st: &AppState, user: &AuthUser, map_id: i64, conn: u64, can_edit: bool, v: Value) {
    match v["t"].as_str().unwrap_or("") {
        "place" => {
            let place: String = v["place"].as_str().unwrap_or("").chars().take(80).collect();
            st.hub.set_presence(map_id, conn, Some(Presence { uid: user.id, name: user.name.clone(), place }));
            st.hub.send(map_id, st.hub.presence_list(map_id));
        }
        "cursor" => {
            let place: String = v["place"].as_str().unwrap_or("").chars().take(80).collect();
            st.hub.send(map_id, json!({
                "t": "cursor", "uid": user.id, "name": user.name, "place": place,
                "x": v["x"].as_f64().unwrap_or(0.0), "y": v["y"].as_f64().unwrap_or(0.0)
            }));
        }
        // Édition à plusieurs dans l'éditeur Verse : mises à jour Yjs relayées telles quelles.
        "yjs" if can_edit => {
            let path: String = v["path"].as_str().unwrap_or("").chars().take(400).collect();
            st.hub.send(map_id, json!({ "t": "yjs", "uid": user.id, "conn": conn, "path": path, "update": v["update"].as_str().unwrap_or(""), "full": v["full"].as_bool().unwrap_or(false), "base": v["base"].as_str().unwrap_or(""), "cid": v["cid"].as_str().unwrap_or("") }));
        }
        "yreq" => {
            let path: String = v["path"].as_str().unwrap_or("").chars().take(400).collect();
            st.hub.send(map_id, json!({ "t": "yreq", "uid": user.id, "conn": conn, "path": path, "cid": v["cid"].as_str().unwrap_or("") }));
        }
        "board" if can_edit => {
            if let Err(e) = crate::boards::apply_op(st, user, map_id, &v).await {
                tracing::warn!("op tableau refusée : {e:?}");
            }
        }
        _ => {}
    }
}
