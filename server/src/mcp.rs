//! Serveur MCP de POP : expose à l'IA connectée du créateur les mêmes actions que l'Organiseur et l'éditeur Verse.
//! Tout est journalisé ; une suppression attend la validation d'un humain.
use crate::{
    access::{map_ctx, notify, Role},
    auth::{random_token, sha256_hex, AuthUser},
    error::{bad, check_len, ApiResult, AppError},
    uefn::{exec, snapshot, Origin},
    AppState,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

// ---------- Jetons personnels ----------

#[derive(Deserialize)]
pub struct TokenIn {
    name: String,
    kind: Option<String>,
    scope: Option<String>,
}

const KINDS: &[&str] = &["claude_code", "claude_desktop", "cursor", "codex", "windsurf", "vscode", "gemini", "autre"];

pub async fn create_token(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<TokenIn>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?.require(Role::Freelance)?;
    check_len("Nom", &b.name, 1, 60)?;
    let token = format!("pop_{}", random_token());
    let kind = b.kind.filter(|k| KINDS.contains(&k.as_str())).unwrap_or_else(|| "autre".into());
    let scope = if b.scope.as_deref() == Some("read") { "read" } else { "write" };
    let id: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, map_id, name, token_hash, kind, scope) VALUES (?, ?, ?, ?, ?, ?) RETURNING id").bind(u.id).bind(mid).bind(b.name.trim()).bind(sha256_hex(&token)).bind(&kind).bind(scope).fetch_one(&st.db).await?;
    Ok(Json(json!({ "token": token, "id": id })))
}

pub async fn tokens(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, String, String, String, Option<String>, i64)> = sqlx::query_as("SELECT id, name, created_at, kind, scope, last_used, calls FROM api_tokens WHERE user_id = ? AND map_id = ? ORDER BY id DESC").bind(u.id).bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|(id, n, at, kind, scope, last, calls)| json!({"id": id, "name": n, "created_at": at, "kind": kind, "scope": scope, "last_used": last, "calls": calls})).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct ScopeIn {
    scope: String,
}

pub async fn set_scope(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>, Json(b): Json<ScopeIn>) -> ApiResult<Value> {
    let scope = if b.scope == "read" { "read" } else { "write" };
    sqlx::query("UPDATE api_tokens SET scope = ? WHERE id = ? AND user_id = ?").bind(scope).bind(id).bind(u.id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_token(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>) -> ApiResult<Value> {
    sqlx::query("DELETE FROM api_tokens WHERE id = ? AND user_id = ?").bind(id).bind(u.id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------- Protocole MCP (JSON-RPC sur HTTP) ----------

fn tool(name: &str, desc: &str, props: Value, required: &[&str]) -> Value {
    json!({ "name": name, "description": desc, "inputSchema": { "type": "object", "properties": props, "required": required } })
}

fn tools() -> Value {
    let ids = json!({ "type": "array", "items": { "type": "string" }, "description": "Identifiants des acteurs" });
    let task = json!({ "type": "integer", "description": "N° de la tâche POP liée (facultatif)" });
    json!([
        tool("uefn_status", "État du relais et d'UEFN (connecté, fermé, hors ligne) et nombre d'actions en attente.", json!({}), &[]),
        tool("list_actors", "Liste les acteurs du niveau. Filtre facultatif : texte cherché dans le nom, le type, le dossier, les tags et les options.", json!({ "query": { "type": "string" } }), &[]),
        tool("get_actor", "Détails d'un acteur : transformation, tags, options, propriétés Verse, liens d'événements.", json!({ "id": { "type": "string" } }), &["id"]),
        tool("update_actors", "Modifie un ou plusieurs acteurs. changes peut contenir name, folder, hidden, transform {location,rotation,scale}, options {clé: valeur}, props {clé: valeur}, tags_add, tags_remove, events.", json!({ "ids": ids, "changes": { "type": "object" }, "task": task }), &["ids", "changes"]),
        tool("duplicate_actors", "Duplique des acteurs.", json!({ "ids": ids }), &["ids"]),
        tool("delete_actors", "Demande la suppression d'acteurs. Un humain doit valider dans POP avant l'exécution.", json!({ "ids": ids }), &["ids"]),
        tool("list_verse_files", "Liste les fichiers .verse du projet.", json!({}), &[]),
        tool("read_verse_file", "Lit un fichier .verse.", json!({ "path": { "type": "string" } }), &["path"]),
        tool("write_verse_file", "Écrit un fichier .verse dans le projet (crée ou remplace).", json!({ "path": { "type": "string" }, "content": { "type": "string" }, "task": task }), &["path", "content"]),
        tool("compile_verse", "Compile le Verse dans UEFN et renvoie erreurs et avertissements.", json!({}), &[]),
        tool("push_to_session", "Pousse les changements dans la session de test.", json!({}), &[]),
        tool("save_level", "Enregistre le niveau dans UEFN.", json!({}), &[]),
        tool("lore_checkin", "Archive dans Lore avec un message (« #n ce qui a été fait »).", json!({ "message": { "type": "string" } }), &["message"]),
        tool("undo_action", "Annule une action du journal par son identifiant.", json!({ "id": { "type": "integer" } }), &["id"]),
    ])
}

const READ_TOOLS: &[&str] = &["uefn_status", "list_actors", "get_actor", "list_verse_files", "read_verse_file"];

fn rpc_ok(id: Value, result: Value) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response()
}
fn rpc_err(id: Value, code: i64, msg: &str) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } })).into_response()
}
fn text(v: impl Into<String>, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": v.into() }], "isError": is_error })
}

