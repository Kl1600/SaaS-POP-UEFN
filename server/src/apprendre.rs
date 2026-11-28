//! Bibliothèque qui apprend de chaque map (#120) : garder ce qui a marché pour la map suivante.
//! - Les fichiers Verse écrits pendant une tâche finie sont proposés à la bibliothèque du studio (aperçu, un clic).
//! - Une IA propose une leçon (« ce qui a marché, ce qui a échoué ») ; un membre la valide et elle devient une
//!   compétence du studio, lue par les IA suivantes (get_skill).
//! Rien n'est ajouté sans la validation d'un membre ; un invité ou un freelance ne valide rien.
use crate::{
    access::{map_ctx, Role},
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

/// Fichiers Verse écrits par une action réussie pendant une tâche finie, encore présents dans le projet, ni déjà
/// gardés dans la bibliothèque, ni écartés.
async fn verse_proposes(st: &AppState, mid: i64, sid: i64) -> Result<Vec<Value>, AppError> {
    let rows: Vec<(String, i64, String, String)> = sqlx::query_as(
        "SELECT f.path, MAX(t.number), MAX(t.title), f.content
         FROM uefn_actions a
         JOIN tasks t ON t.map_id = a.map_id AND t.number = a.task_number AND t.status = 'done'
         JOIN verse_files f ON f.map_id = a.map_id AND f.path = json_extract(a.args, '$.path')
         WHERE a.map_id = ? AND a.action = 'write_file' AND a.status = 'done'
           AND f.path NOT IN (SELECT path FROM bibliotheque_ignores WHERE map_id = ?)
           AND NOT EXISTS (SELECT 1 FROM verse_modules m WHERE m.studio_id = ? AND m.source_map_id = ? AND m.source_path = f.path AND m.content = f.content)
         GROUP BY f.path ORDER BY MAX(a.id) DESC LIMIT 30",
    )
    .bind(mid)
    .bind(mid)
    .bind(sid)
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(path, n, titre, c)| {
            json!({
                "path": path, "tache": n, "titre_tache": titre, "lignes": c.lines().count(),
                "apercu": c.lines().take(30).collect::<Vec<_>>().join("\n"),
            })
        })
        .collect())
}

/// `GET /api/maps/{mid}/apprendre` : ce qui attend une décision (fichiers Verse et leçons des IA).
pub async fn liste(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let lecons: Vec<(i64, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT l.id, l.titre, l.contenu, t.name, l.created_at FROM lecons l LEFT JOIN api_tokens t ON t.id = l.agent_id
         WHERE l.studio_id = ? AND l.statut = 'propose' ORDER BY l.id DESC LIMIT 30",
    )
    .bind(ctx.studio_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "peut_valider": ctx.role >= Role::Member,
        "verse": verse_proposes(&st, mid, ctx.studio_id).await?,
        "lecons": lecons.into_iter().map(|(id, titre, contenu, agent, at)| json!({ "id": id, "titre": titre, "contenu": contenu, "agent": agent, "created_at": at })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct VerseIn {
    path: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// Nom du module d'après le fichier : « jeu/minuteur_manche.verse » → « minuteur_manche ».
fn nom_de(path: &str) -> String {
    let f = path.rsplit(['/', '\\']).next().unwrap_or(path);
    f.strip_suffix(".verse")
        .unwrap_or(f)
        .chars()
        .take(80)
        .collect()
}

/// `POST /api/maps/{mid}/apprendre/verse` : ajoute le fichier (contenu actuel) à la bibliothèque du studio. Un même
/// fichier déjà gardé est mis à jour (nouvelle version).
pub async fn ajouter_verse(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<VerseIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    let contenu: String =
        sqlx::query_scalar("SELECT content FROM verse_files WHERE map_id = ? AND path = ?")
            .bind(mid)
            .bind(&b.path)
            .fetch_optional(&st.db)
            .await?
            .ok_or_else(|| bad("Ce fichier n'est plus dans le projet."))?;
    let nom = b
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(String::from)
        .unwrap_or_else(|| nom_de(&b.path));
    check_len("Nom", &nom, 1, 80)?;
    let desc = b.description.unwrap_or_default();
    check_len("Description", &desc, 0, 1000)?;
    let carte: String = sqlx::query_scalar("SELECT name FROM maps WHERE id = ?")
        .bind(mid)
        .fetch_one(&st.db)
        .await?;
    let desc = if desc.trim().is_empty() {
        format!("Écrit pour la map « {carte} » ({}).", b.path)
    } else {
        desc
    };
    let deja: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM verse_modules WHERE studio_id = ? AND (name = ? OR (source_map_id = ? AND source_path = ?))",
    )
    .bind(ctx.studio_id)
    .bind(&nom)
    .bind(mid)
    .bind(&b.path)
    .fetch_optional(&st.db)
    .await?;
    let id = if let Some(id) = deja {
        sqlx::query("UPDATE verse_modules SET content = ?, source_map_id = ?, source_path = ?, author_id = ?, version = version + 1, updated_at = datetime('now') WHERE id = ?")
            .bind(&contenu).bind(mid).bind(&b.path).bind(u.id).bind(id).execute(&st.db).await?;
        id
    } else {
        sqlx::query_scalar("INSERT INTO verse_modules (studio_id, name, category, description, content, author_id, source_map_id, source_path) VALUES (?, ?, 'autre', ?, ?, ?, ?, ?) RETURNING id")
            .bind(ctx.studio_id).bind(&nom).bind(desc.trim()).bind(&contenu).bind(u.id).bind(mid).bind(&b.path)
            .fetch_one(&st.db).await?
    };
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "apprendre" }));
    Ok(Json(
        json!({ "id": id, "name": nom, "mis_a_jour": deja.is_some() }),
    ))
}

