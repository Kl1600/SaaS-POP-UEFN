mod access;
mod account;
mod analytics;
mod ai;
mod mail;
mod manage;
mod engine;
mod guide;
mod integrations;
mod link;
mod catalog;
mod mcp;
mod uefn;
mod verse;
mod auth;
mod boards;
mod error;
mod launch;
mod money;
mod pages;
mod plan;
mod studio;
mod tasks;
mod ws;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::{
    collections::HashMap,
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Instant,
};
use tower_http::{
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
};

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub hub: Arc<ws::Hub>,
    pub limiter: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    pub board_lock: Arc<tokio::sync::Mutex<()>>,
    pub secure_cookies: bool,
    pub dummy_hash: String,
    pub relays: Arc<uefn::RelayHub>,
    pub http: reqwest::Client,
}

/// Protection CSRF : toute requête qui modifie des données doit porter l'en-tête « X-POP: 1 »,
/// qu'un autre site ne peut pas ajouter sans autorisation CORS (que l'on ne donne jamais).
async fn csrf(req: Request<Body>, next: Next) -> Response {
    let m = req.method().clone();
    let path = req.uri().path().to_string();
    if m != Method::GET && m != Method::HEAD && m != Method::OPTIONS && !path.starts_with("/api/hooks/") && !path.starts_with("/api/relay/") && path != "/mcp" {
        let ok = req.headers().get("x-pop").and_then(|v| v.to_str().ok()) == Some("1");
        if !ok {
            return (StatusCode::FORBIDDEN, Json(json!({ "error": "Requête refusée (CSRF)." }))).into_response();
        }
    }
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", "nosniff".parse().unwrap());
    h.insert("x-frame-options", "DENY".parse().unwrap());
    h.insert("referrer-policy", "same-origin".parse().unwrap());
    res
}

/// Rapport du soir (texte prêt à poster sur Discord).
async fn report(State(st): State<AppState>, u: auth::AuthUser, Path(mid): Path<i64>) -> Result<Json<Value>, error::AppError> {
    access::map_ctx(&st, &u, mid).await?;
    Ok(Json(json!({ "text": report_text(&st, mid).await? })))
}

pub async fn report_text(st: &AppState, mid: i64) -> Result<String, error::AppError> {
    let st = st.clone();
    let (name,): (String,) = sqlx::query_as("SELECT name FROM maps WHERE id = ?").bind(mid).fetch_one(&st.db).await?;
    let done: Vec<(i64, String)> = sqlx::query_as("SELECT number, title FROM tasks WHERE map_id = ? AND status = 'done' AND done_at >= date('now') ORDER BY number").bind(mid).fetch_all(&st.db).await?;
    let (doing, blocked): (i64, i64) = sqlx::query_as("SELECT COALESCE(SUM(status = 'doing'), 0), COALESCE(SUM(status = 'blocked'), 0) FROM tasks WHERE map_id = ?").bind(mid).fetch_one(&st.db).await?;
    let next: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT t.number, t.title, u.name FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id WHERE t.map_id = ? AND t.status IN ('todo', 'doing') AND t.planned_day <= date('now', '+1 day') ORDER BY t.planned_day, t.number LIMIT 5",
    )
    .bind(mid)
    .fetch_all(&st.db)
    .await?;
    let ms: Option<(String, String)> = sqlx::query_as("SELECT title, date FROM milestones WHERE map_id = ? AND date >= date('now') ORDER BY date LIMIT 1").bind(mid).fetch_optional(&st.db).await?;
    let mut t = format!("**Rapport du soir · {name}**\n");
    t.push_str(&format!("Fait aujourd'hui ({}) : {}\n", done.len(), if done.is_empty() { "rien".into() } else { done.iter().map(|(n, x)| format!("#{n} {x}")).collect::<Vec<_>>().join(" · ") }));
    t.push_str(&format!("En cours : {doing} · Bloqué : {blocked}\n"));
    t.push_str(&format!("Demain : {}\n", next.iter().map(|(n, x, w)| format!("#{n} {x} ({})", w.clone().unwrap_or_default())).collect::<Vec<_>>().join(" · ")));
    if let Some((title, date)) = ms {
        t.push_str(&format!("Prochain jalon : {title}, {date}\n"));
    }
    Ok(t)
}

