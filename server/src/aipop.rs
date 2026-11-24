//! IA POP payée en crédits : POP appelle Claude avec sa propre clé et retire au client le coût réel de chaque réponse.
//!
//! Garde-fous pour ne jamais perdre d'argent :
//! - le client ne peut utiliser que des crédits déjà payés (ajoutés par le webhook Stripe une fois l'argent encaissé) ;
//! - avant chaque appel, POP réserve le coût MAXIMUM possible de la réponse ; sans solde suffisant, pas d'appel ;
//! - après la réponse, POP retire le coût réel (jetons mesurés par Claude) multiplié par la marge, et rend le reste ;
//! - plafonds : par client et par jour, et pour tout POP par jour.
//!
//! Variables : `ANTHROPIC_API_KEY`, `POP_AI_MODEL` (défaut claude-sonnet-5-5),
//! `POP_AI_USD_IN` / `POP_AI_USD_OUT` (prix en dollars par million de jetons du modèle — à vérifier sur la grille d'Anthropic),
//! `POP_CREDITS_PER_USD` (crédits retirés par dollar de coût Claude, défaut 200 : 1 crédit ≈ 0,5 cent de coût, vendu ~1 centime),
//! `POP_AI_USER_DAILY` (crédits par client et par jour, défaut 2000), `POP_AI_GLOBAL_DAILY_USD` (coût Claude max par jour, défaut 50).
use crate::{
    auth::AuthUser,
    error::{bad, AppError},
    AppState,
};
use serde_json::{json, Value};

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}
fn envf(k: &str, d: f64) -> f64 {
    env(k)
        .and_then(|v| v.replace(',', ".").parse().ok())
        .unwrap_or(d)
}
pub fn configured() -> bool {
    env("ANTHROPIC_API_KEY").is_some()
}
fn model() -> String {
    env("POP_AI_MODEL").unwrap_or_else(|| "claude-sonnet-5-5".into())
}

/// Crédits correspondant à un coût Claude en dollars (arrondi au crédit supérieur, au moins 1).
fn credits_for(usd: f64) -> i64 {
    ((usd * envf("POP_CREDITS_PER_USD", 200.0)).ceil() as i64).max(1)
}
fn usd_for(tin: f64, tout: f64) -> f64 {
    tin * envf("POP_AI_USD_IN", 3.0) / 1e6 + tout * envf("POP_AI_USD_OUT", 15.0) / 1e6
}

pub struct Reply {
    pub text: String,
    pub credits: i64,
    pub balance: i64,
}

