//! Assistant de l'éditeur Verse : moteur local de POP (engine.rs), sans service externe. Les propositions reviennent en fichier complet,
//! affiché en diff côté navigateur et accepté ligne par ligne.
use crate::{
    access::{log_activity, map_ctx, notify, Role},
    auth::AuthUser,
    engine,
    error::{bad, ApiResult},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct AiIn {
    mode: String, // explain | fix | write | review
    path: String,
    content: String,
    selection: Option<String>,
    diagnostics: Option<Value>,
    task: Option<i64>,
    document: Option<bool>,
}

fn blocks_text(blocks: &str) -> String {
    let v: Value = serde_json::from_str(blocks).unwrap_or(json!([]));
    v.as_array().cloned().unwrap_or_default().iter().map(|b| {
        let mut s = b["text"].as_str().unwrap_or("").to_string();
        if let Some(rows) = b["rows"].as_array() {
            s.push_str(&rows.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("\n"));
        }
        s
    }).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n")
}

pub async fn verse_ai(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<AiIn>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    if b.content.len() > 1_000_000 {
        return Err(bad("Fichier trop gros pour l’assistant."));
    }
    // Règles et conventions du projet (page « Règles et conventions »).
    let rules: Vec<(String, String)> = sqlx::query_as("SELECT title, blocks FROM pages WHERE map_id = ? AND (title LIKE '%gles%' OR title LIKE '%onvention%') AND share <> 'private'").bind(mid).fetch_all(&st.db).await?;
    let conventions = rules.iter().map(|(t, bl)| format!("{t}\n{}", blocks_text(bl))).collect::<Vec<_>>().join("\n");
    let (sym, tags, actors): (String, String, String) = sqlx::query_as("SELECT symbols, tags, actors FROM uefn_state WHERE map_id = ?").bind(mid).fetch_optional(&st.db).await?.unwrap_or(("[]".into(), "[]".into(), "[]".into()));
    let actors: Vec<Value> = serde_json::from_str(&actors).unwrap_or_default();
    let project = engine::Project {
        symbols: serde_json::from_str(&sym).unwrap_or_default(),
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        devices: actors.iter().filter(|a| a["type"] == "device" || a["type"] == "verse_device").map(|a| (a["name"].as_str().unwrap_or("").to_string(), a["class"].as_str().unwrap_or("").to_string())).collect(),
    };
    let sel = b.selection.clone().filter(|s| !s.trim().is_empty());
    let (explanation, code) = match b.mode.as_str() {
        "explain" => (engine::explain(&b.path, &b.content, sel.as_deref(), &project), None),
        "fix" => {
            let diags = b.diagnostics.clone().and_then(|d| d.as_array().cloned()).unwrap_or_default();
            let (code, notes) = engine::fix(&b.content, &diags, &project);
            if notes.is_empty() { ("Aucune correction automatique trouvée. Ouvrez l’aide de l’erreur (onglet Aide) pour la corriger à la main.".to_string(), None) }
            else { (format!("{} correction(s) proposée(s) :\n{}", notes.len(), notes.iter().map(|n| format!("- {n}")).collect::<Vec<_>>().join("\n")), Some(code)) }
        }
        "write" => {
            let task = match b.task {
                Some(n) => sqlx::query_as::<_, (String, String)>("SELECT title, description FROM tasks WHERE map_id = ? AND number = ?").bind(mid).bind(n).fetch_optional(&st.db).await?.map(|(t, d)| (n, t, d)),
                None => None,
            };
            let (code, expl) = engine::write(&b.path, &b.content, task, &project, &conventions);
            (expl, Some(code))
        }
        "review" => { let (code, expl) = engine::review(&b.path, &b.content, &project); (expl, Some(code)) }
        _ => return Err(bad("Mode inconnu.")),
    };
    // Relecture : la documentation va dans une page du projet.
    let mut page_id = None;
    if b.mode == "review" && b.document.unwrap_or(false) {
        let blocks = json!([{ "id": "doc", "type": "p", "text": explanation }, { "id": "code", "type": "code", "lang": "verse", "text": code.clone().unwrap_or_default() }]);
        let pid: i64 = sqlx::query_scalar("INSERT INTO pages (map_id, title, icon, blocks, created_by, updated_by) VALUES (?, ?, '{}', ?, ?, ?) RETURNING id")
            .bind(mid).bind(format!("Doc · {}", b.path.rsplit('/').next().unwrap_or(&b.path))).bind(blocks.to_string()).bind(u.id).bind(u.id).fetch_one(&st.db).await?;
        notify(&st, mid, json!({ "t": "refresh", "what": "pages" }));
        log_activity(&st, mid, Some(u.id), "page", "Documentation Verse générée", &b.path).await?;
        page_id = Some(pid);
    }
    Ok(Json(json!({ "explanation": explanation, "code": code, "page_id": page_id })))
}
