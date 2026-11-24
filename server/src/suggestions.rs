//! Premiers pas d'une map neuve avec son IA connectée (#146) : « Démarrer le document de design », « Démarrer le
//! layout 2D », « Démarrer le brainstorming », « Définir la boucle de gameplay ». Un clic crée le vrai document (page
//! ou tableau blanc depuis les modèles existants) et une tâche liée, donnée à l'IA de la map, avec la consigne et le
//! lien vers le document. L'état suit les documents réels de la map : rien n'est stocké à part.
use crate::{
    access::{map_ctx, MapCtx, Role},
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    tasks::{insert_task, NewTask},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

/// Clé, libellé du bouton, sorte de document, modèle, titre de la tâche, ce que l'IA doit remplir.
const LISTE: &[(&str, &str, &str, &str, &str, &str)] = &[
    (
        "design",
        "Démarrer le document de design",
        "page",
        "gdd",
        "Remplir le document de design",
        "chaque section (pitch, boucle, joueurs, progression…)",
    ),
    (
        "layout",
        "Démarrer le layout 2D",
        "board",
        "plan",
        "Dessiner le layout 2D de la map",
        "les zones, le parcours du joueur et les points d'apparition",
    ),
    (
        "brainstorm",
        "Démarrer le brainstorming",
        "board",
        "brainstorm",
        "Lancer le brainstorming d'idées",
        "des idées en notes, regroupées par thème",
    ),
    (
        "boucle",
        "Définir la boucle de gameplay",
        "board",
        "loop",
        "Définir la boucle de gameplay",
        "chaque étape de la boucle et ce qui pousse à recommencer",
    ),
];

fn titre_du_modele(sorte: &str, modele: &str) -> &'static str {
    if sorte == "page" {
        crate::pages::template(modele).map(|t| t.0).unwrap_or("")
    } else {
        crate::boards::board_template(modele)
            .map(|t| t.0)
            .unwrap_or("")
    }
}

/// Le document déjà créé pour cette suggestion (page ou tableau du même titre, hors corbeille).
async fn existant(
    st: &AppState,
    mid: i64,
    sorte: &str,
    titre: &str,
) -> Result<Option<i64>, AppError> {
    let sql = if sorte == "page" {
        "SELECT id FROM pages WHERE map_id = ? AND title = ? AND deleted_at IS NULL ORDER BY id LIMIT 1"
    } else {
        "SELECT id FROM boards WHERE map_id = ? AND title = ? AND deleted_at IS NULL ORDER BY id LIMIT 1"
    };
    Ok(sqlx::query_scalar(sql)
        .bind(mid)
        .bind(titre)
        .fetch_optional(&st.db)
        .await?)
}

/// L'IA qui travaille sur la map (la plus récemment active).
async fn ia_de_la_map(st: &AppState, mid: i64) -> Result<Option<(i64, String)>, AppError> {
    Ok(sqlx::query_as(
        "SELECT id, name FROM api_tokens WHERE map_id = ? ORDER BY last_used IS NULL, last_used DESC, id DESC LIMIT 1",
    )
    .bind(mid)
    .fetch_optional(&st.db)
    .await?)
}

/// `GET /api/maps/{mid}/suggestions`
pub async fn liste(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let ia = ia_de_la_map(&st, mid).await?;
    let mut items = vec![];
    for (cle, libelle, sorte, modele, _, _) in LISTE {
        let id = existant(&st, mid, sorte, titre_du_modele(sorte, modele)).await?;
        items.push(
            json!({ "key": cle, "label": libelle, "kind": sorte, "done": id.is_some(), "id": id }),
        );
    }
    Ok(Json(
        json!({ "ai": ia.map(|a| a.1), "can_start": ctx.role >= Role::Member, "items": items }),
    ))
}