#[derive(Deserialize)]
pub struct CheminIn {
    path: String,
}

/// `POST /api/maps/{mid}/apprendre/verse/ignorer` : ce fichier ne sera plus proposé.
pub async fn ignorer_verse(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<CheminIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    sqlx::query("INSERT OR IGNORE INTO bibliotheque_ignores (map_id, path) VALUES (?, ?)")
        .bind(mid)
        .bind(&b.path)
        .execute(&st.db)
        .await?;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "apprendre" }));
    Ok(Json(json!({ "ok": true })))
}

/// Outil MCP `propose_lesson` : une IA propose une leçon ; elle attend la validation d'un membre.
pub async fn proposer(
    st: &AppState,
    mid: i64,
    sid: i64,
    agent: Option<i64>,
    a: &Value,
) -> Result<Value, AppError> {
    let titre = a["title"].as_str().unwrap_or("").trim();
    let contenu = a["content"].as_str().unwrap_or("").trim();
    check_len("Titre", titre, 3, 120)?;
    check_len("Contenu", contenu, 10, 5000)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO lecons (studio_id, map_id, agent_id, titre, contenu) VALUES (?, ?, ?, ?, ?) RETURNING id")
        .bind(sid).bind(mid).bind(agent).bind(titre).bind(contenu).fetch_one(&st.db).await?;
    crate::access::notify(st, mid, json!({ "t": "refresh", "what": "apprendre" }));
    Ok(
        json!({ "id": id, "message": "Leçon proposée : un membre du studio doit la valider (Forge → Bibliothèque) avant qu'elle devienne une compétence lue par les IA." }),
    )
}

async fn lecon_du_studio(
    st: &AppState,
    u: &AuthUser,
    mid: i64,
    id: i64,
) -> Result<(crate::access::MapCtx, String, String), AppError> {
    let ctx = map_ctx(st, u, mid).await?;
    ctx.require(Role::Member)?;
    let (titre, contenu): (String, String) = sqlx::query_as(
        "SELECT titre, contenu FROM lecons WHERE id = ? AND studio_id = ? AND statut = 'propose'",
    )
    .bind(id)
    .bind(ctx.studio_id)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| bad("Cette leçon a déjà été validée ou refusée."))?;
    Ok((ctx, titre, contenu))
}

/// `POST /api/maps/{mid}/apprendre/lecons/{id}/valider` : la leçon devient une compétence du studio (« lecon-N »).
pub async fn valider(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    let (ctx, titre, contenu) = lecon_du_studio(&st, &u, mid, id).await?;
    let nom = format!("lecon-{id}");
    sqlx::query("INSERT INTO skills (studio_id, name, title, content, author_id) VALUES (?, ?, ?, ?, ?) ON CONFLICT(studio_id, name) DO UPDATE SET title = excluded.title, content = excluded.content, updated_at = datetime('now')")
        .bind(ctx.studio_id).bind(&nom).bind(format!("Leçon : {titre}")).bind(&contenu).bind(u.id).execute(&st.db).await?;
    sqlx::query("UPDATE lecons SET statut = 'valide', decide_par = ?, competence = ? WHERE id = ?")
        .bind(u.id)
        .bind(&nom)
        .bind(id)
        .execute(&st.db)
        .await?;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "apprendre" }));
    Ok(Json(json!({ "competence": nom })))
}

