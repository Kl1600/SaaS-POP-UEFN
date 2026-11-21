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

const KINDS: &[&str] = &[
    "claude_code",
    "claude_desktop",
    "cursor",
    "codex",
    "windsurf",
    "vscode",
    "gemini",
    "autre",
];

pub async fn create_token(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<TokenIn>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?.require(Role::Freelance)?;
    check_len("Nom", &b.name, 1, 60)?;
    let token = format!("pop_{}", random_token());
    let kind = b
        .kind
        .filter(|k| KINDS.contains(&k.as_str()))
        .unwrap_or_else(|| "autre".into());
    let scope = if b.scope.as_deref() == Some("read") {
        "read"
    } else {
        "write"
    };
    let id: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, map_id, name, token_hash, kind, scope) VALUES (?, ?, ?, ?, ?, ?) RETURNING id").bind(u.id).bind(mid).bind(b.name.trim()).bind(sha256_hex(&token)).bind(&kind).bind(scope).fetch_one(&st.db).await?;
    Ok(Json(json!({ "token": token, "id": id })))
}

pub async fn tokens(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, String, String, String, Option<String>, i64)> = sqlx::query_as("SELECT id, name, created_at, kind, scope, last_used, calls FROM api_tokens WHERE user_id = ? AND map_id = ? ORDER BY id DESC").bind(u.id).bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|(id, n, at, kind, scope, last, calls)| json!({"id": id, "name": n, "created_at": at, "kind": kind, "scope": scope, "last_used": last, "calls": calls})).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct ScopeIn {
    scope: String,
}

pub async fn set_scope(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<ScopeIn>,
) -> ApiResult<Value> {
    let scope = if b.scope == "read" { "read" } else { "write" };
    sqlx::query("UPDATE api_tokens SET scope = ? WHERE id = ? AND user_id = ?")
        .bind(scope)
        .bind(id)
        .bind(u.id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_token(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    sqlx::query("DELETE FROM api_tokens WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(u.id)
        .execute(&st.db)
        .await?;
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
        tool("check_level", "Vérifie le niveau sur le dernier état connu : références d'appareils vers un acteur absent (bloquant), réglages non reliés, options réglées sans effet, tags Verse inutilisés. Chaque problème a sa gravité, l'acteur et une explication.", json!({}), &[]),
        tool("list_verse_files", "Liste les fichiers .verse du projet.", json!({}), &[]),
        tool("read_verse_file", "Lit un fichier .verse.", json!({ "path": { "type": "string" } }), &["path"]),
        tool("write_verse_file", "Écrit un fichier .verse dans le projet (crée ou remplace).", json!({ "path": { "type": "string" }, "content": { "type": "string" }, "task": task }), &["path", "content"]),
        tool("compile_verse", "Compile le Verse dans UEFN et renvoie erreurs et avertissements.", json!({}), &[]),
        tool("push_to_session", "Pousse les changements dans la session de test.", json!({}), &[]),
        tool("save_level", "Enregistre le niveau dans UEFN.", json!({}), &[]),
        tool("lore_checkin", "Archive dans Lore avec un message (« #n ce qui a été fait »).", json!({ "message": { "type": "string" } }), &["message"]),
        tool("list_tasks", "Liste les tâches de la map (numéro, titre, statut, étiquette, personne, jour prévu, dépendances en attente). Filtres facultatifs : status (todo, doing, blocked, done), mine (vos tâches seulement), query (texte).", json!({ "status": { "type": "string" }, "mine": { "type": "boolean" }, "query": { "type": "string" } }), &[]),
        tool("get_task", "Détail d'une tâche par son numéro : description, dépendances, sous-tâches, commentaires.", json!({ "number": { "type": "integer" } }), &["number"]),
        tool("next_task", "Ce que Boussole conseille de faire maintenant : retards, tâches du jour et tâches qui en débloquent d'autres.", json!({}), &[]),
        tool("create_task", "Crée une tâche d'environ une heure. tag : code, carte, conception ou test. depends_on : numéros de tâches. assign_to_me : vous l'assigner.", json!({ "title": { "type": "string" }, "description": { "type": "string" }, "tag": { "type": "string" }, "planned_day": { "type": "string", "description": "AAAA-MM-JJ" }, "depends_on": { "type": "array", "items": { "type": "integer" } }, "assign_to_me": { "type": "boolean" } }), &["title"]),
        tool("set_task_status", "Change le statut d'une tâche : todo (à faire), doing (en cours), blocked (bloquée), done (faite). Passer en cours exige que ses dépendances soient faites. note : commentaire facultatif (ce qui a été fait, ce qui bloque).", json!({ "number": { "type": "integer" }, "status": { "type": "string", "enum": ["todo", "doing", "blocked", "done"] }, "note": { "type": "string" } }), &["number", "status"]),
        tool("claim_task", "Réserve une tâche POP pour vous (agent IA) : une seule tâche à la fois, et une tâche n'a qu'un agent. À faire avant de travailler dessus ; refusé si un autre agent l'a déjà. Vos fichiers Verse de cette tâche ne peuvent pas être modifiés par un autre agent.", json!({ "number": { "type": "integer" } }), &["number"]),
        tool("release_task", "Libère la tâche que vous avez réservée (claim_task), quand vous arrêtez sans la finir.", json!({}), &[]),
        tool("comment_task", "Ajoute un commentaire à une tâche.", json!({ "number": { "type": "integer" }, "text": { "type": "string" } }), &["number", "text"]),
        tool("search_library", "Cherche dans la bibliothèque Verse du studio (modules prêts : économie, équipes, minuteur, HUD, sauvegarde…). À faire AVANT d'écrire du Verse neuf : réutilisez un module existant quand il convient. Filtres facultatifs : query (texte du nom ou de la description), category (economie, equipes, minuteur, hud, sauvegarde, interface, jeu, outils, autre).", json!({ "query": { "type": "string" }, "category": { "type": "string" } }), &[]),
        tool("get_library_module", "Lit un module de la bibliothèque Verse du studio : code complet, appareils requis, version.", json!({ "id": { "type": "integer" } }), &["id"]),
        tool("undo_action", "Annule une action du journal par son identifiant.", json!({ "id": { "type": "integer" } }), &["id"]),
    ])
}

const READ_TOOLS: &[&str] = &[
    "uefn_status",
    "check_level",
    "list_actors",
    "get_actor",
    "list_verse_files",
    "read_verse_file",
    "list_tasks",
    "get_task",
    "next_task",
    "search_library",
    "get_library_module",
];

fn rpc_ok(id: Value, result: Value) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response()
}
fn rpc_err(id: Value, code: i64, msg: &str) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } }))
        .into_response()
}
fn text(v: impl Into<String>, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": v.into() }], "isError": is_error })
}

