//! Garde-fou de compilation et versions du Verse qui compilent (#301). UEFN ne reçoit que du Verse qui compile :
//! l'envoi en session est refusé tant que la dernière compilation a des erreurs, par une IA comme par un humain. À
//! chaque compilation sans erreur, POP garde tous les fichiers Verse de la map ; « Revenir à cette version » les remet
//! par des actions journalisées et annulables (une écriture ou une suppression par fichier), puis recompile.
use crate::{
    access::map_ctx,
    auth::AuthUser,
    error::{ApiResult, AppError},
    uefn::{exec, Origin},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

/// Versions gardées par map.
const GARDE: i64 = 50;

/// Erreurs de la dernière compilation de la map (None : jamais compilée).
pub async fn erreurs_derniere(st: &AppState, mid: i64) -> Result<Option<usize>, AppError> {
    let after: Option<Option<String>> = sqlx::query_scalar(
        "SELECT after FROM uefn_actions WHERE map_id = ? AND action = 'compile' AND status = 'done' ORDER BY id DESC LIMIT 1",
    )
    .bind(mid)
    .fetch_optional(&st.db)
    .await?;
    Ok(after.map(|a| {
        erreurs(
            &a.and_then(|a| serde_json::from_str(&a).ok())
                .unwrap_or(Value::Null),
        )
    }))
}

pub fn erreurs(r: &Value) -> usize {
    r["diagnostics"]
        .as_array()
        .map(|d| d.iter().filter(|x| x["severity"] == "error").count())
        .unwrap_or(0)
}

/// Refuse l'envoi en session si la dernière compilation a échoué.
pub async fn garde_envoi(st: &AppState, mid: i64) -> Result<(), AppError> {
    match erreurs_derniere(st, mid).await? {
        Some(n) if n > 0 => Err(AppError::Conflict(format!(
            "La dernière compilation a {n} erreur(s) : rien n'est envoyé dans la session, UEFN garde la dernière version qui compile. Corrigez puis recompilez (compile_verse), ou revenez à la dernière version qui compilait."
        ))),
        _ => Ok(()),
    }
}

/// Après une compilation sans erreur : garde tous les fichiers Verse de la map.
pub async fn garder(
    st: &AppState,
    mid: i64,
    user: Option<i64>,
    agent: Option<i64>,
) -> Result<(), AppError> {
    let files: Vec<(String, String)> =
        sqlx::query_as("SELECT path, content FROM verse_files WHERE map_id = ? ORDER BY path")
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
    let files = json!(files
        .into_iter()
        .map(|(p, c)| json!({ "path": p, "content": c }))
        .collect::<Vec<_>>());
    // Rien n'a changé depuis la version précédente : pas de doublon.
    let prec: Option<String> = sqlx::query_scalar(
        "SELECT files FROM verse_builds WHERE map_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(mid)
    .fetch_optional(&st.db)
    .await?;
    if prec.as_deref() == Some(files.to_string().as_str()) {
        return Ok(());
    }
    sqlx::query("INSERT INTO verse_builds (map_id, user_id, agent_id, files) VALUES (?, ?, ?, ?)")
        .bind(mid)
        .bind(user)
        .bind(agent)
        .bind(files.to_string())
        .execute(&st.db)
        .await?;
    sqlx::query("DELETE FROM verse_builds WHERE map_id = ? AND id NOT IN (SELECT id FROM verse_builds WHERE map_id = ? ORDER BY id DESC LIMIT ?)")
        .bind(mid)
        .bind(mid)
        .bind(GARDE)
        .execute(&st.db)
        .await?;
    Ok(())
}

/// Les versions qui compilent, la plus récente d'abord, et l'état de la dernière compilation.
pub async fn liste(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT b.id, b.at, us.name, a.name, json_array_length(b.files) FROM verse_builds b LEFT JOIN users us ON us.id = b.user_id
         LEFT JOIN api_tokens a ON a.id = b.agent_id WHERE b.map_id = ? ORDER BY b.id DESC",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "derniere_compilation": erreurs_derniere(&st, mid).await?.map(|n| json!({ "erreurs": n })),
        "versions": rows.into_iter().map(|(id, at, qui, agent, n)| json!({ "id": id, "at": at, "par": agent.or(qui), "fichiers": n })).collect::<Vec<_>>(),
    })))
}

