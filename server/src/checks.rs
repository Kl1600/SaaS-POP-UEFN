//! Vérifications automatiques du niveau (#96), sur le dernier état connu (acteurs lus par le relais, fichiers Verse).
//! Chaque problème a une gravité (« bloquant » ou « à vérifier »), l'acteur concerné et une explication en français.
//! Les liens d'événements des appareils classiques ne sont pas lisibles par le serveur MCP d'UEFN : ils ne sont pas
//! vérifiés ici (voir KB/UEFN.md).
use crate::{
    access::map_ctx,
    auth::AuthUser,
    error::{ApiResult, AppError},
    uefn::snapshot,
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashSet;

fn probleme(gravite: &str, regle: &str, a: &Value, message: String) -> Value {
    json!({ "gravite": gravite, "regle": regle, "acteur": a["id"], "nom": a["name"], "message": message })
}

fn nom(a: &Value) -> String {
    a["name"]
        .as_str()
        .or(a["id"].as_str())
        .unwrap_or("?")
        .to_string()
}

/// Les vérifications, des plus graves aux moins graves.
pub fn verifier(acteurs: &[Value], verse: &[(String, String)]) -> Vec<Value> {
    let ids: HashSet<String> = acteurs
        .iter()
        .filter_map(|a| a["id"].as_str().map(|s| s.to_lowercase()))
        .collect();
    let mut out = vec![];
    for a in acteurs {
        // 1. Références d'un appareil vers un acteur absent du niveau (bloquant), ou vides (à vérifier).
        for champ in ["options", "props"] {
            let Some(m) = a[champ].as_object() else {
                continue;
            };
            for (k, v) in m {
                if let Some(r) = v["refPath"].as_str().filter(|r| !r.is_empty()) {
                    if !ids.contains(&r.to_lowercase()) {
                        out.push(probleme("bloquant", "lien_casse", a, format!(
                            "« {} » : le réglage « {k} » pointe vers un acteur absent du niveau ({r}).", nom(a)
                        )));
                    }
                } else if champ == "props" && a["type"] == "verse_device" && v.is_null() {
                    out.push(probleme(
                        "a_verifier",
                        "reference_vide",
                        a,
                        format!(
                            "« {} » : le réglage « {k} » n'est relié à aucun acteur.",
                            nom(a)
                        ),
                    ));
                }
            }
        }
        // 2. Options réglées mais sans effet, d'après les règles du catalogue (option inactive).
        let mut e = a.clone();
        crate::catalog::enrich(&mut e);
        for o in e["schema"].as_array().into_iter().flatten() {
            let Some(pourquoi) = o["disabled"].as_str() else {
                continue;
            };
            let k = o["key"].as_str().unwrap_or("");
            let valeur = a["options"][k].clone();
            if !valeur.is_null() && valeur != o["default"] {
                out.push(probleme(
                    "a_verifier",
                    "option_inactive",
                    a,
                    format!(
                    "« {} » : l'option « {} » est réglée ({}) mais n'a aucun effet : {pourquoi}",
                    nom(a), o["label"].as_str().unwrap_or(k), valeur
                ),
                ));
            }
        }
    }
    // 3. Tags Verse : posés sur un acteur sans être utilisés dans le Verse, ou définis sans être posés.
    if !verse.is_empty() {
        let code: String = verse
            .iter()
            .map(|(_, c)| c.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let mut poses = HashSet::new();
        for a in acteurs {
            for t in a["tags"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|t| t.as_str())
            {
                poses.insert(t.to_string());
                if !code.contains(t) {
                    out.push(probleme(
                        "a_verifier",
                        "tag_inutilise",
                        a,
                        format!(
                            "« {} » : le tag « {t} » n'est utilisé dans aucun fichier Verse.",
                            nom(a)
                        ),
                    ));
                }
            }
        }
        let def = Regex::new(r"(?m)^\s*([A-Za-z_]\w*)\s*:=\s*class\s*\(\s*tag\s*\)").unwrap();
        for (path, c) in verse {
            for cap in def.captures_iter(c) {
                let t = &cap[1];
                if !poses
                    .iter()
                    .any(|p| p == t || p.ends_with(&format!(".{t}")))
                {
                    out.push(json!({ "gravite": "a_verifier", "regle": "tag_sans_acteur", "acteur": null, "nom": path,
                        "message": format!("Le tag « {t} » défini dans {path} n'est posé sur aucun acteur.") }));
                }
            }
        }
    }
    out.sort_by_key(|p| p["gravite"] != "bloquant");
    out
}

/// Vérifications de la map, avec le nombre de problèmes par gravité.
pub async fn rapport(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let snap = snapshot(st, mid).await?;
    let verse: Vec<(String, String)> =
        sqlx::query_as("SELECT path, content FROM verse_files WHERE map_id = ? ORDER BY path")
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
    let maj: Option<String> =
        sqlx::query_scalar("SELECT updated_at FROM uefn_state WHERE map_id = ?")
            .bind(mid)
            .fetch_optional(&st.db)
            .await?;
    let p = verifier(&snap.actors, &verse);
    let bloquants = p.iter().filter(|x| x["gravite"] == "bloquant").count();
    Ok(json!({
        "problemes": p, "bloquants": bloquants, "a_verifier": p.len() - bloquants,
        "acteurs": snap.actors.len(), "fichiers_verse": verse.len(), "etat_du": maj,
        "note": "Sur le dernier état connu du niveau. Les liens d'événements des appareils classiques ne sont pas lisibles par UEFN MCP et ne sont pas vérifiés.",
    }))
}

pub async fn get(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    Ok(Json(rapport(&st, mid).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regles() {
        let acteurs = vec![
            json!({ "id": "L.Bouton", "name": "Bouton", "type": "device", "class": "Button_C", "tags": [], "options": {}, "props": {} }),
            json!({ "id": "L.Jeu", "name": "Gestion du jeu", "type": "verse_device", "class": "jeu_C", "tags": ["Ennemi"],
                    "options": {}, "props": { "bouton": { "refPath": "L.Bouton" }, "porte": { "refPath": "L.Porte" }, "minuteur": null } }),
        ];
        let verse = vec![(
            "Content/jeu.verse".to_string(),
            "Joueur := class(tag){}\njeu := class(creative_device):".to_string(),
        )];
        let p = verifier(&acteurs, &verse);
        let regles: Vec<&str> = p.iter().map(|x| x["regle"].as_str().unwrap()).collect();
        // Lien cassé (bloquant, en premier), référence vide, tag posé non utilisé, tag défini non posé.
        assert_eq!(p[0]["regle"], "lien_casse");
        assert_eq!(p[0]["gravite"], "bloquant");
        assert!(p[0]["message"].as_str().unwrap().contains("porte"));
        assert_eq!(p[0]["acteur"], "L.Jeu");
        for r in ["reference_vide", "tag_inutilise", "tag_sans_acteur"] {
            assert!(regles.contains(&r), "{r} manquant : {regles:?}");
        }
        // Le lien valide (bouton) ne donne rien.
        assert_eq!(p.iter().filter(|x| x["regle"] == "lien_casse").count(), 1);
        // Niveau propre : aucun problème.
        let propre = vec![
            json!({ "id": "L.Jeu", "name": "Jeu", "type": "verse_device", "tags": ["Joueur"], "options": {}, "props": { "x": 3 } }),
        ];
        assert!(
            verifier(&propre, &verse).is_empty(),
            "{:?}",
            verifier(&propre, &verse)
        );
    }

    #[test]
    fn option_sans_effet() {
        // Cherche dans le catalogue une option qui dépend d'une autre, et la règle de façon à la rendre inactive.
        let classes = [
            "item_spawner",
            "Timer",
            "Barrier",
            "Trigger",
            "Score_Manager",
            "Elimination_Manager",
            "Mutator_Zone",
            "Button",
            "HUD_Message",
            "Player_Spawner",
            "Teleporter",
            "Class_Designer",
        ];
        let mut trouve = false;
        for c in classes {
            let Some((_, _, cat, rules)) = crate::catalog::device(c) else {
                continue;
            };
            for (opt, ctrl, ok, _) in &rules {
                let Some(o) = cat.iter().find(|x| x["key"] == *opt) else {
                    continue;
                };
                let autre = if o["type"] == "enum" {
                    o["values"]
                        .as_array()
                        .and_then(|v| v.iter().find(|x| **x != o["default"]).cloned())
                } else {
                    Some(json!("valeur réglée"))
                };
                let Some(autre) = autre else { continue };
                let a = json!({ "id": "L.A", "name": "Appareil", "type": "device", "class": c, "tags": [],
                    "options": { *opt: autre, *ctrl: format!("pas-{}", ok.first().unwrap_or(&"x")) }, "props": {} });
                let p = verifier(&[a], &[]);
                assert!(
                    p.iter().any(|x| x["regle"] == "option_inactive"),
                    "{c} {opt} : {p:?}"
                );
                trouve = true;
                break;
            }
            if trouve {
                break;
            }
        }
        assert!(trouve, "aucune règle « inactive si » dans le catalogue");
    }

    #[tokio::test]
    async fn rapport_de_la_map() {
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
        crate::uefn::snapshot(&st, mid).await.unwrap();
        let acteurs = json!([{ "id": "L.Jeu", "name": "Jeu", "type": "verse_device", "tags": [], "options": {}, "props": { "porte": { "refPath": "L.Absente" } } }]);
        sqlx::query("UPDATE uefn_state SET actors = ? WHERE map_id = ?")
            .bind(acteurs.to_string())
            .bind(mid)
            .execute(&st.db)
            .await
            .unwrap();
        let r = rapport(&st, mid).await.unwrap();
        assert_eq!(r["bloquants"], 1);
        assert_eq!(r["acteurs"], 1);
        assert_eq!(r["problemes"][0]["acteur"], "L.Jeu");
    }
}
