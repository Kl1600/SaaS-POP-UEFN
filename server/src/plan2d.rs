//! Plan 2D de la map, étage par étage, à l'échelle (#162). Positions et tailles en centimètres, comme le panneau
//! Détails d'UEFN (X vers l'avant, Y vers la droite). Un écran « Plan 2D » par map, le calque « niveau réel » lu dans
//! l'instantané du relais, et deux outils MCP (lire, écrire) pour qu'une IA propose ou applique un plan.
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

/// Sortes d'objets du plan. Les appareils reprennent la bibliothèque UEFN du tableau blanc (apparition, vente…).
pub const SORTES: &[&str] = &["zone", "cercle", "polygone", "chemin", "mur", "appareil"];
/// 10 km : au-delà, c'est une faute de frappe (une île UEFN fait au plus quelques kilomètres).
const LIMITE_CM: f64 = 1_000_000.0;

fn ok_nombre(v: f64) -> bool {
    v.is_finite() && v.abs() <= LIMITE_CM
}

/// Le plan de la map : étages (créés au premier accès : « Rez-de-chaussée ») et objets.
pub async fn plan(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plan_floors WHERE map_id = ?")
        .bind(mid)
        .fetch_one(&st.db)
        .await?;
    if n == 0 {
        sqlx::query("INSERT INTO plan_floors (map_id, name, z_min, z_max, position) VALUES (?, 'Rez-de-chaussée', 0, 400, 0)")
            .bind(mid)
            .execute(&st.db)
            .await?;
    }
    let etages: Vec<(i64, String, f64, f64, i64)> =
        sqlx::query_as("SELECT id, name, z_min, z_max, position FROM plan_floors WHERE map_id = ? ORDER BY position, id")
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
    let objets: Vec<(i64, i64, String, String, String, f64, f64, f64, f64, f64, String)> = sqlx::query_as(
        "SELECT id, floor_id, kind, name, device, x, y, w, h, rotation, points FROM plan_objects WHERE map_id = ? ORDER BY id",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(json!({
        "unit": "cm",
        "axes": "X vers l'avant, Y vers la droite (comme le panneau Détails d'UEFN)",
        "floors": etages.into_iter().map(|e| json!({ "id": e.0, "name": e.1, "z_min": e.2, "z_max": e.3, "position": e.4 })).collect::<Vec<_>>(),
        "objects": objets.into_iter().map(|o| json!({
            "id": o.0, "floor_id": o.1, "kind": o.2, "name": o.3, "device": o.4, "x": o.5, "y": o.6, "w": o.7, "h": o.8,
            "rotation": o.9, "points": serde_json::from_str::<Value>(&o.10).unwrap_or(json!([])),
        })).collect::<Vec<_>>(),
    }))
}

#[derive(Deserialize)]
pub struct Etage {
    name: Option<String>,
    z_min: Option<f64>,
    z_max: Option<f64>,
    position: Option<i64>,
}

#[derive(Deserialize, Default)]
pub struct Objet {
    pub floor_id: Option<i64>,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub device: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub w: Option<f64>,
    pub h: Option<f64>,
    pub rotation: Option<f64>,
    pub points: Option<Vec<[f64; 2]>>,
}

async fn etage_de_la_map(st: &AppState, mid: i64, fid: i64) -> Result<(), AppError> {
    let ok: Option<i64> =
        sqlx::query_scalar("SELECT id FROM plan_floors WHERE id = ? AND map_id = ?")
            .bind(fid)
            .bind(mid)
            .fetch_optional(&st.db)
            .await?;
    ok.map(|_| ())
        .ok_or_else(|| bad("Étage introuvable sur cette map."))
}

fn verifier(o: &Objet) -> Result<(), AppError> {
    if let Some(k) = &o.kind {
        if !SORTES.contains(&k.as_str()) {
            return Err(bad(format!(
                "Sorte d'objet inconnue : {k} (zone, cercle, polygone, chemin, mur ou appareil)."
            )));
        }
    }
    if let Some(n) = &o.name {
        check_len("Nom", n, 0, 80)?;
    }
    if let Some(d) = &o.device {
        check_len("Appareil", d, 0, 60)?;
    }
    for v in [o.x, o.y, o.w, o.h, o.rotation].into_iter().flatten() {
        if !ok_nombre(v) {
            return Err(bad(
                "Position ou taille hors limites (centimètres, 10 km au plus).",
            ));
        }
    }
    if o.w.is_some_and(|w| w < 0.0) || o.h.is_some_and(|h| h < 0.0) {
        return Err(bad("Une taille ne peut pas être négative."));
    }
    if let Some(p) = &o.points {
        if p.len() > 500 || p.iter().flatten().any(|v| !ok_nombre(*v)) {
            return Err(bad("Points : 500 au plus, en centimètres."));
        }
    }
    Ok(())
}

/// Crée un objet ; rend son id.
pub async fn creer(st: &AppState, ctx: &MapCtx, o: &Objet) -> Result<i64, AppError> {
    ctx.can_edit()?;
    verifier(o)?;
    let fid = match o.floor_id {
        Some(f) => f,
        None => {
            plan(st, ctx.map_id).await?;
            sqlx::query_scalar(
                "SELECT id FROM plan_floors WHERE map_id = ? ORDER BY position, id LIMIT 1",
            )
            .bind(ctx.map_id)
            .fetch_one(&st.db)
            .await?
        }
    };
    etage_de_la_map(st, ctx.map_id, fid).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO plan_objects (map_id, floor_id, kind, name, device, x, y, w, h, rotation, points, updated_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(ctx.map_id)
    .bind(fid)
    .bind(o.kind.as_deref().unwrap_or("zone"))
    .bind(o.name.as_deref().unwrap_or("").trim())
    .bind(o.device.as_deref().unwrap_or(""))
    .bind(o.x.unwrap_or(0.0))
    .bind(o.y.unwrap_or(0.0))
    .bind(o.w.unwrap_or(1000.0))
    .bind(o.h.unwrap_or(1000.0))
    .bind(o.rotation.unwrap_or(0.0))
    .bind(json!(o.points.clone().unwrap_or_default()).to_string())
    .bind(ctx.user.id)
    .fetch_one(&st.db)
    .await?;
    notify(st, ctx.map_id, json!({ "t": "refresh", "what": "plan2d" }));
    Ok(id)
}

pub async fn modifier(st: &AppState, ctx: &MapCtx, id: i64, o: &Objet) -> Result<(), AppError> {
    ctx.can_edit()?;
    verifier(o)?;
    if let Some(f) = o.floor_id {
        etage_de_la_map(st, ctx.map_id, f).await?;
    }
    let n = sqlx::query(
        "UPDATE plan_objects SET floor_id = COALESCE(?, floor_id), kind = COALESCE(?, kind), name = COALESCE(?, name), device = COALESCE(?, device),
           x = COALESCE(?, x), y = COALESCE(?, y), w = COALESCE(?, w), h = COALESCE(?, h), rotation = COALESCE(?, rotation),
           points = COALESCE(?, points), updated_by = ?, updated_at = datetime('now') WHERE id = ? AND map_id = ?",
    )
    .bind(o.floor_id)
    .bind(&o.kind)
    .bind(o.name.as_deref().map(str::trim))
    .bind(&o.device)
    .bind(o.x)
    .bind(o.y)
    .bind(o.w)
    .bind(o.h)
    .bind(o.rotation)
    .bind(o.points.as_ref().map(|p| json!(p).to_string()))
    .bind(ctx.user.id)
    .bind(id)
    .bind(ctx.map_id)
    .execute(&st.db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    notify(st, ctx.map_id, json!({ "t": "refresh", "what": "plan2d" }));
    Ok(())
}

pub async fn route_plan(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    Ok(Json(plan(&st, mid).await?))
}

pub async fn route_creer(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(o): Json<Objet>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    Ok(Json(json!({ "id": creer(&st, &ctx, &o).await? })))
}

pub async fn route_modifier(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
    Json(o): Json<Objet>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    modifier(&st, &ctx, id, &o).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn route_supprimer(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    sqlx::query("DELETE FROM plan_objects WHERE id = ? AND map_id = ?")
        .bind(id)
        .bind(mid)
        .execute(&st.db)
        .await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "plan2d" }));
    Ok(Json(json!({ "ok": true })))
}

fn verifier_etage(e: &Etage) -> Result<(), AppError> {
    if let Some(n) = &e.name {
        check_len("Nom de l'étage", n.trim(), 1, 40)?;
    }
    if [e.z_min, e.z_max]
        .into_iter()
        .flatten()
        .any(|v| !ok_nombre(v))
    {
        return Err(bad("Hauteurs hors limites (centimètres)."));
    }
    if let (Some(a), Some(b)) = (e.z_min, e.z_max) {
        if a >= b {
            return Err(bad("Z min doit être plus bas que Z max."));
        }
    }
    Ok(())
}

pub async fn route_creer_etage(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(e): Json<Etage>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    verifier_etage(&e)?;
    plan(&st, mid).await?;
    let (pos, haut): (i64, f64) = sqlx::query_as("SELECT COALESCE(MAX(position), -1) + 1, COALESCE(MAX(z_max), 0) FROM plan_floors WHERE map_id = ?")
        .bind(mid)
        .fetch_one(&st.db)
        .await?;
    let id: i64 = sqlx::query_scalar("INSERT INTO plan_floors (map_id, name, z_min, z_max, position) VALUES (?, ?, ?, ?, ?) RETURNING id")
        .bind(mid)
        .bind(e.name.as_deref().map(str::trim).unwrap_or("Étage"))
        .bind(e.z_min.unwrap_or(haut))
        .bind(e.z_max.unwrap_or(haut + 400.0))
        .bind(e.position.unwrap_or(pos))
        .fetch_one(&st.db)
        .await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "plan2d" }));
    Ok(Json(json!({ "id": id })))
}

