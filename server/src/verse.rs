//! Éditeur Verse : fichiers du projet, historique, recherche et chaîne « Envoyer à UEFN ».
use crate::{
    access::{log_activity, map_ctx, notify, Role},
    auth::AuthUser,
    error::{bad, ApiResult},
    uefn::{exec, snapshot, valid_path, Origin},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn files(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as("SELECT path, sha, updated_at, length(content) FROM verse_files WHERE map_id = ? ORDER BY path").bind(mid).fetch_all(&st.db).await?;
    let snap = snapshot(&st, mid).await?;
    let queued: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT json_extract(args, '$.path') FROM uefn_actions WHERE map_id = ? AND status = 'queued' AND action = 'write_file'").bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|(p, sha, at, size)| {
        let lock = crate::uefn::locked_by(&snap, &p).map(String::from);
        json!({ "path": p, "sha": sha, "updated_at": at, "size": size, "locked_by": lock, "pending": queued.iter().any(|q| q.0 == p) })
    }).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct PathQ {
    path: String,
}

pub async fn file(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Query(q): Query<PathQ>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let (content, sha): (String, String) =
        sqlx::query_as("SELECT content, sha FROM verse_files WHERE map_id = ? AND path = ?")
            .bind(mid)
            .bind(&q.path)
            .fetch_one(&st.db)
            .await?;
    Ok(Json(
        json!({ "path": q.path, "content": content, "sha": sha }),
    ))
}

pub async fn all_contents(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT path, content FROM verse_files WHERE map_id = ? ORDER BY path")
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(p, c)| json!({"path": p, "content": c}))
        .collect::<Vec<_>>())))
}

pub async fn versions(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Query(q): Query<PathQ>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, Option<String>, Option<i64>, String, String, i64)> = sqlx::query_as(
        "SELECT v.id, u.name, v.task_number, v.note, v.at, length(v.content) FROM verse_versions v LEFT JOIN users u ON u.id = v.user_id WHERE v.map_id = ? AND v.path = ? ORDER BY v.id DESC LIMIT 100",
    ).bind(mid).bind(&q.path).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|(id, who, t, note, at, size)| json!({"id": id, "who": who, "task": t, "note": note, "at": at, "size": size})).collect::<Vec<_>>())))
}

pub async fn version(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (mid, path, content): (i64, String, String) =
        sqlx::query_as("SELECT map_id, path, content FROM verse_versions WHERE id = ?")
            .bind(id)
            .fetch_one(&st.db)
            .await?;
    map_ctx(&st, &u, mid).await?;
    Ok(Json(json!({ "id": id, "path": path, "content": content })))
}

pub async fn symbols(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let (sym, tags): (String, String) =
        sqlx::query_as("SELECT symbols, tags FROM uefn_state WHERE map_id = ?")
            .bind(mid)
            .fetch_optional(&st.db)
            .await?
            .unwrap_or(("[]".into(), "[]".into()));
    let snap = snapshot(&st, mid).await?;
    // Les appareils de la map servent aussi à l'autocomplétion.
    let devices: Vec<Value> = snap
        .actors
        .iter()
        .filter(|a| a["type"] == "device" || a["type"] == "verse_device")
        .map(|a| json!({"name": a["name"], "class": a["class"]}))
        .collect();
    Ok(Json(
        json!({ "symbols": serde_json::from_str::<Value>(&sym).unwrap_or(json!([])), "tags": serde_json::from_str::<Value>(&tags).unwrap_or(json!([])), "devices": devices }),
    ))
}

#[derive(Deserialize)]
pub struct SendFile {
    path: String,
    content: String,
}

#[derive(Deserialize)]
pub struct SendIn {
    files: Vec<SendFile>,
    task: Option<i64>,
    push: bool,
    lore: bool,
    message: Option<String>,
    run: String,
}

fn step(st: &AppState, mid: i64, run: &str, n: u8, status: &str, detail: Value) {
    notify(
        st,
        mid,
        json!({ "t": "verse", "what": "send", "run": run, "step": n, "status": status, "detail": detail }),
    );
}