#[derive(Deserialize)]
pub struct McpQ {
    token: Option<String>,
}

pub async fn mcp(State(st): State<AppState>, headers: HeaderMap, axum::extract::Query(q): axum::extract::Query<McpQ>, Json(req): Json<Value>) -> Response {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    // Jeton dans l'en-tête Authorization, ou dans l'adresse (?token=) pour les applications qui ne gèrent pas les en-têtes.
    let token = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).map(String::from).or(q.token).unwrap_or_default();
    let token = token.as_str();
    let row: Option<(i64, i64, String, String, String)> = match sqlx::query_as(
        "SELECT t.user_id, t.map_id, u.email, u.name, t.scope FROM api_tokens t JOIN users u ON u.id = t.user_id WHERE t.token_hash = ?",
    ).bind(sha256_hex(token)).fetch_optional(&st.db).await {
        Ok(r) => r,
        Err(_) => return rpc_err(id, -32603, "Erreur interne"),
    };
    let Some((uid, mid, email, name, scope)) = row else {
        return (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Jeton POP invalide." }))).into_response();
    };
    let _ = sqlx::query("UPDATE api_tokens SET last_used = datetime('now'), calls = calls + 1 WHERE token_hash = ?").bind(sha256_hex(token)).execute(&st.db).await;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "ai" }));
    let user = AuthUser { id: uid, email, name };
    let method = req["method"].as_str().unwrap_or("");
    match method {
        "initialize" => rpc_ok(id, json!({
            "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "pop", "version": "0.1.0" },
            "instructions": "Outils POP pour piloter UEFN via le relais du créateur. Toute action est journalisée et annulable. Les suppressions attendent une validation humaine."
        })),
        m if m.starts_with("notifications/") => StatusCode::ACCEPTED.into_response(),
        "ping" => rpc_ok(id, json!({})),
        "tools/list" => rpc_ok(id, json!({ "tools": tools().as_array().cloned().unwrap_or_default().into_iter().filter(|t| scope != "read" || READ_TOOLS.contains(&t["name"].as_str().unwrap_or(""))).collect::<Vec<_>>() })),
        "tools/call" => {
            let name = req["params"]["name"].as_str().unwrap_or("");
            if scope == "read" && !READ_TOOLS.contains(&name) {
                return rpc_ok(id, text("Cette IA est en lecture seule dans POP (IA connectées → autoriser les modifications).", true));
            }
            let args = req["params"]["arguments"].clone();
            match call(&st, &user, mid, name, args).await {
                Ok(v) => rpc_ok(id, text(serde_json::to_string_pretty(&v).unwrap_or_default(), false)),
                Err(e) => rpc_ok(id, text(match e { AppError::BadRequest(m) | AppError::Conflict(m) => m, AppError::Forbidden => "Action non autorisée pour votre rôle.".into(), AppError::NotFound => "Introuvable.".into(), other => format!("{other}") }, true)),
            }
        }
        _ => rpc_err(id, -32601, "Méthode inconnue"),
    }
}

