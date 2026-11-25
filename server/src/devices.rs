//! Catalogue des appareils et graphe des liens entre appareils (#119).
//! Catalogue : lu dans l'Asset Registry d'UEFN par le relais (au plus une fois par jour, gardé par version d'UEFN),
//! cherché en français ou en anglais (« bouton » trouve le Button Device) pour poser un appareil avec place_device.
//! Graphe : liens d'événements des appareils classiques et références @editable des appareils Verse, avec les liens
//! cassés (cible absente du niveau).
use crate::{
    access::map_ctx,
    auth::AuthUser,
    error::{ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Mots français → morceaux de noms d'appareils (anglais) d'UEFN.
const ALIAS: &[(&str, &[&str])] = &[
    ("bouton", &["button"]),
    ("minuteur", &["timer"]),
    ("chrono", &["timer"]),
    ("distributeur", &["granter", "spawner", "vending"]),
    ("vente", &["vending"]),
    ("déclencheur", &["trigger"]),
    ("declencheur", &["trigger"]),
    ("barrière", &["barrier"]),
    ("barriere", &["barrier"]),
    ("apparition", &["spawn"]),
    ("caméra", &["camera"]),
    ("camera", &["camera"]),
    ("musique", &["music", "audio"]),
    ("son", &["audio", "sound"]),
    ("fin", &["end_game", "endgame"]),
    ("équipe", &["team"]),
    ("equipe", &["team"]),
    ("score", &["score"]),
    ("message", &["hud_message", "message"]),
    ("zone", &["zone", "volume"]),
    ("objectif", &["objective"]),
    ("véhicule", &["vehicle"]),
    ("vehicule", &["vehicle"]),
    ("téléporteur", &["teleporter"]),
    ("teleporteur", &["teleporter"]),
    ("porte", &["door"]),
    ("lumière", &["light"]),
    ("lumiere", &["light"]),
    ("réglages", &["island_settings", "settings"]),
];

/// Morceaux à chercher dans le nom : le mot lui-même et ses équivalents anglais.
pub fn termes(q: &str) -> Vec<String> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return vec![];
    }
    let mut out = vec![q.replace(' ', "_")];
    for mot in q.split_whitespace() {
        for (fr, en) in ALIAS {
            if mot == *fr {
                out.extend(en.iter().map(|e| e.to_string()));
            }
        }
    }
    out
}

