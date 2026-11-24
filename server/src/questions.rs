//! Parcours de questions-boutons (#135) : le créateur répond par des clics, une question à la fois. Les questions
//! viennent de POP (cadrage de la map, posées une fois par map) ou de son IA (outil MCP `ask_user`, réponse lue
//! avec `get_answer`). Répondre demande le droit d'édition de la map ; un lecteur voit les questions sans répondre.
use crate::{
    access::{map_ctx, notify, MapCtx},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Questions de cadrage de POP : clé, question, choix (2 à 5). « Autre » (texte libre) est toujours proposé.
pub const CADRAGE: &[(&str, &str, &[&str])] = &[
    (
        "genre",
        "Quel genre de map ?",
        &[
            "Tycoon",
            "Combat (PvP)",
            "Parkour / Deathrun",
            "Horreur",
            "Simulateur / Roleplay",
        ],
    ),
    (
        "joueurs",
        "Combien de joueurs par partie ?",
        &["1 à 4", "5 à 16", "17 à 50", "Plus de 50"],
    ),
    (
        "taille",
        "Quelle taille pour l'île ?",
        &[
            "Petite (une zone)",
            "Moyenne (quelques zones)",
            "Grande (monde ouvert)",
        ],
    ),
    (
        "duree",
        "Combien de temps dure une partie ?",
        &[
            "Moins de 5 min",
            "5 à 20 min",
            "Plus de 20 min",
            "Sans fin (on revient quand on veut)",
        ],
    ),
    (
        "ambiance",
        "Quelle ambiance ?",
        &["Fun et coloré", "Réaliste", "Sombre", "Futuriste"],
    ),
    (
        "equipe",
        "Vous la faites seul ou à plusieurs ?",
        &["Seul, avec mon IA", "À plusieurs, en studio"],
    ),
];

/// Ajoute les questions de cadrage qui manquent à la map (une fois par map, dans l'ordre).
async fn semer(st: &AppState, mid: i64) -> Result<(), AppError> {
    for (i, (cle, q, choix)) in CADRAGE.iter().enumerate() {
        sqlx::query("INSERT OR IGNORE INTO map_questions (map_id, cle, auteur, question, choix, ordre) VALUES (?, ?, 'pop', ?, ?, ?)")
            .bind(mid)
            .bind(cle)
            .bind(q)
            .bind(json!(choix).to_string())
            .bind(i as i64)
            .execute(&st.db)
            .await?;
    }
    Ok(())
}

/// Les questions de la map : celles de l'IA d'abord (les plus anciennes en tête), puis le cadrage dans l'ordre.
pub async fn liste(st: &AppState, mid: i64) -> Result<Vec<Value>, AppError> {
    semer(st, mid).await?;
    let rows: Vec<(i64, Option<String>, String, Option<String>, String, String, bool, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT q.id, q.cle, q.auteur, a.name, q.question, q.choix, q.autre, q.reponse, u.name, q.repondu_le
         FROM map_questions q LEFT JOIN api_tokens a ON a.id = q.agent_id LEFT JOIN users u ON u.id = q.repondu_par
         WHERE q.map_id = ? ORDER BY q.auteur = 'pop', q.ordre, q.id",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.0, "key": r.1, "author": r.2, "agent": r.3, "question": r.4,
                "choices": serde_json::from_str::<Value>(&r.5).unwrap_or(json!([])), "other": r.6,
                "answer": r.7, "answered_by": r.8, "answered_at": r.9,
            })
        })
        .collect())
}

/// `GET /api/maps/{mid}/questions`
pub async fn route_liste(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    Ok(Json(
        json!({ "questions": liste(&st, mid).await?, "can_answer": ctx.can_edit().is_ok() }),
    ))
}

#[derive(Deserialize)]
pub struct Reponse {
    answer: String,
}

/// Enregistre une réponse (ou la change). Le genre répondu remplit aussi le genre de la map s'il est vide.
pub async fn repondre(st: &AppState, ctx: &MapCtx, qid: i64, rep: &str) -> Result<Value, AppError> {
    ctx.can_edit()?;
    let rep = rep.trim();
    check_len("Réponse", rep, 1, 200)?;
    let (cle, choix, autre): (Option<String>, String, bool) =
        sqlx::query_as("SELECT cle, choix, autre FROM map_questions WHERE id = ? AND map_id = ?")
            .bind(qid)
            .bind(ctx.map_id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    let choix: Vec<String> = serde_json::from_str(&choix).unwrap_or_default();
    if !autre && !choix.iter().any(|c| c == rep) {
        return Err(bad("Choisissez une des réponses proposées."));
    }
    sqlx::query("UPDATE map_questions SET reponse = ?, repondu_par = ?, repondu_le = datetime('now') WHERE id = ?")
        .bind(rep)
        .bind(ctx.user.id)
        .bind(qid)
        .execute(&st.db)
        .await?;
    if cle.as_deref() == Some("genre") {
        sqlx::query("UPDATE maps SET genre = ? WHERE id = ? AND genre = ''")
            .bind(rep)
            .bind(ctx.map_id)
            .execute(&st.db)
            .await?;
    }
    notify(
        st,
        ctx.map_id,
        json!({ "t": "refresh", "what": "questions" }),
    );
    Ok(json!({ "ok": true }))
}

/// `POST /api/maps/{mid}/questions/{qid}`
pub async fn route_repondre(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, qid)): Path<(i64, i64)>,
    Json(b): Json<Reponse>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    Ok(Json(repondre(&st, &ctx, qid, &b.answer).await?))
}