async fn call(st: &AppState, user: &AuthUser, mid: i64, name: &str, a: Value) -> Result<Value, AppError> {
    let ctx = map_ctx(st, user, mid).await?;
    let task = a["task"].as_i64();
    let o = || Origin { ctx: &ctx, source: "mcp", task };
    match name {
        "uefn_status" => {
            let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uefn_actions WHERE map_id = ? AND status = 'queued'").bind(mid).fetch_one(&st.db).await?;
            Ok(json!({ "status": st.relays.status(mid), "pending": pending }))
        }
        "list_actors" => {
            let snap = snapshot(st, mid).await?;
            let q = a["query"].as_str().unwrap_or("").to_lowercase();
            let list: Vec<Value> = snap.actors.iter().filter(|x| q.is_empty() || x.to_string().to_lowercase().contains(&q)).take(300)
                .map(|x| json!({ "id": x["id"], "name": x["name"], "type": x["type"], "class": x["class"], "folder": x["folder"], "tags": x["tags"] })).collect();
            Ok(json!({ "count": list.len(), "actors": list }))
        }
        "get_actor" => {
            let snap = snapshot(st, mid).await?;
            snap.actors.into_iter().find(|x| x["id"] == a["id"]).ok_or(AppError::NotFound)
        }
        "update_actors" => exec(st, o(), "actor_update", json!({ "ids": a["ids"], "changes": a["changes"] })).await,
        "duplicate_actors" => exec(st, o(), "actor_duplicate", json!({ "ids": a["ids"] })).await,
        "delete_actors" => {
            ctx.require(Role::Member)?;
            let snap = snapshot(st, mid).await?;
            let ids: Vec<String> = a["ids"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_str().map(String::from)).collect();
            if ids.is_empty() {
                return Err(bad("Aucun acteur."));
            }
            let names: Vec<String> = ids.iter().map(|i| snap.actors.iter().find(|x| x["id"].as_str() == Some(i)).and_then(|x| x["name"].as_str()).unwrap_or(i).to_string()).collect();
            let summary = format!("L’IA de {} demande de supprimer : {}", user.name, names.join(", "));
            let aid: i64 = sqlx::query_scalar("INSERT INTO approvals (map_id, user_id, action, args, summary) VALUES (?, ?, 'actor_delete', ?, ?) RETURNING id")
                .bind(mid).bind(user.id).bind(json!({ "ids": ids }).to_string()).bind(&summary).fetch_one(&st.db).await?;
            let admins: Vec<i64> = sqlx::query_scalar("SELECT m.user_id FROM memberships m JOIN maps p ON p.studio_id = m.studio_id WHERE p.id = ? AND m.role IN ('owner', 'admin', 'member')").bind(mid).fetch_all(&st.db).await.unwrap_or_default();
            for a in admins { crate::account::notify_user(st, a, mid, "Une IA connectée demande à supprimer des acteurs : validation requise", "/organiseur").await; }
            notify(st, mid, json!({ "t": "uefn", "what": "actors" }));
            Ok(json!({ "pending_approval": aid, "message": "Suppression en attente : un membre doit la valider dans l’Organiseur de POP." }))
        }
        "list_verse_files" => {
            let rows: Vec<(String, String)> = sqlx::query_as("SELECT path, updated_at FROM verse_files WHERE map_id = ? ORDER BY path").bind(mid).fetch_all(&st.db).await?;
            Ok(json!(rows.into_iter().map(|(p, at)| json!({"path": p, "updated_at": at})).collect::<Vec<_>>()))
        }
        "read_verse_file" => {
            let (c,): (String,) = sqlx::query_as("SELECT content FROM verse_files WHERE map_id = ? AND path = ?").bind(mid).bind(a["path"].as_str().unwrap_or("")).fetch_one(&st.db).await?;
            Ok(json!({ "path": a["path"], "content": c }))
        }
        "write_verse_file" => exec(st, o(), "write_file", json!({ "path": a["path"], "content": a["content"] })).await,
        "compile_verse" => exec(st, o(), "compile", json!({})).await,
        "push_to_session" => exec(st, o(), "push_session", json!({})).await,
        "save_level" => exec(st, o(), "save_level", json!({})).await,
        "lore_checkin" => exec(st, o(), "lore_checkin", json!({ "message": a["message"] })).await,
        "undo_action" => {
            let id = a["id"].as_i64().ok_or_else(|| bad("id manquant"))?;
            crate::uefn::undo(State(st.clone()), user.clone(), Path(id)).await.map(|j| j.0)
        }
        _ => Err(bad("Outil inconnu.")),
    }
}
