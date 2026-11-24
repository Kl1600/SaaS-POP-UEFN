//! « Faire signe » à un membre depuis la scène du studio (#176) : une notification « X vous fait signe », poussée
//! en direct. Membre de la map seulement, pas à soi-même, une fois par minute et par personne.
use crate::{
    access::map_ctx,
    auth::AuthUser,
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct Ping {
    uid: i64,
}

pub async fn ping(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Json(b): Json<Ping>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    if b.uid == u.id {
        return Err(bad("Vous ne pouvez pas vous faire signe à vous-même."));
    }
    let membre: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM memberships WHERE studio_id = ? AND user_id = ?")
            .bind(ctx.studio_id)
            .bind(b.uid)
            .fetch_optional(&st.db)
            .await?;
    if membre.is_none() {
        return Err(AppError::NotFound);
    }
    let texte = format!("{} vous fait signe", u.name);
    let recent: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE user_id = ? AND map_id = ? AND text = ? AND at > datetime('now', '-1 minute')",
    )
    .bind(b.uid)
    .bind(mid)
    .bind(&texte)
    .fetch_one(&st.db)
    .await?;
    if recent > 0 {
        return Err(bad(
            "Vous lui avez déjà fait signe il y a moins d'une minute.",
        ));
    }
    crate::account::notify_user(&st, b.uid, mid, &texte, &format!("/map/{mid}")).await;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn faire_signe() {
        let st = crate::test_state().await;
        let q = |s: &'static str| sqlx::query_scalar::<_, i64>(s);
        let sid = q("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let autre = q("INSERT INTO studios (name) VALUES ('Autre') RETURNING id")
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
        for (mail, s) in [("a@a.fr", sid), ("b@a.fr", sid), ("x@a.fr", autre)] {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
            )
            .bind(mail)
            .bind(mail)
            .fetch_one(&st.db)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, 'member')",
            )
            .bind(s)
            .bind(id)
            .execute(&st.db)
            .await
            .unwrap();
            us.push(AuthUser {
                id,
                email: mail.into(),
                name: mail.into(),
                sid: String::new(),
            });
        }
        let p = |u: &AuthUser, uid: i64| {
            ping(State(st.clone()), u.clone(), Path(mid), Json(Ping { uid }))
        };
        p(&us[0], us[1].id).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id = ? AND text = 'a@a.fr vous fait signe'").bind(us[1].id).fetch_one(&st.db).await.unwrap();
        assert_eq!(n, 1);
        assert!(p(&us[0], us[1].id).await.is_err(), "une fois par minute");
        assert!(p(&us[0], us[0].id).await.is_err(), "pas à soi-même");
        assert!(p(&us[0], us[2].id).await.is_err(), "pas hors du studio");
        assert!(
            p(&us[2], us[0].id).await.is_err(),
            "un membre d'un autre studio n'a pas accès à la map"
        );
    }
}
