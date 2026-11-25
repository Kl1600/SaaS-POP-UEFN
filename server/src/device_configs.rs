//! Configurations d'appareils réutilisables (#89) : on enregistre un appareil bien réglé (classe, options
//! classiques, réglages @editable) dans la bibliothèque du studio, puis on l'applique à d'autres appareils ou on pose
//! un appareil déjà configuré. Tout passe par `uefn::exec` : journalisé, annulable, refusé sur un acteur verrouillé.
use crate::{
    access::{map_ctx, studio_role, MapCtx, Role},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    uefn::{exec, snapshot, Origin},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

type Ligne = (
    i64,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
);

fn fiche(r: Ligne) -> Value {
    json!({
        "id": r.0, "name": r.1, "description": r.2, "class": r.3, "verse_class": r.4, "asset": r.5,
        "options": serde_json::from_str::<Value>(&r.6).unwrap_or(json!({})),
        "props": serde_json::from_str::<Value>(&r.7).unwrap_or(json!({})),
        "author": r.8, "created_at": r.9,
    })
}

const COLONNES: &str = "c.id, c.name, c.description, c.class, c.verse_class, c.asset, c.options, c.props, u.name, c.created_at";

pub async fn liste(st: &AppState, sid: i64, q: &str) -> Result<Vec<Value>, AppError> {
    let motif = format!("%{}%", q.trim().to_lowercase());
    let rows: Vec<Ligne> = sqlx::query_as(crate::sql!(
        "SELECT {COLONNES} FROM device_configs c LEFT JOIN users u ON u.id = c.author_id
         WHERE c.studio_id = ? AND (lower(c.name) LIKE ? OR lower(c.class) LIKE ? OR lower(c.description) LIKE ?) ORDER BY c.name"
    ))
    .bind(sid)
    .bind(&motif)
    .bind(&motif)
    .bind(&motif)
    .fetch_all(&st.db)
    .await?;
    Ok(rows.into_iter().map(fiche).collect())
}

async fn une(st: &AppState, sid: i64, id: i64) -> Result<Value, AppError> {
    let r: Option<Ligne> = sqlx::query_as(crate::sql!(
        "SELECT {COLONNES} FROM device_configs c LEFT JOIN users u ON u.id = c.author_id WHERE c.id = ? AND c.studio_id = ?"
    ))
    .bind(id)
    .bind(sid)
    .fetch_optional(&st.db)
    .await?;
    r.map(fiche).ok_or(AppError::NotFound)
}

#[derive(Deserialize, Default)]
pub struct ListeQ {
    q: Option<String>,
}

/// `GET /api/studios/{sid}/device-configs?q=`
pub async fn route_liste(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Query(q): Query<ListeQ>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    Ok(Json(json!(
        liste(&st, sid, q.q.as_deref().unwrap_or("")).await?
    )))
}

/// `DELETE /api/studios/{sid}/device-configs/{id}` (membre et plus).
pub async fn route_supprimer(
    State(st): State<AppState>,
    u: AuthUser,
    Path((sid, id)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let n = sqlx::query("DELETE FROM device_configs WHERE id = ? AND studio_id = ?")
        .bind(id)
        .bind(sid)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct Enregistrer {
    pub actor_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Chemin de l'asset de l'appareil (facultatif) : pour le poser dans une map qui n'en a pas encore.
    #[serde(default)]
    pub asset: String,
    /// Options déjà lues (sinon POP les lit dans UEFN par le relais).
    pub options: Option<Value>,
    pub props: Option<Value>,
}

/// Enregistre la configuration d'un appareil du niveau. Les réglages viennent d'UEFN (lecture par le relais).
pub async fn enregistrer(st: &AppState, ctx: &MapCtx, b: &Enregistrer) -> Result<Value, AppError> {
    ctx.require(Role::Member)?;
    let nom = b.name.trim();
    check_len("Nom", nom, 1, 80)?;
    check_len("Description", &b.description, 0, 1000)?;
    check_len("Asset", &b.asset, 0, 300)?;
    let snap = snapshot(st, ctx.map_id).await?;
    let a = snap
        .actors
        .iter()
        .find(|a| a["id"].as_str() == Some(b.actor_id.as_str()))
        .cloned()
        .ok_or_else(|| bad("Appareil introuvable dans le niveau : resynchronisez le projet."))?;
    let (options, props) = match (&b.options, &b.props) {
        (Some(o), p) => (o.clone(), p.clone().unwrap_or(json!({}))),
        (None, Some(p)) => (json!({}), p.clone()),
        (None, None) => {
            let q = serde_json::from_value(json!({ "id": b.actor_id }))
                .map_err(|e| bad(e.to_string()))?;
            let Json(v) = crate::uefn::props(
                State(st.clone()),
                ctx.user.clone(),
                Path(ctx.map_id),
                Query(q),
            )
            .await?;
            (v["options"].clone(), v["props"].clone())
        }
    };
    let objet = |v: &Value| if v.is_object() { v.clone() } else { json!({}) };
    let (options, props) = (objet(&options), objet(&props));
    if options.as_object().is_some_and(|o| o.is_empty())
        && props.as_object().is_some_and(|o| o.is_empty())
    {
        return Err(bad("Cet appareil n'a aucun réglage à enregistrer."));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO device_configs (studio_id, name, description, class, verse_class, asset, options, props, author_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (studio_id, name) DO UPDATE SET description = excluded.description, class = excluded.class, verse_class = excluded.verse_class,
           asset = CASE WHEN excluded.asset <> '' THEN excluded.asset ELSE asset END, options = excluded.options, props = excluded.props,
           author_id = excluded.author_id, created_at = datetime('now')
         RETURNING id",
    )
    .bind(ctx.studio_id)
    .bind(nom)
    .bind(b.description.trim())
    .bind(a["class"].as_str().unwrap_or(""))
    .bind(a["verse_class"].as_str().unwrap_or(""))
    .bind(b.asset.trim())
    .bind(options.to_string())
    .bind(props.to_string())
    .bind(ctx.user.id)
    .fetch_one(&st.db)
    .await?;
    une(st, ctx.studio_id, id).await
}

/// Applique une configuration à des appareils du niveau (même classe), en une action annulable.
pub async fn appliquer(
    st: &AppState,
    o: Origin<'_>,
    cid: i64,
    ids: &[String],
) -> Result<Value, AppError> {
    let c = une(st, o.ctx.studio_id, cid).await?;
    let snap = snapshot(st, o.ctx.map_id).await?;
    for id in ids {
        let a = snap
            .actors
            .iter()
            .find(|a| a["id"].as_str() == Some(id.as_str()))
            .ok_or_else(|| bad(format!("Appareil introuvable : {id}")))?;
        if a["class"].as_str() != c["class"].as_str() {
            return Err(bad(format!(
                "« {} » n'est pas du même type que la configuration « {} » ({}).",
                a["name"].as_str().unwrap_or(id),
                c["name"].as_str().unwrap_or(""),
                c["class"].as_str().unwrap_or("")
            )));
        }
    }
    let mut changes = json!({});
    if c["options"].as_object().is_some_and(|x| !x.is_empty()) {
        changes["options"] = c["options"].clone();
    }
    if c["props"].as_object().is_some_and(|x| !x.is_empty()) {
        changes["props"] = c["props"].clone();
    }
    exec(
        st,
        o,
        "actor_update",
        json!({ "ids": ids, "changes": changes }),
    )
    .await
}

/// Pose un appareil déjà configuré : depuis son asset, ou en copiant un appareil du même type déjà dans le niveau,
/// puis ses réglages sont appliqués (deux actions du journal, annulables).
pub async fn poser(
    st: &AppState,
    ctx: &MapCtx,
    source: &str,
    agent: Option<i64>,
    cid: i64,
    a: &Value,
) -> Result<Value, AppError> {
    let c = une(st, ctx.studio_id, cid).await?;
    let snap = snapshot(st, ctx.map_id).await?;
    let modele = snap
        .actors
        .iter()
        .find(|x| x["class"] == c["class"])
        .and_then(|x| x["id"].as_str().map(String::from));
    let asset = c["asset"].as_str().unwrap_or("");
    if asset.is_empty() && modele.is_none() {
        return Err(bad("Aucun appareil de ce type dans le niveau, et pas d'asset enregistré : posez-en un dans UEFN, ou ajoutez le chemin de l'asset à la configuration."));
    }
    let o = || Origin {
        ctx,
        source,
        agent,
        task: None,
    };
    let mut args = json!({ "name": a["name"].as_str().unwrap_or(c["name"].as_str().unwrap_or("")), "folder": a["folder"], "location": a["location"], "rotation": a["rotation"] });
    if asset.is_empty() {
        args["like"] = json!(modele);
    } else {
        args["asset"] = json!(asset);
    }
    let pose = exec(st, o(), "actor_spawn", args).await?;
    let ids: Vec<String> = pose["created_ids"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|x| x.as_str().map(String::from))
        .collect();
    if ids.is_empty() {
        return Ok(
            json!({ "queued": true, "message": "Pose en attente du relais : appliquez la configuration quand l'appareil sera posé." }),
        );
    }
    let reglages = appliquer(st, o(), cid, &ids).await?;
    Ok(json!({ "created_ids": ids, "applied": reglages }))
}

/// `POST /api/maps/{mid}/device-configs`
pub async fn route_enregistrer(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<Enregistrer>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    Ok(Json(enregistrer(&st, &ctx, &b).await?))
}

#[derive(Deserialize)]
pub struct Appliquer {
    ids: Vec<String>,
}

/// `POST /api/maps/{mid}/device-configs/{id}/apply`
pub async fn route_appliquer(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
    Json(b): Json<Appliquer>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    Ok(Json(
        appliquer(
            &st,
            Origin {
                ctx: &ctx,
                source: "web",
                agent: None,
                task: None,
            },
            id,
            &b.ids,
        )
        .await?,
    ))
}

/// `POST /api/maps/{mid}/device-configs/{id}/place`
pub async fn route_poser(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
    Json(b): Json<Value>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    Ok(Json(poser(&st, &ctx, "web", None, id, &b).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enregistrer_lister_et_refus() {
        let st = crate::test_state().await;
        let q = |s: &'static str| sqlx::query_scalar::<_, i64>(s);
        let sid = q("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let autre = q("INSERT INTO studios (name) VALUES ('A') RETURNING id")
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
        for (m, r, s) in [
            ("m@a.fr", "member", sid),
            ("v@a.fr", "viewer", sid),
            ("x@a.fr", "owner", autre),
        ] {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
            )
            .bind(m)
            .bind(m)
            .fetch_one(&st.db)
            .await
            .unwrap();
            sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, ?)")
                .bind(s)
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
        snapshot(&st, mid).await.unwrap();
        let acteurs = json!([
            { "id": "/Game/L.L:B1", "name": "Bouton 1", "type": "device", "class": "BP_ButtonDevice_C" },
            { "id": "/Game/L.L:T1", "name": "Minuteur", "type": "device", "class": "BP_TimerDevice_C" },
        ]);
        sqlx::query("UPDATE uefn_state SET actors = ? WHERE map_id = ?")
            .bind(acteurs.to_string())
            .bind(mid)
            .execute(&st.db)
            .await
            .unwrap();
        let ctx = map_ctx(&st, &us[0], mid).await.unwrap();
        let b = |nom: &str, opts: Value| Enregistrer {
            actor_id: "/Game/L.L:B1".into(),
            name: nom.into(),
            description: "Bouton de départ".into(),
            asset: String::new(),
            options: Some(opts),
            props: None,
        };
        assert!(
            enregistrer(&st, &ctx, &b("Vide", json!({}))).await.is_err(),
            "rien à enregistrer"
        );
        let c = enregistrer(
            &st,
            &ctx,
            &b(
                "Bouton départ",
                json!({ "InteractTime": 2.0, "Text": "Jouer" }),
            ),
        )
        .await
        .unwrap();
        assert_eq!(
            (c["class"].as_str(), c["options"]["Text"].as_str()),
            (Some("BP_ButtonDevice_C"), Some("Jouer"))
        );
        // Même nom : mise à jour, pas de doublon.
        enregistrer(
            &st,
            &ctx,
            &b("Bouton départ", json!({ "InteractTime": 3.0 })),
        )
        .await
        .unwrap();
        let l = liste(&st, sid, "bouton").await.unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0]["options"]["InteractTime"], 3.0);
        // Lecteur : pas d'enregistrement ; autre studio : ne voit rien.
        let vctx = map_ctx(&st, &us[1], mid).await.unwrap();
        assert!(matches!(
            enregistrer(&st, &vctx, &b("X", json!({ "a": 1 }))).await,
            Err(AppError::Forbidden)
        ));
        assert!(liste(&st, autre, "").await.unwrap().is_empty());
        let cid = c["id"].as_i64().unwrap();
        // Appliquer à un appareil d'un autre type : refus clair, rien n'est envoyé.
        let e = appliquer(
            &st,
            Origin {
                ctx: &ctx,
                source: "web",
                agent: None,
                task: None,
            },
            cid,
            &["/Game/L.L:T1".to_string()],
        )
        .await;
        assert!(e.is_err_and(|e| format!("{e:?}").contains("même type")));
        assert!(
            route_supprimer(State(st.clone()), us[1].clone(), Path((sid, cid)))
                .await
                .is_err()
        );
        route_supprimer(State(st.clone()), us[0].clone(), Path((sid, cid)))
            .await
            .unwrap();
    }
}
