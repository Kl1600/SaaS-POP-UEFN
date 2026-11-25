//! Serveur MCP de POP : expose à l'IA connectée du créateur les mêmes actions que l'Organiseur et l'éditeur Verse.
//! Tout est journalisé ; une suppression attend la validation d'un humain.
//! Un jeton est lié au compte (#145) : l'IA choisit sa map de travail par `list_maps` puis `use_map`.
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
    /// Rôle de l'agent (#111), facultatif.
    role: Option<String>,
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

/// Crée un jeton MCP du compte (#145) : il n'est lié à aucune map, l'IA choisit sa map de travail par `use_map`.
pub async fn create_token(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<TokenIn>,
) -> ApiResult<Value> {
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
    let role = role_valide(b.role.as_deref())?;
    let id: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, name, token_hash, kind, scope, role) VALUES (?, ?, ?, ?, ?, ?) RETURNING id").bind(u.id).bind(b.name.trim()).bind(sha256_hex(&token)).bind(&kind).bind(scope).bind(role).fetch_one(&st.db).await?;
    Ok(Json(json!({ "token": token, "id": id })))
}

/// Jetons du compte, avec leur map de travail (seulement si le compte y a encore accès).
pub async fn tokens(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    let rows: Vec<(i64, String, String, String, String, Option<String>, i64, Option<i64>, Option<String>, bool, Option<String>)> = sqlx::query_as(
        "SELECT t.id, t.name, t.created_at, t.kind, t.scope, t.last_used, t.calls, m.id, m.name, t.oauth_client IS NOT NULL, t.role FROM api_tokens t
         LEFT JOIN maps m ON m.id = t.map_id AND EXISTS (SELECT 1 FROM memberships ms WHERE ms.studio_id = m.studio_id AND ms.user_id = t.user_id)
         WHERE t.user_id = ? ORDER BY t.id DESC",
    ).bind(u.id).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|(id, n, at, kind, scope, last, calls, mid, mname, oauth, role)| json!({"id": id, "name": n, "created_at": at, "kind": kind, "scope": scope, "role": role, "last_used": last, "calls": calls, "oauth": oauth, "map": mid.map(|i| json!({ "id": i, "name": mname }))})).collect::<Vec<_>>())))
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

fn role_valide(r: Option<&str>) -> Result<Option<&'static str>, AppError> {
    match r.map(str::trim).filter(|r| !r.is_empty()) {
        None => Ok(None),
        Some(r) => crate::roles::get(r)
            .map(|x| Some(x.id))
            .ok_or_else(|| bad("Rôle inconnu.")),
    }
}

#[derive(Deserialize)]
pub struct RoleIn {
    role: Option<String>,
}

/// Change le rôle d'un agent du compte (#111) ; null le retire. Pris en compte au prochain appel de l'IA.
pub async fn set_role(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<RoleIn>,
) -> ApiResult<Value> {
    let role = role_valide(b.role.as_deref())?;
    let r = sqlx::query("UPDATE api_tokens SET role = ? WHERE id = ? AND user_id = ?")
        .bind(role)
        .bind(id)
        .bind(u.id)
        .execute(&st.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

/// Les rôles proposés par POP, pour l'interface.
pub async fn roles() -> Json<Value> {
    Json(json!(crate::roles::ROLES
        .iter()
        .map(|r| json!({ "id": r.id, "name": r.nom, "instructions": r.consignes }))
        .collect::<Vec<_>>()))
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
        tool("list_maps", "Liste les maps auxquelles votre compte a accès (espace perso et studios), avec votre rôle ; current : la map de travail actuelle de cette IA.", json!({}), &[]),
        tool("create_map", "Crée une nouvelle map (comme « Nouvelle map » dans POP) et en fait la map de travail de cette IA : les autres outils agissent ensuite sur elle, sans reconnexion. Sans studio, la map est créée dans votre espace perso. Il faut être admin du studio.", json!({
            "name": { "type": "string", "description": "Nom de la map (60 caractères au plus)" },
            "genre": { "type": "string", "description": "Genre (facultatif), par exemple « Tycoon » ou « Box fight »" },
            "release_target": { "type": "string", "description": "Sortie visée (facultatif), AAAA-MM-JJ" },
            "studio": { "description": "Studio (facultatif) : identifiant ou nom, tel que list_maps le montre ; sans lui, l'espace perso" }
        }), &["name"]),
        tool("use_map", "Choisit la map de travail de cette IA (identifiant donné par list_maps). Tous les autres outils agissent ensuite sur cette map ; le choix est gardé par POP, même après un redémarrage de l'IA.", json!({ "map": { "type": "integer", "description": "Identifiant de la map (list_maps)" } }), &["map"]),
        tool("uefn_status", "État du relais et d'UEFN (connecté, fermé, hors ligne) et nombre d'actions en attente.", json!({}), &[]),
        tool("list_actors", "Liste les acteurs du niveau. Filtre facultatif : texte cherché dans le nom, le type, le dossier, les tags et les options.", json!({ "query": { "type": "string" } }), &[]),
        tool("get_actor", "Détails d'un acteur : transformation, tags, options, propriétés Verse, liens d'événements.", json!({ "id": { "type": "string" } }), &["id"]),
        tool("update_actors", "Modifie un ou plusieurs acteurs. changes peut contenir name, folder, hidden, transform {location,rotation,scale}, options {clé: valeur}, props {clé: valeur}, tags_add, tags_remove, events.", json!({ "ids": ids, "changes": { "type": "object" }, "task": task }), &["ids", "changes"]),
        tool("place_device", "Pose un appareil dans le niveau. Soit asset (chemin de l'appareil dans le Content Browser d'UEFN, ex. /Game/…), soit like (identifiant d'un appareil du niveau à copier avec ses réglages). Facultatifs : name, folder, location [x, y, z] et rotation [pitch, yaw, roll] en coordonnées du moteur. Journalisé et annulable (l'annulation demandée par une IA attend un humain).", json!({ "asset": { "type": "string" }, "like": { "type": "string" }, "name": { "type": "string" }, "folder": { "type": "string" }, "location": { "type": "array", "items": { "type": "number" } }, "rotation": { "type": "array", "items": { "type": "number" } }, "task": task }), &[]),
        tool("place_mesh", "Pose un mesh dans le niveau (décor : sol, mur, bloc, mesh importé). Soit shape : forme de base d'1 m de côté (plan, cube, sphere, cylindre, cone ; un plan de 20 m × 20 m = shape plan, scale [20, 20, 1]), soit mesh : chemin d'un mesh du Content Browser (ex. un mesh importé par import_asset). Facultatifs : name, folder, location [x, y, z] en cm, rotation [pitch, yaw, roll] en degrés, scale [x, y, z]. Journalisé et annulable. Un mesh importé est aussi utilisable en Verse (Scene Graph) : voir KB « mesh importé en Verse ».", json!({ "shape": { "type": "string", "enum": ["plan", "cube", "sphere", "cylindre", "cone"] }, "mesh": { "type": "string" }, "name": { "type": "string" }, "folder": { "type": "string" }, "location": { "type": "array", "items": { "type": "number" } }, "rotation": { "type": "array", "items": { "type": "number" } }, "scale": { "type": "array", "items": { "type": "number" } }, "task": task }), &[]),
        tool("get_island_settings", "Lit les réglages de l'île (appareil Réglages de l'île) : nom, libellé, valeur actuelle et valeurs permises. names : noms à lire (ex. [\"MaxPlayers\"]) ; sans names, la liste complète sans les valeurs permises.", json!({ "names": { "type": "array", "items": { "type": "string" } } }), &[]),
        tool("set_island_settings", "Change des réglages de l'île, ex. settings {\"MaxPlayers\": \"16\"}. Chaque nom et chaque valeur sont vérifiés avant l'envoi (get_island_settings donne les valeurs permises). Journalisé et annulable (l'annulation remet les anciennes valeurs).", json!({ "settings": { "type": "object" }, "task": task }), &["settings"]),
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
        tool("lore_checkin", "Archive dans Lore avec un message (« #n ce qui a été fait », ou « #3 #5 … » si le check-in couvre plusieurs tâches). Chaque tâche UEFN en cours citée au début passe alors à done.", json!({ "message": { "type": "string" } }), &["message"]),
        tool("list_tasks", "Liste les tâches de la map (numéro, titre, statut, étiquette, personne, jour prévu, dépendances en attente). Filtres facultatifs : status (todo, doing, blocked, done), mine (vos tâches seulement), query (texte).", json!({ "status": { "type": "string" }, "mine": { "type": "boolean" }, "query": { "type": "string" } }), &[]),
        tool("get_task", "Détail d'une tâche par son numéro : description, dépendances, sous-tâches, commentaires.", json!({ "number": { "type": "integer" } }), &["number"]),
        tool("next_task", "Ce que Popy conseille de faire maintenant : retards, tâches du jour et tâches qui en débloquent d'autres.", json!({}), &[]),
        tool("create_task", "Crée une tâche d'environ une heure. tag : code, carte, conception ou test. depends_on : numéros des tâches sans lesquelles celle-ci ne peut être ni faite ni testée ; pas l'ordre dans lequel le joueur découvre les choses ; si seule une partie a besoin d'une autre tâche, découpez. lot : fonctionnalité de la tâche (ex. « Herbe », « Boutique ») : les tâches d'une même fonctionnalité sont groupées dans la vue Dépendances. assign_to_me : vous l'assigner. uefn : tâche qui touche le projet UEFN (elle ne se ferme qu'avec un check-in Lore « #n … »).", json!({ "title": { "type": "string" }, "description": { "type": "string" }, "tag": { "type": "string" }, "planned_day": { "type": "string", "description": "AAAA-MM-JJ" }, "depends_on": { "type": "array", "items": { "type": "integer" }, "description": "Tâches sans lesquelles celle-ci ne peut être ni faite ni testée (pas l'ordre du jeu)" }, "lot": { "type": "string" }, "assign_to_me": { "type": "boolean" }, "uefn": { "type": "boolean" } }), &["title"]),
        tool("set_task_lot", "Range une tâche dans une fonctionnalité (lot), par exemple « Herbe » ou « Boutique ». lot vide : « Sans fonctionnalité ».", json!({ "number": { "type": "integer" }, "lot": { "type": "string" } }), &["number", "lot"]),
        tool("set_task_status", "Change le statut d'une tâche : todo (à faire), doing (en cours), blocked (bloquée), done (faite). Passer en cours exige que ses dépendances soient faites. note : commentaire facultatif (ce qui a été fait, ce qui bloque). Une tâche UEFN ne passe à done qu'avec un check-in Lore réellement archivé sur la map : checkin = son message exact (« #n … »), ou celui du check-in d'une autre tâche où son travail est parti (noté dans l'historique). Inutile si vous avez archivé par lore_checkin « #n … » depuis que vous l'avez réservée : un check-in « #3 #5 … » ferme d'ailleurs seul chaque tâche UEFN en cours qu'il cite.", json!({ "number": { "type": "integer" }, "status": { "type": "string", "enum": ["todo", "doing", "blocked", "done"] }, "note": { "type": "string" }, "checkin": { "type": "string" } }), &["number", "status"]),
        tool("claim_task", "Réserve une tâche POP pour vous (agent IA) : une seule tâche à la fois, et une tâche n'a qu'un agent. À faire avant de travailler dessus ; refusé si un autre agent l'a déjà. Vos fichiers Verse de cette tâche ne peuvent pas être modifiés par un autre agent.", json!({ "number": { "type": "integer" } }), &["number"]),
        tool("save_progress", "Note où vous en êtes sur votre tâche réservée, pour qu'un autre agent puisse la reprendre : done (ce qui est fait), todo (ce qui reste), files (fichiers touchés). À faire avant de vous arrêter et à chaque étape importante. La note est rendue par claim_task et next_task.", json!({ "done": { "type": "string" }, "todo": { "type": "string" }, "files": { "type": "array", "items": { "type": "string" } } }), &[]),
        tool("list_boards", "Tableaux blancs de la map (identifiant, titre).", json!({}), &[]),
        tool("get_board", "Objets d'un tableau blanc : post-it (sticky), cadres (frame), textes, formes, connecteurs, images… avec id, kind, x, y, w, h, text.", json!({ "board": { "type": "integer" } }), &["board"]),
        tool("create_board", "Crée un tableau blanc. Facultatifs : title, template (modèle de l'interface : brainstorm, etc.).", json!({ "title": { "type": "string" }, "template": { "type": "string" } }), &[]),
        tool("upsert_board_objects", "Crée ou modifie des objets d'un tableau blanc (200 au plus), visibles en direct par les membres qui l'ont ouvert. Chaque objet : kind (sticky, text, shape, frame, connector, image, comment…), x, y, w, h en pixels du tableau, text (2 000 caractères au plus) ; id facultatif pour un nouvel objet, obligatoire pour modifier. sticky : shade 0 à 5 pour la couleur. Les objets invalides sont ignorés et comptés.", json!({ "board": { "type": "integer" }, "objects": { "type": "array", "items": { "type": "object" } } }), &["board", "objects"]),
        tool("delete_board_objects", "Retire des objets d'un tableau blanc par leurs identifiants.", json!({ "board": { "type": "integer" }, "ids": ids }), &["board", "ids"]),
        tool("list_pages", "Pages de la documentation de la map (identifiant, titre, page parente).", json!({}), &[]),
        tool("read_page", "Lit une page de la documentation, en Markdown.", json!({ "page": { "type": "integer" } }), &["page"]),
        tool("write_page", "Écrit une page de la documentation en Markdown (titres #, listes -, cases - [ ], citations >, code ```, tableaux |, ---). Sans page : crée une page (title, parent facultatif). Avec page : remplace son contenu ; l'ancienne version reste dans l'historique de la page.", json!({ "page": { "type": "integer" }, "title": { "type": "string" }, "markdown": { "type": "string" }, "parent": { "type": "integer" } }), &["markdown"]),
        tool("ask_user", "Pose une question au créateur dans POP, avec 2 à 5 réponses en boutons (plus « Autre » en texte libre, sauf allow_other: false). À utiliser quand une décision lui revient. Rend question_id : lisez la réponse plus tard avec get_answer.", json!({
            "question": { "type": "string" },
            "choices": { "type": "array", "items": { "type": "string" }, "description": "2 à 5 réponses courtes" },
            "allow_other": { "type": "boolean", "description": "Autoriser une réponse libre (par défaut : oui)" }
        }), &["question", "choices"]),
        tool("get_answer", "Réponse du créateur à une question posée avec ask_user (null tant qu'il n'a pas répondu).", json!({ "question_id": { "type": "integer" } }), &["question_id"]),
        tool("get_feasibility", "Avis de faisabilité de la map : faisable, tendu ou pas faisable, heures disponibles jusqu'à la sortie, heures nécessaires (1 h par tâche ouverte), marge, et corrections chiffrées (date, heures, tâches à confier à l'IA ou à retirer).", json!({}), &[]),
        tool("list_scenarios", "Scénarios de test de la map (#348) : étapes (action, résultat attendu), statut (draft = proposé par une IA, à valider ; ready = jouable) et dernier passage (version, réussies, échouées, régressions).", json!({}), &[]),
        tool("propose_scenario", "Propose un scénario de test à partir des tâches (#348) : title, steps [{ action, expected }] (1 à 50, une action claire par étape, ex. « Va au spawn et achète l'arme »). Il reste brouillon tant qu'un humain ne l'a pas validé ; ce sont des humains qui le jouent.", json!({ "title": { "type": "string" }, "steps": { "type": "array", "items": { "type": "object", "properties": { "action": { "type": "string" }, "expected": { "type": "string" } } } } }), &["title", "steps"]),
        tool("list_device_configs", "Configurations d'appareils réutilisables du studio (#89) : nom, type (class), options et réglages @editable. Cherchez-y avant de régler un appareil à la main.", json!({ "query": { "type": "string" } }), &[]),
        tool("save_device_config", "Enregistre la configuration d'un appareil du niveau (son type et ses réglages, lus dans UEFN) dans la bibliothèque du studio, sous un nom.", json!({ "actor_id": { "type": "string" }, "name": { "type": "string" }, "description": { "type": "string" }, "asset": { "type": "string", "description": "Chemin de l'asset de l'appareil (facultatif)" } }), &["actor_id", "name"]),
        tool("apply_device_config", "Applique une configuration enregistrée à des appareils du même type (une action annulable).", json!({ "config_id": { "type": "integer" }, "ids": { "type": "array", "items": { "type": "string" } } }), &["config_id", "ids"]),
        tool("place_configured_device", "Pose un appareil déjà configuré (depuis son asset, ou copie d'un appareil du même type du niveau), puis applique ses réglages. Annulable.", json!({ "config_id": { "type": "integer" }, "name": { "type": "string" }, "folder": { "type": "string" }, "location": { "type": "array", "items": { "type": "number" } }, "rotation": { "type": "array", "items": { "type": "number" } } }), &["config_id"]),
        tool("list_milestones", "Jalons et objectifs du planning de la map (date, heure, type, détail).", json!({}), &[]),
        tool("add_milestone", "Ajoute un jalon au planning : title, date AAAA-MM-JJ ; facultatifs : time HH:MM, detail, kind (jalon par défaut).", json!({ "title": { "type": "string" }, "date": { "type": "string" }, "time": { "type": "string" }, "detail": { "type": "string" }, "kind": { "type": "string" } }), &["title", "date"]),
        tool("list_agents", "Agents IA de la map : identifiant, nom, rôle, tâche en cours, actif ou non, et lequel est l'Orchestrateur.", json!({}), &[]),
        tool("plan_tasks", "Réservé à l'Orchestrateur. Découpe une demande en tâches d'environ une heure, créées d'un coup. Chaque tâche : title, description, tag (code, carte, conception, test), lot, uefn, key (nom court pour s'y référer plus bas), depends_on (numéros de tâches existantes ou key d'une tâche plus haut dans le plan), agent (identifiant ou nom d'un agent de la map, voir list_agents : la tâche lui est attribuée). Tout est vérifié avant de créer quoi que ce soit.", json!({ "tasks": { "type": "array", "items": { "type": "object", "properties": { "key": { "type": "string" }, "title": { "type": "string" }, "description": { "type": "string" }, "tag": { "type": "string" }, "lot": { "type": "string" }, "uefn": { "type": "boolean" }, "depends_on": { "type": "array", "items": { "type": ["integer", "string"] } }, "agent": { "type": ["integer", "string"] } }, "required": ["title"] } } }), &["tasks"]),
        tool("assign_task", "Réservé à l'Orchestrateur. Attribue une tâche à un agent de la map (identifiant ou nom, voir list_agents) : lui seul pourra la réserver. agent vide ou null : la tâche redevient libre. Refusé si un autre agent travaille déjà dessus.", json!({ "number": { "type": "integer" }, "agent": { "type": ["integer", "string", "null"] } }), &["number"]),
        tool("release_task", "Libère la tâche que vous avez réservée (claim_task), quand vous arrêtez sans la finir.", json!({}), &[]),
        tool("comment_task", "Ajoute un commentaire à une tâche.", json!({ "number": { "type": "integer" }, "text": { "type": "string" } }), &["number", "text"]),
        tool("search_library", "Cherche dans la bibliothèque Verse du studio (modules prêts : économie, équipes, minuteur, HUD, sauvegarde…). À faire AVANT d'écrire du Verse neuf : réutilisez un module existant quand il convient. Filtres facultatifs : query (texte du nom ou de la description), category (economie, equipes, minuteur, hud, sauvegarde, interface, jeu, outils, autre).", json!({ "query": { "type": "string" }, "category": { "type": "string" } }), &[]),
        tool("get_library_module", "Lit un module de la bibliothèque Verse du studio : code complet, appareils requis, version.", json!({ "id": { "type": "integer" } }), &["id"]),
        tool("list_skills", "Liste les compétences : méthodes de POP (Verse, appareils, session de test, Lore, rangement) et règles écrites par le studio. Lisez celles qui concernent votre tâche avant de commencer.", json!({}), &[]),
        tool("get_skill", "Lit une compétence par son nom (voir list_skills).", json!({ "name": { "type": "string" } }), &["name"]),
        tool("undo_action", "Annule une action du journal par son identifiant.", json!({ "id": { "type": "integer" } }), &["id"]),
    ])
}

