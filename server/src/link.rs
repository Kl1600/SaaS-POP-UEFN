//! Liaison UEFN simplifiée : POP Forge relie tous les projets UEFN du PC avec un code de studio (#404).
//! Au premier lancement, le relais envoie les projets UEFN trouvés sur le PC ; POP crée une map par projet.
use crate::{
    access::{studio_role, Role},
    auth::{random_token, sha256_hex, AuthUser},
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Code de studio valable 24 h (#404) : POP Forge l'utilise pour créer la map de chaque projet UEFN du PC et les
/// relier d'un coup (`/api/relay/pair-studio`). Remplace le pop-link.exe « déjà configuré ».
pub async fn code(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let code = random_token()[..24].to_string();
    sqlx::query("INSERT INTO studio_pairings (code_hash, studio_id, user_id, expires_at) VALUES (?, ?, ?, datetime('now', '+1 day'))")
        .bind(sha256_hex(&code)).bind(sid).bind(u.id).execute(&st.db).await?;
    Ok(Json(json!({ "code": code })))
}

#[derive(Deserialize)]
pub struct ProjectIn {
    name: String,
    /// Espace Creator du projet (`klmaps@fortnite.com`), lu par le relais dans `projectVersePath` (#143).
    #[serde(default)]
    creator_space: Option<String>,
    /// Identifiant Lore du projet (`.lore/id`), s'il existe.
    #[serde(default)]
    lore_id: Option<String>,
}

/// Espace Creator normalisé : `klmaps@fortnite.com`, en minuscules ; vide → aucun.
pub fn espace(s: Option<&str>) -> Option<String> {
    let s = s?.trim().trim_matches('/').to_lowercase();
    let s = s.split('/').next().unwrap_or("").to_string();
    (!s.is_empty() && s.len() <= 120 && s.contains('@')).then_some(s)
}

/// Studio où ranger un projet de l'espace `space` (#143) : celui relié à cet espace dont la personne est admin,
/// sinon celui du code de liaison.
async fn studio_cible(
    st: &AppState,
    uid: i64,
    sid: i64,
    space: Option<&str>,
) -> Result<i64, AppError> {
    let Some(space) = space else { return Ok(sid) };
    let relie: Option<i64> = sqlx::query_scalar(
        "SELECT s.id FROM studios s JOIN memberships m ON m.studio_id = s.id WHERE s.creator_space = ? AND m.user_id = ? AND m.role IN ('owner', 'admin') ORDER BY s.id LIMIT 1",
    )
    .bind(space)
    .bind(uid)
    .fetch_optional(&st.db)
    .await?;
    Ok(relie.unwrap_or(sid))
}

/// Map déjà créée pour ce projet (#143) : même id Lore dans un studio de la personne, sinon même nom dans le
/// studio cible, sinon même nom et même espace Creator dans un studio de la personne.
async fn map_existante(
    st: &AppState,
    uid: i64,
    cible: i64,
    name: &str,
    space: Option<&str>,
    lore: Option<&str>,
) -> Result<Option<i64>, AppError> {
    if let Some(l) = lore {
        let m: Option<i64> = sqlx::query_scalar("SELECT p.id FROM maps p JOIN memberships m ON m.studio_id = p.studio_id WHERE m.user_id = ? AND p.lore_id = ? ORDER BY p.id LIMIT 1")
            .bind(uid).bind(l).fetch_optional(&st.db).await?;
        if m.is_some() {
            return Ok(m);
        }
    }
    let m: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM maps WHERE studio_id = ? AND lower(name) = lower(?) ORDER BY id LIMIT 1",
    )
    .bind(cible)
    .bind(name)
    .fetch_optional(&st.db)
    .await?;
    if m.is_some() {
        return Ok(m);
    }
    let Some(space) = space else { return Ok(None) };
    Ok(sqlx::query_scalar("SELECT p.id FROM maps p JOIN memberships m ON m.studio_id = p.studio_id WHERE m.user_id = ? AND lower(p.name) = lower(?) AND p.creator_space = ? ORDER BY p.id LIMIT 1")
        .bind(uid).bind(name).bind(space).fetch_optional(&st.db).await?)
}

#[derive(Deserialize)]
pub struct PairStudioIn {
    code: String,
    device: Option<String>,
    projects: Vec<ProjectIn>,
}

/// Appelé par le relais : une map par projet UEFN (réutilise la map du même nom), un jeton par map.
pub async fn pair_studio(
    State(st): State<AppState>,
    Json(b): Json<PairStudioIn>,
) -> ApiResult<Value> {
    let row: Option<(i64, i64)> = sqlx::query_as("SELECT studio_id, user_id FROM studio_pairings WHERE code_hash = ? AND expires_at > datetime('now')").bind(sha256_hex(b.code.trim())).fetch_optional(&st.db).await?;
    let (sid, uid) = row.ok_or_else(|| {
        bad("Code expiré : relancez « Créer les maps et relier » dans POP Forge.")
    })?;
    if b.projects.is_empty() {
        return Err(bad(
            "Aucun projet UEFN trouvé sur ce PC (Documents\\Fortnite Projects).",
        ));
    }
    let device = b
        .device
        .unwrap_or_else(|| "PC UEFN".into())
        .chars()
        .take(60)
        .collect::<String>();
    // Le premier espace Creator vu en solo devient celui de la personne (le plus fréquent de cette liaison) :
    // ses propres projets ne sont pas proposés « à ranger » (#143).
    if crate::studio::is_personal(&st, sid).await? {
        let a: Option<String> = sqlx::query_scalar("SELECT creator_space FROM users WHERE id = ?")
            .bind(uid)
            .fetch_one(&st.db)
            .await?;
        if a.is_none() {
            let mut n: Vec<(String, usize)> = vec![];
            for p in &b.projects {
                if let Some(e) = espace(p.creator_space.as_deref()) {
                    match n.iter_mut().find(|x| x.0 == e) {
                        Some(x) => x.1 += 1,
                        None => n.push((e, 1)),
                    }
                }
            }
            if let Some((e, _)) = n.iter().fold(None::<&(String, usize)>, |m, x| match m {
                Some(m) if m.1 >= x.1 => Some(m),
                _ => Some(x),
            }) {
                sqlx::query("UPDATE users SET creator_space = ? WHERE id = ?")
                    .bind(e)
                    .bind(uid)
                    .execute(&st.db)
                    .await?;
            }
        }
    }
    let mut links = vec![];
    for p in b.projects.iter().take(40) {
        let name: String = p.name.trim().chars().take(80).collect();
        if name.is_empty() {
            continue;
        }
        let space = espace(p.creator_space.as_deref());
        let lore: Option<String> = p
            .lore_id
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty() && l.len() <= 100)
            .map(String::from);
        let cible = studio_cible(&st, uid, sid, space.as_deref()).await?;
        let mid =
            match map_existante(&st, uid, cible, &name, space.as_deref(), lore.as_deref()).await? {
                Some(m) => m,
                None => crate::studio::insert_map(&st, cible, &name, "", None).await?,
            };
        sqlx::query("UPDATE maps SET creator_space = COALESCE(?, creator_space), lore_id = COALESCE(?, lore_id) WHERE id = ?")
            .bind(&space)
            .bind(&lore)
            .bind(mid)
            .execute(&st.db)
            .await?;
        // Un seul relais actif par map et par PC : on remplace l'ancien jeton du même appareil.
        sqlx::query("DELETE FROM relay_devices WHERE map_id = ? AND user_id = ? AND name = ?")
            .bind(mid)
            .bind(uid)
            .bind(&device)
            .execute(&st.db)
            .await?;
        let token = random_token();
        sqlx::query(
            "INSERT INTO relay_devices (map_id, user_id, name, token_hash) VALUES (?, ?, ?, ?)",
        )
        .bind(mid)
        .bind(uid)
        .bind(&device)
        .bind(sha256_hex(&token))
        .execute(&st.db)
        .await?;
        links.push(json!({ "project": name, "map_id": mid, "token": token }));
    }
    sqlx::query("UPDATE studio_pairings SET used_at = datetime('now') WHERE code_hash = ?")
        .bind(sha256_hex(b.code.trim()))
        .execute(&st.db)
        .await?;
    if let Some(m) = links.first().and_then(|l| l["map_id"].as_i64()) {
        crate::access::notify(&st, m, json!({ "t": "refresh", "what": "me" }));
    }
    Ok(Json(json!({ "links": links })))
}

