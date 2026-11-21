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
        tool("place_device", "Pose un appareil dans le niveau. Soit asset (chemin de l'appareil dans le Content Browser d'UEFN, ex. /Game/…), soit like (identifiant d'un appareil du niveau à copier avec ses réglages). Facultatifs : name, folder, location [x, y, z] et rotation [pitch, yaw, roll] en coordonnées du moteur. Journalisé et annulable (l'annulation demandée par une IA attend un humain).", json!({ "asset": { "type": "string" }, "like": { "type": "string" }, "name": { "type": "string" }, "folder": { "type": "string" }, "location": { "type": "array", "items": { "type": "number" } }, "rotation": { "type": "array", "items": { "type": "number" } }, "task": task }), &[]),
        tool("import_asset", "Importe un fichier (FBX, OBJ, PNG, JPG, TGA, WAV, OGG, MP3 ; 200 Mo au plus) dans le projet UEFN ouvert. local_path : chemin du fichier sur le PC d'UEFN ; ou url : adresse sur le serveur POP. folder : dossier du Content Browser (facultatif, sinon /<projet>/POP). Journalisé ; l'annulation retire l'asset s'il n'est pas utilisé (demandée par une IA, elle attend un humain).", json!({ "local_path": { "type": "string" }, "url": { "type": "string" }, "filename": { "type": "string" }, "folder": { "type": "string" }, "task": task }), &[]),
        tool("duplicate_actors", "Duplique des acteurs.", json!({ "ids": ids }), &["ids"]),
        tool("delete_actors", "Demande la suppression d'acteurs. Un humain doit valider dans POP avant l'exécution.", json!({ "ids": ids }), &["ids"]),
        tool("check_level", "Vérifie le niveau sur le dernier état connu : références d'appareils vers un acteur absent (bloquant), réglages non reliés, options réglées sans effet, tags Verse inutilisés. Chaque problème a sa gravité, l'acteur et une explication.", json!({}), &[]),
        tool("list_verse_files", "Liste les fichiers .verse du projet.", json!({}), &[]),
        tool("read_verse_file", "Lit un fichier .verse.", json!({ "path": { "type": "string" } }), &["path"]),
        tool("write_verse_file", "Écrit un fichier .verse dans le projet (crée ou remplace).", json!({ "path": { "type": "string" }, "content": { "type": "string" }, "task": task }), &["path", "content"]),
        tool("create_widget", "Crée une interface de jeu (HUD ou menu) : un appareil Verse prêt à poser dans le niveau, écrit dans Content/POP_UI/pop_ui_<name>.verse. kind : texte (affiché à tous), compteur (libellé + nombre par joueur, +1 à chaque activation d'un déclencheur ; Ajouter/Definir depuis le Verse), bouton (s'ouvre par un déclencheur, un clic active un appareil Cible). position : haut, centre ou bas. Ensuite : compile_verse, puis place_device ou poser l'appareil à la main. Annulable tant qu'aucun acteur ne l'utilise.", json!({ "kind": { "type": "string", "enum": ["texte", "compteur", "bouton"] }, "name": { "type": "string", "description": "minuscules, chiffres, _ (ex. score)" }, "text": { "type": "string" }, "position": { "type": "string", "enum": ["haut", "centre", "bas"] }, "task": task }), &["kind", "name", "text"]),
        tool("compile_verse", "Compile le Verse dans UEFN et renvoie erreurs et avertissements.", json!({}), &[]),
        tool("start_session", "Démarre la session de test UEFN (peut prendre plusieurs minutes). Ensuite : push_to_session après une modification, session_output pour lire les Print et les erreurs.", json!({}), &[]),
        tool("stop_session", "Arrête la session de test UEFN.", json!({}), &[]),
        tool("session_output", "Lit la sortie Verse de la session de test : Print et erreurs, avec horodatage, fichier, ligne et runId de la partie. pattern : texte cherché (facultatif) ; max_results (100 par défaut) ; cursor : reprendre après la dernière lecture (renvoyé par l'appel précédent).", json!({ "pattern": { "type": "string" }, "max_results": { "type": "integer" }, "cursor": {} }), &[]),
        tool("push_to_session", "Pousse les changements dans la session de test.", json!({}), &[]),
        tool("save_level", "Enregistre le niveau dans UEFN.", json!({}), &[]),
        tool("lore_checkin", "Archive dans Lore avec un message (« #n ce qui a été fait »).", json!({ "message": { "type": "string" } }), &["message"]),
        tool("list_tasks", "Liste les tâches de la map (numéro, titre, statut, étiquette, personne, jour prévu, dépendances en attente). Filtres facultatifs : status (todo, doing, blocked, done), mine (vos tâches seulement), query (texte).", json!({ "status": { "type": "string" }, "mine": { "type": "boolean" }, "query": { "type": "string" } }), &[]),
        tool("get_task", "Détail d'une tâche par son numéro : description, dépendances, sous-tâches, commentaires.", json!({ "number": { "type": "integer" } }), &["number"]),
        tool("next_task", "Ce que Boussole conseille de faire maintenant : retards, tâches du jour et tâches qui en débloquent d'autres.", json!({}), &[]),
        tool("create_task", "Crée une tâche d'environ une heure. tag : code, carte, conception ou test. depends_on : numéros de tâches. assign_to_me : vous l'assigner. uefn : tâche qui touche le projet UEFN (elle ne se ferme qu'avec un check-in Lore « #n … »).", json!({ "title": { "type": "string" }, "description": { "type": "string" }, "tag": { "type": "string" }, "planned_day": { "type": "string", "description": "AAAA-MM-JJ" }, "depends_on": { "type": "array", "items": { "type": "integer" } }, "assign_to_me": { "type": "boolean" }, "uefn": { "type": "boolean" } }), &["title"]),
        tool("set_task_status", "Change le statut d'une tâche : todo (à faire), doing (en cours), blocked (bloquée), done (faite). Passer en cours exige que ses dépendances soient faites. note : commentaire facultatif (ce qui a été fait, ce qui bloque). Une tâche UEFN ne passe à done qu'avec son check-in Lore : checkin = message qui commence par #n, inutile si vous avez archivé par lore_checkin « #n … » depuis que vous l'avez réservée.", json!({ "number": { "type": "integer" }, "status": { "type": "string", "enum": ["todo", "doing", "blocked", "done"] }, "note": { "type": "string" }, "checkin": { "type": "string" } }), &["number", "status"]),
        tool("claim_task", "Réserve une tâche POP pour vous (agent IA) : une seule tâche à la fois, et une tâche n'a qu'un agent. À faire avant de travailler dessus ; refusé si un autre agent l'a déjà. Vos fichiers Verse de cette tâche ne peuvent pas être modifiés par un autre agent.", json!({ "number": { "type": "integer" } }), &["number"]),
        tool("release_task", "Libère la tâche que vous avez réservée (claim_task), quand vous arrêtez sans la finir.", json!({}), &[]),
        tool("comment_task", "Ajoute un commentaire à une tâche.", json!({ "number": { "type": "integer" }, "text": { "type": "string" } }), &["number", "text"]),
        tool("search_library", "Cherche dans la bibliothèque Verse du studio (modules prêts : économie, équipes, minuteur, HUD, sauvegarde…). À faire AVANT d'écrire du Verse neuf : réutilisez un module existant quand il convient. Filtres facultatifs : query (texte du nom ou de la description), category (economie, equipes, minuteur, hud, sauvegarde, interface, jeu, outils, autre).", json!({ "query": { "type": "string" }, "category": { "type": "string" } }), &[]),
        tool("get_library_module", "Lit un module de la bibliothèque Verse du studio : code complet, appareils requis, version.", json!({ "id": { "type": "integer" } }), &["id"]),
        tool("list_skills", "Liste les compétences : méthodes de POP (Verse, appareils, session de test, Lore, rangement) et règles écrites par le studio. Lisez celles qui concernent votre tâche avant de commencer.", json!({}), &[]),
        tool("get_skill", "Lit une compétence par son nom (voir list_skills).", json!({ "name": { "type": "string" } }), &["name"]),
        tool("undo_action", "Annule une action du journal par son identifiant.", json!({ "id": { "type": "integer" } }), &["id"]),
    ])
}