/// Noms de tous les outils du serveur MCP.
pub(crate) fn tool_names() -> Vec<String> {
    tools()
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| t["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

const READ_TOOLS: &[&str] = &[
    "list_maps",
    "use_map",
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
    "list_agents",
    "list_boards",
    "get_board",
    "list_pages",
    "read_page",
    "list_milestones",
    "get_answer",
    "get_feasibility",
    "list_scenarios",
    "list_device_configs",
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
    pub token: Option<String>,
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
    let row: Option<(i64, Option<i64>, String, String, String, i64, Option<String>)> = match sqlx::query_as(
        "SELECT t.user_id, t.map_id, u.email, u.name, t.scope, t.id, t.role FROM api_tokens t JOIN users u ON u.id = t.user_id WHERE t.token_hash = ? AND u.disabled = 0 AND (t.expires_at IS NULL OR t.expires_at > datetime('now'))",
    ).bind(sha256_hex(token)).fetch_optional(&st.db).await {
        Ok(r) => r,
        Err(_) => return rpc_err(id, -32603, "Erreur interne"),
    };
    let Some((uid, mid, email, name, scope, agent, role)) = row else {
        // L'en-tête dit à l'IA où trouver la connexion OAuth de POP (#285).
        return (
            StatusCode::UNAUTHORIZED,
            [(
                axum::http::header::WWW_AUTHENTICATE,
                crate::oauth::www_authenticate(),
            )],
            Json(json!({ "error": "Jeton POP invalide ou expiré." })),
        )
            .into_response();
    };
    let _ = sqlx::query(
        "UPDATE api_tokens SET last_used = datetime('now'), calls = calls + 1 WHERE token_hash = ?",
    )
    .bind(sha256_hex(token))
    .execute(&st.db)
    .await;
    if let Some(mid) = mid {
        crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "ai" }));
    }
    let user = AuthUser {
        id: uid,
        email,
        name,
        sid: String::new(),
    };
    // Orchestrateur de la map (#112) : rôle choisi par un humain, sinon le premier agent Claude actif.
    let orch = crate::orchestre::est(&st, agent, mid).await;
    let permis = |n: &str| {
        crate::roles::allows(role.as_deref(), n) || (orch && crate::orchestre::OUTILS.contains(&n))
    };
    let method = req["method"].as_str().unwrap_or("");
    match method {
        "initialize" => rpc_ok(
            id,
            json!({
                "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "pop", "version": "0.1.0" },
                "instructions": consignes(role.as_deref(), orch, "Commencez par list_maps puis use_map pour choisir la map de travail (gardée par POP). Outils POP : gérer les tâches de la map (list_tasks, next_task, create_task, set_task_status, comment_task) et piloter UEFN via le relais du créateur. Avant de travailler, lisez les compétences utiles (list_skills, get_skill : la méthode de POP et les règles du studio), prenez une tâche avec next_task et passez-la en cours ; notez votre progression avec save_progress à chaque étape et avant de vous arrêter (un autre agent pourra reprendre) ; quand elle est finie, passez-la en done avec une note. Toute action UEFN est journalisée et annulable ; les suppressions attendent une validation humaine.")
            }),
        ),
        m if m.starts_with("notifications/") => StatusCode::ACCEPTED.into_response(),
        "ping" => rpc_ok(id, json!({})),
        "tools/list" => rpc_ok(
            id,
            json!({ "tools": tools().as_array().cloned().unwrap_or_default().into_iter().filter(|t| { let n = t["name"].as_str().unwrap_or(""); (scope != "read" || READ_TOOLS.contains(&n)) && permis(n) }).collect::<Vec<_>>() }),
        ),
        "tools/call" => {
            let name = req["params"]["name"].as_str().unwrap_or("");
            if scope == "read" && !READ_TOOLS.contains(&name) {
                return rpc_ok(id, text("Cette IA est en lecture seule dans POP (IA connectées → autoriser les modifications).", true));
            }
            if !permis(name) {
                let msg = if crate::orchestre::OUTILS.contains(&name) {
                    crate::orchestre::refus(name)
                } else {
                    crate::roles::refus(role.as_deref().unwrap_or(""), name)
                };
                return rpc_ok(id, text(msg, true));
            }
            let args = req["params"]["arguments"].clone();
            let res = token_call(&st, &user, mid, agent, name, args.clone()).await;
            // Fil d'activité de l'agent (#298) : seulement sur une map de travail, pas pour list_maps / use_map.
            if let (Some(m), false) = (mid, matches!(name, "list_maps" | "use_map" | "create_map"))
            {
                crate::agents::noter_appel(
                    &st,
                    agent,
                    m,
                    name,
                    &args,
                    res.as_ref().map_err(|_| ()),
                )
                .await;
            }
            match res {
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

/// Consignes d'initialisation : celles de POP, précédées de celles du rôle de l'agent (#111), et du rôle
/// d'Orchestrateur par défaut quand l'agent l'a sans l'avoir reçu d'un humain (#112).
fn consignes(role: Option<&str>, orch: bool, base: &str) -> String {
    match role.and_then(crate::roles::get) {
        Some(r) => format!(
            "Votre rôle dans l'équipe : {}. {}\n\n{base}",
            r.nom, r.consignes
        ),
        None if orch => format!("Vous êtes aussi l'Orchestrateur de cette map (premier agent Claude lancé) : quand l'humain fait une demande qui demande plusieurs tâches, découpez-la en tâches d'environ une heure avec plan_tasks, puis attribuez-les aux agents selon leur rôle (list_agents, assign_task). Un humain peut confier ce rôle à un autre agent.\n\n{base}"),
        None => base.to_string(),
    }
}

const SANS_MAP: &str = "Aucune map de travail choisie pour cette IA. Appelez list_maps pour voir vos maps, puis use_map avec l'identifiant de la map sur laquelle travailler.";

/// `create_map` (#144) : mêmes règles que « Nouvelle map » dans POP (admin du studio, nom de 1 à 60 caractères,
/// date AAAA-MM-JJ), ligne dans le fil d'activité de la map, puis la map devient la map de travail du jeton.
async fn create_map(
    st: &AppState,
    user: &AuthUser,
    work_map: Option<i64>,
    agent: i64,
    a: &Value,
) -> Result<Value, AppError> {
    let nom = a["name"].as_str().unwrap_or("").trim().to_string();
    if nom.is_empty() {
        return Err(bad("Donnez un nom à la map (name)."));
    }
    crate::error::check_len("Nom de la map", &nom, 1, 60)?;
    let genre = a["genre"].as_str().unwrap_or("").trim().to_string();
    crate::error::check_len("Genre", &genre, 0, 60)?;
    let sortie = a["release_target"].as_str().map(str::trim);
    let mes_studios: Vec<(i64, String, i64, String)> = sqlx::query_as(
        "SELECT s.id, s.name, s.personal, ms.role FROM studios s JOIN memberships ms ON ms.studio_id = s.id AND ms.user_id = ?
         ORDER BY s.personal DESC, s.id",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    let studio = match &a["studio"] {
        Value::Null => mes_studios.iter().find(|s| s.2 != 0),
        Value::Number(n) => mes_studios.iter().find(|s| Some(s.0) == n.as_i64()),
        Value::String(s) if s.trim().parse::<i64>().is_ok() => {
            let id = s.trim().parse::<i64>().ok();
            mes_studios.iter().find(|x| Some(x.0) == id)
        }
        Value::String(s) => {
            let s = s.trim().to_lowercase();
            mes_studios.iter().find(|x| {
                x.1.to_lowercase() == s || (x.2 != 0 && (s == "espace perso" || s == "perso"))
            })
        }
        _ => None,
    }
    .ok_or_else(|| {
        bad("Studio introuvable, ou vous n'en êtes pas membre. Appelez list_maps pour voir vos studios, ou omettez studio pour votre espace perso.")
    })?;
    let role = crate::access::Role::parse(&studio.3).unwrap_or(crate::access::Role::Freelance);
    if role < crate::access::Role::Admin {
        return Err(bad(format!(
            "Votre rôle dans « {} » ne permet pas de créer une map : il faut être admin du studio. Demandez à un admin de la créer dans POP.",
            if studio.2 != 0 { "Espace perso" } else { &studio.1 }
        )));
    }
    let mid = crate::studio::insert_map(st, studio.0, &nom, &genre, sortie).await?;
    let detail = if genre.is_empty() {
        String::new()
    } else {
        format!("Genre : {genre}")
    };
    crate::access::log_activity(
        st,
        mid,
        Some(user.id),
        "map",
        &format!("Map « {nom} » créée par l'IA"),
        &detail,
    )
    .await?;
    sqlx::query("UPDATE api_tokens SET map_id = ?, task_id = NULL, task_since = NULL WHERE id = ?")
        .bind(mid)
        .bind(agent)
        .execute(&st.db)
        .await?;
    if let Some(old) = work_map {
        notify(st, old, json!({ "t": "refresh", "what": "ai" }));
    }
    notify(st, mid, json!({ "t": "refresh", "what": "ai" }));
    Ok(json!({
        "map": { "id": mid, "name": nom, "studio": if studio.2 != 0 { "Espace perso".to_string() } else { studio.1.clone() } },
        "message": format!("Map « {nom} » créée : c'est maintenant la map de travail, les outils agissent sur elle."),
    }))
}

/// Appel d'un outil par un jeton MCP du compte (#145) : `list_maps` et `use_map` choisissent la map de travail ;
/// les autres outils agissent sur elle, après vérification de l'appartenance et du rôle à chaque appel.
async fn token_call(
    st: &AppState,
    user: &AuthUser,
    work_map: Option<i64>,
    agent: i64,
    name: &str,
    a: Value,
) -> Result<Value, AppError> {
    match name {
        "list_maps" => {
            let rows: Vec<(i64, String, String, String, i64, String)> = sqlx::query_as(
                "SELECT m.id, m.name, s.name, ms.role, s.personal, m.status FROM maps m JOIN studios s ON s.id = m.studio_id
                 JOIN memberships ms ON ms.studio_id = m.studio_id AND ms.user_id = ? ORDER BY s.personal DESC, s.name, m.name",
            )
            .bind(user.id)
            .fetch_all(&st.db)
            .await?;
            Ok(json!({
                "maps": rows.into_iter().map(|(id, n, s, role, personal, status)| json!({
                    "id": id, "name": n, "studio": if personal != 0 { "Espace perso".to_string() } else { s },
                    "role": role, "status": status, "current": Some(id) == work_map,
                })).collect::<Vec<_>>(),
                "message": "Choisissez la map de travail avec use_map { \"map\": <id> }.",
            }))
        }
        "create_map" => create_map(st, user, work_map, agent, &a).await,
        "use_map" => {
            let mid = a["map"]
                .as_i64()
                .ok_or_else(|| bad("Identifiant de la map manquant (map, voir list_maps)."))?;
            let ctx = map_ctx(st, user, mid).await.map_err(|e| match e {
                AppError::NotFound => bad(format!(
                    "La map n° {mid} n'existe pas ou vous n'en êtes pas membre. Appelez list_maps pour voir vos maps."
                )),
                e => e,
            })?;
            let nom: String = sqlx::query_scalar("SELECT name FROM maps WHERE id = ?")
                .bind(mid)
                .fetch_one(&st.db)
                .await?;
            // Une tâche réservée dans une autre map est libérée : un agent n'a qu'une tâche, dans sa map de travail.
            sqlx::query(
                "UPDATE api_tokens SET map_id = ?,
                   task_id = CASE WHEN task_id IN (SELECT id FROM tasks WHERE map_id = ?) THEN task_id END,
                   task_since = CASE WHEN task_id IN (SELECT id FROM tasks WHERE map_id = ?) THEN task_since END
                 WHERE id = ?",
            )
            .bind(mid)
            .bind(mid)
            .bind(mid)
            .bind(agent)
            .execute(&st.db)
            .await?;
            if let Some(old) = work_map.filter(|&o| o != mid) {
                notify(st, old, json!({ "t": "refresh", "what": "ai" }));
            }
            notify(st, mid, json!({ "t": "refresh", "what": "ai" }));
            Ok(json!({
                "map": { "id": mid, "name": nom, "role": ctx.role.as_str() },
                "message": format!("Map de travail : « {nom} ». Les outils agissent maintenant sur cette map."),
            }))
        }
        _ => {
            let mid = work_map.ok_or_else(|| bad(SANS_MAP))?;
            // Membre retiré du studio : rien n'est lu ni modifié.
            map_ctx(st, user, mid).await.map_err(|e| match e {
                AppError::NotFound => bad(format!(
                    "Vous n'avez plus accès à votre map de travail (n° {mid}). Appelez list_maps puis use_map pour en choisir une autre."
                )),
                e => e,
            })?;
            call_as(st, user, mid, Some(agent), name, a).await
        }
    }
}

/// Tableaux blancs pour les IA (#147) : mêmes contrôles que l'interface et la WebSocket ; les changements partent en
/// direct aux membres qui ont le tableau ouvert.
async fn tableau(
    st: &AppState,
    ctx: &crate::access::MapCtx,
    name: &str,
    a: &Value,
) -> Result<Value, AppError> {
    let mid = ctx.map_id;
    let u = &ctx.user;
    let du_tableau = || async {
        let bid = a["board"]
            .as_i64()
            .ok_or_else(|| bad("Identifiant du tableau manquant (board, voir list_boards)."))?;
        let ok: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM boards WHERE id = ? AND map_id = ? AND deleted_at IS NULL",
        )
        .bind(bid)
        .bind(mid)
        .fetch_optional(&st.db)
        .await?;
        ok.ok_or_else(|| {
            bad(format!(
                "Tableau {bid} introuvable dans cette map (list_boards)."
            ))
        })
    };
    match name {
        "list_boards" => {
            let crate::Json(v) =
                crate::boards::list(State(st.clone()), u.clone(), axum::extract::Path(mid)).await?;
            Ok(v)
        }
        "get_board" => {
            let bid = du_tableau().await?;
            let crate::Json(mut v) =
                crate::boards::get(State(st.clone()), u.clone(), axum::extract::Path(bid)).await?;
            let objets: Vec<Value> = v["objects"]
                .as_object()
                .map(|m| {
                    m.iter()
                        .filter(|(k, _)| *k != "__meta")
                        .map(|(_, o)| o.clone())
                        .collect()
                })
                .unwrap_or_default();
            v["objects"] = json!(objets);
            Ok(v)
        }
        "create_board" => {
            let b =
                serde_json::from_value(json!({ "title": a["title"], "template": a["template"] }))
                    .map_err(|_| bad("title et template : du texte."))?;
            let crate::Json(v) = crate::boards::create(
                State(st.clone()),
                u.clone(),
                axum::extract::Path(mid),
                crate::Json(b),
            )
            .await?;
            notify(st, mid, json!({ "t": "refresh", "what": "boards" }));
            Ok(
                json!({ "board": v["id"], "message": "Tableau créé : ajoutez des objets avec upsert_board_objects." }),
            )
        }
        "upsert_board_objects" => {
            ctx.can_edit()?;
            let bid = du_tableau().await?;
            let list = a["objects"]
                .as_array()
                .filter(|l| !l.is_empty())
                .ok_or_else(|| bad("objects : au moins un objet."))?;
            if list.len() > 200 {
                return Err(bad("200 objets au plus par appel."));
            }
            let mut objs = vec![];
            let mut refus = vec![];
            for (i, o) in list.iter().enumerate() {
                let mut o = o.clone();
                let kind = o["kind"].as_str().unwrap_or("");
                let raison = if !o.is_object() || !crate::boards::KINDS.contains(&kind) {
                    Some(format!("type « {kind} » inconnu"))
                } else if o["text"]
                    .as_str()
                    .map(|t| t.chars().count() > 2000)
                    .unwrap_or(false)
                {
                    Some("texte de plus de 2 000 caractères".to_string())
                } else if o["url"]
                    .as_str()
                    .map(|t| t.len() > 200_000)
                    .unwrap_or(false)
                {
                    Some("image de plus de 200 Ko".to_string())
                } else if kind == "stroke" {
                    Some("dessin à main levée non pris en charge".to_string())
                } else {
                    None
                };
                if let Some(r) = raison {
                    refus.push(format!("objet {} : {r}", i + 1));
                    continue;
                }
                if o["id"].as_str().map(|s| s.is_empty()).unwrap_or(true) {
                    o["id"] = json!(crate::boards::oid());
                }
                objs.push(o);
            }
            let out = if objs.is_empty() {
                json!({ "objs": [] })
            } else {
                crate::boards::apply_op(
                    st,
                    u,
                    mid,
                    &json!({ "board": bid, "op": "upsert", "objs": objs }),
                )
                .await?
            };
            let gardes: Vec<Value> = out["objs"].as_array().cloned().unwrap_or_default();
            let ids: Vec<Value> = gardes.iter().map(|o| o["id"].clone()).collect();
            let ignores = list.len() - gardes.len();
            if gardes.is_empty() {
                return Err(bad(format!(
                    "Aucun objet gardé : {}.",
                    if refus.is_empty() {
                        "objets invalides".to_string()
                    } else {
                        refus.join(" ; ")
                    }
                )));
            }
            Ok(
                json!({ "ids": ids, "ignored": ignores, "details": refus, "message": format!("{} objet(s) enregistrés{}.", gardes.len(), if ignores > 0 { format!(", {ignores} ignoré(s)") } else { String::new() }) }),
            )
        }
        "delete_board_objects" => {
            ctx.can_edit()?;
            let bid = du_tableau().await?;
            let out = crate::boards::apply_op(
                st,
                u,
                mid,
                &json!({ "board": bid, "op": "delete", "ids": a["ids"] }),
            )
            .await?;
            Ok(
                json!({ "message": format!("{} objet(s) retiré(s).", out["ids"].as_array().map(|x| x.len()).unwrap_or(0)) }),
            )
        }
        _ => Err(bad("Outil inconnu.")),
    }
}

/// Documentation pour les IA (#147) : lecture en Markdown ; écriture d'une page, avec une version gardée à chaque
/// écriture (l'interface n'en garde qu'une toutes les 2 minutes).
async fn documentation(
    st: &AppState,
    ctx: &crate::access::MapCtx,
    name: &str,
    a: &Value,
) -> Result<Value, AppError> {
    let mid = ctx.map_id;
    let u = &ctx.user;
    let page_de_la_map = |pid: i64| async move {
        let crate::Json(p) =
            crate::pages::get(State(st.clone()), u.clone(), axum::extract::Path(pid)).await?;
        if p["map_id"].as_i64() != Some(mid) {
            return Err(bad(format!(
                "Page {pid} introuvable dans cette map (list_pages)."
            )));
        }
        Ok::<Value, AppError>(p)
    };
    match name {
        "list_pages" => {
            let crate::Json(v) =
                crate::pages::tree(State(st.clone()), u.clone(), axum::extract::Path(mid)).await?;
            Ok(v)
        }
        "read_page" => {
            let pid = a["page"]
                .as_i64()
                .ok_or_else(|| bad("Identifiant de la page manquant (page)."))?;
            let p = page_de_la_map(pid).await.map_err(|e| match e {
                AppError::NotFound => bad(format!(
                    "Page {pid} introuvable dans cette map (list_pages)."
                )),
                e => e,
            })?;
            Ok(
                json!({ "id": pid, "title": p["title"], "version": p["version"], "updated_at": p["updated_at"], "markdown": crate::pages::page_markdown(&p) }),
            )
        }
        "write_page" => {
            ctx.can_edit()?;
            let md = a["markdown"].as_str().unwrap_or("");
            if md.len() > 1_000_000 {
                return Err(bad("Page trop volumineuse (1 Mo de Markdown au plus)."));
            }
            let mut blocks = crate::pages::from_markdown(md);
            // Un titre « # … » en tête devient le titre de la page s'il n'est pas donné.
            let mut titre = a["title"]
                .as_str()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if titre.is_none() && blocks.first().map(|b| b["type"] == "h1").unwrap_or(false) {
                titre = blocks
                    .first()
                    .and_then(|b| b["text"].as_str())
                    .map(String::from);
                blocks.remove(0);
            }
            let pid = match a["page"].as_i64() {
                Some(pid) => {
                    page_de_la_map(pid).await?;
                    pid
                }
                None => {
                    let nb = serde_json::from_value(json!({ "title": titre.clone().unwrap_or_else(|| "Sans titre".into()), "parent_id": a["parent"] }))
                        .map_err(|_| bad("parent : identifiant de page."))?;
                    let crate::Json(v) = crate::pages::create(
                        State(st.clone()),
                        u.clone(),
                        axum::extract::Path(mid),
                        crate::Json(nb),
                    )
                    .await?;
                    v["id"].as_i64().unwrap_or(0)
                }
            };
            let patch = serde_json::from_value(
                json!({ "title": titre, "blocks": blocks, "client": "mcp" }),
            )
            .map_err(|_| bad("Page invalide."))?;
            let crate::Json(r) = crate::pages::update(
                State(st.clone()),
                u.clone(),
                axum::extract::Path(pid),
                crate::Json(patch),
            )
            .await?;
            let version = r["version"].as_i64().unwrap_or(1);
            let deja: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM page_versions WHERE page_id = ? AND version = ?",
            )
            .bind(pid)
            .bind(version)
            .fetch_optional(&st.db)
            .await?;
            if deja.is_none() {
                let (t, b): (String, String) =
                    sqlx::query_as("SELECT title, blocks FROM pages WHERE id = ?")
                        .bind(pid)
                        .fetch_one(&st.db)
                        .await?;
                crate::pages::snapshot(st, pid, version, &t, &b, u.id).await?;
            }
            notify(st, mid, json!({ "t": "refresh", "what": "pages" }));
            Ok(
                json!({ "page": pid, "version": version, "message": format!("Page enregistrée (version {version}).") }),
            )
        }
        _ => Err(bad("Outil inconnu.")),
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
        "list_agents" => crate::orchestre::liste(st, mid).await,
        "list_boards" | "get_board" | "create_board" | "upsert_board_objects"
        | "delete_board_objects" => tableau(st, &ctx, name, &a).await,
        "list_pages" | "read_page" | "write_page" => documentation(st, &ctx, name, &a).await,
        "list_milestones" => {
            let rows: Vec<(i64, String, String, String, Option<String>, String)> = sqlx::query_as(
                "SELECT id, title, date, kind, time, detail FROM milestones WHERE map_id = ? ORDER BY date, id",
            )
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
            Ok(json!(rows.into_iter().map(|r| json!({ "id": r.0, "title": r.1, "date": r.2, "kind": r.3, "time": r.4, "detail": r.5 })).collect::<Vec<_>>()))
        }
        "add_milestone" => {
            let b = serde_json::from_value(json!({ "title": a["title"].as_str().unwrap_or(""), "date": a["date"].as_str().unwrap_or(""), "kind": a["kind"], "time": a["time"], "detail": a["detail"] }))
                .map_err(|_| bad("title et date (AAAA-MM-JJ) attendus."))?;
            let crate::Json(v) = crate::plan::add_milestone(State(st.clone()), user.clone(), axum::extract::Path(mid), crate::Json(b)).await?;
            Ok(json!({ "id": v["id"], "message": format!("Jalon « {} » ajouté au {}.", a["title"].as_str().unwrap_or(""), a["date"].as_str().unwrap_or("")) }))
        }
        "plan_tasks" => crate::orchestre::plan(st, &ctx, agent_requis()?, &a).await,
        "assign_task" => crate::orchestre::assign(st, &ctx, agent_requis()?, &a).await,
        "save_progress" => crate::agents::save_progress(st, agent_requis()?, &a).await,
        "ask_user" => crate::questions::ask_user(st, mid, agent_requis()?, &a).await,
        "get_answer" => crate::questions::get_answer(st, mid, &a).await,
        "list_device_configs" => Ok(json!(crate::device_configs::liste(st, ctx.studio_id, a["query"].as_str().unwrap_or("")).await?)),
        "save_device_config" => {
            let b: crate::device_configs::Enregistrer = serde_json::from_value(json!({ "actor_id": a["actor_id"], "name": a["name"], "description": a["description"].as_str().unwrap_or(""), "asset": a["asset"].as_str().unwrap_or("") }))
                .map_err(|_| bad("actor_id et name : du texte."))?;
            crate::device_configs::enregistrer(st, &ctx, &b).await
        }
        "apply_device_config" => {
            let ids: Vec<String> = a["ids"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_str().map(String::from)).collect();
            crate::device_configs::appliquer(st, o(), a["config_id"].as_i64().ok_or_else(|| bad("config_id attendu."))?, &ids).await
        }
        "place_configured_device" => crate::device_configs::poser(st, &ctx, "mcp", agent, a["config_id"].as_i64().ok_or_else(|| bad("config_id attendu."))?, &a).await,
        "get_feasibility" => crate::faisabilite::avis_de_la_map(st, mid).await,
        "list_scenarios" => crate::scenarios::liste(st, mid).await,
        "propose_scenario" => {
            let id = crate::scenarios::creer(st, &ctx, a["title"].as_str().unwrap_or(""), &a["steps"], Some(agent_requis()?)).await?;
            Ok(json!({ "id": id, "status": "draft", "message": "Scénario proposé : un humain doit le valider (Lancer et suivre → Test) avant qu'il soit joué." }))
        }
        "list_skills" => Ok(json!(crate::skills::liste(st, ctx.studio_id).await?)),
        "get_skill" => {
            crate::skills::lire(st, ctx.studio_id, a["name"].as_str().unwrap_or("")).await
        }
        "list_tasks" => {
            let rows: Vec<(i64, String, String, String, Option<String>, Option<i64>, Option<String>, i64, Option<String>)> = sqlx::query_as(
                "SELECT t.number, t.title, t.status, t.tag, u.name, t.assignee_id, t.planned_day, t.id, g.name FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id LEFT JOIN api_tokens g ON g.id = t.agent_id WHERE t.map_id = ? ORDER BY t.status = 'done', t.planned_day IS NULL, t.planned_day, t.number",
            ).bind(mid).fetch_all(&st.db).await?;
            let deps: Vec<(i64, i64)> = sqlx::query_as("SELECT d.task_id, o.number FROM task_deps d JOIN tasks o ON o.id = d.depends_on JOIN tasks t ON t.id = d.task_id WHERE t.map_id = ? AND o.status != 'done'").bind(mid).fetch_all(&st.db).await?;
            let q = a["query"].as_str().unwrap_or("").to_lowercase();
            let out: Vec<Value> = rows.into_iter().filter(|r| a["status"].as_str().map(|s| s == r.2).unwrap_or(true) && (!a["mine"].as_bool().unwrap_or(false) || r.5 == Some(user.id)) && (q.is_empty() || r.1.to_lowercase().contains(&q)))
                .take(200).map(|r| json!({ "number": r.0, "title": r.1, "status": r.2, "tag": r.3, "assignee": r.4, "planned_day": r.6, "agent": r.8, "waiting_on": deps.iter().filter(|d| d.0 == r.7).map(|d| d.1).collect::<Vec<_>>() })).collect();
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
            let mut r = json!({ "résumé": v["points"], "suggestions": v["suggestions"] });
            // #114 : la tâche réservée par l'agent et les tâches interrompues à reprendre, avec leur note.
            if let Some(ag) = agent {
                let p = crate::agents::pour_next_task(st, ag, mid).await?;
                r["ta_tache"] = p["ta_tache"].clone();
                r["a_reprendre"] = p["a_reprendre"].clone();
                r["attribuees"] = p["attribuees"].clone();
            }
            Ok(r)
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
                lot: a["lot"].as_str().map(|l| l.trim().chars().take(60).collect()),
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
        "set_task_lot" => {
            ctx.can_edit()?;
            let tid = task_by_number(a["number"].as_i64()).await?;
            let old: String = sqlx::query_scalar("SELECT lot FROM tasks WHERE id = ?")
                .bind(tid)
                .fetch_one(&st.db)
                .await?;
            let lot = a["lot"].as_str().unwrap_or("");
            crate::tasks::set_lot(st, user, tid, &old, lot).await?;
            crate::access::notify(st, mid, json!({ "t": "refresh", "what": "tasks" }));
            Ok(json!({ "message": if lot.trim().is_empty() { "Tâche sans fonctionnalité.".to_string() } else { format!("Tâche rangée dans « {} ».", lot.trim()) } }))
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
            let status = crate::uefn::statut(st, mid).await?;
            let message = match status {
                "connected" => "UEFN est ouvert et relié : les actions partent tout de suite.",
                "closed" => "Le relais tourne mais UEFN est fermé : les actions attendent son ouverture.",
                "unlinked" => "Aucun projet UEFN n'est relié à cette map : demandez à l'humain de relier son projet (Organiseur → UEFN Link → code d'appairage). Les actions UEFN seront mises en attente.",
                _ => "Le relais de cette map est hors ligne : les actions attendent qu'il revienne.",
            };
            Ok(json!({ "status": status, "pending": pending, "message": message }))
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
        "place_mesh" => {
            exec(
                st,
                o(),
                "mesh_spawn",
                json!({ "shape": a["shape"], "mesh": a["mesh"], "name": a["name"], "folder": a["folder"], "location": a["location"], "rotation": a["rotation"], "scale": a["scale"] }),
            )
            .await
        }
        "get_island_settings" => {
            let r = exec(st, o(), "island_settings", json!({})).await?;
            let names: Vec<String> = a["names"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect();
            Ok(island_view(&r, &names))
        }
        "set_island_settings" => {
            let wanted = a["settings"]
                .as_object()
                .filter(|m| !m.is_empty() && m.len() <= 50)
                .cloned()
                .ok_or_else(|| bad("settings : au moins un réglage, ex. {\"MaxPlayers\": \"16\"}."))?;
            let r = exec(st, o(), "island_settings", json!({})).await?;
            let (id, options, current) = island_check(&r, &wanted)?;
            // L'annulation repart des valeurs actuelles : on les mémorise dans le dernier état connu.
            let snap = snapshot(st, mid).await?;
            let mut all = snap.actors.clone();
            let x = all
                .iter_mut()
                .find(|x| x["id"].as_str() == Some(id.as_str()))
                .ok_or_else(|| bad("Réglages de l’île absents du dernier état connu : actualisez les acteurs."))?;
            if !x["options"].is_object() {
                x["options"] = json!({});
            }
            for (k, v) in current {
                x["options"][k] = v;
            }
            sqlx::query("UPDATE uefn_state SET actors = ? WHERE map_id = ?")
                .bind(json!(all).to_string())
                .bind(mid)
                .execute(&st.db)
                .await?;
            exec(
                st,
                o(),
                "actor_update",
                json!({ "ids": [id], "changes": { "options": options } }),
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
            prevenir_validation(st, mid, "Une IA connectée demande à supprimer des acteurs : validation requise").await;
            notify(st, mid, json!({ "t": "uefn", "what": "actors" }));
            Ok(
                json!({ "pending_approval": aid, "message": "Suppression en attente : un membre doit la valider dans POP (Paramètres → UEFN Link) ou dans Forge." }),
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
                    prevenir_validation(st, m, "Une IA connectée demande une annulation qui retire des éléments : validation requise").await;
                    return Ok(json!({ "pending_approval": aid, "message": "Annulation en attente : un membre doit la valider dans POP (Paramètres → UEFN Link) ou dans Forge." }));
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
/// Prévient les membres de la map qu'une action demandée par une IA attend leur validation (cloche du site et de
/// Forge, #298).
async fn prevenir_validation(st: &AppState, mid: i64, texte: &str) {
    let membres: Vec<i64> = sqlx::query_scalar("SELECT m.user_id FROM memberships m JOIN maps p ON p.studio_id = m.studio_id WHERE p.id = ? AND m.role IN ('owner', 'admin', 'member')")
        .bind(mid)
        .fetch_all(&st.db)
        .await
        .unwrap_or_default();
    for u in membres {
        crate::account::notify_user(st, u, mid, texte, "/parametres?onglet=uefn").await;
    }
}

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
        assert!(r.as_array().unwrap().iter().any(|m| m["id"] == id));
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
        // #182 : create_task avec lot range la tâche ; set_task_lot la déplace.
        let r = call_as(
            &st,
            &u,
            mid,
            Some(a),
            "create_task",
            json!({ "title": "Couper l'herbe", "lot": "Herbe" }),
        )
        .await
        .unwrap();
        let m = r["number"].as_i64().unwrap();
        let lot = |num: i64| {
            let st = st.clone();
            async move {
                sqlx::query_scalar::<_, String>(
                    "SELECT lot FROM tasks WHERE map_id = ? AND number = ?",
                )
                .bind(mid)
                .bind(num)
                .fetch_one(&st.db)
                .await
                .unwrap()
            }
        };
        assert_eq!(lot(m).await, "Herbe");
        call_as(
            &st,
            &u,
            mid,
            Some(a),
            "set_task_lot",
            json!({ "number": m, "lot": "Terrains" }),
        )
        .await
        .unwrap();
        assert_eq!(lot(m).await, "Terrains");
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

#[cfg(test)]
mod tests_dependances {
    /// #160 : l'IA lit ce qu'est une dépendance dans la description de create_task.
    #[test]
    fn create_task_definit_une_dependance() {
        let tools = super::tools();
        let t = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "create_task")
            .unwrap();
        let d = t["description"].as_str().unwrap();
        assert!(d.contains("ni faite ni testée"), "{d}");
        assert!(d.contains("découvre"), "{d}");
    }
}

/// Réglages de l'île rendus à l'IA (#148) : valeurs permises seulement pour les noms demandés (la liste complète
/// en compte des centaines).
fn island_view(r: &Value, names: &[String]) -> Value {
    let list = r["settings"].as_array().cloned().unwrap_or_default();
    let out: Vec<Value> = list
        .iter()
        .filter(|o| names.is_empty() || names.iter().any(|n| o["name"].as_str() == Some(n.as_str())))
        .map(|o| {
            let mut v = json!({ "name": o["name"], "label": o["displayName"], "value": o["currentValue"], "value_label": o["currentLabel"] });
            if !names.is_empty() {
                v["description"] = o["description"].clone();
                v["allowed"] = json!(o["allowedValues"].as_array().cloned().unwrap_or_default().iter().map(|x| x["value"].clone()).collect::<Vec<_>>());
            }
            v
        })
        .collect();
    json!({ "device": r["id"], "count": out.len(), "settings": out })
}

/// Vérifie les réglages demandés contre ceux d'UEFN (#148) : nom connu, valeur permise (valeur ou libellé).
/// Rend l'acteur, les options à envoyer et leurs valeurs actuelles.
fn island_check(
    r: &Value,
    wanted: &serde_json::Map<String, Value>,
) -> Result<(String, Value, Vec<(String, Value)>), AppError> {
    let id = r["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("Aucun appareil « Réglages de l’île » dans le niveau."))?
        .to_string();
    let list = r["settings"].as_array().cloned().unwrap_or_default();
    let mut options = json!({});
    let mut current = vec![];
    for (k, v) in wanted {
        let o = list
            .iter()
            .find(|o| o["name"].as_str() == Some(k.as_str()))
            .ok_or_else(|| {
                bad(format!(
                    "Réglage de l’île inconnu : « {k} » (voir get_island_settings)."
                ))
            })?;
        let txt = match v {
            Value::String(s) => s.trim().to_string(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => return Err(bad(format!("{k} : valeur texte ou nombre attendue."))),
        };
        let allowed = o["allowedValues"].as_array().cloned().unwrap_or_default();
        let value = if allowed.is_empty() {
            txt.clone()
        } else {
            allowed
                .iter()
                .find(|a| {
                    a["value"].as_str() == Some(txt.as_str())
                        || a["label"]
                            .as_str()
                            .map(|l| l.eq_ignore_ascii_case(&txt))
                            .unwrap_or(false)
                })
                .and_then(|a| a["value"].as_str().map(String::from))
                .ok_or_else(|| {
                    let ex: Vec<String> = allowed
                        .iter()
                        .take(12)
                        .filter_map(|a| a["label"].as_str().map(String::from))
                        .collect();
                    bad(format!(
                        "{k} : valeur « {txt} » refusée. Valeurs permises : {}{}.",
                        ex.join(", "),
                        if allowed.len() > 12 { "…" } else { "" }
                    ))
                })?
        };
        options[k] = json!(value);
        current.push((k.clone(), o["currentValue"].clone()));
    }
    Ok((id, options, current))
}

#[cfg(test)]
mod tests_compte {
    //! #145 : un jeton MCP par compte, map de travail choisie par list_maps / use_map.
    use super::*;

    struct Monde {
        st: AppState,
        uid: i64,
        perso: i64,
        equipe: i64,
        sid_equipe: i64,
        autre: i64,
    }

    impl Monde {
        async fn perso_studio(&self) -> i64 {
            q(
                &self.st,
                &format!("SELECT studio_id FROM maps WHERE id = {}", self.perso),
            )
            .await
        }
    }

    async fn q(st: &AppState, sql: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_string()))
            .fetch_one(&st.db)
            .await
            .unwrap()
    }

    async fn monde() -> Monde {
        let st = crate::test_state().await;
        let uid = q(&st, "INSERT INTO users (email, name, password_hash) VALUES ('ia@a.fr', 'Yanis', 'x') RETURNING id").await;
        let intrus = q(&st, "INSERT INTO users (email, name, password_hash) VALUES ('b@a.fr', 'Bob', 'x') RETURNING id").await;
        let sp = q(
            &st,
            "INSERT INTO studios (name, personal) VALUES ('Perso', 1) RETURNING id",
        )
        .await;
        let se = q(
            &st,
            "INSERT INTO studios (name) VALUES ('Equipe') RETURNING id",
        )
        .await;
        let sa = q(
            &st,
            "INSERT INTO studios (name) VALUES ('Ailleurs') RETURNING id",
        )
        .await;
        for (s, u, r) in [
            (sp, uid, "owner"),
            (se, uid, "member"),
            (sa, intrus, "owner"),
        ] {
            sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, ?)")
                .bind(s)
                .bind(u)
                .bind(r)
                .execute(&st.db)
                .await
                .unwrap();
        }
        let map = |s: i64, n: &'static str| {
            let st = st.clone();
            async move {
                let m: i64 = sqlx::query_scalar(
                    "INSERT INTO maps (studio_id, name) VALUES (?, ?) RETURNING id",
                )
                .bind(s)
                .bind(n)
                .fetch_one(&st.db)
                .await
                .unwrap();
                sqlx::query("INSERT INTO tasks (map_id, number, title) VALUES (?, 1, ?)")
                    .bind(m)
                    .bind(format!("Tâche de {n}"))
                    .execute(&st.db)
                    .await
                    .unwrap();
                m
            }
        };
        Monde {
            perso: map(sp, "Solo").await,
            equipe: map(se, "GrassCutter").await,
            autre: map(sa, "Secrète").await,
            sid_equipe: se,
            st,
            uid,
        }
    }

    /// Appel MCP complet (comme une IA) : jeton dans l'en-tête, réponse JSON-RPC.
    async fn mcp_call(st: &AppState, token: &str, name: &str, args: Value) -> (bool, String) {
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        let r = mcp(
            State(st.clone()),
            h,
            axum::extract::Query(McpQ { token: None }),
            Json(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": args } })),
        )
        .await;
        let b = axum::body::to_bytes(r.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&b).unwrap();
        (
            v["result"]["isError"].as_bool().unwrap_or(true),
            v["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        )
    }

    async fn jeton(m: &Monde, map: Option<i64>, scope: &str) -> String {
        let t = format!("pop_test_{}", random_token());
        sqlx::query("INSERT INTO api_tokens (user_id, map_id, name, token_hash, scope) VALUES (?, ?, 'IA', ?, ?)")
            .bind(m.uid)
            .bind(map)
            .bind(sha256_hex(&t))
            .bind(scope)
            .execute(&m.st.db)
            .await
            .unwrap();
        t
    }

    #[tokio::test]
    async fn creer_un_jeton_de_compte() {
        let m = monde().await;
        let u = AuthUser {
            id: m.uid,
            email: "ia@a.fr".into(),
            name: "Yanis".into(),
            sid: String::new(),
        };
        let Json(r) = create_token(
            State(m.st.clone()),
            u.clone(),
            Json(TokenIn {
                name: "Claude Code".into(),
                kind: Some("claude_code".into()),
                scope: None,
                role: Some("testeur".into()),
            }),
        )
        .await
        .unwrap();
        let t = r["token"].as_str().unwrap().to_string();
        let map: Option<i64> = sqlx::query_scalar("SELECT map_id FROM api_tokens WHERE id = ?")
            .bind(r["id"].as_i64().unwrap())
            .fetch_one(&m.st.db)
            .await
            .unwrap();
        assert_eq!(map, None, "un jeton de compte n'est lié à aucune map");
        let Json(l) = tokens(State(m.st.clone()), u).await.unwrap();
        assert_eq!(l.as_array().unwrap().len(), 1);
        assert_eq!(
            l[0]["role"], "testeur",
            "le rôle choisi à la création est gardé (#111)"
        );
        // Sans map de travail : l'erreur explique list_maps / use_map, et rien n'est lu.
        let (err, msg) = mcp_call(&m.st, &t, "list_tasks", json!({})).await;
        assert!(
            err && msg.contains("list_maps") && msg.contains("use_map"),
            "{msg}"
        );
    }

    /// #144 : create_map crée la map (espace perso ou studio dont on est admin), la journalise et en fait la map
    /// de travail ; refus clair pour un rôle insuffisant, un nom vide ou un studio inconnu, sans rien créer.
    #[tokio::test]
    async fn creer_une_map_par_le_mcp() {
        let m = monde().await;
        let t = jeton(&m, None, "write").await;
        let combien = || async { q(&m.st, "SELECT COUNT(*) FROM maps").await };
        let avant = combien().await;
        let (err, msg) = mcp_call(&m.st, &t, "create_map", json!({ "name": "  " })).await;
        assert!(err && msg.contains("nom"), "{msg}");
        let (err, msg) = mcp_call(&m.st, &t, "create_map", json!({ "name": "x".repeat(61) })).await;
        assert!(err, "{msg}");
        let (err, msg) = mcp_call(
            &m.st,
            &t,
            "create_map",
            json!({ "name": "Arène", "studio": "Equipe" }),
        )
        .await;
        assert!(
            err && msg.contains("admin"),
            "membre du studio : refusé : {msg}"
        );
        let (err, msg) = mcp_call(
            &m.st,
            &t,
            "create_map",
            json!({ "name": "Arène", "studio": "Ailleurs" }),
        )
        .await;
        assert!(
            err && msg.contains("introuvable"),
            "studio d'un autre : {msg}"
        );
        let (err, msg) = mcp_call(
            &m.st,
            &t,
            "create_map",
            json!({ "name": "Arène", "release_target": "bientôt" }),
        )
        .await;
        assert!(err && msg.contains("AAAA-MM-JJ"), "{msg}");
        assert_eq!(combien().await, avant, "rien n'est créé en cas de refus");

        let (err, msg) = mcp_call(
            &m.st,
            &t,
            "create_map",
            json!({ "name": "Grass Cutter", "genre": "Tycoon", "release_target": "2027-02-01" }),
        )
        .await;
        assert!(!err, "{msg}");
        let r: Value = serde_json::from_str(&msg).unwrap();
        let mid = r["map"]["id"].as_i64().unwrap();
        assert_eq!(r["map"]["studio"], "Espace perso");
        let (sid, genre, sortie): (i64, String, Option<String>) =
            sqlx::query_as("SELECT studio_id, genre, release_date FROM maps WHERE id = ?")
                .bind(mid)
                .fetch_one(&m.st.db)
                .await
                .unwrap();
        assert_eq!(
            (sid, genre.as_str(), sortie.as_deref()),
            (m.perso_studio().await, "Tycoon", Some("2027-02-01"))
        );
        let journal = q(
            &m.st,
            &format!("SELECT COUNT(*) FROM activity WHERE map_id = {mid} AND kind = 'map'"),
        )
        .await;
        assert_eq!(
            journal, 1,
            "la création est dans le fil d'activité de la map"
        );
        // Map de travail : list_tasks répond sur elle sans use_map.
        let (err, msg) = mcp_call(&m.st, &t, "list_tasks", json!({})).await;
        assert!(!err, "{msg}");
        let (_, l) = mcp_call(&m.st, &t, "list_maps", json!({})).await;
        let l: Value = serde_json::from_str(&l).unwrap();
        assert!(
            l["maps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["id"] == mid && x["current"] == true),
            "{l}"
        );
    }

    /// #134 : la section « Mes agents » lit last_used : vide = en attente de connexion, rempli = connectée.
    #[tokio::test]
    async fn agent_en_attente_puis_connecte() {
        let m = monde().await;
        let t = jeton(&m, None, "write").await;
        let last = || async {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT last_used FROM api_tokens WHERE user_id = ?",
            )
            .bind(m.uid)
            .fetch_one(&m.st.db)
            .await
            .unwrap()
        };
        assert!(
            last().await.is_none(),
            "jeton créé : en attente de connexion"
        );
        let (err, txt) = mcp_call(&m.st, &t, "list_maps", json!({})).await;
        assert!(!err, "{txt}");
        assert!(
            last().await.is_some(),
            "après le premier appel MCP : connectée"
        );
    }

    #[tokio::test]
    async fn list_maps_puis_use_map() {
        let m = monde().await;
        let t = jeton(&m, None, "write").await;
        let (err, txt) = mcp_call(&m.st, &t, "list_maps", json!({})).await;
        assert!(!err, "{txt}");
        let v: Value = serde_json::from_str(&txt).unwrap();
        let ids: Vec<i64> = v["maps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_i64().unwrap())
            .collect();
        assert_eq!(ids.len(), 2, "{txt}");
        assert!(ids.contains(&m.perso) && ids.contains(&m.equipe) && !ids.contains(&m.autre));
        assert!(
            txt.contains("\"role\": \"member\"") && txt.contains("\"role\": \"owner\""),
            "{txt}"
        );
        // use_map sur l'équipe : list_tasks lit les tâches de GrassCutter.
        let (err, txt) = mcp_call(&m.st, &t, "use_map", json!({ "map": m.equipe })).await;
        assert!(!err, "{txt}");
        let (_, txt) = mcp_call(&m.st, &t, "list_tasks", json!({})).await;
        assert!(txt.contains("Tâche de GrassCutter"), "{txt}");
        // Changement de map sans reconnecter : même jeton, autre map ; le choix est gardé en base.
        mcp_call(&m.st, &t, "use_map", json!({ "map": m.perso })).await;
        let (_, txt) = mcp_call(&m.st, &t, "list_tasks", json!({})).await;
        assert!(
            txt.contains("Tâche de Solo") && !txt.contains("GrassCutter"),
            "{txt}"
        );
        let gardee: Option<i64> =
            sqlx::query_scalar("SELECT map_id FROM api_tokens WHERE token_hash = ?")
                .bind(sha256_hex(&t))
                .fetch_one(&m.st.db)
                .await
                .unwrap();
        assert_eq!(gardee, Some(m.perso));
        let (_, txt) = mcp_call(&m.st, &t, "list_maps", json!({})).await;
        assert!(txt.contains("\"current\": true"), "{txt}");
    }

    #[tokio::test]
    async fn map_inaccessible_et_retrait_du_studio() {
        let m = monde().await;
        let t = jeton(&m, None, "write").await;
        // Map d'un autre studio : refus clair, la map de travail ne change pas.
        let (err, msg) = mcp_call(&m.st, &t, "use_map", json!({ "map": m.autre })).await;
        assert!(err && msg.contains("pas membre"), "{msg}");
        let (err, msg) = mcp_call(&m.st, &t, "use_map", json!({ "map": 999_999 })).await;
        assert!(err && msg.contains("list_maps"), "{msg}");
        let (err, _) = mcp_call(&m.st, &t, "list_tasks", json!({})).await;
        assert!(err);
        // Retiré du studio : l'appel suivant est refusé, aucune tâche créée.
        mcp_call(&m.st, &t, "use_map", json!({ "map": m.equipe })).await;
        sqlx::query("DELETE FROM memberships WHERE studio_id = ? AND user_id = ?")
            .bind(m.sid_equipe)
            .bind(m.uid)
            .execute(&m.st.db)
            .await
            .unwrap();
        let (err, msg) = mcp_call(&m.st, &t, "create_task", json!({ "title": "Intrusion" })).await;
        assert!(err && msg.contains("plus accès"), "{msg}");
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ?")
            .bind(m.equipe)
            .fetch_one(&m.st.db)
            .await
            .unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn ancien_jeton_de_map() {
        let m = monde().await;
        // Un jeton créé avant #145 a sa map : il marche sans use_map.
        let t = jeton(&m, Some(m.equipe), "write").await;
        let (err, txt) = mcp_call(&m.st, &t, "list_tasks", json!({})).await;
        assert!(!err && txt.contains("Tâche de GrassCutter"), "{txt}");
        let (err, txt) = mcp_call(
            &m.st,
            &t,
            "create_task",
            json!({ "title": "Couper l'herbe" }),
        )
        .await;
        assert!(!err, "{txt}");
    }

    /// Appel JSON-RPC d'une méthode (initialize, tools/list) : le résultat brut.
    async fn rpc(st: &AppState, token: &str, method: &str) -> Value {
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        let r = mcp(
            State(st.clone()),
            h,
            axum::extract::Query(McpQ { token: None }),
            Json(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": {} })),
        )
        .await;
        let b = axum::body::to_bytes(r.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice::<Value>(&b).unwrap()["result"].clone()
    }

    /// #111 : rôle d'un agent : consignes à l'initialisation, outils filtrés, refus clair, changement au prochain appel.
    #[tokio::test]
    async fn roles_des_agents() {
        let m = monde().await;
        let t = jeton(&m, None, "write").await;
        let id: i64 = sqlx::query_scalar("SELECT id FROM api_tokens WHERE token_hash = ?")
            .bind(sha256_hex(&t))
            .fetch_one(&m.st.db)
            .await
            .unwrap();
        let user = AuthUser {
            id: m.uid,
            email: "a@b.c".into(),
            name: "A".into(),
            sid: String::new(),
        };
        let role = |r: Option<&str>| {
            Json(RoleIn {
                role: r.map(String::from),
            })
        };
        mcp_call(&m.st, &t, "use_map", json!({ "map": m.equipe })).await;

        // Codeur Verse : ses consignes, ses outils, pas ceux du Level designer.
        set_role(
            State(m.st.clone()),
            user.clone(),
            Path(id),
            role(Some("codeur_verse")),
        )
        .await
        .unwrap();
        let init = rpc(&m.st, &t, "initialize").await;
        assert!(
            init["instructions"]
                .as_str()
                .unwrap()
                .contains("Codeur Verse"),
            "{init}"
        );
        let noms: Vec<String> = rpc(&m.st, &t, "tools/list").await["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["name"].as_str().unwrap().to_string())
            .collect();
        assert!(
            noms.contains(&"write_verse_file".into()) && !noms.contains(&"place_device".into())
        );
        let (err, msg) = mcp_call(&m.st, &t, "place_device", json!({})).await;
        assert!(err && msg.contains("« Codeur Verse »"), "{msg}");

        // Relecteur : aucune modification, au prochain appel.
        set_role(
            State(m.st.clone()),
            user.clone(),
            Path(id),
            role(Some("relecteur")),
        )
        .await
        .unwrap();
        let (err, msg) = mcp_call(&m.st, &t, "create_task", json!({ "title": "Non" })).await;
        assert!(err && msg.contains("« Relecteur »"), "{msg}");
        let (err, txt) = mcp_call(&m.st, &t, "list_tasks", json!({})).await;
        assert!(!err, "{txt}");

        // Sans rôle : tout revient ; un rôle inconnu est refusé ; le jeton d'un autre compte n'est pas touché.
        set_role(State(m.st.clone()), user.clone(), Path(id), role(None))
            .await
            .unwrap();
        let (err, txt) = mcp_call(&m.st, &t, "create_task", json!({ "title": "Oui" })).await;
        assert!(!err, "{txt}");
        assert!(set_role(
            State(m.st.clone()),
            user.clone(),
            Path(id),
            role(Some("pirate"))
        )
        .await
        .is_err());
        let autre = AuthUser {
            id: m.uid + 999,
            ..user
        };
        assert!(matches!(
            set_role(
                State(m.st.clone()),
                autre,
                Path(id),
                role(Some("relecteur"))
            )
            .await,
            Err(AppError::NotFound)
        ));
    }

    /// Agent de test sur la map `map` : type et rôle donnés ; `il_y_a` : heure de son dernier appel (SQLite).
    async fn agent(
        m: &Monde,
        map: i64,
        nom: &str,
        kind: &str,
        role: Option<&str>,
        il_y_a: &str,
    ) -> (String, i64) {
        let t = format!("pop_test_{}", random_token());
        let id: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, map_id, name, token_hash, kind, role, created_at, last_used) VALUES (?, ?, ?, ?, ?, ?, datetime('now', ?), datetime('now', ?)) RETURNING id")
            .bind(m.uid).bind(map).bind(nom).bind(sha256_hex(&t)).bind(kind).bind(role).bind(il_y_a).bind(il_y_a)
            .fetch_one(&m.st.db).await.unwrap();
        (t, id)
    }

    async fn outils_visibles(st: &AppState, token: &str) -> String {
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        let r = mcp(
            State(st.clone()),
            h,
            axum::extract::Query(McpQ { token: None }),
            Json(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })),
        )
        .await;
        String::from_utf8(
            axum::body::to_bytes(r.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap()
    }

    /// #112, critères 1 et 2 : l'Orchestrateur (premier Claude lancé) crée 3 tâches liées et les attribue à deux
    /// agents, qui les voient avec next_task ; les autres agents ne peuvent ni planifier ni attribuer.
    #[tokio::test]
    async fn orchestrateur_planifie_et_attribue() {
        let m = monde().await;
        let (orch, _) = agent(
            &m,
            m.equipe,
            "Claude chef",
            "claude_code",
            None,
            "-30 minutes",
        )
        .await;
        let (verse, _) = agent(
            &m,
            m.equipe,
            "Codex Verse",
            "codex",
            Some("codeur_verse"),
            "-5 minutes",
        )
        .await;
        let (ld, ld_id) = agent(
            &m,
            m.equipe,
            "Claude niveau",
            "claude_code",
            Some("level_designer"),
            "-5 minutes",
        )
        .await;
        let (err, txt) = mcp_call(&m.st, &orch, "plan_tasks", json!({ "tasks": [
            { "key": "zone", "title": "Poser la zone de capture", "tag": "carte", "agent": "Claude niveau" },
            { "key": "score", "title": "Compter les captures en Verse", "depends_on": ["zone"], "agent": "Codex Verse" },
            { "title": "Afficher le score", "depends_on": ["score", 1], "agent": ld_id },
        ] })).await;
        assert!(!err, "{txt}");
        let v: Value = serde_json::from_str(&txt).unwrap();
        let n: Vec<i64> = v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["number"].as_i64().unwrap())
            .collect();
        assert_eq!(n, vec![2, 3, 4]);
        // Dépendances : 3 attend 2, 4 attend 3 et 1.
        let deps: Vec<(i64, i64)> = sqlx::query_as("SELECT t.number, o.number FROM task_deps d JOIN tasks t ON t.id = d.task_id JOIN tasks o ON o.id = d.depends_on WHERE t.map_id = ? ORDER BY 1, 2")
            .bind(m.equipe).fetch_all(&m.st.db).await.unwrap();
        assert_eq!(deps, vec![(3, 2), (4, 1), (4, 3)]);
        // Chacun voit les siennes avec next_task.
        let (_, txt) = mcp_call(&m.st, &verse, "next_task", json!({})).await;
        let v: Value = serde_json::from_str(&txt).unwrap();
        assert_eq!(v["attribuees"][0]["number"], 3, "{txt}");
        assert_eq!(v["attribuees"][0]["waiting_on"], json!([2]));
        let (_, txt) = mcp_call(&m.st, &ld, "next_task", json!({})).await;
        let v: Value = serde_json::from_str(&txt).unwrap();
        let siennes: Vec<i64> = v["attribuees"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["number"].as_i64().unwrap())
            .collect();
        assert_eq!(siennes, vec![2, 4], "{txt}");
        // Une tâche attribuée ne se réserve que par son agent.
        let (err, msg) = mcp_call(&m.st, &verse, "claim_task", json!({ "number": 2 })).await;
        assert!(
            err && msg.contains("attribuée à l'agent « Claude niveau »"),
            "{msg}"
        );
        let (err, msg) = mcp_call(&m.st, &ld, "claim_task", json!({ "number": 2 })).await;
        assert!(!err, "{msg}");
        // Critère 2 : un agent qui n'est pas l'Orchestrateur ne peut pas attribuer, ni planifier.
        for (t, outil, args) in [
            (
                &verse,
                "assign_task",
                json!({ "number": 4, "agent": "Codex Verse" }),
            ),
            (&ld, "plan_tasks", json!({ "tasks": [{ "title": "Non" }] })),
        ] {
            let (err, msg) = mcp_call(&m.st, t, outil, args).await;
            assert!(
                err && msg.contains("réservé à l'Orchestrateur"),
                "{outil} : {msg}"
            );
            assert!(!outils_visibles(&m.st, t).await.contains(outil));
        }
        assert!(outils_visibles(&m.st, &orch).await.contains("assign_task"));
        // On n'arrache pas une tâche en cours ; une tâche libre se réattribue, et l'historique le garde.
        let (err, msg) = mcp_call(
            &m.st,
            &orch,
            "assign_task",
            json!({ "number": 2, "agent": "Codex Verse" }),
        )
        .await;
        assert!(
            err && msg.contains("en cours chez l'agent « Claude niveau »"),
            "{msg}"
        );
        let (err, msg) = mcp_call(
            &m.st,
            &orch,
            "assign_task",
            json!({ "number": 4, "agent": null }),
        )
        .await;
        assert!(!err, "{msg}");
        let (err, msg) = mcp_call(&m.st, &verse, "claim_task", json!({ "number": 4 })).await;
        assert!(!err, "{msg}");
        let hist: Vec<String> = sqlx::query_scalar("SELECT e.what FROM task_events e JOIN tasks t ON t.id = e.task_id WHERE t.map_id = ? AND t.number = 4 ORDER BY e.id")
            .bind(m.equipe).fetch_all(&m.st.db).await.unwrap();
        assert!(
            hist.iter()
                .any(|h| h.starts_with("Attribuée à l'agent « Claude niveau »"))
                && hist.iter().any(|h| h.starts_with("Rendue libre")),
            "{hist:?}"
        );
        // Un plan invalide ne crée rien.
        let (err, msg) = mcp_call(
            &m.st,
            &orch,
            "plan_tasks",
            json!({ "tasks": [{ "title": "A" }, { "title": "B", "depends_on": ["inconnue"] }] }),
        )
        .await;
        assert!(err && msg.contains("dépendance"), "{msg}");
        assert_eq!(
            q(
                &m.st,
                &format!("SELECT COUNT(*) FROM tasks WHERE map_id = {}", m.equipe)
            )
            .await,
            4
        );
    }

    /// Décision du 24/11 : le premier Claude lancé et actif est l'Orchestrateur ; s'il s'arrête, le Claude suivant
    /// reprend ; un rôle « Orchestrateur » donné par un humain passe avant.
    #[tokio::test]
    async fn qui_est_orchestrateur() {
        let m = monde().await;
        let (_, premier) =
            agent(&m, m.equipe, "Claude 1", "claude_desktop", None, "-3 hours").await;
        let (_, second) = agent(&m, m.equipe, "Claude 2", "claude_code", None, "-1 hours").await;
        agent(&m, m.equipe, "Codex", "codex", None, "-1 minutes").await;
        // Claude 1 est silencieux depuis 3 h : Claude 2 reprend.
        assert_eq!(
            crate::orchestre::de_la_map(&m.st, m.equipe).await.unwrap(),
            Some(second)
        );
        sqlx::query("UPDATE api_tokens SET last_used = datetime('now') WHERE id = ?")
            .bind(premier)
            .execute(&m.st.db)
            .await
            .unwrap();
        assert_eq!(
            crate::orchestre::de_la_map(&m.st, m.equipe).await.unwrap(),
            Some(premier)
        );
        // Choix d'un humain : le Codex devient Orchestrateur.
        sqlx::query("UPDATE api_tokens SET role = 'orchestrateur' WHERE name = 'Codex'")
            .execute(&m.st.db)
            .await
            .unwrap();
        let codex: i64 = q(&m.st, "SELECT id FROM api_tokens WHERE name = 'Codex'").await;
        assert_eq!(
            crate::orchestre::de_la_map(&m.st, m.equipe).await.unwrap(),
            Some(codex)
        );
    }

    /// #147 : tableau blanc, documentation et jalons depuis une IA ; diffusion en direct ; lecture seule.
    #[tokio::test]
    async fn conception_par_l_ia() {
        let m = monde().await;
        let t = jeton(&m, Some(m.equipe), "write").await;
        let mut ws = m.st.hub.subscribe(m.equipe);
        let (err, txt) = mcp_call(
            &m.st,
            &t,
            "create_board",
            json!({ "title": "Principe de jeu" }),
        )
        .await;
        assert!(!err, "{txt}");
        let bid = serde_json::from_str::<Value>(&txt).unwrap()["board"]
            .as_i64()
            .unwrap();
        let (err, txt) = mcp_call(&m.st, &t, "upsert_board_objects", json!({ "board": bid, "objects": [
            { "kind": "frame", "x": 0, "y": 0, "w": 800, "h": 400, "text": "Boucle de jeu" },
            { "kind": "sticky", "x": 20, "y": 60, "w": 160, "h": 120, "text": "Couper l'herbe", "shade": 2 },
            { "kind": "sticky", "x": 200, "y": 60, "w": 160, "h": 120, "text": "Vendre" },
            { "kind": "inconnu", "x": 0, "y": 0, "text": "non" },
            { "kind": "sticky", "x": 0, "y": 0, "text": "x".repeat(2001) },
        ] }))
        .await;
        assert!(!err, "{txt}");
        let r: Value = serde_json::from_str(&txt).unwrap();
        assert_eq!(r["ignored"], 2, "{txt}");
        assert!(
            r["details"].to_string().contains("inconnu")
                && r["details"].to_string().contains("2 000")
        );
        // Critère 1 : diffusé en direct aux membres qui ont le tableau ouvert.
        let mut vu = false;
        while let Ok(msg) = ws.try_recv() {
            let v: Value = serde_json::from_str(&msg).unwrap_or(Value::Null);
            if v["t"] == "board"
                && v["board"] == bid
                && v["objs"].as_array().map(|o| o.len()) == Some(3)
            {
                vu = true;
            }
        }
        assert!(vu, "op « board » diffusée");
        // Critère 2 : relire, modifier un texte, retirer.
        let id = r["ids"][1].as_str().unwrap().to_string();
        let (_, txt) = mcp_call(&m.st, &t, "get_board", json!({ "board": bid })).await;
        assert!(
            txt.contains("Couper l'herbe") && !txt.contains("__meta"),
            "{txt}"
        );
        let (err, txt) = mcp_call(&m.st, &t, "upsert_board_objects", json!({ "board": bid, "objects": [{ "id": id, "kind": "sticky", "x": 20, "y": 60, "text": "Couper et ramasser" }] })).await;
        assert!(!err, "{txt}");
        let (_, txt) = mcp_call(&m.st, &t, "get_board", json!({ "board": bid })).await;
        assert!(txt.contains("Couper et ramasser") && !txt.contains("Couper l'herbe"));
        let (err, _) = mcp_call(
            &m.st,
            &t,
            "delete_board_objects",
            json!({ "board": bid, "ids": [id] }),
        )
        .await;
        assert!(!err);
        let (_, txt) = mcp_call(&m.st, &t, "get_board", json!({ "board": bid })).await;
        assert!(!txt.contains("Couper et ramasser") && txt.contains("Vendre"));
        // Un tableau d'une autre map reste introuvable.
        let autre: i64 = q(&m.st, &format!("INSERT INTO boards (map_id, title, objects) VALUES ({}, 'Secret', '{{}}') RETURNING id", m.autre)).await;
        let (err, msg) = mcp_call(&m.st, &t, "get_board", json!({ "board": autre })).await;
        assert!(err && msg.contains("introuvable"), "{msg}");
        // Critère 4 : documentation en Markdown, avec l'ancienne version gardée.
        let (err, txt) = mcp_call(&m.st, &t, "write_page", json!({ "markdown": "# Règles\n\n## Objectif\nCouper toute l'herbe.\n\n- Un joueur\n- [ ] Tester\n\n| Pièce | Prix |\n| --- | --- |\n| Faux | 10 |" })).await;
        assert!(!err, "{txt}");
        let pid = serde_json::from_str::<Value>(&txt).unwrap()["page"]
            .as_i64()
            .unwrap();
        let (err, txt) = mcp_call(
            &m.st,
            &t,
            "write_page",
            json!({ "page": pid, "markdown": "## Objectif\nCouper l'herbe en 5 minutes." }),
        )
        .await;
        assert!(!err, "{txt}");
        let (_, txt) = mcp_call(&m.st, &t, "read_page", json!({ "page": pid })).await;
        assert!(
            txt.contains("# Règles") && txt.contains("5 minutes"),
            "{txt}"
        );
        let versions: Vec<String> =
            sqlx::query_scalar("SELECT blocks FROM page_versions WHERE page_id = ? ORDER BY id")
                .bind(pid)
                .fetch_all(&m.st.db)
                .await
                .unwrap();
        assert!(
            versions.iter().any(|b| b.contains("Couper toute l'herbe")),
            "ancienne version gardée"
        );
        assert!(versions.last().unwrap().contains("5 minutes"));
        let (_, txt) = mcp_call(&m.st, &t, "list_pages", json!({})).await;
        assert!(txt.contains("Règles"));
        // Critère 5 : jalon visible dans le planning.
        let (err, txt) = mcp_call(
            &m.st,
            &t,
            "add_milestone",
            json!({ "title": "Bêta fermée", "date": "2026-12-15", "time": "18:00" }),
        )
        .await;
        assert!(!err, "{txt}");
        let (_, txt) = mcp_call(&m.st, &t, "list_milestones", json!({})).await;
        assert!(
            txt.contains("Bêta fermée") && txt.contains("2026-12-15"),
            "{txt}"
        );
        let (err, msg) = mcp_call(
            &m.st,
            &t,
            "add_milestone",
            json!({ "title": "Sans date", "date": "demain" }),
        )
        .await;
        assert!(err && msg.contains("Date invalide"), "{msg}");
        // Critère 6 : en lecture seule, les écritures sont absentes et refusées ; les lectures marchent.
        let l = jeton(&m, Some(m.equipe), "read").await;
        let outils = outils_visibles(&m.st, &l).await;
        for o in [
            "create_board",
            "upsert_board_objects",
            "delete_board_objects",
            "write_page",
            "add_milestone",
        ] {
            assert!(
                !outils.contains(&format!("\"{o}\"")),
                "{o} visible en lecture seule"
            );
            let (err, msg) = mcp_call(&m.st, &l, o, json!({ "board": bid, "objects": [], "ids": [], "markdown": "x", "title": "x", "date": "2026-12-01" })).await;
            assert!(err && msg.contains("lecture seule"), "{o} : {msg}");
        }
        for o in ["list_boards", "get_board", "list_pages", "list_milestones"] {
            let (err, msg) = mcp_call(&m.st, &l, o, json!({ "board": bid })).await;
            assert!(!err, "{o} : {msg}");
        }
    }

    /// #152, critère 5 : uefn_status d'une map jamais reliée renvoie « unlinked » avec une phrase claire.
    #[tokio::test]
    async fn statut_map_non_reliee() {
        let m = monde().await;
        let t = jeton(&m, Some(m.equipe), "read").await;
        let (err, txt) = mcp_call(&m.st, &t, "uefn_status", json!({})).await;
        assert!(!err, "{txt}");
        let v: Value = serde_json::from_str(&txt).unwrap();
        assert_eq!(v["status"], "unlinked");
        assert!(v["message"].as_str().unwrap().contains("Aucun projet UEFN"));
    }

    /// #298, critère 5 : next_task ne propose pas une tâche dont une dépendance n'est pas faite.
    #[tokio::test]
    async fn next_task_respecte_les_dependances() {
        let m = monde().await;
        let t = jeton(&m, None, "write").await;
        mcp_call(&m.st, &t, "use_map", json!({ "map": m.equipe })).await;
        let (err, txt) = mcp_call(
            &m.st,
            &t,
            "create_task",
            json!({ "title": "Poser les bases" }),
        )
        .await;
        assert!(!err, "{txt}");
        let n1 = serde_json::from_str::<Value>(&txt).unwrap()["number"]
            .as_i64()
            .unwrap();
        let (err, txt) = mcp_call(
            &m.st,
            &t,
            "create_task",
            json!({ "title": "Tester les captures", "depends_on": [n1] }),
        )
        .await;
        assert!(!err, "{txt}");
        let (err, txt) = mcp_call(&m.st, &t, "next_task", json!({})).await;
        assert!(!err, "{txt}");
        assert!(
            txt.contains("Poser les bases") && !txt.contains("Tester les captures"),
            "{txt}"
        );
    }

    #[tokio::test]
    async fn lecture_seule_sur_toutes_les_maps() {
        let m = monde().await;
        let t = jeton(&m, None, "read").await;
        // Choisir une map reste permis, modifier non, sur chaque map.
        for map in [m.perso, m.equipe] {
            let (err, txt) = mcp_call(&m.st, &t, "use_map", json!({ "map": map })).await;
            assert!(!err, "{txt}");
            let (err, msg) = mcp_call(&m.st, &t, "create_task", json!({ "title": "Non" })).await;
            assert!(err && msg.contains("lecture seule"), "{msg}");
            let (err, _) = mcp_call(&m.st, &t, "list_tasks", json!({})).await;
            assert!(!err);
        }
    }

    #[tokio::test]
    async fn revoquer_et_changer_de_map_libere_la_tache() {
        let m = monde().await;
        let t = jeton(&m, Some(m.equipe), "write").await;
        let (err, txt) = mcp_call(&m.st, &t, "claim_task", json!({ "number": 1 })).await;
        assert!(!err, "{txt}");
        mcp_call(&m.st, &t, "use_map", json!({ "map": m.perso })).await;
        let tache: Option<i64> =
            sqlx::query_scalar("SELECT task_id FROM api_tokens WHERE token_hash = ?")
                .bind(sha256_hex(&t))
                .fetch_one(&m.st.db)
                .await
                .unwrap();
        assert_eq!(tache, None, "la tâche de l'ancienne map est libérée");
        // Révoqué : le jeton ne marche plus du tout.
        let u = AuthUser {
            id: m.uid,
            email: "ia@a.fr".into(),
            name: "Yanis".into(),
            sid: String::new(),
        };
        let id: i64 = sqlx::query_scalar("SELECT id FROM api_tokens WHERE token_hash = ?")
            .bind(sha256_hex(&t))
            .fetch_one(&m.st.db)
            .await
            .unwrap();
        delete_token(State(m.st.clone()), u, Path(id))
            .await
            .unwrap();
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {t}").parse().unwrap());
        let r = mcp(
            State(m.st.clone()),
            h,
            axum::extract::Query(McpQ { token: None }),
            Json(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })),
        )
        .await;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    /// La migration 0034 garde un ancien jeton de map (sa map devient la map de travail) et son journal.
    #[tokio::test]
    async fn migration_garde_les_anciens_jetons() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str("sqlite::memory:")
                    .unwrap()
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        let mut avant = sqlx::migrate!("./migrations");
        avant.migrations = std::borrow::Cow::Owned(
            avant
                .migrations
                .iter()
                .filter(|m| m.version < 34)
                .cloned()
                .collect(),
        );
        avant.run(&db).await.unwrap();
        let ex = |sql: &'static str| {
            let db = db.clone();
            async move {
                sqlx::query(sql).execute(&db).await.unwrap();
            }
        };
        ex("INSERT INTO users (id, email, name, password_hash) VALUES (1, 'a@a.fr', 'A', 'x')")
            .await;
        ex("INSERT INTO studios (id, name) VALUES (1, 'S')").await;
        ex("INSERT INTO maps (id, studio_id, name) VALUES (7, 1, 'M')").await;
        ex("INSERT INTO tasks (id, map_id, number, title) VALUES (3, 7, 1, 'T')").await;
        ex("INSERT INTO api_tokens (id, user_id, map_id, name, token_hash, kind, scope, calls, task_id) VALUES (5, 1, 7, 'Vieux', 'h', 'codex', 'read', 9, 3)").await;
        ex("INSERT INTO uefn_actions (id, map_id, user_id, source, action, args, summary, status, agent_id) VALUES (11, 7, 1, 'mcp', 'save', '{}', 'x', 'done', 5)").await;
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        let r: (Option<i64>, String, String, i64, Option<i64>) = sqlx::query_as(
            "SELECT map_id, kind, scope, calls, task_id FROM api_tokens WHERE id = 5",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(r, (Some(7), "codex".into(), "read".into(), 9, Some(3)));
        let agent: Option<i64> =
            sqlx::query_scalar("SELECT agent_id FROM uefn_actions WHERE id = 11")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(agent, Some(5), "le journal garde l'agent");
        // La map supprimée : le jeton reste, sans map de travail ; le journal disparaît avec la map.
        ex("DELETE FROM maps WHERE id = 7").await;
        let m: Option<i64> = sqlx::query_scalar("SELECT map_id FROM api_tokens WHERE id = 5")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(m, None);
        // Supprimer le jeton vide toujours l'agent du journal (clé étrangère vers la nouvelle table).
        ex("INSERT INTO maps (id, studio_id, name) VALUES (8, 1, 'N')").await;
        ex("INSERT INTO uefn_actions (id, map_id, user_id, source, action, args, summary, status, agent_id) VALUES (12, 8, 1, 'mcp', 'save', '{}', 'x', 'done', 5)").await;
        ex("DELETE FROM api_tokens WHERE id = 5").await;
        let agent: Option<i64> =
            sqlx::query_scalar("SELECT agent_id FROM uefn_actions WHERE id = 12")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(agent, None);
    }
}

#[cfg(test)]
mod tests_decor {
    use super::*;
    use crate::uefn::check_mesh_args;

    /// Poser un mesh (#148) : arguments refusés en français avant tout envoi, forme de base traduite en chemin.
    #[test]
    fn mesh_valide_ou_refuse() {
        let mut a =
            json!({ "shape": "plan", "name": "Sol", "location": [0, 0, 0], "scale": [20, 20, 1] });
        assert_eq!(check_mesh_args(&mut a).unwrap(), "Sol (Plane)");
        assert_eq!(a["mesh"], "/Engine/BasicShapes/Plane");
        assert!(a.get("shape").is_none());
        let mut a = json!({ "mesh": "/Projet/POP/Bloc" });
        assert_eq!(check_mesh_args(&mut a).unwrap(), "Bloc");
        for bad in [
            json!({}),
            json!({ "shape": "plan", "mesh": "/Game/X" }),
            json!({ "shape": "pyramide" }),
            json!({ "mesh": "Game/X" }),
            json!({ "shape": "cube", "scale": [0, 1, 1] }),
            json!({ "shape": "cube", "scale": [1, 1, 5000] }),
            json!({ "shape": "cube", "scale": [1, 1] }),
            json!({ "shape": "cube", "location": [0, 0, 3_000_000] }),
            json!({ "shape": "cube", "location": ["a", 0, 0] }),
            json!({ "shape": "cube", "rotation": [0, 9999, 0] }),
        ] {
            let mut b = bad.clone();
            let e = check_mesh_args(&mut b)
                .err()
                .unwrap_or_else(|| panic!("accepté : {bad}"));
            assert!(matches!(e, AppError::BadRequest(_)), "{bad}");
        }
    }

    fn reglages() -> Value {
        json!({ "id": "L.Ile", "settings": [
            { "name": "MaxPlayers", "displayName": "Max Players", "currentValue": "16", "currentLabel": "16",
              "allowedValues": [{ "label": "12", "value": "12" }, { "label": "16", "value": "16" }] },
            { "name": "TeamMode", "displayName": "Teams", "currentValue": "0", "currentLabel": "Free for all",
              "allowedValues": [{ "label": "Free for all", "value": "0" }, { "label": "Teams", "value": "1" }] }
        ] })
    }

    /// Réglages de l'île (#148) : nom inconnu ou valeur non permise refusés, libellé accepté, ancienne valeur gardée.
    #[test]
    fn reglages_de_l_ile() {
        let r = reglages();
        let w = |v: Value| v.as_object().unwrap().clone();
        let (id, opts, cur) =
            island_check(&r, &w(json!({ "MaxPlayers": 12, "TeamMode": "teams" }))).unwrap();
        assert_eq!(id, "L.Ile");
        assert_eq!(opts, json!({ "MaxPlayers": "12", "TeamMode": "1" }));
        assert!(cur.contains(&("MaxPlayers".into(), json!("16"))));
        assert!(island_check(&r, &w(json!({ "MaxPlayers": "17" }))).is_err());
        assert!(island_check(&r, &w(json!({ "Inconnu": "1" }))).is_err());
        assert!(island_check(&r, &w(json!({ "MaxPlayers": [1] }))).is_err());
        assert!(island_check(
            &json!({ "settings": [] }),
            &w(json!({ "MaxPlayers": "12" }))
        )
        .is_err());
        // Vue rendue à l'IA : valeurs permises seulement pour les noms demandés.
        let v = island_view(&r, &[]);
        assert_eq!(v["count"], 2);
        assert!(v["settings"][0].get("allowed").is_none());
        let v = island_view(&r, &["MaxPlayers".into()]);
        assert_eq!(v["settings"][0]["allowed"], json!(["12", "16"]));
    }
}