/// Pour l'écran d'accueil : le studio a-t-il des maps et des relais connectés ?
pub async fn studio_status(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    let maps: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM maps WHERE studio_id = ? ORDER BY id")
            .bind(sid)
            .fetch_all(&st.db)
            .await?;
    Ok(Json(
        json!({ "maps": maps.iter().map(|(id, n)| json!({ "id": id, "name": n, "status": st.relays.status(*id) })).collect::<Vec<_>>() }),
    ))
}

/// « Continuer sans UEFN » : une première map vide.
pub async fn skip(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<Value>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let name = b["name"]
        .as_str()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("Ma première map")
        .chars()
        .take(80)
        .collect::<String>();
    let mid = crate::studio::insert_map(&st, sid, &name, "", None).await?;
    Ok(Json(json!({ "map_id": mid })))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn user(st: &AppState, email: &str) -> (i64, AuthUser) {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
        )
        .bind(email)
        .bind(email)
        .fetch_one(&st.db)
        .await
        .unwrap();
        let u = AuthUser {
            id,
            email: email.into(),
            name: email.into(),
            sid: String::new(),
        };
        (crate::studio::ensure_personal(st, id).await.unwrap(), u)
    }

    async fn relier(st: &AppState, sid: i64, uid: i64, projets: Value) -> Value {
        let code = random_token()[..24].to_string();
        sqlx::query("INSERT INTO studio_pairings (code_hash, studio_id, user_id, expires_at) VALUES (?, ?, ?, datetime('now', '+1 day'))")
            .bind(sha256_hex(&code)).bind(sid).bind(uid).execute(&st.db).await.unwrap();
        let b: PairStudioIn =
            serde_json::from_value(json!({ "code": code, "device": "PC", "projects": projets }))
                .unwrap();
        pair_studio(State(st.clone()), Json(b)).await.unwrap().0
    }

    async fn studio_de(st: &AppState, nom: &str) -> i64 {
        sqlx::query_scalar("SELECT studio_id FROM maps WHERE name = ?")
            .bind(nom)
            .fetch_one(&st.db)
            .await
            .unwrap()
    }

    /// Les 10 projets du PC de test (#143) : relevé du 21/11 dans l'issue.
    fn projets() -> Value {
        let k = "/klmaps@fortnite.com/VaultRush";
        let b = "/beysir@fortnite.com/X";
        json!([
            { "name": "VaultRush", "creator_space": k, "lore_id": "lore-vault" },
            { "name": "TestPopHub", "creator_space": "/klmaps@fortnite.com/TestPopHub" },
            { "name": "AuraDuels", "creator_space": b, "lore_id": "lore-aura" },
            { "name": "Escape", "creator_space": b },
            { "name": "HeistOnRails", "creator_space": b },
            { "name": "InfintyBattleKPop", "creator_space": b },
            { "name": "CTFBasic", "creator_space": "" },
            { "name": "DarkConvoy_MVP" },
            { "name": "First_Island" },
            { "name": "PinBallMadness" }
        ])
    }

    #[tokio::test]
    async fn ranger_par_espace() {
        let st = crate::test_state().await;
        let (perso, u) = user(&st, "kl@a.fr").await;
        // 1. Tout arrive en solo ; seuls VaultRush et TestPopHub sont « à ranger · klmaps ».
        relier(&st, perso, u.id, projets()).await;
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM maps WHERE studio_id = ?")
            .bind(perso)
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(n, 10);
        let r = crate::ranger::liste(State(st.clone()), u.clone())
            .await
            .unwrap()
            .0;
        assert_eq!(r["mine"], "beysir@fortnite.com");
        assert_eq!(r["groups"].as_array().unwrap().len(), 1, "{r}");
        assert_eq!(r["groups"][0]["label"], "klmaps");
        let noms: Vec<&str> = r["groups"][0]["maps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["name"].as_str().unwrap())
            .collect();
        assert_eq!(noms, ["TestPopHub", "VaultRush"]);
        // 2. Créer le studio et y passer les maps, relier l'espace (ce que fait l'interface).
        let vault = r["groups"][0]["maps"][1]["id"].as_i64().unwrap();
        let hub = r["groups"][0]["maps"][0]["id"].as_i64().unwrap();
        let m: crate::studio::MoveIn =
            serde_json::from_value(json!({ "new_studio": "klmaps" })).unwrap();
        let sid = crate::studio::move_map(State(st.clone()), u.clone(), Path(vault), Json(m))
            .await
            .unwrap()
            .0["studio_id"]
            .as_i64()
            .unwrap();
        let m: crate::studio::MoveIn = serde_json::from_value(json!({ "studio_id": sid })).unwrap();
        crate::studio::move_map(State(st.clone()), u.clone(), Path(hub), Json(m))
            .await
            .unwrap();
        let e: crate::ranger::EspaceIn =
            serde_json::from_value(json!({ "space": "klmaps" })).unwrap();
        crate::ranger::changer_espace_studio(State(st.clone()), u.clone(), Path(sid), Json(e))
            .await
            .unwrap();
        let r = crate::ranger::liste(State(st.clone()), u.clone())
            .await
            .unwrap()
            .0;
        assert!(r["groups"].as_array().unwrap().is_empty(), "{r}");
        // 3. Seconde liaison depuis le perso : aucun doublon, VaultRush reste dans le studio.
        let l = relier(&st, perso, u.id, projets()).await;
        assert_eq!(l["links"].as_array().unwrap().len(), 10);
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM maps")
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(n, 10, "aucune map en double");
        assert_eq!(studio_de(&st, "VaultRush").await, sid);
        assert_eq!(studio_de(&st, "Escape").await, perso);
        // Un nouveau projet klmaps va directement dans le studio relié.
        relier(
            &st,
            perso,
            u.id,
            json!([{ "name": "Nouveau", "creator_space": "/klmaps@fortnite.com/Nouveau" }]),
        )
        .await;
        assert_eq!(studio_de(&st, "Nouveau").await, sid);

        // 4. Un membre non admin du studio klmaps : ses projets klmaps restent chez lui, « à ranger ».
        let (perso2, u2) = user(&st, "membre@a.fr").await;
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')")
            .bind(sid)
            .bind(u2.id)
            .execute(&st.db)
            .await
            .unwrap();
        relier(&st, perso2, u2.id, json!([{ "name": "Autre", "creator_space": "/klmaps@fortnite.com/Autre" }, { "name": "Perso2", "creator_space": "/membre@fortnite.com/P" }, { "name": "Perso3", "creator_space": "/membre@fortnite.com/Q" }])).await;
        assert_eq!(studio_de(&st, "Autre").await, perso2);
        let r = crate::ranger::liste(State(st.clone()), u2.clone())
            .await
            .unwrap()
            .0;
        assert_eq!(r["groups"][0]["label"], "klmaps", "{r}");
        // 5. « Laisser en solo » : le bloc disparaît pour cet espace.
        let e: crate::ranger::EspaceIn =
            serde_json::from_value(json!({ "space": "klmaps@fortnite.com" })).unwrap();
        crate::ranger::laisser_solo(State(st.clone()), u2.clone(), Json(e))
            .await
            .unwrap();
        let r = crate::ranger::liste(State(st.clone()), u2.clone())
            .await
            .unwrap()
            .0;
        assert!(r["groups"].as_array().unwrap().is_empty());
    }
}