/// « Envoyer à UEFN » : écriture, compilation, erreurs, session de test, archivage Lore.
pub async fn send(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<SendIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Freelance)?;
    if b.files.len() > 200 {
        return Err(bad("Trop de fichiers."));
    }
    for f in &b.files {
        if !valid_path(&f.path) {
            return Err(bad(format!("Chemin invalide : {}", f.path)));
        }
    }
    let run = b.run.chars().take(32).collect::<String>();
    let o = || Origin {
        ctx: &ctx,
        source: "web",
        agent: None,
        task: b.task,
    };
    // 1. Écriture des fichiers dans le projet.
    step(&st, mid, &run, 1, "running", json!(null));
    let mut queued = false;
    for f in &b.files {
        let r = exec(
            &st,
            o(),
            "write_file",
            json!({ "path": f.path, "content": f.content }),
        )
        .await;
        match r {
            Ok(v) => queued |= v["queued"].as_bool() == Some(true),
            Err(e) => {
                step(&st, mid, &run, 1, "failed", json!(format!("{e}")));
                return Err(e);
            }
        }
    }
    if queued {
        step(
            &st,
            mid,
            &run,
            1,
            "queued",
            json!("Relais hors ligne : écriture en attente."),
        );
        return Ok(Json(
            json!({ "queued": true, "steps": [{ "step": 1, "status": "queued" }] }),
        ));
    }
    step(&st, mid, &run, 1, "done", json!(b.files.len()));
    // 2. Compilation dans UEFN.
    step(&st, mid, &run, 2, "running", json!(null));
    let c = match exec(&st, o(), "compile", json!({})).await {
        Ok(c) if c["queued"].as_bool() == Some(true) => {
            step(
                &st,
                mid,
                &run,
                2,
                "queued",
                json!("UEFN est fermé : compilation en attente."),
            );
            return Ok(Json(json!({ "queued": true })));
        }
        Ok(c) => c,
        Err(e) => {
            step(&st, mid, &run, 2, "failed", json!(format!("{e}")));
            return Err(e);
        }
    };
    let diags = c["diagnostics"].clone();
    let errors = diags
        .as_array()
        .map(|d| d.iter().filter(|x| x["severity"] == "error").count())
        .unwrap_or(0);
    step(
        &st,
        mid,
        &run,
        2,
        if errors == 0 { "done" } else { "failed" },
        json!({ "errors": errors }),
    );
    // 3. Erreurs et avertissements renvoyés.
    step(&st, mid, &run, 3, "done", diags.clone());
    let mut out =
        json!({ "diagnostics": diags, "errors": errors, "pushed": false, "archived": false });
    if errors > 0 {
        step(
            &st,
            mid,
            &run,
            4,
            "skipped",
            json!("Corrigez les erreurs pour envoyer en session."),
        );
        step(
            &st,
            mid,
            &run,
            5,
            "skipped",
            json!("Corrigez les erreurs pour archiver."),
        );
        return Ok(Json(out));
    }
    // 4. Session de test.
    if b.push {
        step(&st, mid, &run, 4, "running", json!(null));
        // Seul le Verse a changé : envoi rapide sans recuisson des assets (#5).
        match exec(&st, o(), "push_session", json!({ "verse_only": true })).await {
            Ok(_) => {
                step(&st, mid, &run, 4, "done", json!(null));
                out["pushed"] = json!(true);
            }
            Err(e) => {
                step(&st, mid, &run, 4, "failed", json!(format!("{e}")));
                out["push_error"] = json!(format!("{e}"));
            }
        }
    } else {
        step(&st, mid, &run, 4, "skipped", json!(null));
    }
    // 5. Archivage Lore.
    if b.lore {
        let msg = b
            .message
            .clone()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| "Mise à jour du Verse".into());
        let msg = match b.task {
            Some(n) if !msg.contains(&format!("#{n}")) => format!("#{n} {msg}"),
            _ => msg,
        };
        step(&st, mid, &run, 5, "running", json!(null));
        match exec(&st, o(), "lore_checkin", json!({ "message": msg, "paths": b.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>() })).await {
            Ok(v) if v["queued"].as_bool() == Some(true) => { step(&st, mid, &run, 5, "queued", json!("UEFN est fermé : archivage en attente.")); }
            Ok(_) => { step(&st, mid, &run, 5, "done", json!(null)); out["archived"] = json!(true); }
            Err(e) => { step(&st, mid, &run, 5, "failed", json!(format!("{e}"))); out["lore_error"] = json!(format!("{e}")); }
        }
    } else {
        step(&st, mid, &run, 5, "skipped", json!(null));
    }
    log_activity(
        &st,
        mid,
        Some(u.id),
        "checkin",
        "Verse envoyé à UEFN",
        &format!(
            "{} fichier(s){}",
            b.files.len(),
            b.task.map(|t| format!(" · #{t}")).unwrap_or_default()
        ),
    )
    .await?;
    Ok(Json(out))
}

/// Revient à une version : la réécrit dans le projet puis la renvoie à UEFN si possible.
pub async fn revert(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (mid, path, content): (i64, String, String) =
        sqlx::query_as("SELECT map_id, path, content FROM verse_versions WHERE id = ?")
            .bind(id)
            .fetch_one(&st.db)
            .await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    let r = exec(
        &st,
        Origin {
            ctx: &ctx,
            source: "web",
            agent: None,
            task: None,
        },
        "write_file",
        json!({ "path": path, "content": content }),
    )
    .await?;
    let compiled = if r["queued"].as_bool() != Some(true) {
        exec(
            &st,
            Origin {
                ctx: &ctx,
                source: "web",
                agent: None,
                task: None,
            },
            "compile",
            json!({}),
        )
        .await
        .ok()
    } else {
        None
    };
    Ok(Json(
        json!({ "ok": true, "queued": r["queued"], "compile": compiled }),
    ))
}