pub async fn route_modifier_etage(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
    Json(e): Json<Etage>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    verifier_etage(&e)?;
    let n = sqlx::query("UPDATE plan_floors SET name = COALESCE(?, name), z_min = COALESCE(?, z_min), z_max = COALESCE(?, z_max), position = COALESCE(?, position) WHERE id = ? AND map_id = ?")
        .bind(e.name.as_deref().map(str::trim))
        .bind(e.z_min)
        .bind(e.z_max)
        .bind(e.position)
        .bind(id)
        .bind(mid)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    notify(&st, mid, json!({ "t": "refresh", "what": "plan2d" }));
    Ok(Json(json!({ "ok": true })))
}

pub async fn route_supprimer_etage(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plan_floors WHERE map_id = ?")
        .bind(mid)
        .fetch_one(&st.db)
        .await?;
    if n <= 1 {
        return Err(bad("Le plan garde au moins un étage."));
    }
    sqlx::query("DELETE FROM plan_floors WHERE id = ? AND map_id = ?")
        .bind(id)
        .bind(mid)
        .execute(&st.db)
        .await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "plan2d" }));
    Ok(Json(json!({ "ok": true })))
}

/// Calque « niveau réel » : les acteurs de l'instantané UEFN vus de dessus (position, et taille quand le relais
/// fournit les bornes), avec leur dossier pour filtrer.
pub async fn niveau(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let snap = crate::uefn::snapshot(st, mid).await?;
    let acteurs: Vec<Value> = snap
        .actors
        .iter()
        .filter_map(|a| {
            let l = a["transform"]["location"].as_array()?;
            let (x, y, z) = (l.first()?.as_f64()?, l.get(1)?.as_f64()?, l.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0));
            // Bornes vues de dessus : centre et demi-tailles en cm (get_actor_bounds) ; sinon un repère d'un mètre.
            let b = &a["bounds"];
            let (cx, cy, ex, ey) = match (b["center"].as_array(), b["extent"].as_array()) {
                (Some(c), Some(e)) => (c.first()?.as_f64()?, c.get(1)?.as_f64()?, e.first()?.as_f64()?, e.get(1)?.as_f64()?),
                _ => (x, y, 50.0, 50.0),
            };
            Some(json!({ "id": a["id"], "name": a["name"], "type": a["type"], "folder": a["folder"], "x": cx, "y": cy, "z": z,
                "w": ex * 2.0, "h": ey * 2.0, "yaw": a["transform"]["rotation"][1], "sized": b.is_object() }))
        })
        .collect();
    Ok(json!({ "unit": "cm", "actors": acteurs }))
}