/// Appel à Claude payé en crédits. `reason` est noté dans l'historique (ex. « plan de production »).
pub async fn run(
    st: &AppState,
    u: &AuthUser,
    reason: &str,
    system: &str,
    prompt: &str,
    max_tokens: u32,
) -> Result<Reply, AppError> {
    let key =
        env("ANTHROPIC_API_KEY").ok_or_else(|| bad("L’IA de POP n’est pas encore ouverte."))?;
    // Coût maximum possible : jetons d'entrée estimés (≈ 1 jeton pour 3 caractères, avec marge) + réponse la plus longue.
    let est_in = ((system.len() + prompt.len()) as f64 / 3.0).ceil() + 50.0;
    let reserve = credits_for(usd_for(est_in, max_tokens as f64));

    // Plafonds journaliers.
    let used_today: i64 = sqlx::query_scalar("SELECT COALESCE(-SUM(delta), 0) FROM ai_ledger WHERE user_id = ? AND delta < 0 AND at > datetime('now', 'start of day')").bind(u.id).fetch_one(&st.db).await?;
    if used_today + reserve > envf("POP_AI_USER_DAILY", 2000.0) as i64 {
        return Err(AppError::Guard(
            "credits",
            "Limite d’utilisation de l’IA atteinte pour aujourd’hui. Elle se remet à zéro demain."
                .into(),
        ));
    }
    let cost_today: f64 = sqlx::query_scalar("SELECT COALESCE(SUM(cost_usd), 0.0) FROM ai_ledger WHERE at > datetime('now', 'start of day')").fetch_one(&st.db).await?;
    if cost_today + usd_for(est_in, max_tokens as f64) > envf("POP_AI_GLOBAL_DAILY_USD", 50.0) {
        tracing::warn!("IA POP : plafond global du jour atteint ({cost_today:.2} $)");
        return Err(AppError::Guard(
            "credits",
            "L’IA de POP est très demandée aujourd’hui. Réessayez demain.".into(),
        ));
    }
    // Réservation : retirée tout de suite, seulement si le solde suffit (pas de solde négatif possible).
    let ok = sqlx::query(
        "UPDATE users SET ai_credits = ai_credits - ? WHERE id = ? AND ai_credits >= ?",
    )
    .bind(reserve)
    .bind(u.id)
    .bind(reserve)
    .execute(&st.db)
    .await?
    .rows_affected()
        == 1;
    if !ok {
        return Err(AppError::Guard("credits", format!("Il faut au moins {reserve} crédits IA pour cette demande. Achetez des crédits dans Paramètres.")));
    }

    let base = env("POP_AI_BASE").unwrap_or_else(|| "https://api.anthropic.com".into());
    let res = st.http.post(format!("{base}/v1/messages"))
        .header("x-api-key", key).header("anthropic-version", "2023-06-01")
        .json(&json!({ "model": model(), "max_tokens": max_tokens, "system": system, "messages": [{ "role": "user", "content": prompt }] }))
        .timeout(std::time::Duration::from_secs(120)).send().await;
    let refund = |st: &AppState| {
        let st = st.clone();
        let uid = u.id;
        async move {
            let _ = sqlx::query("UPDATE users SET ai_credits = ai_credits + ? WHERE id = ?")
                .bind(reserve)
                .bind(uid)
                .execute(&st.db)
                .await;
        }
    };
    let r = match res {
        Ok(r) => r,
        Err(e) => {
            refund(st).await;
            tracing::warn!("IA POP : {e}");
            return Err(bad(
                "L’IA ne répond pas pour l’instant. Vos crédits n’ont pas été utilisés.",
            ));
        }
    };
    let status = r.status();
    let v: Value = match r.json().await {
        Ok(v) => v,
        Err(_) => {
            refund(st).await;
            return Err(bad(
                "Réponse de l’IA illisible. Vos crédits n’ont pas été utilisés.",
            ));
        }
    };
    if !status.is_success() {
        refund(st).await;
        tracing::warn!("IA POP : {status} {}", v["error"]["message"]);
        return Err(bad(
            "L’IA ne répond pas pour l’instant. Vos crédits n’ont pas été utilisés.",
        ));
    }
    let tin = v["usage"]["input_tokens"].as_f64().unwrap_or(est_in);
    let tout = v["usage"]["output_tokens"]
        .as_f64()
        .unwrap_or(max_tokens as f64);
    let usd = usd_for(tin, tout);
    let cost = credits_for(usd).min(reserve);
    let text: String = v["content"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| c["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    // Règlement : on rend la différence entre la réservation et le coût réel.
    let mut tx = st.db.begin().await?;
    sqlx::query("UPDATE users SET ai_credits = ai_credits + ? WHERE id = ?")
        .bind(reserve - cost)
        .bind(u.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO ai_ledger (user_id, delta, reason, tokens_in, tokens_out, cost_usd) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(u.id).bind(-cost).bind(reason).bind(tin as i64).bind(tout as i64).bind(usd).execute(&mut *tx).await?;
    tx.commit().await?;
    let balance: i64 = sqlx::query_scalar("SELECT ai_credits FROM users WHERE id = ?")
        .bind(u.id)
        .fetch_one(&st.db)
        .await?;
    Ok(Reply {
        text,
        credits: cost,
        balance,
    })
}

/// Tableau de contrôle (Administration) : encaissé, coût réel chez Claude, marge, crédits vendus non utilisés.
pub async fn admin_report(
    axum::extract::State(st): axum::extract::State<AppState>,
    u: AuthUser,
) -> crate::error::ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    let row = |q: &'static str| {
        let st = st.clone();
        async move {
            sqlx::query_as::<_, (f64, f64, i64, i64)>(q)
                .fetch_one(&st.db)
                .await
        }
    };
    let all = row("SELECT COALESCE(SUM(amount_cents), 0) / 100.0, COALESCE(SUM(cost_usd), 0.0), COALESCE(SUM(CASE WHEN delta > 0 THEN delta END), 0), COALESCE(-SUM(CASE WHEN delta < 0 THEN delta END), 0) FROM ai_ledger").await?;
    let month = row("SELECT COALESCE(SUM(amount_cents), 0) / 100.0, COALESCE(SUM(cost_usd), 0.0), COALESCE(SUM(CASE WHEN delta > 0 THEN delta END), 0), COALESCE(-SUM(CASE WHEN delta < 0 THEN delta END), 0) FROM ai_ledger WHERE at > datetime('now', 'start of month')").await?;
    let outstanding: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(ai_credits), 0) FROM users")
        .fetch_one(&st.db)
        .await?;
    let usd_eur = envf("POP_USD_EUR", 0.92);
    let fmt = |r: (f64, f64, i64, i64)| json!({ "revenue_eur": r.0, "claude_cost_usd": r.1, "claude_cost_eur": r.1 * usd_eur, "margin_eur": r.0 - r.1 * usd_eur, "credits_sold": r.2, "credits_used": r.3 });
    Ok(axum::Json(json!({
        "configured": configured(), "model": model(),
        "all": fmt(all), "month": fmt(month), "outstanding_credits": outstanding,
        "outstanding_max_cost_eur": outstanding as f64 / envf("POP_CREDITS_PER_USD", 200.0) * usd_eur,
        "settings": { "credits_per_usd": envf("POP_CREDITS_PER_USD", 200.0), "usd_in": envf("POP_AI_USD_IN", 3.0), "usd_out": envf("POP_AI_USD_OUT", 15.0), "user_daily": envf("POP_AI_USER_DAILY", 2000.0), "global_daily_usd": envf("POP_AI_GLOBAL_DAILY_USD", 50.0) },
    })))
}

