//! Paiements Stripe : abonnements Starter / Pro, packs de crédits IA, portail client, webhook.
//!
//! Variables d'environnement :
//! - `STRIPE_SECRET_KEY` (sk_live_… ou sk_test_…), `STRIPE_WEBHOOK_SECRET` (whsec_…)
//! - `STRIPE_PRICE_STARTER`, `STRIPE_PRICE_PRO` : identifiants des prix mensuels (price_…)
//! - `POP_CREDIT_PACKS` : packs de crédits « crédits:prix_affiché:price_id », séparés par des points-virgules,
//!   par exemple `500:4,99 €:price_A;1500:12,99 €:price_B;5000:39,99 €:price_C`
//!
//! Tant que Stripe n'est pas configuré, rien n'est bloqué (bêta).
//! Accès : un compte « manual » (créé par l'administrateur) n'est jamais bloqué ; un compte en essai l'est
//! à la fin de ses 7 jours s'il n'a pas d'abonnement actif.
use crate::{auth::AuthUser, error::{bad, ApiResult, AppError}, AppState};
use axum::{body::Bytes, extract::State, http::HeaderMap, Json};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;

fn env(k: &str) -> Option<String> { std::env::var(k).ok().filter(|v| !v.trim().is_empty()) }
pub fn configured() -> bool { env("STRIPE_SECRET_KEY").is_some() }

/// Packs de crédits : (crédits, prix affiché, price_id).
pub fn packs() -> Vec<(i64, String, String)> {
    env("POP_CREDIT_PACKS").unwrap_or_default().split(';').filter_map(|p| {
        let mut it = p.trim().splitn(3, ':');
        let n = it.next()?.trim().parse::<i64>().ok()?;
        let label = it.next()?.trim().to_string();
        let price = it.next()?.trim().to_string();
        (n > 0 && price.starts_with("price_")).then_some((n, label, price))
    }).collect()
}

fn plan_of_price(price: &str) -> Option<&'static str> {
    if env("STRIPE_PRICE_PRO").as_deref() == Some(price) { Some("pro") } else if env("STRIPE_PRICE_STARTER").as_deref() == Some(price) { Some("starter") } else { None }
}

async fn stripe_post(st: &AppState, path: &str, form: &[(String, String)]) -> Result<Value, AppError> {
    let key = env("STRIPE_SECRET_KEY").ok_or_else(|| bad("Les paiements ne sont pas encore ouverts."))?;
    let base = env("POP_STRIPE_BASE").unwrap_or_else(|| "https://api.stripe.com".into());
    let r = st.http.post(format!("{base}/v1/{path}")).bearer_auth(key).form(form).send().await.map_err(|e| AppError::Internal(e.into()))?;
    let status = r.status();
    let v: Value = r.json().await.map_err(|e| AppError::Internal(e.into()))?;
    if !status.is_success() {
        tracing::warn!("Stripe {path} : {status} {}", v["error"]["message"]);
        return Err(bad("Le paiement n’a pas pu être préparé. Réessayez dans un instant."));
    }
    Ok(v)
}

/// État de l'accès d'un compte : "ok" ou "expired" (essai fini sans abonnement actif).
pub async fn access_state(st: &AppState, uid: i64) -> Result<&'static str, AppError> {
    if !configured() { return Ok("ok") }
    let r: Option<(String, bool, Option<String>)> = sqlx::query_as(
        "SELECT plan_source, COALESCE(trial_until > datetime('now'), 0), sub_status FROM users WHERE id = ?").bind(uid).fetch_optional(&st.db).await?;
    let Some((source, trial, sub)) = r else { return Ok("ok") };
    if source == "manual" || trial || matches!(sub.as_deref(), Some("active" | "trialing" | "past_due")) { Ok("ok") } else { Ok("expired") }
}

/// Chemins encore utilisables quand l'essai est fini (choisir un abonnement, exporter ses données, partir).
pub fn allowed_when_expired(path: &str) -> bool {
    path == "/api/me" || path.starts_with("/api/auth/") || path.starts_with("/api/billing") || path == "/api/me/export" || path == "/api/legal" || path == "/api/notifications"
}

