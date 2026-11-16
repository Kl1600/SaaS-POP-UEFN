use crate::{
    access::{log_activity, map_ctx, notify, Role},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    studio::DEFAULT_CHECKS,
    AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn checks(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT key, status, note FROM publish_checks WHERE map_id = ?").bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!(DEFAULT_CHECKS.iter().map(|(k, title, rule)| {
        let r = rows.iter().find(|r| r.0 == *k);
        json!({ "key": k, "title": title, "rule": rule, "status": r.map(|r| r.1.as_str()).unwrap_or("todo"), "note": r.map(|r| r.2.as_str()).unwrap_or("") })
    }).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct CheckIn {
    status: String,
    note: Option<String>,
}

pub async fn set_check(State(st): State<AppState>, u: AuthUser, Path((mid, key)): Path<(i64, String)>, Json(b): Json<CheckIn>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?.require(Role::Freelance)?;
    if !DEFAULT_CHECKS.iter().any(|(k, _, _)| *k == key) {
        return Err(AppError::NotFound);
    }
    if !["ok", "warn", "block", "todo"].contains(&b.status.as_str()) {
        return Err(bad("Statut inconnu."));
    }
    let note = b.note.unwrap_or_default();
    check_len("Note", &note, 0, 400)?;
    sqlx::query("INSERT INTO publish_checks (map_id, key, status, note) VALUES (?, ?, ?, ?) ON CONFLICT(map_id, key) DO UPDATE SET status = excluded.status, note = excluded.note")
        .bind(mid).bind(&key).bind(&b.status).bind(note).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn steps(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, String, String, i64)> = sqlx::query_as("SELECT id, time, title, owner, done FROM launch_steps WHERE map_id = ? ORDER BY time, id").bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|(id, t, title, owner, done)| json!({"id": id, "time": t, "title": title, "owner": owner, "done": done == 1})).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct StepIn {
    time: String,
    title: String,
    owner: Option<String>,
}

pub async fn add_step(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<StepIn>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?.require(Role::Member)?;
    check_len("Heure", &b.time, 4, 5)?;
    check_len("Étape", &b.title, 1, 140)?;
    sqlx::query("INSERT INTO launch_steps (map_id, time, title, owner) VALUES (?, ?, ?, ?)").bind(mid).bind(&b.time).bind(b.title.trim()).bind(b.owner.unwrap_or_default()).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct DoneIn {
    done: bool,
}

pub async fn toggle_step(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>, Json(b): Json<DoneIn>) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM launch_steps WHERE id = ?").bind(id).fetch_one(&st.db).await?;
    map_ctx(&st, &u, mid).await?.require(Role::Freelance)?;
    sqlx::query("UPDATE launch_steps SET done = ? WHERE id = ?").bind(b.done as i64).bind(id).execute(&st.db).await?;
    notify(&st, mid, json!({ "t": "refresh", "what": "launch" }));
    Ok(Json(json!({ "ok": true })))
}

// ---------- Échange de playtests à crédits ----------