#[derive(Deserialize)]
pub struct McpQ {
    token: Option<String>,
}

pub async fn mcp(
    State(st): State<AppState>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<McpQ>,
    Json(req): Json<Value>,
) -> Response {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    // Jeton dans l'en-tête Authorization, ou dans l'adresse (?token=) pour les applications qui ne gèrent pas les en-têtes.
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(String::from)
        .or(q.token)
        .unwrap_or_default();
    let token = token.as_str();
    let row: Option<(i64, i64, String, String, String, i64)> = match sqlx::query_as(
        "SELECT t.user_id, t.map_id, u.email, u.name, t.scope, t.id FROM api_tokens t JOIN users u ON u.id = t.user_id WHERE t.token_hash = ? AND u.disabled = 0",
    ).bind(sha256_hex(token)).fetch_optional(&st.db).await {
        Ok(r) => r,
        Err(_) => return rpc_err(id, -32603, "Erreur interne"),
    };
    let Some((uid, mid, email, name, scope, agent)) = row else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Jeton POP invalide." })),
        )
            .into_response();
    };
    let _ = sqlx::query(
        "UPDATE api_tokens SET last_used = datetime('now'), calls = calls + 1 WHERE token_hash = ?",
    )
    .bind(sha256_hex(token))
    .execute(&st.db)
    .await;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "ai" }));
    let user = AuthUser {
        id: uid,
        email,
        name,
        sid: String::new(),
    };
    let method = req["method"].as_str().unwrap_or("");
    match method {
        "initialize" => rpc_ok(
            id,
            json!({
                "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "pop", "version": "0.1.0" },
                "instructions": "Outils POP : gérer les tâches de la map (list_tasks, next_task, create_task, set_task_status, comment_task) et piloter UEFN via le relais du créateur. Avant de travailler, prenez une tâche avec next_task et passez-la en cours ; quand elle est finie, passez-la en done avec une note. Toute action UEFN est journalisée et annulable ; les suppressions attendent une validation humaine."
            }),
        ),
        m if m.starts_with("notifications/") => StatusCode::ACCEPTED.into_response(),
        "ping" => rpc_ok(id, json!({})),
        "tools/list" => rpc_ok(
            id,
            json!({ "tools": tools().as_array().cloned().unwrap_or_default().into_iter().filter(|t| scope != "read" || READ_TOOLS.contains(&t["name"].as_str().unwrap_or(""))).collect::<Vec<_>>() }),
        ),
        "tools/call" => {
            let name = req["params"]["name"].as_str().unwrap_or("");
            if scope == "read" && !READ_TOOLS.contains(&name) {
                return rpc_ok(id, text("Cette IA est en lecture seule dans POP (IA connectées → autoriser les modifications).", true));
            }
            let args = req["params"]["arguments"].clone();
            match call_as(&st, &user, mid, Some(agent), name, args).await {
                Ok(v) => rpc_ok(
                    id,
                    text(serde_json::to_string_pretty(&v).unwrap_or_default(), false),
                ),
                Err(e) => rpc_ok(
                    id,
                    text(
                        match e {
                            AppError::BadRequest(m) | AppError::Conflict(m) => m,
                            AppError::Forbidden => "Action non autorisée pour votre rôle.".into(),
                            AppError::NotFound => "Introuvable.".into(),
                            other => format!("{other}"),
                        },
                        true,
                    ),
                ),
            }
        }
        _ => rpc_err(id, -32601, "Méthode inconnue"),
    }
}