pub async fn demarrer(st: &AppState, ctx: &MapCtx, cle: &str) -> Result<Value, AppError> {
    ctx.require(Role::Member)?;
    let (_, _, sorte, modele, titre_tache, quoi) = LISTE
        .iter()
        .find(|x| x.0 == cle)
        .copied()
        .ok_or_else(|| bad("Suggestion inconnue."))?;
    let titre = titre_du_modele(sorte, modele);
    // Déjà fait : on rouvre le document, sans doublon (ni document ni tâche).
    if let Some(id) = existant(st, ctx.map_id, sorte, titre).await? {
        return Ok(json!({ "kind": sorte, "id": id, "created": false }));
    }
    let id = if sorte == "page" {
        let b = serde_json::from_value(json!({ "template": modele }))
            .map_err(|e| bad(e.to_string()))?;
        crate::pages::create(
            State(st.clone()),
            ctx.user.clone(),
            Path(ctx.map_id),
            Json(b),
        )
        .await?
        .0["id"]
            .as_i64()
    } else {
        let b = serde_json::from_value(json!({ "template": modele }))
            .map_err(|e| bad(e.to_string()))?;
        crate::boards::create(
            State(st.clone()),
            ctx.user.clone(),
            Path(ctx.map_id),
            Json(b),
        )
        .await?
        .0["id"]
            .as_i64()
    }
    .ok_or_else(|| bad("Document non créé."))?;
    let (outil_lire, outil_ecrire, arg, lien) = if sorte == "page" {
        (
            "read_page",
            "write_page",
            "page",
            format!("/map/{}/pages/{id}", ctx.map_id),
        )
    } else {
        (
            "get_board",
            "upsert_board_objects",
            "board",
            format!("/map/{}/tableau/{id}", ctx.map_id),
        )
    };
    let consigne = format!(
        "Document à remplir : « {titre} » ({} n° {id}, {lien}). Lisez-le avec {outil_lire} {{ \"{arg}\": {id} }}, puis remplissez {quoi} avec {outil_ecrire}. Partez des réponses de cadrage de la map ; quand une décision revient au créateur, posez-lui la question avec ask_user plutôt que d'inventer.",
        if sorte == "page" { "page" } else { "tableau blanc" }
    );
    let ia = ia_de_la_map(st, ctx.map_id).await?;
    let (tid, numero) = insert_task(
        st,
        ctx,
        &NewTask {
            title: titre_tache.into(),
            tag: Some("conception".into()),
            description: Some(consigne),
            assignee_id: None,
            deps: None,
            planned_day: None,
            lot: None,
            status: None,
            ai: Some(true),
            origin: Some("suggestion".into()),
            uefn: None,
        },
    )
    .await?;
    if let Some((ag, _)) = &ia {
        sqlx::query("UPDATE tasks SET agent_id = ? WHERE id = ?")
            .bind(ag)
            .bind(tid)
            .execute(&st.db)
            .await?;
    }
    crate::access::notify(st, ctx.map_id, json!({ "t": "refresh", "what": "tasks" }));
    Ok(json!({ "kind": sorte, "id": id, "created": true, "task": numero, "ai": ia.map(|a| a.1) }))
}

/// `POST /api/maps/{mid}/suggestions/{cle}` : crée le document et la tâche de l'IA, ou rouvre le document existant.
pub async fn route_demarrer(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, cle)): Path<(i64, String)>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    Ok(Json(demarrer(&st, &ctx, &cle).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn premiers_pas() {
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
        let mut us = vec![];
        for (mail, role) in [("m@a.fr", "member"), ("v@a.fr", "viewer")] {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
            )
            .bind(mail)
            .bind(mail)
            .fetch_one(&st.db)
            .await
            .unwrap();
            sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, ?)")
                .bind(sid)
                .bind(id)
                .bind(role)
                .execute(&st.db)
                .await
                .unwrap();
            us.push(AuthUser {
                id,
                email: mail.into(),
                name: mail.into(),
                sid: String::new(),
            });
        }
        // Sans IA : ai null (l'interface n'affiche rien).
        let Json(l) = liste(State(st.clone()), us[0].clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(l["ai"], Value::Null);
        assert_eq!(l["items"].as_array().unwrap().len(), 4);
        let ag: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, map_id, name, token_hash, kind, last_used) VALUES (?, ?, 'Claude', 'h', 'claude_code', datetime('now')) RETURNING id").bind(us[0].id).bind(mid).fetch_one(&st.db).await.unwrap();
        let Json(l) = liste(State(st.clone()), us[0].clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(l["ai"], "Claude");

        let ctx = map_ctx(&st, &us[0], mid).await.unwrap();
        for cle in ["design", "layout", "brainstorm", "boucle"] {
            let r = demarrer(&st, &ctx, cle).await.unwrap();
            assert_eq!(r["created"], true, "{r}");
            // Deuxième clic : même document, aucune tâche de plus.
            let r2 = demarrer(&st, &ctx, cle).await.unwrap();
            assert_eq!(
                (r2["created"].clone(), r2["id"].clone()),
                (json!(false), r["id"].clone())
            );
        }
        let pages: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pages WHERE map_id = ? AND title = 'Document de conception'",
        )
        .bind(mid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        let tableaux: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM boards WHERE map_id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await
            .unwrap();
        let taches: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ? AND agent_id = ?")
                .bind(mid)
                .bind(ag)
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!((pages, tableaux, taches), (1, 3, 4));
        let Json(l) = liste(State(st.clone()), us[0].clone(), Path(mid))
            .await
            .unwrap();
        assert!(l["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|x| x["done"] == true));
        // L'IA reçoit ses tâches avec la consigne et le lien.
        let nt = crate::agents::pour_next_task(&st, ag, mid).await.unwrap();
        let a = &nt["attribuees"][0];
        assert!(
            a["description"].as_str().unwrap().contains("read_page")
                && a["description"].as_str().unwrap().contains("/pages/"),
            "{nt}"
        );
        // Lecteur : voit, ne lance pas.
        let vctx = map_ctx(&st, &us[1], mid).await.unwrap();
        assert!(matches!(
            demarrer(&st, &vctx, "design").await,
            Err(AppError::Forbidden)
        ));
        let Json(l) = liste(State(st.clone()), us[1].clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(l["can_start"], false);
    }
}