/// Admin : ajouter (ou retirer) des crédits à un compte, par exemple un geste commercial. Noté dans l'historique.
#[derive(serde::Deserialize)]
pub struct GrantIn {
    credits: i64,
}
pub async fn admin_grant(
    axum::extract::State(st): axum::extract::State<AppState>,
    u: AuthUser,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Json(b): axum::Json<GrantIn>,
) -> crate::error::ApiResult<Value> {
    crate::security::require_admin(&st, &u).await?;
    if b.credits == 0 || b.credits.abs() > 100_000 {
        return Err(bad("Nombre de crédits invalide."));
    }
    let n = sqlx::query("UPDATE users SET ai_credits = MAX(0, ai_credits + ?) WHERE id = ?")
        .bind(b.credits)
        .bind(id)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    sqlx::query(
        "INSERT INTO ai_ledger (user_id, delta, reason, ref) VALUES (?, ?, 'administration', ?)",
    )
    .bind(id)
    .bind(b.credits)
    .bind(u.id.to_string())
    .execute(&st.db)
    .await?;
    // Journal des actions d'administration (#66).
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(&st.db)
        .await?;
    crate::security::audit(
        &st,
        Some(u.id),
        "admin_crédits_ia",
        &format!("{email} : {:+} crédits", b.credits),
        "",
    )
    .await;
    Ok(axum::Json(json!({ "ok": true })))
}
