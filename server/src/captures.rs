//! Photos de l'île (#306) : le relais place la caméra de l'éditeur, passe en vue jeu, capture en 1920 × 1080, remet la
//! vue comme avant et envoie l'image directement au stockage objet (#121) par un lien signé ; POP garde la liste.
use crate::{
    access::map_ctx,
    auth::{random_token, AuthUser},
    error::{bad, ApiResult, AppError},
    s3::S3,
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

/// Vérifie une caméra : position (cm, 10 km au plus) et rotation [pitch, yaw, roll] en degrés.
pub fn check_shot(n: usize, s: &Value) -> Result<(Value, Value), AppError> {
    let trois = |v: &Value, max: f64| {
        v.as_array().is_some_and(|a| {
            a.len() == 3
                && a.iter()
                    .all(|x| x.as_f64().is_some_and(|f| f.is_finite() && f.abs() <= max))
        })
    };
    if !trois(&s["location"], 1_000_000.0) {
        return Err(bad(format!(
            "Angle {n} : position invalide (trois nombres en cm, 10 km au plus)."
        )));
    }
    if !trois(&s["rotation"], 3600.0) {
        return Err(bad(format!(
            "Angle {n} : rotation invalide ([pitch, yaw, roll] en degrés)."
        )));
    }
    Ok((s["location"].clone(), s["rotation"].clone()))
}

/// Lien de lecture (1 h) d'une capture.
fn lien(s3: &S3, key: &str) -> String {
    s3.presign("GET", key, 3600, chrono::Utc::now())
}

/// Prend une photo par angle (1 à 6), l'une après l'autre.
pub async fn capturer(st: &AppState, mid: i64, uid: i64, shots: &Value) -> Result<Value, AppError> {
    let shots = shots.as_array().cloned().unwrap_or_default();
    if shots.is_empty() || shots.len() > 6 {
        return Err(bad("Photos de l’île : de 1 à 6 angles (shots)."));
    }
    let angles = shots
        .iter()
        .enumerate()
        .map(|(i, s)| check_shot(i + 1, s))
        .collect::<Result<Vec<_>, _>>()?;
    let s3 = S3::from_env().ok_or_else(|| bad("Stockage objet non configuré sur ce serveur POP : les photos ne peuvent pas être rangées."))?;
    if st.relays.status(mid) != "connected" {
        return Err(bad(
            "UEFN n’est pas connecté : ouvrez le projet avec le relais POP, puis recommencez.",
        ));
    }
    let mut out = vec![];
    for (loc, rot) in angles {
        let key = format!("captures/{mid}/{}.png", &random_token()[..24]);
        let put = s3.presign("PUT", &key, 900, chrono::Utc::now());
        let r = crate::uefn::relay_call(st, mid, "capture_island", &json!({ "location": loc, "rotation": rot, "width": 1920, "height": 1080, "put_url": put }), 150).await?;
        let id: i64 = sqlx::query_scalar("INSERT INTO island_captures (map_id, s3_key, location, rotation, width, height, bytes, created_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id")
            .bind(mid)
            .bind(&key)
            .bind(loc.to_string())
            .bind(rot.to_string())
            .bind(r["width"].as_i64().unwrap_or(1920))
            .bind(r["height"].as_i64().unwrap_or(1080))
            .bind(r["bytes"].as_i64().unwrap_or(0))
            .bind(uid)
            .fetch_one(&st.db)
            .await?;
        out.push(json!({ "id": id, "url": lien(&s3, &key), "width": r["width"], "height": r["height"], "location": loc, "rotation": rot }));
    }
    Ok(json!({ "captures": out }))
}

/// Les 50 dernières photos de la map, avec un lien de lecture.
pub async fn liste(st: &AppState, mid: i64) -> Result<Value, AppError> {
    let rows: Vec<(i64, String, String, String, i64, i64, String)> = sqlx::query_as("SELECT id, s3_key, location, rotation, width, height, created_at FROM island_captures WHERE map_id = ? ORDER BY id DESC LIMIT 50")
        .bind(mid)
        .fetch_all(&st.db)
        .await?;
    let s3 = S3::from_env();
    Ok(
        json!({ "captures": rows.into_iter().map(|(id, k, l, r, w, h, at)| json!({
        "id": id, "url": s3.as_ref().map(|s| lien(s, &k)), "width": w, "height": h, "created_at": at,
        "location": serde_json::from_str::<Value>(&l).unwrap_or(Value::Null), "rotation": serde_json::from_str::<Value>(&r).unwrap_or(Value::Null),
    })).collect::<Vec<_>>() }),
    )
}

/// `GET /api/maps/{mid}/captures`
pub async fn route_liste(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    Ok(Json(liste(&st, mid).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn angles_verifies_avant_tout_envoi() {
        assert!(check_shot(
            1,
            &json!({ "location": [0, 0, 5000], "rotation": [-30, 45, 0] })
        )
        .is_ok());
        assert!(check_shot(
            1,
            &json!({ "location": [0, "x", 0], "rotation": [0, 0, 0] })
        )
        .is_err());
        assert!(check_shot(
            1,
            &json!({ "location": [0, 0, 1e9], "rotation": [0, 0, 0] })
        )
        .is_err());
        assert!(check_shot(1, &json!({ "location": [0, 0, 0], "rotation": [0, 0] })).is_err());
        let st = crate::test_state().await;
        assert!(
            capturer(&st, 1, 1, &json!([])).await.is_err(),
            "au moins un angle"
        );
        let sept: Vec<Value> = (0..7)
            .map(|_| json!({ "location": [0, 0, 0], "rotation": [0, 0, 0] }))
            .collect();
        assert!(
            capturer(&st, 1, 1, &json!(sept)).await.is_err(),
            "six au plus"
        );
        assert!(liste(&st, 1).await.unwrap()["captures"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}
