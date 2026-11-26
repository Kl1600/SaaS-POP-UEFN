//! États et dépannage de Forge (#300) : diagnostic de la map (UEFN fermé, serveur MCP arrêté, relais à mettre à jour,
//! compilation en échec, agent qui ne répond plus), chaque cas avec son action, et les pages d'aide de Forge que les
//! agents lisent avec l'outil MCP `forge_help`.
use crate::{access::map_ctx, auth::AuthUser, error::ApiResult, error::AppError, AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

/// Pages d'aide de Forge : (identifiant, titre, texte).
pub const PAGES: &[(&str, &str, &str)] = &[
    ("uefn-ferme", "UEFN n'est pas ouvert", "Forge pilote UEFN par son serveur MCP local. Ouvrez le projet relié dans UEFN : Forge se reconnecte tout seul en quelques secondes. Si UEFN est ouvert mais que Forge dit encore « UEFN fermé », le serveur MCP d'UEFN est arrêté (voir « serveur MCP arrêté »)."),
    ("mcp-arrete", "Serveur MCP d'UEFN arrêté", "Dans UEFN : menu Outils → MCP → Start Server. Pour ne plus avoir à le faire, activez « Auto Start Server » dans les réglages du projet (Project Settings → UEFN MCP). Il faut aussi les extensions Python Editor Scripting et UEFN MCP Toolsets. Ensuite, Forge se reconnecte tout seul."),
    ("relais-a-mettre-a-jour", "Relais à mettre à jour", "Le relais de Forge (ou un ancien pop-link.exe) est plus ancien que le serveur POP : certaines actions sont refusées avec « Relais périmé ». Avec Forge : installez la dernière version depuis la page Télécharger POP Forge (pop-uefn.com/forge) puis relancez-le. Avec le relais seul : téléchargez le nouveau pop-link.exe depuis POP → Organiseur → UEFN Link, et relancez-le."),
    ("compilation-echec", "Compilation en échec", "L'écran Vérifications de Forge liste chaque erreur de la dernière compilation avec son fichier et sa ligne. Ouvrez le fichier dans l'éditeur Verse de POP ou demandez à une IA de corriger : elle lit les erreurs avec compile_verse. Une erreur « Relais périmé » ou « UEFN non connecté » n'est pas une erreur de code : voir ces pages."),
    ("agent-muet", "Un agent ne répond plus", "L'agent tient une tâche mais n'a rien appelé depuis longtemps : son IA est peut-être fermée, en attente d'une réponse ou hors connexion. Rouvrez-la, ou écrivez-lui depuis Forge → Agents → Écrire. Si elle ne revient pas, attribuez la tâche à une autre IA (« Attribuer une tâche… ») ou coupez l'agent."),
    ("liaison", "Relier une map à son projet UEFN", "Forge → Accueil : chaque map du site a son bouton « Relier ». Forge propose le projet ouvert dans UEFN et ceux trouvés dans Documents ; « Parcourir… » pour un autre dossier. Une map n'est reliée qu'à un projet à la fois."),
    ("abonnement", "Forge et l'abonnement Pro", "Forge est gratuit comme relais UEFN : sans Pro, il relie vos projets et pilote UEFN pour le site et vos IA connectées par MCP. Les écrans de construction (Agents, Vérifications, Bibliothèque, Outils) sont inclus dans l'abonnement Pro, essai gratuit compris ; ils se débloquent seuls quand l'abonnement est actif."),
];

/// Outil MCP `forge_help` : les pages qui contiennent la recherche (toutes si vide).
pub fn chercher(q: &str) -> Value {
    let q = crate::account::fold_str(q.trim());
    let mots: Vec<&str> = q.split_whitespace().filter(|m| m.len() > 2).collect();
    let mut p: Vec<(usize, Value)> = PAGES
        .iter()
        .map(|(id, t, x)| {
            let f = crate::account::fold_str(&format!("{t} {x}"));
            (
                mots.iter().filter(|m| f.contains(*m)).count(),
                json!({ "id": id, "title": t, "text": x }),
            )
        })
        .filter(|(n, _)| mots.is_empty() || *n > 0)
        .collect();
    p.sort_by(|a, b| b.0.cmp(&a.0));
    json!({ "pages": p.into_iter().map(|x| x.1).collect::<Vec<_>>() })
}

/// Problèmes de la map vus par Forge, chacun avec la page d'aide et l'action proposée.
pub async fn diagnostic(st: &AppState, mid: i64, uid: i64) -> Result<Value, AppError> {
    let mut pb = vec![];
    let mut ajoute = |code: &str, titre: String, action: &str, label: &str| {
        let page = PAGES
            .iter()
            .find(|p| p.0 == code)
            .map(|p| p.2)
            .unwrap_or("");
        pb.push(
            json!({ "code": code, "title": titre, "help": page, "action": action, "label": label }),
        );
    };
    match st.relays.status(mid) {
        "offline" => ajoute(
            "liaison",
            "Cette map n'est pas reliée à UEFN".into(),
            "accueil",
            "Relier",
        ),
        "closed" => ajoute(
            "mcp-arrete",
            "UEFN est fermé ou son serveur MCP est arrêté".into(),
            "reessayer",
            "Réessayer",
        ),
        _ => {}
    }
    if st.relays.stale(mid) {
        ajoute(
            "relais-a-mettre-a-jour",
            "Relais à mettre à jour".into(),
            "telecharger",
            "Mettre à jour",
        );
    }
    let compile: Option<(String, Option<String>)> = sqlx::query_as("SELECT status, after FROM uefn_actions WHERE map_id = ? AND action = 'compile' ORDER BY id DESC LIMIT 1")
        .bind(mid)
        .fetch_optional(&st.db)
        .await?;
    if let Some((s, after)) = compile {
        let n = after
            .as_deref()
            .and_then(|a| serde_json::from_str::<Value>(a).ok())
            .and_then(|v| {
                v["diagnostics"]
                    .as_array()
                    .map(|d| d.iter().filter(|x| x["severity"] == "error").count())
            })
            .unwrap_or(0);
        if s == "failed" || n > 0 {
            ajoute(
                "compilation-echec",
                if n > 0 {
                    format!("Compilation en échec : {n} erreur(s)")
                } else {
                    "Compilation en échec".into()
                },
                "verifier",
                "Voir les erreurs",
            );
        }
    }
    let muets: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM api_tokens WHERE map_id = ? AND user_id = ? AND task_id IS NOT NULL AND scope <> 'off' AND (last_used IS NULL OR last_used < datetime('now', '-30 minutes'))",
    )
    .bind(mid)
    .bind(uid)
    .fetch_all(&st.db)
    .await?;
    for (n,) in muets {
        ajoute(
            "agent-muet",
            format!("L'agent « {n} » ne répond plus depuis plus de 30 min"),
            "agents",
            "Voir l'agent",
        );
    }
    Ok(json!({ "problems": pb }))
}