/// Outil MCP `ask_user` : une question et 2 à 5 choix, affichés en boutons dans POP, en direct.
pub async fn ask_user(st: &AppState, mid: i64, agent: i64, a: &Value) -> Result<Value, AppError> {
    let q = a["question"].as_str().unwrap_or("").trim();
    check_len("Question", q, 3, 300)?;
    let choix: Vec<String> = a["choices"]
        .as_array()
        .map(|c| {
            c.iter()
                .filter_map(|x| x.as_str())
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if !(2..=5).contains(&choix.len()) {
        return Err(bad("Donnez de 2 à 5 choix courts (choices)."));
    }
    for c in &choix {
        check_len("Choix", c, 1, 80)?;
    }
    let autre = a["allow_other"].as_bool().unwrap_or(true);
    let id: i64 = sqlx::query_scalar("INSERT INTO map_questions (map_id, auteur, agent_id, question, choix, autre) VALUES (?, 'ia', ?, ?, ?, ?) RETURNING id")
        .bind(mid)
        .bind(agent)
        .bind(q)
        .bind(json!(choix).to_string())
        .bind(autre)
        .fetch_one(&st.db)
        .await?;
    notify(st, mid, json!({ "t": "refresh", "what": "questions" }));
    Ok(json!({
        "question_id": id,
        "message": "Question affichée dans POP. Lisez la réponse avec get_answer { \"question_id\": … } ; en attendant, continuez sur autre chose.",
    }))
}

/// Outil MCP `get_answer` : la réponse à une question de cette map (null tant que personne n'a répondu).
pub async fn get_answer(st: &AppState, mid: i64, a: &Value) -> Result<Value, AppError> {
    let qid = a["question_id"]
        .as_i64()
        .ok_or_else(|| bad("question_id attendu (rendu par ask_user)."))?;
    let r: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT q.question, q.reponse, u.name, q.repondu_le FROM map_questions q LEFT JOIN users u ON u.id = q.repondu_par
         WHERE q.id = ? AND q.map_id = ?",
    )
    .bind(qid)
    .bind(mid)
    .fetch_optional(&st.db)
    .await?;
    let (question, reponse, par, le) =
        r.ok_or_else(|| bad("Question introuvable sur votre map de travail."))?;
    Ok(match reponse {
        Some(rep) => {
            json!({ "question": question, "answer": rep, "answered_by": par, "answered_at": le })
        }
        None => {
            json!({ "question": question, "answer": null, "message": "Pas encore de réponse : redemandez plus tard." })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn monde() -> (AppState, i64, i64, AuthUser, AuthUser) {
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
        let mut users = vec![];
        for (mail, role) in [("e@a.fr", "member"), ("v@a.fr", "viewer")] {
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
            users.push(AuthUser {
                id,
                email: mail.into(),
                name: mail.into(),
                sid: String::new(),
            });
        }
        let ag: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, map_id, name, token_hash, kind) VALUES (?, ?, 'Claude', 'h', 'claude_code') RETURNING id").bind(users[0].id).bind(mid).fetch_one(&st.db).await.unwrap();
        (st, mid, ag, users[0].clone(), users[1].clone())
    }

    #[tokio::test]
    async fn parcours() {
        let (st, mid, ag, editeur, lecteur) = monde().await;
        let l = liste(&st, mid).await.unwrap();
        assert_eq!(l.len(), CADRAGE.len());
        assert_eq!(
            liste(&st, mid).await.unwrap().len(),
            CADRAGE.len(),
            "semées une seule fois"
        );
        let genre = l[0]["id"].as_i64().unwrap();
        let ctx = map_ctx(&st, &editeur, mid).await.unwrap();
        repondre(&st, &ctx, genre, "Tycoon").await.unwrap();
        let g: String = sqlx::query_scalar("SELECT genre FROM maps WHERE id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(g, "Tycoon", "le genre de la map est rempli");
        // Réponse changée ensuite, texte libre (« Autre »).
        repondre(&st, &ctx, genre, "Course de voitures")
            .await
            .unwrap();
        assert_eq!(
            liste(&st, mid).await.unwrap()[0]["answer"],
            "Course de voitures"
        );
        // Lecteur : voit, ne répond pas.
        let lctx = map_ctx(&st, &lecteur, mid).await.unwrap();
        assert!(matches!(
            repondre(&st, &lctx, genre, "Horreur").await,
            Err(AppError::Forbidden)
        ));

        // Question de l'IA : en tête, choix imposés si allow_other = false, réponse lue par get_answer.
        assert!(ask_user(
            &st,
            mid,
            ag,
            &json!({ "question": "Quel HUD ?", "choices": ["Un"] })
        )
        .await
        .is_err());
        let r = ask_user(&st, mid, ag, &json!({ "question": "Le score s'affiche où ?", "choices": ["En haut", "En bas"], "allow_other": false })).await.unwrap();
        let qid = r["question_id"].as_i64().unwrap();
        let l = liste(&st, mid).await.unwrap();
        assert_eq!(l[0]["id"], qid);
        assert_eq!(l[0]["agent"], "Claude");
        assert_eq!(
            get_answer(&st, mid, &json!({ "question_id": qid }))
                .await
                .unwrap()["answer"],
            Value::Null
        );
        assert!(
            repondre(&st, &ctx, qid, "À gauche").await.is_err(),
            "hors des choix"
        );
        repondre(&st, &ctx, qid, "En haut").await.unwrap();
        let a = get_answer(&st, mid, &json!({ "question_id": qid }))
            .await
            .unwrap();
        assert_eq!(a["answer"], "En haut");
        assert_eq!(a["answered_by"], "e@a.fr");
        // Une question d'une autre map n'est pas lisible.
        assert!(get_answer(&st, mid + 1, &json!({ "question_id": qid }))
            .await
            .is_err());
    }
}