pub async fn exchange(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let credits: i64 = sqlx::query_scalar("SELECT credits FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    // Maps des autres studios encore ouvertes au test.
    let open: Vec<(i64, String, String, String, i64, i64, i64, String, i64)> = sqlx::query_as(
        "SELECT l.id, m.name, m.genre, s.name, l.minutes, l.reward, l.slots, l.island_code,
                EXISTS (SELECT 1 FROM playtest_feedback f WHERE f.listing_id = l.id AND f.tester_id = ?)
         FROM playtest_listings l JOIN maps m ON m.id = l.map_id JOIN studios s ON s.id = m.studio_id
         WHERE l.slots > 0 AND m.studio_id NOT IN (SELECT studio_id FROM memberships WHERE user_id = ?) ORDER BY l.id DESC LIMIT 30",
    )
    .bind(u.id)
    .bind(u.id)
    .fetch_all(&st.db)
    .await?;
    let mine: Vec<(i64, i64, i64, i64, String)> = sqlx::query_as("SELECT id, minutes, reward, slots, created_at FROM playtest_listings WHERE map_id = ? ORDER BY id DESC").bind(mid).fetch_all(&st.db).await?;
    let fb: Vec<(String, i64, String, String, String, i64, String)> = sqlx::query_as(
        "SELECT u.name, f.left_minute, f.reason, f.understood, f.comeback, f.score, f.at FROM playtest_feedback f JOIN playtest_listings l ON l.id = f.listing_id JOIN users u ON u.id = f.tester_id WHERE l.map_id = ? ORDER BY f.id DESC LIMIT 50",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let _ = ctx;
    Ok(Json(json!({
        "credits": credits,
        "open": open.into_iter().map(|(id, map, genre, studio, min, reward, slots, code, done)| json!({"id": id, "map": map, "genre": genre, "studio": studio, "minutes": min, "reward": reward, "slots": slots, "island_code": code, "done": done == 1})).collect::<Vec<_>>(),
        "mine": mine.into_iter().map(|(id, min, reward, slots, at)| json!({"id": id, "minutes": min, "reward": reward, "slots": slots, "created_at": at})).collect::<Vec<_>>(),
        "feedback": fb.into_iter().map(|(who, m, reason, und, cb, score, at)| json!({"who": who, "left_minute": m, "reason": reason, "understood": und, "comeback": cb, "score": score, "at": at})).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct ListingIn {
    slots: i64,
    reward: i64,
    minutes: i64,
    island_code: Option<String>,
}

pub async fn create_listing(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<ListingIn>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?.require(Role::Member)?;
    if !(1..=20).contains(&b.slots) || !(5..=100).contains(&b.reward) || !(5..=60).contains(&b.minutes) {
        return Err(bad("Valeurs hors limites."));
    }
    let cost = b.slots * b.reward;
    let mut tx = st.db.begin().await?;
    let r = sqlx::query("UPDATE users SET credits = credits - ? WHERE id = ? AND credits >= ?").bind(cost).bind(u.id).bind(cost).execute(&mut *tx).await?;
    if r.rows_affected() == 0 {
        return Err(bad(format!("Il faut {cost} crédits.")));
    }
    sqlx::query("INSERT INTO playtest_listings (map_id, owner_id, minutes, reward, slots, island_code) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(mid).bind(u.id).bind(b.minutes).bind(b.reward).bind(b.slots).bind(b.island_code.unwrap_or_default().chars().take(20).collect::<String>())
        .execute(&mut *tx).await?;
    tx.commit().await?;
    log_activity(&st, mid, Some(u.id), "playtest", "Demande de playtest", &format!("{} testeurs, {} crédits", b.slots, cost)).await?;
    Ok(Json(json!({ "ok": true, "cost": cost })))
}

#[derive(Deserialize)]
pub struct FeedbackIn {
    understood: String,
    left_minute: i64,
    reason: String,
    comeback: Option<String>,
    score: i64,
}

/// Le testeur répond au questionnaire guidé et gagne les crédits de la demande.
pub async fn give_feedback(State(st): State<AppState>, u: AuthUser, Path(lid): Path<i64>, Json(b): Json<FeedbackIn>) -> ApiResult<Value> {
    let (map_id, studio_id, reward): (i64, i64, i64) = sqlx::query_as(
        "SELECT l.map_id, m.studio_id, l.reward FROM playtest_listings l JOIN maps m ON m.id = l.map_id WHERE l.id = ? AND l.slots > 0",
    )
    .bind(lid)
    .fetch_one(&st.db)
    .await?;
    let member: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM memberships WHERE studio_id = ? AND user_id = ?").bind(studio_id).bind(u.id).fetch_optional(&st.db).await?;
    if member.is_some() {
        return Err(bad("Vous ne pouvez pas tester une map de votre propre studio."));
    }
    if !(0..=180).contains(&b.left_minute) || !(1..=5).contains(&b.score) {
        return Err(bad("Valeurs hors limites."));
    }
    check_len("Raison", &b.reason, 1, 200)?;
    let mut tx = st.db.begin().await?;
    let ins = sqlx::query("INSERT OR IGNORE INTO playtest_feedback (listing_id, tester_id, understood, left_minute, reason, comeback, score) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(lid).bind(u.id).bind(&b.understood).bind(b.left_minute).bind(&b.reason).bind(b.comeback.unwrap_or_default().chars().take(1000).collect::<String>()).bind(b.score)
        .execute(&mut *tx).await?;
    if ins.rows_affected() == 0 {
        return Err(AppError::Conflict("Vous avez déjà testé cette map.".into()));
    }
    sqlx::query("UPDATE playtest_listings SET slots = slots - 1 WHERE id = ?").bind(lid).execute(&mut *tx).await?;
    sqlx::query("UPDATE users SET credits = credits + ? WHERE id = ?").bind(reward).bind(u.id).execute(&mut *tx).await?;
    tx.commit().await?;
    log_activity(&st, map_id, None, "playtest", "Nouveau retour de playtest", &format!("Décrochage à {} min : {}", b.left_minute, b.reason)).await?;
    Ok(Json(json!({ "ok": true, "earned": reward })))
}

/// « Pourquoi ils partent » : départs par minute de jeu et raisons citées.
pub async fn churn(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT f.left_minute, f.reason FROM playtest_feedback f JOIN playtest_listings l ON l.id = f.listing_id WHERE l.map_id = ?").bind(mid).fetch_all(&st.db).await?;
    let mut per = vec![0i64; 16];
    let mut reasons: std::collections::BTreeMap<String, i64> = Default::default();
    for (m, r) in &rows {
        per[(*m).clamp(0, 15) as usize] += 1;
        *reasons.entry(r.clone()).or_default() += 1;
    }
    let mut rs: Vec<(String, i64)> = reasons.into_iter().collect();
    rs.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(Json(json!({ "testers": rows.len(), "per_minute": per, "reasons": rs.into_iter().map(|(r, n)| json!({"reason": r, "count": n})).collect::<Vec<_>>() })))
}