#[cfg(test)]
async fn call(
    st: &AppState,
    user: &AuthUser,
    mid: i64,
    name: &str,
    a: Value,
) -> Result<Value, AppError> {
    call_as(st, user, mid, None, name, a).await
}

/// Appel d'un outil par l'agent `agent` (jeton MCP), au nom de l'humain qui a créé le jeton.
async fn call_as(
    st: &AppState,
    user: &AuthUser,
    mid: i64,
    agent: Option<i64>,
    name: &str,
    a: Value,
) -> Result<Value, AppError> {
    let ctx = map_ctx(st, user, mid).await?;
    let task = a["task"].as_i64();
    let o = || Origin {
        ctx: &ctx,
        source: "mcp",
        agent,
        task,
    };
    let agent_requis =
        || agent.ok_or_else(|| crate::error::bad("Réservé aux agents IA (jeton MCP)."));
    let task_by_number = |n: Option<i64>| async move {
        let n = n.ok_or_else(|| crate::error::bad("Numéro de tâche manquant."))?;
        let r: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM tasks WHERE map_id = ? AND number = ?")
                .bind(mid)
                .bind(n)
                .fetch_optional(&st.db)
                .await?;
        r.map(|r| r.0)
            .ok_or_else(|| crate::error::bad(&format!("Tâche #{n} introuvable dans cette map.")))
    };
    match name {
        "check_level" => crate::checks::rapport(st, mid).await,
        "search_library" => {
            let q = crate::library::SearchQ {
                q: a["query"].as_str().map(String::from),
                category: a["category"].as_str().map(String::from),
            };
            Ok(json!(crate::library::search(st, ctx.studio_id, &q).await?))
        }
        "get_library_module" => {
            let id = a["id"]
                .as_i64()
                .ok_or_else(|| crate::error::bad("Identifiant du module manquant."))?;
            crate::library::module(st, ctx.studio_id, id).await
        }
        "claim_task" => {
            let n = a["number"]
                .as_i64()
                .ok_or_else(|| crate::error::bad("Numéro de tâche manquant."))?;
            crate::agents::claim(st, agent_requis()?, mid, n).await
        }
        "release_task" => crate::agents::release(st, agent_requis()?).await,
        "list_tasks" => {
            let rows: Vec<(i64, String, String, String, Option<String>, Option<i64>, Option<String>, i64)> = sqlx::query_as(
                "SELECT t.number, t.title, t.status, t.tag, u.name, t.assignee_id, t.planned_day, t.id FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id WHERE t.map_id = ? ORDER BY t.status = 'done', t.planned_day IS NULL, t.planned_day, t.number",
            ).bind(mid).fetch_all(&st.db).await?;
            let deps: Vec<(i64, i64)> = sqlx::query_as("SELECT d.task_id, o.number FROM task_deps d JOIN tasks o ON o.id = d.depends_on JOIN tasks t ON t.id = d.task_id WHERE t.map_id = ? AND o.status != 'done'").bind(mid).fetch_all(&st.db).await?;
            let q = a["query"].as_str().unwrap_or("").to_lowercase();
            let out: Vec<Value> = rows.into_iter().filter(|r| a["status"].as_str().map(|s| s == r.2).unwrap_or(true) && (!a["mine"].as_bool().unwrap_or(false) || r.5 == Some(user.id)) && (q.is_empty() || r.1.to_lowercase().contains(&q)))
                .take(200).map(|r| json!({ "number": r.0, "title": r.1, "status": r.2, "tag": r.3, "assignee": r.4, "planned_day": r.6, "waiting_on": deps.iter().filter(|d| d.0 == r.7).map(|d| d.1).collect::<Vec<_>>() })).collect();
            Ok(json!(out))
        }
        "get_task" => {
            let tid = task_by_number(a["number"].as_i64()).await?;
            let crate::Json(v) =
                crate::tasks::detail(State(st.clone()), user.clone(), axum::extract::Path(tid))
                    .await?;
            Ok(v)
        }
        "next_task" => {
            let crate::Json(v) = crate::guide::guide(
                State(st.clone()),
                user.clone(),
                axum::extract::Path(mid),
                axum::extract::Query(Default::default()),
            )
            .await?;
            Ok(json!({ "résumé": v["points"], "suggestions": v["suggestions"] }))
        }
        "create_task" => {
            ctx.can_edit()?;
            let b = crate::tasks::NewTask {
                title: a["title"].as_str().unwrap_or("").to_string(),
                tag: a["tag"].as_str().map(String::from),
                description: a["description"].as_str().map(String::from),
                assignee_id: if a["assign_to_me"].as_bool().unwrap_or(false) {
                    Some(user.id)
                } else {
                    None
                },
                deps: a["depends_on"]
                    .as_array()
                    .map(|d| d.iter().filter_map(|x| x.as_i64()).collect()),
                planned_day: a["planned_day"].as_str().map(String::from),
                lot: None,
                status: None,
                ai: Some(true),
                origin: Some("ia".into()),
            };
            if b.title.trim().is_empty() {
                return Err(crate::error::bad("Titre manquant."));
            }
            let (tid, number) = crate::tasks::insert_task(st, &ctx, &b).await?;
            crate::access::notify(st, mid, json!({ "t": "refresh", "what": "tasks" }));
            Ok(json!({ "id": tid, "number": number, "message": format!("Tâche #{number} créée.") }))
        }
        "set_task_status" => {
            ctx.can_edit()?;
            let tid = task_by_number(a["number"].as_i64()).await?;
            let status = a["status"].as_str().unwrap_or("");
            crate::tasks::set_status_internal(st, mid, tid, user.id, status, "IA").await?;
            if let Some(note) = a["note"].as_str().filter(|n| !n.trim().is_empty()) {
                sqlx::query(
                    "INSERT INTO comments (map_id, target, user_id, body) VALUES (?, ?, ?, ?)",
                )
                .bind(mid)
                .bind(format!("task:{tid}"))
                .bind(user.id)
                .bind(format!("[IA] {}", note.trim()))
                .execute(&st.db)
                .await?;
            }
            crate::access::notify(st, mid, json!({ "t": "refresh", "what": "tasks" }));
            Ok(
                json!({ "message": format!("Tâche #{} : {}.", a["number"], crate::tasks::status_label(status)) }),
            )
        }
        "comment_task" => {
            ctx.can_edit()?;
            let tid = task_by_number(a["number"].as_i64()).await?;
            let body = a["text"].as_str().unwrap_or("").trim();
            if body.is_empty() {
                return Err(crate::error::bad("Commentaire vide."));
            }
            sqlx::query("INSERT INTO comments (map_id, target, user_id, body) VALUES (?, ?, ?, ?)")
                .bind(mid)
                .bind(format!("task:{tid}"))
                .bind(user.id)
                .bind(format!("[IA] {body}"))
                .execute(&st.db)
                .await?;
            crate::access::notify(st, mid, json!({ "t": "refresh", "what": "tasks" }));
            Ok(json!({ "message": "Commentaire ajouté." }))
        }
        "uefn_status" => {
            let pending: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM uefn_actions WHERE map_id = ? AND status = 'queued'",
            )
            .bind(mid)
            .fetch_one(&st.db)
            .await?;
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
            snap.actors
                .into_iter()
                .find(|x| x["id"] == a["id"])
                .ok_or(AppError::NotFound)
        }
        "update_actors" => {
            exec(
                st,
                o(),
                "actor_update",
                json!({ "ids": a["ids"], "changes": a["changes"] }),
            )
            .await
        }
        "duplicate_actors" => exec(st, o(), "actor_duplicate", json!({ "ids": a["ids"] })).await,
        "delete_actors" => {
            ctx.require(Role::Member)?;
            let snap = snapshot(st, mid).await?;
            let ids: Vec<String> = a["ids"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect();
            if ids.is_empty() {
                return Err(bad("Aucun acteur."));
            }
            let names: Vec<String> = ids
                .iter()
                .map(|i| {
                    snap.actors
                        .iter()
                        .find(|x| x["id"].as_str() == Some(i))
                        .and_then(|x| x["name"].as_str())
                        .unwrap_or(i)
                        .to_string()
                })
                .collect();
            let summary = format!(
                "L’IA de {} demande de supprimer : {}",
                user.name,
                names.join(", ")
            );
            let aid: i64 = sqlx::query_scalar("INSERT INTO approvals (map_id, user_id, action, args, summary) VALUES (?, ?, 'actor_delete', ?, ?) RETURNING id")
                .bind(mid).bind(user.id).bind(json!({ "ids": ids }).to_string()).bind(&summary).fetch_one(&st.db).await?;
            let admins: Vec<i64> = sqlx::query_scalar("SELECT m.user_id FROM memberships m JOIN maps p ON p.studio_id = m.studio_id WHERE p.id = ? AND m.role IN ('owner', 'admin', 'member')").bind(mid).fetch_all(&st.db).await.unwrap_or_default();
            for a in admins {
                crate::account::notify_user(
                    st,
                    a,
                    mid,
                    "Une IA connectée demande à supprimer des acteurs : validation requise",
                    "/organiseur",
                )
                .await;
            }
            notify(st, mid, json!({ "t": "uefn", "what": "actors" }));
            Ok(
                json!({ "pending_approval": aid, "message": "Suppression en attente : un membre doit la valider dans l’Organiseur de POP." }),
            )
        }
        "list_verse_files" => {
            let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT path, updated_at FROM verse_files WHERE map_id = ? ORDER BY path",
            )
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
            Ok(json!(rows
                .into_iter()
                .map(|(p, at)| json!({"path": p, "updated_at": at}))
                .collect::<Vec<_>>()))
        }
        "read_verse_file" => {
            let (c,): (String,) =
                sqlx::query_as("SELECT content FROM verse_files WHERE map_id = ? AND path = ?")
                    .bind(mid)
                    .bind(a["path"].as_str().unwrap_or(""))
                    .fetch_one(&st.db)
                    .await?;
            Ok(json!({ "path": a["path"], "content": c }))
        }
        "write_verse_file" => {
            if let Some(ag) = agent {
                crate::agents::check_file(st, ag, mid, a["path"].as_str().unwrap_or("")).await?;
            }
            exec(
                st,
                o(),
                "write_file",
                json!({ "path": a["path"], "content": a["content"] }),
            )
            .await
        }
        "compile_verse" => exec(st, o(), "compile", json!({})).await,
        "push_to_session" => exec(st, o(), "push_session", json!({})).await,
        "save_level" => exec(st, o(), "save_level", json!({})).await,
        "lore_checkin" => exec(st, o(), "lore_checkin", json!({ "message": a["message"] })).await,
        "undo_action" => {
            let id = a["id"].as_i64().ok_or_else(|| bad("id manquant"))?;
            crate::uefn::undo(State(st.clone()), user.clone(), Path(id))
                .await
                .map(|j| j.0)
        }
        _ => Err(bad("Outil inconnu.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une IA connectée cherche dans la bibliothèque Verse de son studio, puis lit un module (#85).
    #[tokio::test]
    async fn bibliotheque_par_mcp() {
        let st = crate::test_state().await;
        let sid: i64 =
            sqlx::query_scalar("INSERT INTO studios (name) VALUES ('Studio') RETURNING id")
                .fetch_one(&st.db)
                .await
                .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'Map') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let uid: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('ia@a.fr', 'IA', 'x') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')")
            .bind(sid)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        let id: i64 = sqlx::query_scalar("INSERT INTO verse_modules (studio_id, name, category, content) VALUES (?, 'Banque de pièces', 'economie', 'banque := class:') RETURNING id")
            .bind(sid)
            .fetch_one(&st.db)
            .await
            .unwrap();
        let u = AuthUser {
            id: uid,
            email: "ia@a.fr".into(),
            name: "IA".into(),
            sid: String::new(),
        };
        let r = call(&st, &u, mid, "search_library", json!({ "query": "pièces" }))
            .await
            .unwrap();
        assert_eq!(r[0]["id"], id);
        let r = call(&st, &u, mid, "search_library", json!({ "category": "hud" }))
            .await
            .unwrap();
        assert_eq!(r.as_array().unwrap().len(), 0);
        let m = call(&st, &u, mid, "get_library_module", json!({ "id": id }))
            .await
            .unwrap();
        assert_eq!(m["content"], "banque := class:");
        assert!(
            READ_TOOLS.contains(&"search_library") && READ_TOOLS.contains(&"get_library_module")
        );
    }
}