/// Relit le catalogue dans UEFN si le dernier date de plus d'un jour (ou n'existe pas) et si le relais est là.
async fn rafraichir(st: &AppState, mid: i64) -> Result<(), AppError> {
    let recent: Option<String> = sqlx::query_scalar("SELECT version FROM device_catalog_versions WHERE fetched_at > datetime('now', '-1 day') LIMIT 1")
        .fetch_optional(&st.db)
        .await?;
    if recent.is_some() || st.relays.status(mid) != "connected" {
        return Ok(());
    }
    let r = crate::uefn::relay_call(st, mid, "device_catalog", &json!({}), 120).await?;
    let c = &r["catalog"];
    let version = c["version"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect::<String>();
    if version.is_empty() {
        return Ok(());
    }
    let mut tx = st.db.begin().await?;
    for d in c["devices"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .take(8000)
    {
        let (Some(n), Some(p)) = (d["name"].as_str(), d["path"].as_str()) else {
            continue;
        };
        sqlx::query("INSERT OR IGNORE INTO device_catalog (version, name, path, folder) VALUES (?, ?, ?, ?)")
            .bind(&version)
            .bind(n.chars().take(200).collect::<String>())
            .bind(p.chars().take(400).collect::<String>())
            .bind(d["folder"].as_str().unwrap_or("").chars().take(400).collect::<String>())
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("INSERT INTO device_catalog_versions (version) VALUES (?) ON CONFLICT (version) DO UPDATE SET fetched_at = datetime('now')")
        .bind(&version)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Nom lisible : « PBWA_BP_ButtonDevice_C » → « Button Device ».
fn lisible(n: &str) -> String {
    let mut s = n.trim_end_matches("_C").to_string();
    for p in ["PBWA_", "BP_", "B_"] {
        if let Some(r) = s.strip_prefix(p) {
            s = r.to_string();
        }
    }
    let mut out = String::new();
    for (i, c) in s.replace('_', " ").chars().enumerate() {
        if i > 0 && c.is_uppercase() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push(c);
    }
    out.trim().to_string()
}

/// Cherche dans le catalogue de la version la plus récente (relu au besoin).
pub async fn chercher(st: &AppState, mid: i64, q: &str) -> Result<Value, AppError> {
    if let Err(e) = rafraichir(st, mid).await {
        tracing::warn!("catalogue des appareils : {e:?}");
    }
    let version: Option<String> = sqlx::query_scalar(
        "SELECT version FROM device_catalog_versions ORDER BY fetched_at DESC LIMIT 1",
    )
    .fetch_optional(&st.db)
    .await?;
    let Some(version) = version else {
        return Ok(
            json!({ "version": null, "devices": [], "message": "Catalogue pas encore lu : ouvrez UEFN avec le relais connecté, puis recommencez." }),
        );
    };
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT name, path, folder FROM device_catalog WHERE version = ? ORDER BY name",
    )
    .bind(&version)
    .fetch_all(&st.db)
    .await?;
    let t = termes(q);
    let mut trouves: Vec<(usize, Value)> = rows
        .into_iter()
        .filter_map(|(n, p, f)| {
            let nl = n.to_lowercase().replace(' ', "_");
            let rang = if t.is_empty() {
                Some(0)
            } else {
                t.iter().position(|x| nl.contains(x.as_str()))
            }?;
            let categorie = f.rsplit('/').next().unwrap_or("").to_string();
            Some((
                rang,
                json!({ "name": lisible(&n), "asset": p, "category": categorie }),
            ))
        })
        .collect();
    trouves.sort_by_key(|(r, v)| (*r, v["name"].as_str().map(|s| s.len()).unwrap_or(0)));
    Ok(
        json!({ "version": version, "devices": trouves.into_iter().take(50).map(|x| x.1).collect::<Vec<_>>() }),
    )
}

#[derive(Deserialize, Default)]
pub struct CatQ {
    q: Option<String>,
}

/// `GET /api/maps/{mid}/devices/catalog?q=`
pub async fn route_catalogue(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Query(q): Query<CatQ>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    Ok(Json(
        chercher(&st, mid, q.q.as_deref().unwrap_or("")).await?,
    ))
}

/// Chaînes d'une valeur JSON (références @editable lues dans UEFN).
fn chaines(v: &Value, out: &mut Vec<(String, String)>, cle: &str) {
    match v {
        Value::String(s) => out.push((cle.to_string(), s.clone())),
        Value::Array(a) => a.iter().for_each(|x| chaines(x, out, cle)),
        Value::Object(o) => o
            .iter()
            .for_each(|(k, x)| chaines(x, out, if cle.is_empty() { k } else { cle })),
        _ => {}
    }
}

/// Graphe à partir des acteurs et des réglages lus (id d'appareil Verse → réglages).
pub fn graphe(acteurs: &[Value], reglages: &HashMap<String, Value>) -> Value {
    let par_id: HashMap<&str, &Value> = acteurs
        .iter()
        .filter_map(|a| Some((a["id"].as_str()?, a)))
        .collect();
    let par_nom: HashMap<&str, &str> = acteurs
        .iter()
        .filter_map(|a| Some((a["name"].as_str()?, a["id"].as_str()?)))
        .collect();
    let appareil = |a: &Value| matches!(a["type"].as_str(), Some("device" | "verse_device"));
    let noeuds: Vec<Value> = acteurs
        .iter()
        .filter(|a| appareil(a))
        .map(
            |a| json!({ "id": a["id"], "name": a["name"], "type": a["type"], "class": a["class"] }),
        )
        .collect();
    let mut liens = vec![];
    for a in acteurs.iter().filter(|a| appareil(a)) {
        let de = a["id"].as_str().unwrap_or("");
        for e in a["events"].as_array().cloned().unwrap_or_default() {
            let vers = e["target"].as_str().unwrap_or("");
            liens.push(json!({ "from": de, "to": vers, "kind": "événement", "label": format!("{} → {}", e["event"].as_str().unwrap_or(""), e["function"].as_str().unwrap_or("")), "broken": !par_id.contains_key(vers) }));
        }
        if let Some(r) = reglages.get(de) {
            let mut c = vec![];
            chaines(r, &mut c, "");
            for (cle, s) in c {
                if let Some(id) = par_id
                    .get(s.as_str())
                    .and_then(|x| x["id"].as_str())
                    .or_else(|| par_nom.get(s.as_str()).copied())
                {
                    if id != de {
                        liens.push(json!({ "from": de, "to": id, "kind": "@editable", "label": cle, "broken": false }));
                    }
                } else if s.contains("PersistentLevel.") || s.contains(":PersistentLevel") {
                    // Référence à un acteur du niveau qui n'existe plus : lien cassé.
                    liens.push(json!({ "from": de, "to": s, "kind": "@editable", "label": cle, "broken": true }));
                }
            }
        }
    }
    let casses = liens.iter().filter(|l| l["broken"] == true).count();
    json!({ "nodes": noeuds, "links": liens, "broken": casses })
}

/// Graphe de la map : lit les réglages des appareils Verse (30 au plus) dans UEFN quand le relais est là.
pub async fn graphe_de_la_map(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let snap = crate::uefn::snapshot(st, mid).await?;
    let mut reglages = HashMap::new();
    let connecte = st.relays.status(mid) == "connected";
    if connecte {
        for a in snap
            .actors
            .iter()
            .filter(|a| a["type"] == "verse_device")
            .take(30)
        {
            let id = a["id"].as_str().unwrap_or("").to_string();
            if let Ok(r) = crate::uefn::relay_call(
                st,
                mid,
                "actor_props",
                &json!({ "id": id, "name": a["name"] }),
                45,
            )
            .await
            {
                reglages.insert(id, r["props"].clone());
            }
        }
    }
    let mut g = graphe(&snap.actors, &reglages);
    g["verse_props_read"] = json!(connecte);
    Ok(g)
}

/// `GET /api/maps/{mid}/devices/graph`
pub async fn route_graphe(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    Ok(Json(graphe_de_la_map(&st, mid).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn catalogue_en_francais() {
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
        assert_eq!(
            chercher(&st, mid, "bouton").await.unwrap()["version"],
            Value::Null,
            "pas encore lu, relais absent"
        );
        sqlx::query("INSERT INTO device_catalog_versions (version) VALUES ('5.6.0-Fortnite')")
            .execute(&st.db)
            .await
            .unwrap();
        for (n, p) in [
            (
                "PBWA_BP_ButtonDevice_C",
                "/Fortnite/Devices/Button/PBWA_BP_ButtonDevice.PBWA_BP_ButtonDevice_C",
            ),
            (
                "BP_TimerDevice_C",
                "/Fortnite/Devices/Timer/BP_TimerDevice.BP_TimerDevice_C",
            ),
            (
                "BP_ItemGranterDevice_C",
                "/Fortnite/Devices/ItemGranter/BP_ItemGranterDevice.BP_ItemGranterDevice_C",
            ),
        ] {
            sqlx::query("INSERT INTO device_catalog (version, name, path, folder) VALUES ('5.6.0-Fortnite', ?, ?, ?)").bind(n).bind(p).bind(p.rsplit_once('/').unwrap().0).execute(&st.db).await.unwrap();
        }
        let r = chercher(&st, mid, "bouton").await.unwrap();
        assert_eq!(r["devices"][0]["name"], "Button Device", "{r}");
        assert!(r["devices"][0]["asset"]
            .as_str()
            .unwrap()
            .starts_with("/Fortnite/Devices/Button/"));
        assert_eq!(
            chercher(&st, mid, "distributeur").await.unwrap()["devices"][0]["name"],
            "Item Granter Device"
        );
        assert_eq!(
            chercher(&st, mid, "timer").await.unwrap()["devices"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            chercher(&st, mid, "").await.unwrap()["devices"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn graphe_et_liens_casses() {
        let acteurs = vec![
            json!({ "id": "L:PersistentLevel.Verse_1", "name": "jeu", "type": "verse_device", "class": "VerseDevice_C", "events": [] }),
            json!({ "id": "L:PersistentLevel.Button_1", "name": "Bouton départ", "type": "device", "class": "Button_C",
                    "events": [{ "event": "InteractedWithEvent", "target": "L:PersistentLevel.Timer_9", "function": "Start" }] }),
            json!({ "id": "L:PersistentLevel.Arbre", "name": "Arbre", "type": "prop", "class": "Tree", "events": [] }),
        ];
        let reglages = HashMap::from([(
            "L:PersistentLevel.Verse_1".to_string(),
            json!({ "BoutonDepart": "L:PersistentLevel.Button_1", "Minuteur": "L:PersistentLevel.Timer_2", "Titre": "Bonjour" }),
        )]);
        let g = graphe(&acteurs, &reglages);
        assert_eq!(
            g["nodes"].as_array().unwrap().len(),
            2,
            "les props ne sont pas des appareils"
        );
        let l = g["links"].as_array().unwrap();
        assert!(l.iter().any(|x| x["from"] == "L:PersistentLevel.Verse_1"
            && x["to"] == "L:PersistentLevel.Button_1"
            && x["broken"] == false
            && x["label"] == "BoutonDepart"));
        assert!(l
            .iter()
            .any(|x| x["to"] == "L:PersistentLevel.Timer_2" && x["broken"] == true));
        assert!(
            l.iter()
                .any(|x| x["kind"] == "événement" && x["broken"] == true),
            "cible d'événement absente"
        );
        assert_eq!(g["broken"], 2);
        assert_eq!(l.len(), 3, "« Bonjour » n'est pas une référence");
    }
}
