//! Map courante du compte (#296) : Forge et le site s'ouvrent sur la même map ; la changer d'un côté prévient
//! toutes les fenêtres du même compte en temps réel (canal du compte, `ws.rs`).
use crate::{access::map_ctx, auth::AuthUser, error::ApiResult, error::AppError, AppState};
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

/// Map courante, si le compte y a encore accès (sinon rien : le site prend sa première map).
pub async fn lire(st: &AppState, uid: i64) -> Result<Option<i64>, AppError> {
    let m: Option<i64> = sqlx::query_scalar(
        "SELECT u.current_map_id FROM users u JOIN maps m ON m.id = u.current_map_id
         JOIN memberships ms ON ms.studio_id = m.studio_id AND ms.user_id = u.id WHERE u.id = ?",
    )
    .bind(uid)
    .fetch_optional(&st.db)
    .await?
    .flatten();
    Ok(m)
}

#[derive(Deserialize)]
pub struct CouranteIn {
    map_id: i64,
}

/// `PUT /api/me/current-map` : refuse une map d'un studio dont le compte n'est pas membre.
pub async fn changer(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<CouranteIn>,
) -> ApiResult<Value> {
    map_ctx(&st, &u, b.map_id).await?;
    sqlx::query("UPDATE users SET current_map_id = ? WHERE id = ?")
        .bind(b.map_id)
        .bind(u.id)
        .execute(&st.db)
        .await?;
    st.hub
        .send_user(u.id, json!({ "t": "current_map", "map_id": b.map_id }));
    Ok(Json(json!({ "ok": true, "map_id": b.map_id })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn meme_map_partout() {
        let st = crate::test_state().await;
        let s1: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('A') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let s2: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('B') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let m1: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M1') RETURNING id")
                .bind(s1)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let m2: i64 =
            sqlx::query_scalar("INSERT INTO maps (studio_id, name) VALUES (?, 'M2') RETURNING id")
                .bind(s2)
                .fetch_one(&st.db)
                .await
                .unwrap();
        let uid: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ('a@a.fr', 'A', 'x') RETURNING id").fetch_one(&st.db).await.unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')")
            .bind(s1)
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        let u = AuthUser {
            id: uid,
            email: "a@a.fr".into(),
            name: "A".into(),
            sid: String::new(),
        };
        let mut rx = st.hub.subscribe_user(uid);
        assert_eq!(lire(&st, uid).await.unwrap(), None);
        let _ = changer(
            State(st.clone()),
            u.clone(),
            Json(CouranteIn { map_id: m1 }),
        )
        .await
        .unwrap();
        assert_eq!(lire(&st, uid).await.unwrap(), Some(m1));
        let msg: Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(
            msg,
            json!({ "t": "current_map", "map_id": m1 }),
            "les autres fenêtres du compte sont prévenues"
        );
        assert!(
            changer(
                State(st.clone()),
                u.clone(),
                Json(CouranteIn { map_id: m2 })
            )
            .await
            .is_err(),
            "studio dont le compte n'est pas membre"
        );
        assert_eq!(lire(&st, uid).await.unwrap(), Some(m1));
        // Sorti du studio : la map courante n'est plus rendue.
        sqlx::query("DELETE FROM memberships WHERE user_id = ?")
            .bind(uid)
            .execute(&st.db)
            .await
            .unwrap();
        assert_eq!(lire(&st, uid).await.unwrap(), None);
    }
}
