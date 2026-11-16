//! Rémunération : l'app calcule les parts, elle n'effectue aucun virement.
use crate::{
    access::{log_activity, map_ctx, MapCtx, Role},
    auth::AuthUser,
    error::{bad, check_len, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

fn valid_month(m: &str) -> bool {
    m.len() == 7 && m.as_bytes()[4] == b'-' && m[..4].parse::<u32>().is_ok() && m[5..].parse::<u32>().map(|x| (1..=12).contains(&x)).unwrap_or(false)
}

/// "1 234,56", "1234.56", "1 234 €" → centimes.
fn parse_cents(s: &str) -> Option<i64> {
    let clean: String = s.chars().filter(|c| c.is_ascii_digit() || *c == ',' || *c == '.' || *c == '-').collect();
    if clean.is_empty() {
        return None;
    }
    let norm = if clean.contains(',') && clean.contains('.') { clean.replace(',', "") } else { clean.replace(',', ".") };
    let v: f64 = norm.parse().ok()?;
    (v.is_finite() && v.abs() < 1e10).then(|| (v * 100.0).round() as i64)
}

pub async fn revenues(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(String, i64, i64, String)> = sqlx::query_as("SELECT month, engagement_cents, sales_cents, source FROM revenues WHERE map_id = ? ORDER BY month DESC").bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|(m, e, s, src)| json!({"month": m, "engagement_cents": e, "sales_cents": s, "source": src})).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct RevenueIn {
    month: String,
    engagement_cents: i64,
    sales_cents: i64,
}

async fn upsert_revenue(st: &AppState, mid: i64, month: &str, e: i64, s: i64, src: &str) -> Result<(), AppError> {
    if !valid_month(month) {
        return Err(bad(format!("Mois invalide : {month} (AAAA-MM attendu).")));
    }
    if e < 0 || s < 0 {
        return Err(bad("Montants négatifs refusés."));
    }
    sqlx::query("INSERT INTO revenues (map_id, month, engagement_cents, sales_cents, source) VALUES (?, ?, ?, ?, ?) ON CONFLICT(map_id, month) DO UPDATE SET engagement_cents = excluded.engagement_cents, sales_cents = excluded.sales_cents, source = excluded.source")
        .bind(mid).bind(month).bind(e).bind(s).bind(src).execute(&st.db).await?;
    Ok(())
}

pub async fn set_revenue(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<RevenueIn>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Admin)?;
    upsert_revenue(&st, mid, &b.month, b.engagement_cents, b.sales_cents, "manuel").await?;
    log_activity(&st, mid, Some(u.id), "money", "Gains importés", &format!("Mois {}", b.month)).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct CsvIn {
    csv: String,
}

/// Import CSV : une ligne par mois, colonnes mois ; engagement ; ventes (séparateur , ou ;).
pub async fn import_csv(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<CsvIn>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Admin)?;
    if b.csv.len() > 200_000 {
        return Err(bad("Fichier trop gros."));
    }
    let sep = if b.csv.lines().next().unwrap_or("").contains(';') { ';' } else { ',' };
    let mut n = 0;
    let mut errors = vec![];
    for (i, line) in b.csv.lines().enumerate() {
        let cols: Vec<&str> = line.split(sep).map(|c| c.trim().trim_matches('"')).collect();
        if cols.len() < 3 || cols[0].is_empty() {
            continue;
        }
        let month = if cols[0].len() >= 7 { &cols[0][..7] } else { cols[0] };
        if !valid_month(month) {
            if i > 0 {
                errors.push(format!("Ligne {} ignorée", i + 1));
            }
            continue;
        }
        match (parse_cents(cols[1]), parse_cents(cols[2])) {
            (Some(e), Some(s)) => {
                upsert_revenue(&st, mid, month, e, s, "csv").await?;
                n += 1;
            }
            _ => errors.push(format!("Ligne {} : montants illisibles", i + 1)),
        }
    }
    if n > 0 {
        log_activity(&st, mid, Some(u.id), "money", "Gains importés (CSV)", &format!("{n} mois")).await?;
    }
    Ok(Json(json!({ "imported": n, "errors": errors })))
}

pub async fn rules(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, Option<i64>, Option<String>, String, i64)> = sqlx::query_as(
        "SELECT r.id, r.user_id, u.name, r.kind, r.value FROM share_rules r LEFT JOIN users u ON u.id = r.user_id WHERE r.map_id = ? ORDER BY CASE r.kind WHEN 'fixed' THEN 0 WHEN 'per_task' THEN 1 ELSE 2 END, r.user_id IS NULL, u.name",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows.into_iter().map(|(id, uid, name, kind, value)| json!({"id": id, "user_id": uid, "name": name.unwrap_or_else(|| "Réserve du studio".into()), "kind": kind, "value": value})).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct RuleIn {
    user_id: Option<i64>,
    kind: String,
    value: i64,
}

pub async fn set_rules(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(list): Json<Vec<RuleIn>>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    ctx.require(Role::Admin)?;
    let pct: i64 = list.iter().filter(|r| r.kind == "percent").map(|r| r.value).sum();
    if pct > 10000 {
        return Err(bad("Le total des pourcentages dépasse 100 %."));
    }
    for r in &list {
        if !["percent", "fixed", "per_task"].contains(&r.kind.as_str()) || r.value < 0 {
            return Err(bad("Règle invalide."));
        }
        if let Some(uid) = r.user_id {
            let ok: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM memberships WHERE studio_id = ? AND user_id = ?").bind(ctx.studio_id).bind(uid).fetch_optional(&st.db).await?;
            ok.ok_or_else(|| bad("Membre inconnu."))?;
        }
    }
    let mut tx = st.db.begin().await?;
    sqlx::query("DELETE FROM share_rules WHERE map_id = ?").bind(mid).execute(&mut *tx).await?;
    for r in &list {
        sqlx::query("INSERT INTO share_rules (map_id, user_id, kind, value) VALUES (?, ?, ?, ?)").bind(mid).bind(r.user_id).bind(&r.kind).bind(r.value).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

/// Calcule les parts d'un mois : fixes, puis à la tâche finie, puis pourcentages sur le reste.
pub async fn compute(st: &AppState, mid: i64, month: &str) -> Result<Value, AppError> {
    let rev: Option<(i64, i64)> = sqlx::query_as("SELECT engagement_cents, sales_cents FROM revenues WHERE map_id = ? AND month = ?").bind(mid).bind(month).fetch_optional(&st.db).await?;
    let (e, s) = rev.unwrap_or((0, 0));
    let total = e + s;
    let rules: Vec<(Option<i64>, Option<String>, String, i64)> = sqlx::query_as("SELECT r.user_id, u.name, r.kind, r.value FROM share_rules r LEFT JOIN users u ON u.id = r.user_id WHERE r.map_id = ?").bind(mid).fetch_all(&st.db).await?;
    let mut lines: Vec<Value> = vec![];
    let mut used = 0i64;
    for (uid, name, kind, value) in &rules {
        let amount = match kind.as_str() {
            "fixed" => *value,
            "per_task" => {
                let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE map_id = ? AND assignee_id IS ? AND status = 'done' AND substr(done_at, 1, 7) = ?").bind(mid).bind(uid).bind(month).fetch_one(&st.db).await?;
                lines.push(json!({ "user_id": uid, "name": name, "kind": kind, "value": value, "tasks": n, "amount_cents": value * n }));
                used += value * n;
                continue;
            }
            _ => continue,
        };
        used += amount;
        lines.push(json!({ "user_id": uid, "name": name.clone().unwrap_or_else(|| "Réserve du studio".into()), "kind": kind, "value": value, "amount_cents": amount }));
    }
    let rest = (total - used).max(0);
    for (uid, name, kind, value) in &rules {
        if kind == "percent" {
            lines.push(json!({ "user_id": uid, "name": name.clone().unwrap_or_else(|| "Réserve du studio".into()), "kind": kind, "value": value, "amount_cents": rest * value / 10000 }));
        }
    }
    for l in lines.iter_mut() {
        let a = l["amount_cents"].as_i64().unwrap_or(0);
        l["from_engagement_cents"] = json!(if total > 0 { a * e / total } else { 0 });
        l["from_sales_cents"] = json!(if total > 0 { a - a * e / total } else { 0 });
    }
    Ok(json!({ "month": month, "engagement_cents": e, "sales_cents": s, "total_cents": total, "fixed_and_tasks_cents": used, "shared_cents": rest, "underfunded": used > total, "lines": lines }))
}

#[derive(Deserialize)]
pub struct MonthQ {
    month: Option<String>,
}

fn redact(ctx: &MapCtx, mut v: Value) -> Value {
    if ctx.role < Role::Admin {
        if let Some(lines) = v["lines"].as_array_mut() {
            lines.retain(|l| l["user_id"].as_i64() == Some(ctx.user.id));
        }
    }
    v
}

pub async fn statements(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Query(q): Query<MonthQ>) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let months: Vec<(String,)> = sqlx::query_as("SELECT month FROM revenues WHERE map_id = ? ORDER BY month").bind(mid).fetch_all(&st.db).await?;
    let month = q.month.filter(|m| valid_month(m)).or_else(|| months.last().map(|m| m.0.clone())).unwrap_or_default();
    let current = compute(&st, mid, &month).await?;
    let mut history = vec![];
    for (m,) in &months {
        history.push(redact(&ctx, compute(&st, mid, m).await?));
    }
    Ok(Json(json!({ "month": month, "months": months.into_iter().map(|m| m.0).collect::<Vec<_>>(), "current": redact(&ctx, current), "history": history, "is_admin": ctx.role >= Role::Admin })))
}

pub async fn offers(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?;
    let rows: Vec<(i64, String, i64, String)> = sqlx::query_as("SELECT id, name, price_vbucks, status FROM offers WHERE map_id = ? ORDER BY id").bind(mid).fetch_all(&st.db).await?;
    let sales: Vec<(i64, String, i64, i64)> = sqlx::query_as("SELECT s.offer_id, s.month, s.sales, s.revenue_cents FROM offer_sales s JOIN offers o ON o.id = s.offer_id WHERE o.map_id = ? ORDER BY s.month").bind(mid).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|(id, name, price, status)| json!({
        "id": id, "name": name, "price_vbucks": price, "status": status,
        "sales": sales.iter().filter(|s| s.0 == id).map(|s| json!({"month": s.1, "sales": s.2, "revenue_cents": s.3})).collect::<Vec<_>>()
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct OfferIn {
    name: String,
    price_vbucks: i64,
    status: Option<String>,
}

pub async fn add_offer(State(st): State<AppState>, u: AuthUser, Path(mid): Path<i64>, Json(b): Json<OfferIn>) -> ApiResult<Value> {
    map_ctx(&st, &u, mid).await?.require(Role::Member)?;
    check_len("Nom", &b.name, 1, 80)?;
    if !(1..=100_000).contains(&b.price_vbucks) {
        return Err(bad("Prix invalide."));
    }
    let id: i64 = sqlx::query_scalar("INSERT INTO offers (map_id, name, price_vbucks, status) VALUES (?, ?, ?, ?) RETURNING id")
        .bind(mid).bind(b.name.trim()).bind(b.price_vbucks).bind(b.status.unwrap_or_else(|| "live".into())).fetch_one(&st.db).await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
pub struct SalesIn {
    month: String,
    sales: i64,
    revenue_cents: i64,
}

pub async fn set_sales(State(st): State<AppState>, u: AuthUser, Path(oid): Path<i64>, Json(b): Json<SalesIn>) -> ApiResult<Value> {
    let (mid,): (i64,) = sqlx::query_as("SELECT map_id FROM offers WHERE id = ?").bind(oid).fetch_one(&st.db).await?;
    map_ctx(&st, &u, mid).await?.require(Role::Admin)?;
    if !valid_month(&b.month) || b.sales < 0 || b.revenue_cents < 0 {
        return Err(bad("Valeurs invalides."));
    }
    sqlx::query("INSERT INTO offer_sales (offer_id, month, sales, revenue_cents) VALUES (?, ?, ?, ?) ON CONFLICT(offer_id, month) DO UPDATE SET sales = excluded.sales, revenue_cents = excluded.revenue_cents")
        .bind(oid).bind(&b.month).bind(b.sales).bind(b.revenue_cents).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn cents() {
        assert_eq!(parse_cents("1 234,56"), Some(123456));
        assert_eq!(parse_cents("1,234.56"), Some(123456));
        assert_eq!(parse_cents("986 €"), Some(98600));
        assert!(valid_month("2026-10"));
        assert!(!valid_month("2026-13"));
    }
}