const READ_TOOLS: &[&str] = &[
    "uefn_status",
    "check_level",
    "session_output",
    "list_actors",
    "get_actor",
    "list_verse_files",
    "read_verse_file",
    "list_tasks",
    "get_task",
    "next_task",
    "search_library",
    "get_library_module",
    "list_skills",
    "get_skill",
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
                "instructions": "Outils POP : gérer les tâches de la map (list_tasks, next_task, create_task, set_task_status, comment_task) et piloter UEFN via le relais du créateur. Avant de travailler, lisez les compétences utiles (list_skills, get_skill : la méthode de POP et les règles du studio), prenez une tâche avec next_task et passez-la en cours ; quand elle est finie, passez-la en done avec une note. Toute action UEFN est journalisée et annulable ; les suppressions attendent une validation humaine."
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
        "list_skills" => Ok(json!(crate::skills::liste(st, ctx.studio_id).await?)),
        "get_skill" => {
            crate::skills::lire(st, ctx.studio_id, a["name"].as_str().unwrap_or("")).await
        }
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
                uefn: a["uefn"].as_bool(),
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
            crate::tasks::set_status_checked(
                st,
                mid,
                tid,
                user.id,
                status,
                "IA",
                a["checkin"].as_str(),
                agent,
            )
            .await?;
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
        "place_device" => {
            exec(
                st,
                o(),
                "actor_spawn",
                json!({ "asset": a["asset"], "like": a["like"], "name": a["name"], "folder": a["folder"], "location": a["location"], "rotation": a["rotation"] }),
            )
            .await
        }
        "import_asset" => {
            exec(
                st,
                o(),
                "asset_import",
                json!({ "local_path": a["local_path"], "url": a["url"], "filename": a["filename"], "folder": a["folder"] }),
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
        "create_widget" => {
            let w = crate::widgets::lire(&a).map_err(bad)?;
            let path = w.chemin();
            let deja: Option<(i64,)> =
                sqlx::query_as("SELECT 1 FROM verse_files WHERE map_id = ? AND path = ?")
                    .bind(mid)
                    .bind(&path)
                    .fetch_optional(&st.db)
                    .await?;
            if deja.is_some() {
                return Err(bad(format!("{path} existe déjà : choisissez un autre name, ou modifiez le fichier avec write_verse_file.")));
            }
            let r = exec(
                st,
                o(),
                "write_file",
                json!({ "path": path, "content": crate::widgets::verse(&w) }),
            )
            .await?;
            Ok(
                json!({ "action": r, "path": path, "device": w.classe(), "next": "compile_verse, puis poser l'appareil dans le niveau et relier ses déclencheurs." }),
            )
        }
        "compile_verse" => {
            let mut r = exec(st, o(), "compile", json!({})).await?;
            aide_compilation(st, mid, &mut r).await;
            Ok(r)
        }
        "start_session" => exec(st, o(), "start_session", json!({})).await,
        "stop_session" => exec(st, o(), "stop_session", json!({})).await,
        "session_output" => {
            exec(
                st,
                o(),
                "session_output",
                json!({ "pattern": a["pattern"], "max_results": a["max_results"], "cursor": a["cursor"] }),
            )
            .await
        }
        "push_to_session" => exec(st, o(), "push_session", json!({})).await,
        "save_level" => exec(st, o(), "save_level", json!({})).await,
        "lore_checkin" => exec(st, o(), "lore_checkin", json!({ "message": a["message"] })).await,
        "undo_action" => {
            let id = a["id"].as_i64().ok_or_else(|| bad("id manquant"))?;
            // Annuler une pose, une duplication ou un import supprime des acteurs ou des assets : demandé par une IA, cela attend un humain.
            let u: Option<(i64, Option<String>, String)> = sqlx::query_as(
                "SELECT map_id, undo, summary FROM uefn_actions WHERE id = ? AND status = 'done'",
            )
            .bind(id)
            .fetch_optional(&st.db)
            .await?;
            if let Some((m, Some(undo), resume)) = u.filter(|u| u.0 == mid) {
                let undo: Value = serde_json::from_str(&undo).unwrap_or_default();
                let quoi = match undo["action"].as_str() {
                    Some("actor_delete") => Some("suppression des acteurs créés"),
                    Some("asset_delete") => Some("retrait des assets importés"),
                    _ => None,
                };
                if let Some(quoi) = quoi {
                    let summary = format!("L’IA de {} demande d’annuler « {resume} » ({quoi})", user.name);
                    let aid: i64 = sqlx::query_scalar("INSERT INTO approvals (map_id, user_id, action, args, summary) VALUES (?, ?, ?, ?, ?) RETURNING id")
                        .bind(m).bind(user.id).bind(undo["action"].as_str()).bind(undo["args"].to_string()).bind(&summary).fetch_one(&st.db).await?;
                    notify(st, mid, json!({ "t": "uefn", "what": "actors" }));
                    return Ok(json!({ "pending_approval": aid, "message": "Annulation en attente : un membre doit la valider dans l’Organiseur de POP." }));
                }
            }
            crate::uefn::undo(State(st.clone()), user.clone(), Path(id))
                .await
                .map(|j| j.0)
        }
        _ => Err(bad("Outil inconnu.")),
    }
}

/// Ajoute aux erreurs de compilation la fiche d'aide du moteur local (corrections par règles), quand elle existe.
async fn aide_compilation(st: &AppState, mid: i64, r: &mut Value) {
    let Some(diags) = r["diagnostics"].as_array().cloned() else {
        return;
    };
    let mut fichiers: Vec<String> = diags
        .iter()
        .filter_map(|d| d["path"].as_str().map(String::from))
        .collect();
    fichiers.sort();
    fichiers.dedup();
    let projet = crate::engine::Project {
        symbols: vec![],
        tags: vec![],
        devices: vec![],
    };
    let mut aide = serde_json::Map::new();
    for f in fichiers.into_iter().take(20) {
        let src: Option<String> =
            sqlx::query_scalar("SELECT content FROM verse_files WHERE map_id = ? AND path = ?")
                .bind(mid)
                .bind(&f)
                .fetch_optional(&st.db)
                .await
                .ok()
                .flatten();
        let Some(src) = src else { continue };
        let d: Vec<Value> = diags
            .iter()
            .filter(|d| d["path"] == f.as_str())
            .cloned()
            .collect();
        let (_, notes) = crate::engine::fix(&src, &d, &projet);
        if !notes.is_empty() {
            aide.insert(f, json!(notes));
        }
    }
    if !aide.is_empty() {
        r["aide"] = Value::Object(aide);
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

#[cfg(test)]
mod tests_pose {
    use super::*;

    /// Poser un appareil (#81) : contrôles, journal, et annulation par une IA soumise à un humain.
    #[tokio::test]
    async fn poser_et_annuler() {
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
        let uid: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('ia@a.fr', 'Yanis', 'x') RETURNING id")
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
            email: "ia@a.fr".into(),
            name: "Yanis".into(),
            sid: String::new(),
        };
        // Sans asset ni modèle, ou avec une position invalide : refus.
        assert!(call(&st, &u, mid, "place_device", json!({})).await.is_err());
        assert!(call(
            &st,
            &u,
            mid,
            "place_device",
            json!({ "asset": "/Game/Bouton", "location": [1, 2] })
        )
        .await
        .is_err());
        assert!(
            call(&st, &u, mid, "place_device", json!({ "like": "L.Absent" }))
                .await
                .is_err()
        );
        // Pose valide : journalisée (en attente tant que le relais est absent).
        call(&st, &u, mid, "place_device", json!({ "asset": "/Game/Devices/Bouton", "name": "Bouton départ", "location": [0, 100, 0] }))
            .await
            .unwrap();
        let (id, resume): (i64, String) =
            sqlx::query_as("SELECT id, summary FROM uefn_actions WHERE action = 'actor_spawn'")
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(resume, "Pose : Bouton départ (Bouton)");
        // Pose réussie (comme après la réponse du relais) : l'annulation par l'IA attend un humain.
        sqlx::query("UPDATE uefn_actions SET status = 'done', undo = ? WHERE id = ?")
            .bind(json!({ "action": "actor_delete", "args": { "ids": ["L.Bouton"] } }).to_string())
            .bind(id)
            .execute(&st.db)
            .await
            .unwrap();
        let r = call(&st, &u, mid, "undo_action", json!({ "id": id }))
            .await
            .unwrap();
        assert!(r["pending_approval"].is_i64(), "{r}");
        let (action, args): (String, String) =
            sqlx::query_as("SELECT action, args FROM approvals WHERE map_id = ?")
                .bind(mid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(action, "actor_delete");
        assert!(args.contains("L.Bouton"));
        let statut: String = sqlx::query_scalar("SELECT status FROM uefn_actions WHERE id = ?")
            .bind(id)
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(statut, "done", "rien n'est supprimé avant la validation");
    }
}

#[cfg(test)]
mod tests_session {
    use super::*;

    async fn monde() -> (AppState, AuthUser, i64) {
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
        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO users (email, name, password_hash) VALUES ('ia@a.fr', 'IA', 'x') RETURNING id",
        )
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
            email: "ia@a.fr".into(),
            name: "IA".into(),
            sid: String::new(),
        };
        (st, u, mid)
    }

    /// Sans UEFN : refus clair, rien n'est mis en attente ni envoyé (#82).
    #[tokio::test]
    async fn session_sans_uefn() {
        let (st, u, mid) = monde().await;
        for outil in ["start_session", "stop_session"] {
            match call(&st, &u, mid, outil, json!({})).await {
                Err(AppError::Conflict(m)) => assert!(m.contains("UEFN n’est pas connecté"), "{m}"),
                _ => panic!("{outil} devait être refusé sans UEFN"),
            }
        }
        assert!(call(&st, &u, mid, "session_output", json!({}))
            .await
            .is_err());
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM uefn_actions")
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(n, 0, "rien ne doit être mis en attente");
        assert!(READ_TOOLS.contains(&"session_output"));
        let noms: Vec<String> = tools()
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        for t in ["start_session", "stop_session", "session_output"] {
            assert!(noms.iter().any(|n| n == t), "outil MCP {t} manquant");
        }
    }

    /// Une erreur de compilation reçoit la fiche d'aide du moteur local quand une règle s'applique.
    #[tokio::test]
    async fn aide_sur_erreur() {
        let (st, _, mid) = monde().await;
        sqlx::query(
            "INSERT INTO verse_files (map_id, path, content, sha) VALUES (?, 'Content/jeu.verse', ?, 'x')",
        )
        .bind(mid)
        .bind("jeu := class(creative_device):\n    OnBegin<override>()<suspends>:void =\n        Print(\"Bonjour\"\n")
        .execute(&st.db)
        .await
        .unwrap();
        let mut r = json!({ "diagnostics": [{ "path": "Content/jeu.verse", "line": 3, "code": "3100", "message": "expected ')'" }] });
        aide_compilation(&st, mid, &mut r).await;
        assert!(
            r["aide"]["Content/jeu.verse"]
                .as_array()
                .map(|a| !a.is_empty())
                .unwrap_or(false),
            "{r}"
        );
    }
}

#[cfg(test)]
mod tests_import {
    use super::*;

    /// Importer un fichier (#90) : contrôles, journal, et annulation par une IA soumise à un humain.
    #[tokio::test]
    async fn importer_et_annuler() {
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
        let uid: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('ia@a.fr', 'Yanis', 'x') RETURNING id")
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
            email: "ia@a.fr".into(),
            name: "Yanis".into(),
            sid: String::new(),
        };
        for mauvais in [
            json!({}),
            json!({ "local_path": "C:/x/outil.exe" }),
            json!({ "local_path": "C:/x/arbre.fbx", "folder": "../../Windows" }),
        ] {
            assert!(
                call(&st, &u, mid, "import_asset", mauvais.clone())
                    .await
                    .is_err(),
                "{mauvais}"
            );
        }
        call(
            &st,
            &u,
            mid,
            "import_asset",
            json!({ "local_path": "C:\\Assets\\arbre.fbx", "folder": "/Projet/Decor" }),
        )
        .await
        .unwrap();
        let (id, resume): (i64, String) =
            sqlx::query_as("SELECT id, summary FROM uefn_actions WHERE action = 'asset_import'")
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(resume, "Import : arbre.fbx → /Projet/Decor");
        sqlx::query("UPDATE uefn_actions SET status = 'done', undo = ? WHERE id = ?")
            .bind(
                json!({ "action": "asset_delete", "args": { "paths": ["/Projet/Decor/arbre"] } })
                    .to_string(),
            )
            .bind(id)
            .execute(&st.db)
            .await
            .unwrap();
        let r = call(&st, &u, mid, "undo_action", json!({ "id": id }))
            .await
            .unwrap();
        assert!(r["pending_approval"].is_i64(), "{r}");
        let action: String = sqlx::query_scalar("SELECT action FROM approvals WHERE map_id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(action, "asset_delete");
    }
}

#[cfg(test)]
mod tests_checkin {
    use super::*;

    /// Critère 4 (#127) : après `lore_checkin "#N …"` par l'agent qui a réservé la tâche, `set_task_status done`
    /// passe sans redemander le message. Un autre agent, ou un check-in d'une autre tâche, ne suffit pas.
    #[tokio::test]
    async fn checkin_de_l_agent_repris() {
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
        let u = AuthUser {
            id: uid,
            email: "ia@a.fr".into(),
            name: "IA".into(),
            sid: String::new(),
        };
        let jeton = |nom: &'static str| {
            let st = st.clone();
            async move {
                sqlx::query_scalar::<_, i64>("INSERT INTO api_tokens (user_id, map_id, name, token_hash, kind) VALUES (?, ?, ?, ?, 'claude_code') RETURNING id")
                    .bind(uid).bind(mid).bind(nom).bind(format!("h-{nom}")).fetch_one(&st.db).await.unwrap()
            }
        };
        let (a, b) = (jeton("Claude").await, jeton("Codex").await);
        // create_task avec uefn : la tâche est marquée.
        let r = call_as(
            &st,
            &u,
            mid,
            Some(a),
            "create_task",
            json!({ "title": "Poser les spawns", "uefn": true }),
        )
        .await
        .unwrap();
        let n = r["number"].as_i64().unwrap();
        call_as(&st, &u, mid, Some(a), "claim_task", json!({ "number": n }))
            .await
            .unwrap();
        let fini = |agent: i64| {
            let (st, u) = (st.clone(), u.clone());
            async move {
                call_as(
                    &st,
                    &u,
                    mid,
                    Some(agent),
                    "set_task_status",
                    json!({ "number": n, "status": "done" }),
                )
                .await
            }
        };
        // Sans check-in : refus clair.
        match fini(a).await {
            Err(AppError::Conflict(m)) => assert!(m.contains("check-in Lore"), "{m}"),
            _ => panic!("la tâche UEFN devait être refusée sans check-in"),
        }
        // Check-ins archivés (comme le fait `exec` après lore_checkin) : un d'une autre tâche, un d'un autre agent.
        let archive = |agent: i64, msg: String| {
            let st = st.clone();
            async move {
                sqlx::query("INSERT INTO uefn_actions (map_id, user_id, source, action, args, summary, status, agent_id) VALUES (?, ?, 'mcp', 'lore_checkin', ?, 'Archivage Lore', 'done', ?)")
                    .bind(mid).bind(uid).bind(json!({ "message": msg }).to_string()).bind(agent)
                    .execute(&st.db).await.unwrap();
            }
        };
        archive(a, format!("#{}0 autre tâche", n)).await;
        archive(b, format!("#{n} fait par un autre agent")).await;
        assert!(
            fini(a).await.is_err(),
            "ni #N0 ni le check-in d'un autre agent ne comptent"
        );
        archive(a, format!("#{n} spawns posés")).await;
        fini(a).await.unwrap();
        let what: Vec<String> = sqlx::query_scalar("SELECT e.what FROM task_events e JOIN tasks t ON t.id = e.task_id WHERE t.map_id = ? AND t.number = ?")
            .bind(mid)
            .bind(n)
            .fetch_all(&st.db)
            .await
            .unwrap();
        assert!(
            what.contains(&format!("Check-in Lore : #{n} spawns posés")),
            "{what:?}"
        );
    }
}