pub async fn route_niveau(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    Ok(Json(niveau(&st, mid).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn plan_etages_objets_et_droits() {
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
        let autre: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'A') RETURNING id")
                .bind(sid)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let mut us = vec![];
        for (m, r) in [("e@a.fr", "member"), ("v@a.fr", "viewer")] {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
            )
            .bind(m)
            .bind(m)
            .fetch_one(&st.db)
            .await
            .unwrap();
            sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, ?)")
                .bind(sid)
                .bind(id)
                .bind(r)
                .execute(&st.db)
                .await
                .unwrap();
            us.push(AuthUser {
                id,
                email: m.into(),
                name: m.into(),
                sid: String::new(),
            });
        }
        let Json(p) = route_plan(State(st.clone()), us[0].clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(
            p["floors"].as_array().unwrap().len(),
            1,
            "rez-de-chaussée créé"
        );
        let Json(e) = route_creer_etage(
            State(st.clone()),
            us[0].clone(),
            Path(mid),
            Json(Etage {
                name: Some("Étage 1".into()),
                z_min: None,
                z_max: None,
                position: None,
            }),
        )
        .await
        .unwrap();
        let f1 = e["id"].as_i64().unwrap();
        let ctx = map_ctx(&st, &us[0], mid).await.unwrap();
        let zone = Objet {
            floor_id: Some(f1),
            kind: Some("zone".into()),
            name: Some("Arène".into()),
            x: Some(1000.0),
            y: Some(-500.0),
            w: Some(2000.0),
            h: Some(2000.0),
            ..Default::default()
        };
        let id = creer(&st, &ctx, &zone).await.unwrap();
        let Json(p) = route_plan(State(st.clone()), us[0].clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(
            p["floors"][1]["z_min"], 400.0,
            "nouvel étage au-dessus du précédent"
        );
        let o = &p["objects"][0];
        assert_eq!(
            (o["floor_id"].as_i64(), o["w"].as_f64(), o["h"].as_f64()),
            (Some(f1), Some(2000.0), Some(2000.0)),
            "20 m × 20 m en cm"
        );
        modifier(
            &st,
            &ctx,
            id,
            &Objet {
                x: Some(1500.0),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let Json(p) = route_plan(State(st.clone()), us[0].clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(
            (
                p["objects"][0]["x"].as_f64(),
                p["objects"][0]["name"].as_str()
            ),
            (Some(1500.0), Some("Arène"))
        );
        // Refus : sorte inconnue, taille absurde, étage d'une autre map, lecteur.
        assert!(creer(
            &st,
            &ctx,
            &Objet {
                kind: Some("licorne".into()),
                ..Default::default()
            }
        )
        .await
        .is_err());
        assert!(creer(
            &st,
            &ctx,
            &Objet {
                w: Some(1e9),
                ..Default::default()
            }
        )
        .await
        .is_err());
        let Json(pa) = route_plan(State(st.clone()), us[0].clone(), Path(autre))
            .await
            .unwrap();
        let fa = pa["floors"][0]["id"].as_i64().unwrap();
        assert!(creer(
            &st,
            &ctx,
            &Objet {
                floor_id: Some(fa),
                ..Default::default()
            }
        )
        .await
        .is_err());
        let vctx = map_ctx(&st, &us[1], mid).await.unwrap();
        assert!(matches!(
            creer(&st, &vctx, &zone).await,
            Err(AppError::Forbidden)
        ));
        // Le dernier étage ne se supprime pas ; supprimer un étage supprime ses objets.
        route_supprimer_etage(State(st.clone()), us[0].clone(), Path((mid, f1)))
            .await
            .unwrap();
        let Json(p) = route_plan(State(st.clone()), us[0].clone(), Path(mid))
            .await
            .unwrap();
        assert_eq!(p["objects"].as_array().unwrap().len(), 0);
        let f0 = p["floors"][0]["id"].as_i64().unwrap();
        assert!(
            route_supprimer_etage(State(st.clone()), us[0].clone(), Path((mid, f0)))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn calque_niveau_reel() {
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
        let acteurs = json!([
            { "id": "a", "name": "Terrain Golf", "folder": "Terrains", "type": "mesh", "transform": { "location": [12800.0, 0.0, 0.0], "rotation": [0, 90, 0] },
              "bounds": { "center": [12800.0, 50.0, 0.0], "extent": [6400.0, 6400.0, 10.0] } },
            { "id": "b", "name": "Vente", "folder": "Appareils", "type": "device", "transform": { "location": [100.0, 200.0, 0.0], "rotation": [0, 0, 0] } },
        ]);
        sqlx::query("UPDATE uefn_state SET actors = ? WHERE map_id = ?")
            .bind(acteurs.to_string())
            .bind(mid)
            .execute(&st.db)
            .await
            .unwrap();
        let n = niveau(&st, mid).await.unwrap();
        let a = &n["actors"];
        assert_eq!(
            (
                a[0]["x"].as_f64(),
                a[0]["y"].as_f64(),
                a[0]["w"].as_f64(),
                a[0]["sized"].as_bool()
            ),
            (Some(12800.0), Some(50.0), Some(12800.0), Some(true))
        );
        assert_eq!(
            (
                a[1]["x"].as_f64(),
                a[1]["w"].as_f64(),
                a[1]["sized"].as_bool()
            ),
            (Some(100.0), Some(100.0), Some(false))
        );
    }
}
