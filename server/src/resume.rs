//! Résumés en français des actions de l'IA (#321) : une phrase courte et sans jargon par agent, par tâche terminée
//! et pour l'équipe, fabriquée par POP à partir du fil des appels MCP (`agent_calls`) et des notes `save_progress`.
//! Algorithme local, sans IA externe. Le détail complet reste dans le fil d'activité de l'agent.
use crate::{
    access::map_ctx,
    auth::AuthUser,
    error::{ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

/// Un appel d'outil MCP tel que noté dans le fil : outil, réussite, détail court.
#[derive(Debug, Clone)]
pub struct Appel {
    pub tool: String,
    pub ok: bool,
    pub detail: Option<String>,
}

/// Outils qui changent le niveau UEFN.
const NIVEAU: &[&str] = &[
    "update_actors",
    "place_device",
    "place_mesh",
    "duplicate_actors",
    "delete_actors",
    "set_island_settings",
    "import_asset",
    "create_widget",
];
/// Outils qui organisent les tâches.
const TACHES: &[&str] = &[
    "create_task",
    "set_task_lot",
    "plan_tasks",
    "assign_task",
    "comment_task",
];
/// Outils de documentation (pages, tableaux blancs, jalons).
const DOCS: &[&str] = &[
    "write_page",
    "create_board",
    "upsert_board_objects",
    "delete_board_objects",
    "add_milestone",
];

/// Détail gardé avec un appel : jamais de code ni de texte privé, seulement de quoi écrire le résumé.
pub fn detail(tool: &str, a: &Value, erreurs: usize) -> Option<String> {
    match tool {
        "write_verse_file" => a["path"].as_str().map(|p| {
            p.rsplit(['/', '\\'])
                .next()
                .unwrap_or(p)
                .chars()
                .take(120)
                .collect()
        }),
        "compile_verse" => Some(erreurs.to_string()),
        "set_task_status" => a["status"].as_str().map(|s| s.chars().take(20).collect()),
        _ => None,
    }
}

fn pluriel(n: usize, un: &str, plusieurs: &str) -> String {
    format!("{n} {}", if n > 1 { plusieurs } else { un })
}

/// Première phrase d'une note, courte, avec une majuscule.
fn premiere_phrase(t: &str) -> Option<String> {
    let t = t.trim();
    let fin = t.find(['.', '\n', ';']).unwrap_or(t.len());
    let mut s: String = t[..fin].trim().chars().take(90).collect();
    if s.is_empty() {
        return None;
    }
    if t[..fin].trim().chars().count() > 90 {
        s.push('…');
    }
    let mut c = s.chars();
    Some(
        c.next()
            .map(|p| p.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default(),
    )
}

/// La phrase d'un agent ou d'une tâche, d'après ses appels (du plus ancien au plus récent), la dernière note
/// `save_progress` (ce qui est fait) et le titre de la tâche.
pub fn phrase(appels: &[Appel], fait: Option<&str>, titre: Option<&str>) -> String {
    let a = |noms: &[&str]| {
        appels
            .iter()
            .filter(|x| x.ok && noms.contains(&x.tool.as_str()))
            .count()
    };
    let mut fichiers: Vec<&str> = vec![];
    for x in appels
        .iter()
        .filter(|x| x.ok && x.tool == "write_verse_file")
    {
        if let Some(d) = x.detail.as_deref() {
            if !fichiers.contains(&d) {
                fichiers.push(d);
            }
        }
    }
    let verse = fichiers.len().max(usize::from(
        fichiers.is_empty() && a(&["write_verse_file"]) > 0,
    ));
    let niveau = a(NIVEAU);

    let mut tete = match (fait.and_then(premiere_phrase), titre) {
        (Some(f), _) => f,
        (None, Some(t)) if !t.trim().is_empty() => format!("Travaille sur « {} »", t.trim()),
        _ if verse > 0 => "Écrit du Verse".into(),
        _ if niveau > 0 => "Modifie le niveau".into(),
        _ if a(TACHES) > 0 => "Organise les tâches".into(),
        _ if a(DOCS) > 0 => "Met à jour la documentation".into(),
        _ if appels.is_empty() => return "En attente".into(),
        _ => "Consulte le projet".into(),
    };
    if verse > 0 {
        tete.push_str(&format!(
            " ({})",
            pluriel(verse, "fichier Verse", "fichiers Verse")
        ));
    }
    let mut parts = vec![tete];
    if niveau > 0 {
        parts.push(pluriel(
            niveau,
            "modification du niveau",
            "modifications du niveau",
        ));
    }
    let docs = a(DOCS);
    if docs > 0 && !parts[0].starts_with("Met à jour") {
        parts.push(pluriel(
            docs,
            "mise à jour de la documentation",
            "mises à jour de la documentation",
        ));
    }
    // Compilation : seule la dernière compte ; un test (session) après une compilation réussie = « en test ».
    let derniere_compil = appels.iter().rposition(|x| x.tool == "compile_verse");
    if let Some(i) = derniere_compil {
        let c = &appels[i];
        if c.ok {
            parts.push("compilation OK".into());
        } else {
            let n: usize = c
                .detail
                .as_deref()
                .and_then(|d| d.parse().ok())
                .unwrap_or(0);
            parts.push(if n > 0 {
                format!("compilation en échec ({})", pluriel(n, "erreur", "erreurs"))
            } else {
                "compilation en échec".into()
            });
        }
    }
    let annulees = a(&["undo_action"]);
    if annulees > 0 {
        parts.push(pluriel(annulees, "action annulée", "actions annulées"));
    }
    let refus = appels
        .iter()
        .filter(|x| !x.ok && x.tool != "compile_verse")
        .count();
    if refus > 0 {
        parts.push(pluriel(refus, "action en erreur", "actions en erreur"));
    }
    let statut = appels
        .iter()
        .rev()
        .find(|x| x.ok && x.tool == "set_task_status")
        .and_then(|x| x.detail.as_deref());
    let session = appels.iter().rposition(|x| {
        x.ok && matches!(
            x.tool.as_str(),
            "start_session" | "push_to_session" | "session_output"
        )
    });
    match statut {
        Some("done") => parts.push("tâche terminée".into()),
        Some("review") => parts.push("en relecture".into()),
        Some("blocked") => parts.push("bloqué".into()),
        _ if session.is_some()
            && derniere_compil.is_some_and(|c| appels[c].ok && session > Some(c)) =>
        {
            parts.push("en test".into())
        }
        _ => {}
    }
    parts.join(", ")
}

/// Résumé d'équipe : tâches finies, agents en test, agents dont la dernière compilation échoue.
pub fn equipe(finies: usize, en_test: usize, erreurs: usize) -> String {
    format!(
        "{}, {} en test, {}",
        pluriel(finies, "tâche finie", "tâches finies"),
        en_test,
        pluriel(erreurs, "erreur", "erreurs")
    )
}

async fn appels(
    st: &AppState,
    filtre: &str,
    id: i64,
    depuis: &str,
) -> Result<Vec<Appel>, AppError> {
    let rows: Vec<(String, bool, Option<String>)> = sqlx::query_as(crate::sql!(
        "SELECT tool, ok, detail FROM agent_calls WHERE {filtre} = ? AND at >= ? ORDER BY id"
    ))
    .bind(id)
    .bind(depuis)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(tool, ok, detail)| Appel { tool, ok, detail })
        .collect())
}

/// Les résumés de la map : équipe, chaque agent actif (sa tâche en cours, sinon ses 2 dernières heures) et les
/// tâches terminées par des agents cette semaine (le fil des appels est gardé 7 jours).
pub async fn resumes(st: &AppState, mid: i64) -> Result<Value, AppError> {
    crate::agents::liberer_silencieux(st).await?;
    let agents: Vec<(i64, String, Option<String>, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.name, a.role, a.task_id, a.task_since, t.title FROM api_tokens a LEFT JOIN tasks t ON t.id = a.task_id
         WHERE a.map_id = ? AND (a.task_id IS NOT NULL OR a.last_used >= datetime('now', '-2 hours'))
         ORDER BY a.last_used IS NULL, a.last_used DESC, a.id DESC",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let deux_heures: String = sqlx::query_scalar("SELECT datetime('now', '-2 hours')")
        .fetch_one(&st.db)
        .await?;
    let mut out = vec![];
    let (mut en_test, mut erreurs) = (0, 0);
    for (id, nom, role, tache, depuis, titre) in agents {
        let depuis = match (&tache, depuis) {
            (Some(_), Some(d)) => d,
            _ => deux_heures.clone(),
        };
        let a = appels(st, "agent_id", id, &depuis).await?;
        let fait: Option<String> = match tache {
            Some(t) => sqlx::query_scalar("SELECT done FROM task_progress WHERE task_id = ? AND agent_id = ? AND done <> '' AND created_at >= ? ORDER BY id DESC LIMIT 1")
                .bind(t)
                .bind(id)
                .bind(&depuis)
                .fetch_optional(&st.db)
                .await?,
            None => None,
        };
        if let Some(t) = tache {
            if crate::agents::etape(st, id, t).await? == Some("testé") {
                en_test += 1;
            }
        }
        let derniere: Option<bool> = sqlx::query_scalar("SELECT ok FROM agent_calls WHERE agent_id = ? AND tool = 'compile_verse' ORDER BY id DESC LIMIT 1")
            .bind(id)
            .fetch_optional(&st.db)
            .await?;
        if derniere == Some(false) {
            erreurs += 1;
        }
        out.push(json!({ "id": id, "name": nom, "role": role, "phrase": phrase(&a, fait.as_deref(), titre.as_deref()) }));
    }
    let finies: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT t.id, t.number, t.title, MAX(c.at) FROM tasks t JOIN agent_calls c ON c.task_id = t.id
         WHERE t.map_id = ? AND t.status = 'done' GROUP BY t.id ORDER BY MAX(c.at) DESC, t.id DESC",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let mut taches = vec![];
    for (tid, n, titre, _) in finies.iter().take(10) {
        let a = appels(st, "task_id", *tid, "0000").await?;
        let fait: Option<String> = sqlx::query_scalar("SELECT done FROM task_progress WHERE task_id = ? AND done <> '' ORDER BY id DESC LIMIT 1")
            .bind(tid)
            .fetch_optional(&st.db)
            .await?;
        let par: Vec<String> = sqlx::query_scalar("SELECT DISTINCT a.name FROM agent_calls c JOIN api_tokens a ON a.id = c.agent_id WHERE c.task_id = ? ORDER BY a.name")
            .bind(tid)
            .fetch_all(&st.db)
            .await?;
        let mut p = phrase(&a, fait.as_deref(), None);
        if !p.contains("tâche terminée") {
            p.push_str(", tâche terminée");
        }
        taches.push(json!({ "number": n, "title": titre, "phrase": p, "par": par }));
    }
    Ok(json!({
        "equipe": { "phrase": equipe(finies.len(), en_test, erreurs), "finies": finies.len(), "en_test": en_test, "erreurs": erreurs },
        "agents": out,
        "taches": taches,
    }))
}

/// `GET /api/maps/{mid}/resumes` : les résumés de la map, pour le site et Forge.
pub async fn route(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let mut r = resumes(&st, mid).await?;
    // Chacun ne voit que ses propres IA (#376) : les agents des autres membres et leur nom sont retirés.
    let miens: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM api_tokens WHERE map_id = ? AND user_id = ?")
            .bind(mid)
            .bind(u.id)
            .fetch_all(&st.db)
            .await?;
    if let Some(a) = r["agents"].as_array_mut() {
        a.retain(|x| miens.iter().any(|m| Some(m.0) == x["id"].as_i64()));
    }
    for t in r["taches"].as_array_mut().into_iter().flatten() {
        if let Some(p) = t["par"].as_array_mut() {
            p.retain(|n| {
                n.as_str().is_some_and(|n| {
                    miens
                        .iter()
                        .any(|m| n == m.1 || n.starts_with(&format!("{} · ", m.1)))
                })
            });
        }
    }
    Ok(Json(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ap(tool: &str, ok: bool, detail: Option<&str>) -> Appel {
        Appel {
            tool: tool.into(),
            ok,
            detail: detail.map(Into::into),
        }
    }

    #[test]
    fn phrases() {
        assert_eq!(phrase(&[], None, None), "En attente");
        let a = [
            ap("write_verse_file", true, Some("score.verse")),
            ap("write_verse_file", true, Some("hud_score.verse")),
            ap("write_verse_file", true, Some("score.verse")),
            ap("compile_verse", false, Some("2")),
            ap("compile_verse", true, Some("0")),
            ap("start_session", true, None),
        ];
        assert_eq!(
            phrase(&a, Some("Écrit le système de score. Reste le HUD"), None),
            "Écrit le système de score (2 fichiers Verse), compilation OK, en test"
        );
        // Sans note : le titre de la tâche ; la dernière compilation échoue.
        assert_eq!(
            phrase(&a[..4], None, Some("Système de score")),
            "Travaille sur « Système de score » (2 fichiers Verse), compilation en échec (2 erreurs)"
        );
        // Ni note ni tâche : ce que font les appels.
        let n = [
            ap("place_device", true, None),
            ap("update_actors", true, None),
            ap("delete_actors", false, None),
        ];
        assert_eq!(
            phrase(&n, None, None),
            "Modifie le niveau, 2 modifications du niveau, 1 action en erreur"
        );
        let d = [
            ap("write_page", true, None),
            ap("set_task_status", true, Some("done")),
        ];
        assert_eq!(
            phrase(&d, None, None),
            "Met à jour la documentation, tâche terminée"
        );
        // Un test avant la compilation réussie n'est pas « en test ».
        let t = [
            ap("start_session", true, None),
            ap("compile_verse", true, Some("0")),
        ];
        assert_eq!(phrase(&t, None, None), "Consulte le projet, compilation OK");
    }

    #[test]
    fn equipe_et_details() {
        assert_eq!(equipe(3, 1, 0), "3 tâches finies, 1 en test, 0 erreur");
        assert_eq!(equipe(1, 0, 2), "1 tâche finie, 0 en test, 2 erreurs");
        assert_eq!(
            detail(
                "write_verse_file",
                &json!({ "path": "Verse/jeu/score.verse", "content": "secret" }),
                0
            )
            .as_deref(),
            Some("score.verse")
        );
        assert_eq!(detail("compile_verse", &json!({}), 3).as_deref(), Some("3"));
        assert_eq!(
            detail("write_page", &json!({ "content": "privé" }), 0),
            None
        );
    }

    /// De bout en bout : appels réels notés par le fil, note save_progress, tâche finie, résumé d'équipe.
    #[tokio::test]
    async fn de_bout_en_bout() {
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
        let uid = q("INSERT INTO users (email, name, password_hash) VALUES ('r@a.fr', 'R', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        let mut taches = vec![];
        for (n, titre) in [(1, "Système de score"), (2, "Minuteur")] {
            taches.push(
                sqlx::query_scalar::<_, i64>(
                    "INSERT INTO tasks (map_id, number, title) VALUES (?, ?, ?) RETURNING id",
                )
                .bind(mid)
                .bind(n)
                .bind(titre)
                .fetch_one(&st.db)
                .await
                .unwrap(),
            );
        }
        let mut agents = vec![];
        for nom in ["Claude Code · Codeur", "Claude Code · Codeur 2"] {
            agents.push(
                sqlx::query_scalar::<_, i64>("INSERT INTO api_tokens (user_id, map_id, name, kind, token_hash, last_used) VALUES (?, ?, ?, 'claude_code', ?, datetime('now')) RETURNING id")
                    .bind(uid).bind(mid).bind(nom).bind(format!("h-{nom}")).fetch_one(&st.db).await.unwrap(),
            );
        }
        let (a, b) = (agents[0], agents[1]);
        sqlx::query("UPDATE api_tokens SET task_id = ?, task_since = datetime('now', '-1 minute') WHERE id = ?").bind(taches[0]).bind(a).execute(&st.db).await.unwrap();
        sqlx::query("UPDATE api_tokens SET task_id = ?, task_since = datetime('now', '-1 minute') WHERE id = ?").bind(taches[1]).bind(b).execute(&st.db).await.unwrap();
        let ok = json!({});
        let ko = json!({ "diagnostics": [{ "severity": "error" }] });
        use crate::agents::noter_appel;
        noter_appel(
            &st,
            a,
            mid,
            "write_verse_file",
            &json!({ "path": "score.verse", "content": "x" }),
            Ok(&ok),
        )
        .await;
        noter_appel(
            &st,
            a,
            mid,
            "write_verse_file",
            &json!({ "path": "hud.verse", "content": "x" }),
            Ok(&ok),
        )
        .await;
        noter_appel(&st, a, mid, "compile_verse", &json!({}), Ok(&ok)).await;
        noter_appel(&st, a, mid, "start_session", &json!({}), Ok(&ok)).await;
        crate::agents::save_progress(
            &st,
            a,
            &json!({ "done": "Écrit le système de score", "todo": "Tester" }),
        )
        .await
        .unwrap();
        noter_appel(
            &st,
            b,
            mid,
            "write_verse_file",
            &json!({ "path": "minuteur.verse", "content": "x" }),
            Ok(&ok),
        )
        .await;
        noter_appel(&st, b, mid, "compile_verse", &json!({}), Ok(&ko)).await;

        let r = resumes(&st, mid).await.unwrap();
        let de = |id: i64| {
            r["agents"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["id"] == id)
                .unwrap()["phrase"]
                .clone()
        };
        assert_eq!(
            de(a),
            "Écrit le système de score (2 fichiers Verse), compilation OK, en test",
            "{r}"
        );
        assert_eq!(
            de(b),
            "Travaille sur « Minuteur » (1 fichier Verse), compilation en échec (1 erreur)",
            "{r}"
        );
        assert_eq!(
            r["equipe"]["phrase"], "0 tâche finie, 1 en test, 1 erreur",
            "{r}"
        );

        // La tâche 1 est terminée : elle apparaît dans les tâches finies, et l'équipe compte juste.
        noter_appel(
            &st,
            a,
            mid,
            "set_task_status",
            &json!({ "number": 1, "status": "done" }),
            Ok(&ok),
        )
        .await;
        sqlx::query("UPDATE tasks SET status = 'done' WHERE id = ?")
            .bind(taches[0])
            .execute(&st.db)
            .await
            .unwrap();
        let r = resumes(&st, mid).await.unwrap();
        assert_eq!(r["taches"][0]["number"], 1, "{r}");
        assert_eq!(
            r["taches"][0]["phrase"],
            "Écrit le système de score (2 fichiers Verse), compilation OK, tâche terminée",
            "{r}"
        );
        assert_eq!(r["taches"][0]["par"][0], "Claude Code · Codeur");
        assert_eq!(r["equipe"]["finies"], 1, "{r}");
        // Aucun contenu de fichier dans le fil.
        let fuite: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_calls WHERE detail LIKE '%x%' AND tool = 'write_verse_file' AND detail NOT LIKE '%.verse'").fetch_one(&st.db).await.unwrap();
        assert_eq!(fuite, 0);
    }
}
