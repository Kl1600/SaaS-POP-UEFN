//! « Plan de production par l'IA POP » : à partir de la description d'une map (ou de sa fiche idée),
//! Claude propose des tâches d'environ une heure, rangées par étape, avec leurs dépendances.
//! Le créateur relit la liste et choisit ce qu'il ajoute. Payé en crédits IA (voir ai.rs).
use crate::{
    access::{map_ctx, Role},
    auth::AuthUser,
    error::{bad, ApiResult},
    tasks::{insert_task, NewTask},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

const SYSTEM: &str = "Tu es producteur de jeux pour Fortnite (UEFN, Verse). Tu découpes une map en tâches concrètes d'environ une heure chacune, \
dans l'ordre où une petite équipe les ferait, du prototype jouable à la publication. Chaque tâche commence par un verbe, tient en moins de 80 caractères, \
et se vérifie facilement (« Placer les 12 points d'apparition de l'équipe rouge », pas « Faire la carte »). \
Étiquettes possibles : code (Verse, logique), carte (construction, décor, level design), conception (règles, équilibrage, UI, textes), test (playtests, corrections, vérifications avant publication). \
« after » = les tâches sans lesquelles celle-ci ne peut être ni faite ni testée. Pas l'ordre dans lequel le joueur découvre les choses (le code de l'herbe à couper se teste sur quelques props posés n'importe où, sans attendre le décor de l'île). Si seule une partie a besoin d'une autre tâche, découpe-la. Prévois aussi : vignette et bande-annonce conformes aux règles d'Epic, choix du genre dans le Creator Portal, playtests, et préparation du lancement. \
Réponds UNIQUEMENT avec un tableau JSON, sans texte autour, de la forme : \
[{\"lot\": \"Prototype\", \"title\": \"…\", \"tag\": \"code\", \"description\": \"une phrase\", \"after\": [indices des tâches dont elle dépend]}]";

#[derive(Deserialize)]
pub struct PlanIn {
    brief: Option<String>,
    size: Option<u32>,
}

/// Texte de départ : description donnée, sinon nom et genre de la map et sa fiche idée (si elle existe).
async fn brief_of(st: &AppState, mid: i64, given: Option<String>) -> Result<String, sqlx::Error> {
    if let Some(b) = given
        .map(|b| b.trim().to_string())
        .filter(|b| b.len() >= 10)
    {
        return Ok(b);
    }
    let (name, genre): (String, String) =
        sqlx::query_as("SELECT name, genre FROM maps WHERE id = ?")
            .bind(mid)
            .fetch_one(&st.db)
            .await?;
    let sheet: Option<String> = sqlx::query_scalar("SELECT blocks FROM pages WHERE map_id = ? AND title LIKE 'Fiche idée%' ORDER BY id LIMIT 1").bind(mid).fetch_optional(&st.db).await?;
    let mut out = format!("Map : {name}. Genre : {genre}.");
    if let Some(b) = sheet {
        let v: Value = serde_json::from_str(&b).unwrap_or(json!([]));
        let text: Vec<String> = v
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x["text"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        out.push_str("\nFiche idée :\n");
        out.push_str(&text.join("\n"));
    }
    Ok(out)
}

pub async fn propose(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<PlanIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    let brief = brief_of(&st, mid, b.brief).await?;
    if brief.len() > 6000 {
        return Err(bad("Description trop longue (6 000 caractères au plus)."));
    }
    let size = b.size.unwrap_or(40).clamp(15, 80);
    let prompt = format!("Découpe cette map en environ {size} tâches.\n\n{brief}");
    let r = crate::aipop::run(&st, &u, "plan de production", SYSTEM, &prompt, 6000).await?;
    let start = r.text.find('[');
    let end = r.text.rfind(']');
    let tasks: Vec<Value> = match (start, end) {
        (Some(s), Some(e)) if e > s => serde_json::from_str(&r.text[s..=e]).unwrap_or_default(),
        _ => vec![],
    };
    let clean: Vec<Value> = tasks.iter().filter_map(|t| {
        let title: String = t["title"].as_str()?.trim().chars().take(140).collect();
        if title.is_empty() { return None }
        let tag = t["tag"].as_str().filter(|x| crate::tasks::TAGS.contains(x)).unwrap_or("code");
        Some(json!({ "lot": t["lot"].as_str().unwrap_or("").chars().take(60).collect::<String>(), "title": title, "tag": tag,
            "description": t["description"].as_str().unwrap_or("").chars().take(500).collect::<String>(),
            "after": t["after"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect::<Vec<_>>()).unwrap_or_default() }))
    }).collect();
    if clean.is_empty() {
        return Err(bad("L’IA n’a pas renvoyé de plan lisible. Les crédits utilisés vous sont rendus si vous écrivez au support."));
    }
    Ok(Json(
        json!({ "tasks": clean, "credits": r.credits, "balance": r.balance, "brief": brief }),
    ))
}

#[derive(Deserialize)]
pub struct ApplyIn {
    tasks: Vec<Value>,
}

/// Ajoute les tâches retenues (dans l'ordre), avec leurs dépendances entre elles.
pub async fn apply(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<ApplyIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Member)?;
    if b.tasks.is_empty() || b.tasks.len() > 120 {
        return Err(bad("Choisissez entre 1 et 120 tâches."));
    }
    let mut ids: Vec<Option<i64>> = vec![];
    for t in &b.tasks {
        let deps: Vec<i64> = t["after"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_u64())
                    .filter_map(|i| ids.get(i as usize).copied().flatten())
                    .collect()
            })
            .unwrap_or_default();
        let nt = NewTask {
            title: t["title"].as_str().unwrap_or("").to_string(),
            tag: t["tag"].as_str().map(String::from),
            description: t["description"]
                .as_str()
                .map(String::from)
                .filter(|d| !d.is_empty()),
            assignee_id: None,
            deps: if deps.is_empty() { None } else { Some(deps) },
            planned_day: None,
            lot: t["lot"]
                .as_str()
                .map(String::from)
                .filter(|l| !l.is_empty()),
            status: None,
            ai: Some(true),
            origin: Some("ia-pop".into()),
            uefn: None,
        };
        ids.push(insert_task(&st, &ctx, &nt).await.ok().map(|x| x.0));
    }
    let n = ids.iter().flatten().count();
    crate::access::log_activity(
        &st,
        mid,
        Some(u.id),
        "task",
        "Plan de production ajouté",
        &format!("{n} tâches proposées par l’IA POP"),
    )
    .await?;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "tasks" }));
    Ok(Json(json!({ "added": n })))
}
