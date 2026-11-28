//! 3D Studio (#327) : un modèle 3D depuis un texte, par l'IA de l'utilisateur connectée par MCP. Elle écrit un script
//! Blender (`blender_build`) ; Forge l'exécute isolé sur le PC (#351) et garde le FBX ; POP montre l'aperçu et
//! n'importe qu'après « Accepter » (action `asset_import`, journalisée et annulable, #90). POP n'appelle aucun service
//! payant (décision du 25/11).
use crate::{
    access::map_ctx,
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    uefn::{exec, relay_call, Origin},
    AppState,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::header,
    response::Response,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Le validateur de mesh d'Epic avertit au-delà de 30 000 sommets : on propose Low Mesh à partir de ce nombre de
/// triangles (ordre de grandeur équivalent).
const TRIANGLES_LOURD: i64 = 50_000;

/// Ajouté à la fin du script de l'IA : export FBX (le format d'import vérifié dans UEFN, #90).
const EXPORT_FBX: &str = "\n\n# --- POP (#327) : export pour UEFN ---\nimport bpy as _pop_bpy, os as _pop_os\n_pop_bpy.ops.export_scene.fbx(filepath=_pop_os.path.join(TRAVAIL, \"sortie.fbx\"), object_types={\"MESH\", \"EMPTY\"}, mesh_smooth_type=\"FACE\")\n";

fn nom_propre(n: &str) -> String {
    n.trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(60)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Outil MCP `blender_build` : exécute le script sur le PC du créateur (Forge, isolé) et propose le modèle.
pub async fn construire(
    st: &AppState,
    mid: i64,
    agent: Option<i64>,
    a: &Value,
) -> Result<Value, AppError> {
    let nom = nom_propre(a["name"].as_str().unwrap_or(""));
    if nom.is_empty() {
        return Err(bad("Donnez un nom court au modèle (name)."));
    }
    let script = a["script"].as_str().unwrap_or("");
    if script.trim().is_empty() || script.len() > 60_000 {
        return Err(bad(
            "Script Blender vide ou trop long (60 000 caractères au plus).",
        ));
    }
    if st.relays.status(mid) == "offline" {
        return Err(bad("POP Forge n'est pas ouvert sur le PC du créateur : Blender tourne dans Forge. Demandez à l'humain d'ouvrir Forge."));
    }
    let r = relay_call(
        st,
        mid,
        "blender_run",
        &json!({ "script": format!("{script}{EXPORT_FBX}"), "keep": true, "name": nom, "timeout_s": 120 }),
        330,
    )
    .await?;
    let chemin = r["local_path"].as_str().unwrap_or("");
    let fichier = r["filename"].as_str().unwrap_or("");
    if chemin.is_empty() || !fichier.to_lowercase().ends_with(".fbx") {
        return Err(bad("Le script n'a produit aucun modèle à importer. Mettez à jour POP Forge si le problème continue."));
    }
    let apercu = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(r["preview"].as_str().unwrap_or(""))
            .unwrap_or_default()
    };
    let triangles = r["triangles"].as_i64().unwrap_or(0);
    let id: i64 = sqlx::query_scalar("INSERT INTO studio3d_modeles (map_id, agent_id, nom, script, apercu, triangles, objets, chemin, fichier) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(mid)
        .bind(agent)
        .bind(&nom)
        .bind(script)
        .bind(&apercu)
        .bind(triangles)
        .bind(r["objects"].as_i64().unwrap_or(0))
        .bind(chemin)
        .bind(fichier)
        .fetch_one(&st.db)
        .await?;
    crate::access::notify(st, mid, json!({ "t": "refresh", "what": "studio3d" }));
    Ok(json!({
        "id": id,
        "triangles": triangles,
        "objects": r["objects"],
        "too_heavy": triangles > TRIANGLES_LOURD,
        "message": "Modèle proposé à l'humain dans POP Forge (Outils → 3D Studio) : rien n'est importé tant qu'il ne clique pas « Accepter ». S'il est trop lourd, réduisez-le dans le script, ou l'humain utilisera Low Mesh.",
    }))
}

/// L'IA connectée de la personne sur cette map (la plus récente, active depuis moins d'un jour).
async fn ia(st: &AppState, mid: i64, uid: i64) -> Result<Option<(i64, String)>, AppError> {
    Ok(sqlx::query_as(
        "SELECT id, name FROM api_tokens WHERE map_id = ? AND user_id = ? AND scope = 'write' AND last_used > datetime('now', '-1 day') ORDER BY last_used DESC LIMIT 1",
    )
    .bind(mid)
    .bind(uid)
    .fetch_optional(&st.db)
    .await?)
}

/// `GET /api/maps/{mid}/studio3d` : l'IA à qui parler et les modèles proposés (le plus récent d'abord).
pub async fn liste(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, i64, i64, String, String, String)> = sqlx::query_as(
        "SELECT id, nom, triangles, objets, fichier, statut, created_at FROM studio3d_modeles WHERE map_id = ? ORDER BY id DESC LIMIT 30",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let ia = ia(&st, mid, u.id).await?;
    Ok(Json(json!({
        "ia": ia.map(|(id, name)| json!({ "id": id, "name": name })),
        "modeles": rows.into_iter().map(|(id, nom, tri, obj, f, s, at)| json!({
            "id": id, "nom": nom, "triangles": tri, "objets": obj, "fichier": f, "statut": s, "created_at": at,
            "lourd": tri > TRIANGLES_LOURD, "apercu": format!("/api/maps/{mid}/studio3d/{id}/apercu"),
        })).collect::<Vec<_>>(),
    })))
}

/// `GET /api/maps/{mid}/studio3d/{id}/apercu` : l'aperçu PNG.
pub async fn apercu(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    map_ctx(&st, &u, mid).await?;
    let png: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT apercu FROM studio3d_modeles WHERE id = ? AND map_id = ?")
            .bind(id)
            .bind(mid)
            .fetch_optional(&st.db)
            .await?
            .flatten();
    let png = png.filter(|p| !p.is_empty()).ok_or(AppError::NotFound)?;
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(Body::from(png))
        .unwrap())
}

#[derive(Deserialize)]
pub struct DecisionIn {
    #[serde(default)]
    folder: Option<String>,
}

/// `POST /api/maps/{mid}/studio3d/{id}/accepter` : importe le modèle dans le projet (journalisé et annulable).
pub async fn accepter(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
    Json(b): Json<DecisionIn>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    let (chemin, fichier, statut, agent): (String, String, String, Option<i64>) = sqlx::query_as(
        "SELECT chemin, fichier, statut, agent_id FROM studio3d_modeles WHERE id = ? AND map_id = ?",
    )
    .bind(id)
    .bind(mid)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if statut != "propose" {
        return Err(bad("Ce modèle a déjà été accepté ou refusé."));
    }
    let r = exec(
        &st,
        Origin {
            ctx: &ctx,
            source: "web",
            agent,
            task: None,
        },
        "asset_import",
        json!({ "local_path": chemin, "filename": fichier, "folder": b.folder.unwrap_or_default() }),
    )
    .await?;
    sqlx::query("UPDATE studio3d_modeles SET statut = 'accepte', decide_par = ? WHERE id = ?")
        .bind(u.id)
        .bind(id)
        .execute(&st.db)
        .await?;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "studio3d" }));
    Ok(Json(r))
}