fn api() -> Router<AppState> {
    Router::new()
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/forgot", post(account::forgot))
        .route("/api/auth/reset", post(account::reset))
        .route("/api/me/profile", post(account::update_profile))
        .route("/api/me/password", post(account::change_password))
        .route("/api/notifications", get(account::notifications))
        .route("/api/notifications/read", post(account::read_notifications))
        .route("/api/studios/{sid}/members/{uid}/reset-link", post(account::admin_reset_link))
        .route("/api/maps/{mid}/search", get(account::search))
        .route("/api/analytics/status", get(analytics::status))
        .route("/api/analytics/refresh", post(analytics::refresh))
        .route("/api/analytics/trending", get(analytics::trending))
        .route("/api/analytics/launches", get(analytics::launches))
        .route("/api/analytics/market", get(analytics::market))
        .route("/api/analytics/islands/{code}", get(analytics::island))
        .route("/api/analytics/watch", get(analytics::watched).post(analytics::watch))
        .route("/api/maps/{mid}/island", get(analytics::map_island).put(analytics::set_map_island))
        .route("/api/studios/{sid}/tiktok", get(analytics::tiktok))
        .route("/api/studios/{sid}/tiktok/refresh", post(analytics::tiktok_refresh))
        .route("/api/me", get(auth::me).delete(manage::delete_me))
        .route("/api/me/export", get(manage::export_me))
        .route("/api/studios/{sid}", patch(manage::rename_studio).delete(manage::delete_studio))
        .route("/api/studios/{sid}/transfer", post(manage::transfer))
        .route("/api/studios/{sid}/relay/download", get(link::download))
        .route("/api/studios/{sid}/link-status", get(link::studio_status))
        .route("/api/studios/{sid}/skip-uefn", post(link::skip))
        .route("/api/relay/pair-studio", post(link::pair_studio))
        .route("/api/studios/{sid}/leave", post(manage::leave))
        .route("/api/studios/{sid}/export", get(manage::export_studio))
        .route("/api/invitations/{id}", axum::routing::delete(manage::cancel_invite))
        .route("/api/studios", post(studio::create_studio))
        .route("/api/studios/{sid}/maps", post(studio::create_map).get(studio::maps_summary))
        .route("/api/studios/{sid}/members", get(studio::members))
        .route("/api/studios/{sid}/members/{uid}", patch(studio::update_member).delete(studio::remove_member))
        .route("/api/studios/{sid}/invitations", post(studio::invite))
        .route("/api/invitations/accept", post(studio::accept_invite))
        .route("/api/studios/{sid}/integrations", get(studio::integrations))
        .route("/api/studios/{sid}/integrations/{kind}", put(studio::set_integration))
        .route("/api/hooks/lore/{sid}", post(studio::lore_hook))
        .route("/api/hooks/discord/{sid}", post(integrations::discord_hook))
        .route("/api/hooks/github/{sid}", post(integrations::github_hook))
        .route("/api/studios/{sid}/discord/send", post(integrations::discord_send))
        .route("/api/studios/{sid}/discord/register", post(integrations::discord_register))
        .route("/api/studios/{sid}/github/sync", post(integrations::github_sync))
        .route("/api/maps/{mid}", patch(studio::update_map).delete(manage::delete_map))
        .route("/api/maps/{mid}/ws", get(ws::ws_handler))
        .route("/api/maps/{mid}/overview", get(plan::overview))
        .route("/api/maps/{mid}/guide", get(guide::guide))
        .route("/api/maps/{mid}/report", get(report))
        // Tâches
        .route("/api/maps/{mid}/tasks", get(tasks::list).post(tasks::create))
        .route("/api/maps/{mid}/people", get(tasks::people))
        .route("/api/maps/{mid}/locks", get(tasks::locks))
        .route("/api/tasks/{tid}", get(tasks::detail).patch(tasks::update).delete(tasks::delete))
        .route("/api/tasks/{tid}/status", post(tasks::set_status))
        .route("/api/tasks/{tid}/subtasks", post(tasks::add_subtask))
        .route("/api/tasks/{tid}/comments", post(tasks::comment))
        .route("/api/subtasks/{id}", patch(tasks::toggle_subtask))
        // Pages
        .route("/api/page-templates", get(pages::templates))
        .route("/api/maps/{mid}/pages", get(pages::tree).post(pages::create))
        .route("/api/pages/{pid}", get(pages::get).patch(pages::update).delete(pages::delete))
        .route("/api/pages/{pid}/versions", get(pages::versions))
        .route("/api/pages/{pid}/versions/{vid}/restore", post(pages::restore))
        .route("/api/pages/{pid}/proof", get(pages::proof))
        .route("/api/pages/{pid}/export.md", get(pages::export_md))
        .route("/api/pages/{pid}/comments", get(pages::page_comments).post(pages::add_page_comment))
        .route("/api/comments/{cid}/resolve", post(pages::resolve_comment))
        .route("/api/public/pages/{token}", get(pages::public_page))
        // Tableau blanc
        .route("/api/maps/{mid}/boards", get(boards::list).post(boards::create))
        .route("/api/boards/{bid}", get(boards::get).patch(boards::update).delete(boards::delete))
        .route("/api/boards/{bid}/duplicate", post(boards::duplicate))
        .route("/api/maps/{mid}/trash", get(pages::trash))
        .route("/api/pages/{pid}/restore", post(pages::restore_page))
        .route("/api/pages/{pid}/duplicate", post(pages::duplicate))
        .route("/api/boards/{bid}/convert", post(boards::convert))
        .route("/api/boards/{bid}/frame-to-page", post(boards::frame_to_page))
        // Planifier
        .route("/api/maps/{mid}/milestones", post(plan::add_milestone))
        .route("/api/milestones/{id}", delete(plan::delete_milestone))
        .route("/api/maps/{mid}/plan/preview", post(plan::preview))
        .route("/api/maps/{mid}/plan/apply", post(plan::apply))
        .route("/api/maps/{mid}/plan/timeline", get(plan::timeline))
        .route("/api/maps/{mid}/plan/move", post(plan::move_and_recalc))
        .route("/api/maps/{mid}/plan/recalc", post(plan::recalc))
        .route("/api/maps/{mid}/seasons", get(plan::seasons))
        // Lancer et suivre
        .route("/api/maps/{mid}/checks", get(launch::checks))
        .route("/api/maps/{mid}/checks/{key}", put(launch::set_check))
        .route("/api/maps/{mid}/launch-steps", get(launch::steps).post(launch::add_step))
        .route("/api/launch-steps/{id}", patch(launch::toggle_step))
        .route("/api/maps/{mid}/playtests", get(launch::exchange).post(launch::create_listing))
        .route("/api/playtests/{lid}/feedback", post(launch::give_feedback))
        .route("/api/maps/{mid}/churn", get(launch::churn))
        // Rémunérer
        .route("/api/maps/{mid}/revenues", get(money::revenues).put(money::set_revenue))
        .route("/api/maps/{mid}/revenues/csv", post(money::import_csv))
        .route("/api/maps/{mid}/share-rules", get(money::rules).put(money::set_rules))
        .route("/api/maps/{mid}/statements", get(money::statements))
        .route("/api/maps/{mid}/offers", get(money::offers).post(money::add_offer))
        .route("/api/offers/{oid}/sales", put(money::set_sales))
        // UEFN Link : relais, Organiseur, éditeur Verse, IA, MCP
        .route("/api/maps/{mid}/relay/pair", post(uefn::pair_code))
        .route("/api/maps/{mid}/relay", get(uefn::devices))
        .route("/api/relay-devices/{id}", delete(uefn::delete_device))
        .route("/api/relay/pair", post(uefn::pair))
        .route("/api/relay/ws", get(uefn::relay_ws))
        .route("/api/maps/{mid}/uefn", get(uefn::state))
        .route("/api/maps/{mid}/uefn/props", get(uefn::props))
        .route("/api/maps/{mid}/uefn/topview", get(uefn::topview))
        .route("/api/maps/{mid}/uefn/action", post(uefn::action))
        .route("/api/maps/{mid}/uefn/journal", get(uefn::journal))
        .route("/api/uefn-actions/{id}/undo", post(uefn::undo))
        .route("/api/approvals/{id}/{verdict}", post(uefn::decide))
        .route("/api/maps/{mid}/filters", get(uefn::filters).post(uefn::add_filter))
        .route("/api/filters/{id}", delete(uefn::delete_filter))
        .route("/api/maps/{mid}/verse/files", get(verse::files))
        .route("/api/maps/{mid}/verse/file", get(verse::file))
        .route("/api/maps/{mid}/verse/all", get(verse::all_contents))
        .route("/api/maps/{mid}/verse/versions", get(verse::versions))
        .route("/api/verse-versions/{id}", get(verse::version))
        .route("/api/verse-versions/{id}/revert", post(verse::revert))
        .route("/api/maps/{mid}/verse/symbols", get(verse::symbols))
        .route("/api/maps/{mid}/verse/send", post(verse::send))
        .route("/api/maps/{mid}/ai/verse", post(ai::verse_ai))
        .route("/api/maps/{mid}/tokens", get(mcp::tokens).post(mcp::create_token))
        .route("/api/tokens/{id}/scope", post(mcp::set_scope))
        .route("/api/tokens/{id}", delete(mcp::delete_token))
        .route("/mcp", post(mcp::mcp))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "pop_server=info".into())).init();
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://pop-data.db".into());
    let opts = SqliteConnectOptions::from_str(&db_url)?.create_if_missing(true).journal_mode(SqliteJournalMode::Wal).foreign_keys(true);
    let db = SqlitePoolOptions::new().max_connections(8).connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    let st = AppState {
        db,
        hub: Arc::new(ws::Hub::default()),
        limiter: Arc::new(Mutex::new(HashMap::new())),
        board_lock: Arc::new(tokio::sync::Mutex::new(())),
        secure_cookies: std::env::var("POP_SECURE_COOKIES").map(|v| v == "1").unwrap_or(false),
        dummy_hash: auth::hash_password("mot-de-passe-factice").map_err(|e| anyhow::anyhow!("{e:?}"))?,
        relays: Arc::new(uefn::RelayHub::default()),
        http: reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build()?,
    };
    // Compte de test local « kl » / « kl » (activé par start.sh et Lancer POP.bat, jamais dans l'image Docker).
    if std::env::var("POP_DEV_ACCOUNT").map(|v| v == "1").unwrap_or(false) {
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE email = 'kl'").fetch_optional(&st.db).await?;
        if exists.is_none() {
            let h = auth::hash_password("kl").map_err(|e| anyhow::anyhow!("{e:?}"))?;
            sqlx::query("INSERT INTO users (email, name, password_hash) VALUES ('kl', 'KL', ?)").bind(h).execute(&st.db).await?;
        }
        println!("Compte de test : identifiant « kl », mot de passe « kl »");
    }
    tokio::spawn(integrations::scheduler(st.clone()));
    tokio::spawn(analytics::crawler(st.clone()));
    tokio::spawn(analytics::social_loop(st.clone()));
    tokio::spawn(manage::backup_loop(st.clone(), db_url.clone()));
    let web = std::env::var("POP_WEB_DIR").unwrap_or_else(|_| "../web/dist".into());
    let spa = ServeDir::new(&web).fallback(ServeFile::new(format!("{web}/index.html")));
    let app = api()
        .fallback_service(spa)
        .layer(middleware::from_fn(csrf))
        .layer(RequestBodyLimitLayer::new(16 * 1024 * 1024))
        .with_state(st);
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    // 127.0.0.1 par défaut ; POP_BIND=0.0.0.0 pour le réseau local ou un hébergement.
    let ip: std::net::IpAddr = std::env::var("POP_BIND").ok().and_then(|b| b.parse().ok()).unwrap_or([127, 0, 0, 1].into());
    let addr = SocketAddr::new(ip, port);
    println!("POP écoute sur http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