/// `GET /api/maps/{mid}/diagnostic`
pub async fn route(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    Ok(Json(diagnostic(&st, mid, u.id).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aide_trouvee_par_les_agents() {
        let r = chercher("pourquoi Forge dit relais à mettre à jour ?");
        assert_eq!(r["pages"][0]["id"], "relais-a-mettre-a-jour", "{r}");
        assert_eq!(chercher("")["pages"].as_array().unwrap().len(), PAGES.len());
    }

    #[tokio::test]
    async fn cas_detectes() {
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
        let uid: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('a@a.fr', 'A', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        let codes = |v: Value| {
            v["problems"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["code"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(codes(diagnostic(&st, mid, uid).await.unwrap()), ["liaison"]);
        sqlx::query("INSERT INTO uefn_actions (map_id, user_id, source, action, summary, args, status, after) VALUES (?, ?, 'mcp', 'compile', '', '{}', 'done', ?)")
            .bind(mid).bind(uid).bind(r#"{"diagnostics":[{"severity":"error","path":"a.verse","line":3,"message":"x"}]}"#).execute(&st.db).await.unwrap();
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tasks (map_id, number, title) VALUES (?, 1, 'T') RETURNING id",
        )
        .bind(mid)
        .fetch_one(&st.db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO api_tokens (user_id, map_id, name, token_hash, task_id, last_used) VALUES (?, ?, 'Claude', 'h', ?, datetime('now', '-2 hours'))").bind(uid).bind(mid).bind(tid).execute(&st.db).await.unwrap();
        let c = codes(diagnostic(&st, mid, uid).await.unwrap());
        assert!(
            c.contains(&"compilation-echec".to_string()) && c.contains(&"agent-muet".to_string()),
            "{c:?}"
        );
    }
}