/// `POST /api/maps/{mid}/studio3d/{id}/refuser` : rien n'est importé.
pub async fn refuser(
    State(st): State<AppState>,
    u: AuthUser,
    Path((mid, id)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    let n = sqlx::query("UPDATE studio3d_modeles SET statut = 'refuse', decide_par = ? WHERE id = ? AND map_id = ? AND statut = 'propose'")
        .bind(u.id)
        .bind(id)
        .bind(mid)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(bad("Ce modèle a déjà été accepté ou refusé."));
    }
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "studio3d" }));
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct DemandeIn {
    texte: String,
}

/// `POST /api/maps/{mid}/studio3d/demandes` : la demande de l'humain, envoyée à son IA dans le fil « studio3d ».
pub async fn demander(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<DemandeIn>,
) -> ApiResult<Value> {
    crate::billing::require_forge(&st, u.id).await?;
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.can_edit()?;
    let texte: String = b.texte.trim().chars().take(600).collect();
    if texte.is_empty() {
        return Err(bad("Décrivez le modèle à faire."));
    }
    let (aid, nom) = ia(&st, mid, u.id).await?.ok_or_else(|| {
        bad("Aucune IA connectée à cette map : lancez-en une depuis Agents → + Terminal, ou connectez la vôtre (POP → IA connectées).")
    })?;
    sqlx::query("INSERT INTO agent_messages (map_id, agent_id, user_id, body, canal) VALUES (?, ?, ?, ?, 'studio3d')")
        .bind(mid)
        .bind(aid)
        .bind(u.id)
        .bind(format!("{texte}\n(Outil 3D Studio : lisez la compétence « studio3d » avec get_skill, écrivez un script Blender et envoyez-le avec blender_build ; répondez avec send_message canal « studio3d ».)"))
        .execute(&st.db)
        .await?;
    crate::access::notify(&st, mid, json!({ "t": "refresh", "what": "messages" }));
    Ok(Json(json!({ "agent": nom, "agent_id": aid })))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Critères 1, 2 et 4 de #327 côté POP : script envoyé à Forge avec l'export FBX, modèle proposé avec son
    /// aperçu ; « Refuser » n'importe rien ; sans Forge, message clair ; erreur de Blender rendue telle quelle.
    #[tokio::test]
    async fn modele_propose_puis_refuse() {
        let st = crate::test_state().await;
        let ex = |q: &'static str| {
            let st = st.clone();
            async move {
                sqlx::query(q).execute(&st.db).await.unwrap();
            }
        };
        ex("INSERT INTO users (id, email, name, password_hash, email_verified_at) VALUES (1, 'a@a.fr', 'A', 'x', datetime('now'))").await;
        ex("INSERT INTO studios (id, name) VALUES (1, 'S')").await;
        ex("INSERT INTO memberships (studio_id, user_id, role) VALUES (1, 1, 'owner')").await;
        ex("INSERT INTO maps (id, studio_id, name) VALUES (7, 1, 'M')").await;
        let sans_forge = construire(
            &st,
            7,
            None,
            &json!({ "name": "Arbre", "script": "import bpy" }),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{sans_forge:?}").contains("Forge"),
            "{sans_forge:?}"
        );
        let recues = crate::uefn::relais_de_test(&st, 7, |action, a| {
            assert_eq!(action, "blender_run");
            if a["script"].as_str().unwrap().contains("raise") {
                return Err("Le script Blender a échoué : NameError".into());
            }
            Ok(
                json!({ "local_path": "C:\\x\\Arbre_1.fbx", "filename": "Arbre_1.fbx", "preview": "iVBORw0KGgo=", "triangles": 1200, "objects": 2 }),
            )
        });
        let r = construire(
            &st,
            7,
            None,
            &json!({ "name": "Arbre", "script": "import bpy\nbpy.ops.mesh.primitive_cone_add()" }),
        )
        .await
        .unwrap();
        assert_eq!(r["triangles"], 1200);
        assert_eq!(r["too_heavy"], false);
        let envoye = recues.lock().unwrap()[0].clone();
        assert_eq!(envoye["args"]["keep"], true);
        assert!(envoye["args"]["script"]
            .as_str()
            .unwrap()
            .contains("export_scene.fbx"));
        let e = construire(&st, 7, None, &json!({ "name": "X", "script": "raise" }))
            .await
            .unwrap_err();
        assert!(format!("{e:?}").contains("NameError"));
        let u = AuthUser {
            id: 1,
            email: "a@a.fr".into(),
            name: "A".into(),
            sid: String::new(),
        };
        let Json(l) = liste(State(st.clone()), u.clone(), Path(7)).await.unwrap();
        let id = l["modeles"][0]["id"].as_i64().unwrap();
        assert_eq!(l["modeles"][0]["statut"], "propose");
        refuser(State(st.clone()), u.clone(), Path((7, id)))
            .await
            .unwrap();
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM uefn_actions WHERE action = 'asset_import'")
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(n, 0, "Refuser n'importe rien");
        assert!(
            accepter(
                State(st.clone()),
                u,
                Path((7, id)),
                Json(DecisionIn { folder: None })
            )
            .await
            .is_err(),
            "déjà décidé"
        );
    }

    #[test]
    fn nom_et_export() {
        assert_eq!(nom_propre("  Arbre/../géant  "), "Arbre____géant");
        assert!(EXPORT_FBX.contains("sortie.fbx") && EXPORT_FBX.contains("TRAVAIL"));
    }
}