/// `POST /api/maps/{mid}/apprendre/lecons/{id}/refuser`.
pub async fn refuser(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    lecon_du_studio(&st, &u, mid, id).await?;
    sqlx::query("UPDATE lecons SET statut = 'refuse', decide_par = ? WHERE id = ?")
        .bind(u.id)
        .bind(id)
        .execute(&st.db)
        .await?;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "apprendre" }));
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn q(st: &AppState, s: &'static str) {
        sqlx::query(s).execute(&st.db).await.unwrap();
    }
    fn u(id: i64) -> AuthUser {
        AuthUser {
            id,
            email: format!("{id}@a.fr"),
            name: format!("U{id}"),
            sid: String::new(),
        }
    }

    /// Critères 1, 2 et 3 de #120.
    #[tokio::test]
    async fn proposer_garder_valider() {
        let st = crate::test_state().await;
        q(&st, "INSERT INTO users (id, email, name, password_hash, email_verified_at) VALUES (1, '1@a.fr', 'U1', 'x', datetime('now')), (2, '2@a.fr', 'U2', 'x', datetime('now'))").await;
        q(&st, "INSERT INTO studios (id, name) VALUES (1, 'S')").await;
        q(&st, "INSERT INTO memberships (studio_id, user_id, role) VALUES (1, 1, 'owner'), (1, 2, 'viewer')").await;
        q(
            &st,
            "INSERT INTO maps (id, studio_id, name) VALUES (7, 1, 'GrassCutter')",
        )
        .await;
        q(&st, "INSERT INTO tasks (map_id, number, title, status) VALUES (7, 3, 'Minuteur de manche', 'done'), (7, 4, 'En cours', 'doing')").await;
        q(&st, "INSERT INTO verse_files (map_id, path, content, sha) VALUES (7, 'jeu/minuteur.verse', 'using { /Verse.org }\nminuteur := class {}', 'a'), (7, 'jeu/brouillon.verse', 'x', 'b')").await;
        q(&st, "INSERT INTO uefn_actions (map_id, source, action, args, task_number, status) VALUES (7, 'mcp', 'write_file', '{\"path\":\"jeu/minuteur.verse\"}', 3, 'done'), (7, 'mcp', 'write_file', '{\"path\":\"jeu/brouillon.verse\"}', 4, 'done')").await;
        let Json(l) = liste(State(st.clone()), u(1), Path(7)).await.unwrap();
        assert_eq!(
            l["verse"].as_array().unwrap().len(),
            1,
            "seulement les tâches finies : {l}"
        );
        assert_eq!(l["verse"][0]["path"], "jeu/minuteur.verse");
        assert_eq!(l["verse"][0]["titre_tache"], "Minuteur de manche");
        // Un invité voit, mais ne valide rien.
        let Json(l2) = liste(State(st.clone()), u(2), Path(7)).await.unwrap();
        assert_eq!(l2["peut_valider"], false);
        assert!(ajouter_verse(
            State(st.clone()),
            u(2),
            Path(7),
            Json(VerseIn {
                path: "jeu/minuteur.verse".into(),
                name: None,
                description: None
            })
        )
        .await
        .is_err());
        let Json(r) = ajouter_verse(
            State(st.clone()),
            u(1),
            Path(7),
            Json(VerseIn {
                path: "jeu/minuteur.verse".into(),
                name: None,
                description: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(r["name"], "minuteur");
        let Json(l) = liste(State(st.clone()), u(1), Path(7)).await.unwrap();
        assert_eq!(l["verse"].as_array().unwrap().len(), 0, "déjà gardé");
        let m = crate::library::module(&st, 1, r["id"].as_i64().unwrap())
            .await
            .unwrap();
        assert!(m["content"].as_str().unwrap().contains("minuteur := class"));
        // Leçon d'une IA : en attente, puis validée et lue par get_skill.
        let p = proposer(&st, 7, 1, None, &json!({ "title": "Minuteurs", "content": "Utiliser un timer_device plutôt qu'une boucle Sleep." })).await.unwrap();
        let id = p["id"].as_i64().unwrap();
        let Json(l) = liste(State(st.clone()), u(1), Path(7)).await.unwrap();
        assert_eq!(l["lecons"][0]["titre"], "Minuteurs");
        assert!(
            crate::skills::lire(&st, 1, &format!("lecon-{id}"))
                .await
                .is_err(),
            "pas lue avant validation"
        );
        assert!(
            valider(State(st.clone()), u(2), Path((7, id)))
                .await
                .is_err(),
            "un invité ne valide pas"
        );
        valider(State(st.clone()), u(1), Path((7, id)))
            .await
            .unwrap();
        let s = crate::skills::lire(&st, 1, &format!("lecon-{id}"))
            .await
            .unwrap();
        assert!(s["content"].as_str().unwrap().contains("timer_device"));
        assert!(
            refuser(State(st.clone()), u(1), Path((7, id)))
                .await
                .is_err(),
            "déjà décidée"
        );
    }

    #[test]
    fn nom_du_module() {
        assert_eq!(nom_de("jeu/minuteur_manche.verse"), "minuteur_manche");
        assert_eq!(nom_de("a\\b.verse"), "b");
    }
}