pub async fn status(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    let (plan, source, trial, sub, end, credits, customer): (String, String, Option<String>, Option<String>, Option<String>, i64, Option<String>) = sqlx::query_as(
        "SELECT plan, plan_source, CASE WHEN trial_until > datetime('now') THEN trial_until END, sub_status, sub_end, ai_credits, stripe_customer FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
    Ok(Json(json!({
        "configured": configured(), "state": access_state(&st, u.id).await?,
        "plan": plan, "source": source, "trial_until": trial, "sub_status": sub, "sub_end": end, "has_customer": customer.is_some(),
        "ai_credits": credits, "ai_ready": crate::aipop::configured(),
        "packs": packs().into_iter().enumerate().map(|(i, p)| json!({ "id": i, "credits": p.0, "price": p.1 })).collect::<Vec<_>>(),
        "prices": { "starter": "12,99 €", "pro": "24,99 €" },
    })))
}

async fn customer(st: &AppState, u: &AuthUser) -> Result<String, AppError> {
    if let Some(c) = sqlx::query_scalar::<_, Option<String>>("SELECT stripe_customer FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await? { return Ok(c) }
    let v = stripe_post(st, "customers", &[("email".into(), u.email.clone()), ("name".into(), u.name.clone()), ("metadata[user_id]".into(), u.id.to_string())]).await?;
    let id = v["id"].as_str().ok_or_else(|| bad("Réponse Stripe inattendue."))?.to_string();
    sqlx::query("UPDATE users SET stripe_customer = ? WHERE id = ?").bind(&id).bind(u.id).execute(&st.db).await?;
    Ok(id)
}

#[derive(Deserialize)]
pub struct CheckoutIn { kind: String, pack: Option<usize> }

/// Page de paiement Stripe (abonnement ou pack de crédits).
pub async fn checkout(State(st): State<AppState>, u: AuthUser, Json(b): Json<CheckoutIn>) -> ApiResult<Value> {
    if !configured() { return Err(bad("Les paiements ne sont pas encore ouverts.")) }
    let site = crate::mail::public_url();
    let cus = customer(&st, &u).await?;
    let mut form: Vec<(String, String)> = vec![
        ("customer".into(), cus), ("client_reference_id".into(), u.id.to_string()), ("metadata[user_id]".into(), u.id.to_string()),
        ("line_items[0][quantity]".into(), "1".into()), ("allow_promotion_codes".into(), "true".into()),
        ("locale".into(), "auto".into()),
    ];
    match b.kind.as_str() {
        "starter" | "pro" => {
            // Déjà abonné : le changement de plan se fait dans le portail (prorata géré par Stripe).
            let sub: Option<String> = sqlx::query_scalar("SELECT sub_status FROM users WHERE id = ?").bind(u.id).fetch_one(&st.db).await?;
            if matches!(sub.as_deref(), Some("active" | "trialing" | "past_due")) { return portal(State(st), u).await }
            let price = env(if b.kind == "pro" { "STRIPE_PRICE_PRO" } else { "STRIPE_PRICE_STARTER" }).ok_or_else(|| bad("Ce plan n’est pas encore en vente."))?;
            form.extend([
                ("mode".into(), "subscription".into()), ("line_items[0][price]".into(), price),
                ("subscription_data[metadata][user_id]".into(), u.id.to_string()),
                ("success_url".into(), format!("{site}/parametres?paiement=ok")), ("cancel_url".into(), format!("{site}/parametres?paiement=annule")),
            ]);
        }
        "credits" => {
            let p = packs().into_iter().nth(b.pack.unwrap_or(usize::MAX)).ok_or_else(|| bad("Pack inconnu."))?;
            form.extend([
                ("mode".into(), "payment".into()), ("line_items[0][price]".into(), p.2),
                ("metadata[credits]".into(), p.0.to_string()), ("invoice_creation[enabled]".into(), "true".into()),
                ("success_url".into(), format!("{site}/parametres?credits=ok")), ("cancel_url".into(), format!("{site}/parametres?credits=annule")),
            ]);
        }
        _ => return Err(bad("Achat inconnu.")),
    }
    let v = stripe_post(&st, "checkout/sessions", &form).await?;
    Ok(Json(json!({ "url": v["url"] })))
}

/// Portail client Stripe : changer de plan, carte bancaire, factures, résiliation.
pub async fn portal(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    if !configured() { return Err(bad("Les paiements ne sont pas encore ouverts.")) }
    let cus = customer(&st, &u).await?;
    let v = stripe_post(&st, "billing_portal/sessions", &[("customer".into(), cus), ("return_url".into(), format!("{}/parametres", crate::mail::public_url()))]).await?;
    Ok(Json(json!({ "url": v["url"] })))
}

/// Vérifie la signature Stripe (en-tête Stripe-Signature : t=…,v1=…), à 5 minutes près.
fn verify(payload: &[u8], header: &str, secret: &str) -> bool {
    let mut t = None; let mut sigs = vec![];
    for part in header.split(',') {
        if let Some(v) = part.strip_prefix("t=") { t = Some(v.to_string()) }
        if let Some(v) = part.strip_prefix("v1=") { sigs.push(v.to_string()) }
    }
    let Some(t) = t else { return false };
    let Ok(ts) = t.parse::<i64>() else { return false };
    if (chrono::Utc::now().timestamp() - ts).abs() > 300 { return false }
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else { return false };
    mac.update(t.as_bytes()); mac.update(b"."); mac.update(payload);
    let expected = hex::encode(mac.finalize().into_bytes());
    sigs.iter().any(|s| s.len() == expected.len() && s.bytes().zip(expected.bytes()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0)
}

async fn user_for(st: &AppState, obj: &Value) -> Result<Option<i64>, AppError> {
    if let Some(id) = obj["metadata"]["user_id"].as_str().and_then(|s| s.parse::<i64>().ok()) { return Ok(Some(id)) }
    if let Some(c) = obj["customer"].as_str() { return Ok(sqlx::query_scalar("SELECT id FROM users WHERE stripe_customer = ?").bind(c).fetch_optional(&st.db).await?) }
    Ok(None)
}

/// Webhook Stripe : paiements confirmés, abonnements créés / modifiés / résiliés.
pub async fn webhook(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Result<Json<Value>, AppError> {
    let secret = env("STRIPE_WEBHOOK_SECRET").ok_or(AppError::NotFound)?;
    let sig = headers.get("stripe-signature").and_then(|v| v.to_str().ok()).unwrap_or("");
    if !verify(&body, sig, &secret) { return Err(AppError::Unauthorized) }
    let ev: Value = serde_json::from_slice(&body).map_err(|_| bad("JSON invalide."))?;
    let id = ev["id"].as_str().unwrap_or("").to_string();
    let kind = ev["type"].as_str().unwrap_or("").to_string();
    // Un événement déjà traité (Stripe renvoie parfois) ne crédite jamais deux fois.
    let fresh = sqlx::query("INSERT OR IGNORE INTO stripe_events (id, type) VALUES (?, ?)").bind(&id).bind(&kind).execute(&st.db).await?.rows_affected() == 1;
    if !fresh { return Ok(Json(json!({ "ok": true, "duplicate": true }))) }
    let obj = &ev["data"]["object"];
    match kind.as_str() {
        "checkout.session.completed" | "checkout.session.async_payment_succeeded" => {
            let Some(uid) = user_for(&st, obj).await? else { return Ok(Json(json!({ "ok": true }))) };
            if let Some(c) = obj["customer"].as_str() { sqlx::query("UPDATE users SET stripe_customer = COALESCE(stripe_customer, ?) WHERE id = ?").bind(c).bind(uid).execute(&st.db).await?; }
            // Crédits : seulement une fois l'argent encaissé.
            if obj["mode"] == "payment" && obj["payment_status"] == "paid" {
                let n = obj["metadata"]["credits"].as_str().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                if n > 0 {
                    let mut tx = st.db.begin().await?;
                    sqlx::query("UPDATE users SET ai_credits = ai_credits + ? WHERE id = ?").bind(n).bind(uid).execute(&mut *tx).await?;
                    sqlx::query("INSERT INTO ai_ledger (user_id, delta, reason, ref, amount_cents) VALUES (?, ?, 'achat', ?, ?)")
                        .bind(uid).bind(n).bind(obj["id"].as_str()).bind(obj["amount_total"].as_i64()).execute(&mut *tx).await?;
                    tx.commit().await?;
                    let _ = sqlx::query("INSERT INTO notifications (user_id, map_id, text, link) VALUES (?, NULL, ?, '/parametres')").bind(uid).bind(format!("{n} crédits IA ajoutés à votre compte.")).execute(&st.db).await;
                }
            }
        }
        "customer.subscription.created" | "customer.subscription.updated" | "customer.subscription.deleted" => {
            let Some(uid) = user_for(&st, obj).await? else { return Ok(Json(json!({ "ok": true }))) };
            let status = if kind.ends_with("deleted") { "canceled".to_string() } else { obj["status"].as_str().unwrap_or("").to_string() };
            let item = &obj["items"]["data"][0];
            let price = item["price"]["id"].as_str().unwrap_or("");
            let end = item["current_period_end"].as_i64().or(obj["current_period_end"].as_i64())
                .and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string());
            if let Some(plan) = plan_of_price(price) {
                sqlx::query("UPDATE users SET plan = ?, plan_source = 'stripe', sub_id = ?, sub_status = ?, sub_end = ?, stripe_customer = COALESCE(stripe_customer, ?) WHERE id = ?")
                    .bind(plan).bind(obj["id"].as_str()).bind(&status).bind(&end).bind(obj["customer"].as_str()).bind(uid).execute(&st.db).await?;
            } else {
                sqlx::query("UPDATE users SET sub_status = ?, sub_end = ? WHERE id = ? AND sub_id = ?").bind(&status).bind(&end).bind(uid).bind(obj["id"].as_str()).execute(&st.db).await?;
            }
        }
        _ => {}
    }
    Ok(Json(json!({ "ok": true })))
}