/// Revenir à une version qui compilait : chaque fichier qui diffère est réécrit, chaque fichier ajouté depuis est
/// retiré (actions journalisées et annulables), puis on recompile.
pub async fn restaurer(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (mid, files): (i64, String) =
        sqlx::query_as("SELECT map_id, files FROM verse_builds WHERE id = ?")
            .bind(id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    let voulus: Vec<Value> = serde_json::from_str(&files).unwrap_or_default();
    let actuels: Vec<(String, String)> =
        sqlx::query_as("SELECT path, content FROM verse_files WHERE map_id = ?")
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
    let o = || Origin {
        ctx: &ctx,
        source: "web",
        agent: None,
        task: None,
    };
    let mut faits = 0;
    let mut file = false;
    for f in &voulus {
        let (p, c) = (
            f["path"].as_str().unwrap_or(""),
            f["content"].as_str().unwrap_or(""),
        );
        if actuels.iter().any(|(ap, ac)| ap == p && ac == c) {
            continue;
        }
        let r = exec(&st, o(), "write_file", json!({ "path": p, "content": c })).await?;
        file |= r["queued"].as_bool() == Some(true);
        faits += 1;
    }
    for (p, _) in actuels
        .iter()
        .filter(|(p, _)| !voulus.iter().any(|f| f["path"] == p.as_str()))
    {
        let r = exec(&st, o(), "delete_file", json!({ "path": p })).await?;
        file |= r["queued"].as_bool() == Some(true);
        faits += 1;
    }
    // Sans UEFN, les écritures attendent le relais : on ne compile pas encore.
    let compile = if faits > 0 && !file {
        exec(&st, o(), "compile", json!({})).await.ok()
    } else {
        None
    };
    Ok(Json(
        json!({ "fichiers": faits, "en_attente": file, "compile": compile }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn garde_fou_et_versions() {
        let st = crate::test_state().await;
        let q = |s: &'static str| sqlx::query_scalar::<_, i64>(s);
        let sid = q("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let mid: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let compile = |after: &'static str| {
            let st = st.clone();
            async move {
                sqlx::query("INSERT INTO uefn_actions (map_id, source, action, args, summary, status, after) VALUES (?, 'mcp', 'compile', '{}', 'Compilation Verse', 'done', ?)")
                    .bind(mid).bind(after).execute(&st.db).await.unwrap();
            }
        };
        // Jamais compilée : l'envoi passe ; compilation en échec : refusé ; compilation réussie : il repasse.
        assert!(garde_envoi(&st, mid).await.is_ok());
        compile(r#"{"diagnostics":[{"severity":"error"},{"severity":"warning"}]}"#).await;
        let e = garde_envoi(&st, mid).await.unwrap_err();
        assert!(
            matches!(&e, AppError::Conflict(m) if m.contains("1 erreur(s)")),
            "{e:?}"
        );
        compile(r#"{"diagnostics":[{"severity":"warning"}]}"#).await;
        assert!(garde_envoi(&st, mid).await.is_ok());

        // Une version par compilation réussie, sans doublon, 50 au plus.
        sqlx::query(
            "INSERT INTO verse_files (map_id, path, content, sha) VALUES (?, 'a.verse', 'v1', 'x')",
        )
        .bind(mid)
        .execute(&st.db)
        .await
        .unwrap();
        garder(&st, mid, None, None).await.unwrap();
        garder(&st, mid, None, None).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM verse_builds")
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(n, 1);
        for i in 0..60 {
            sqlx::query("UPDATE verse_files SET content = ? WHERE map_id = ?")
                .bind(format!("v{i}"))
                .bind(mid)
                .execute(&st.db)
                .await
                .unwrap();
            garder(&st, mid, None, None).await.unwrap();
        }
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM verse_builds")
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(n, GARDE);
    }
}
