mod access;
mod account;
mod activation;
mod admin;
mod agents;
mod aiplan;
mod aipop;
mod analytics;
mod announce;
mod auth;
mod billing;
mod boards;
mod catalog;
mod checks;
mod discover;
mod engine;
mod error;
mod faisabilite;
mod fil;
mod guide;
mod ideas;
mod integrations;
mod lancement;
mod launch;
mod legal;
mod library;
mod link;
mod mail;
mod manage;
mod mcp;
mod model;
mod money;
mod oauth;
mod orchestre;
mod pages;
mod plan;
mod privstats;
mod questions;
mod roles;
mod s3;
mod security;
mod skills;
mod stripe;
mod studio;
mod suggestions;
mod tasks;
mod uefn;
mod verify;
mod verse;
mod widgets;
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

/// État d'application pour les tests de routes : base SQLite en mémoire, migrations appliquées.
#[cfg(test)]
pub async fn test_state() -> AppState {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::from_str("sqlite::memory:")
                .unwrap()
                .foreign_keys(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    AppState {
        db,
        hub: Arc::new(ws::Hub::default()),
        limiter: Arc::new(Mutex::new(HashMap::new())),
        board_lock: Arc::new(tokio::sync::Mutex::new(())),
        secure_cookies: false,
        dummy_hash: auth::hash_password("mot-de-passe-factice").unwrap(),
        relays: Arc::new(uefn::RelayHub::default()),
        http: reqwest::Client::new(),
    }
}

/// Protection CSRF : toute requête qui modifie des données doit porter l'en-tête « X-POP: 1 »,
/// qu'un autre site ne peut pas ajouter sans autorisation CORS (que l'on ne donne jamais).
/// CORS des points publics de connexion des IA (#285) : sans cookie, donc sans risque pour les comptes.
async fn cors_public(req: Request<Body>, next: Next) -> Response {
    if !oauth::is_public_path(req.uri().path()) {
        return next.run(req).await;
    }
    let mut res = if req.method() == Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(req).await
    };
    let h = res.headers_mut();
    h.insert("access-control-allow-origin", "*".parse().unwrap());
    h.insert(
        "access-control-allow-methods",
        "GET, POST, OPTIONS".parse().unwrap(),
    );
    h.insert(
        "access-control-allow-headers",
        "authorization, content-type, mcp-protocol-version, mcp-session-id"
            .parse()
            .unwrap(),
    );
    h.insert(
        "access-control-expose-headers",
        "www-authenticate, mcp-session-id".parse().unwrap(),
    );
    res
}

async fn csrf(req: Request<Body>, next: Next) -> Response {
    let m = req.method().clone();
    // HTTPS forcé : derrière le proxy (Caddy, Nginx, Cloudflare…), une requête arrivée en http est renvoyée en https.
    if https_mode() {
        let proto = req
            .headers()
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if proto.eq_ignore_ascii_case("http") {
            let host = req
                .headers()
                .get("host")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            if !host.is_empty()
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))
            {
                let pq = req
                    .uri()
                    .path_and_query()
                    .map(|x| x.as_str().to_string())
                    .unwrap_or_else(|| "/".into());
                return axum::response::Redirect::permanent(&format!("https://{host}{pq}"))
                    .into_response();
            }
        }
    }
    let path = req.uri().path().to_string();
    if m != Method::GET
        && m != Method::HEAD
        && m != Method::OPTIONS
        && !path.starts_with("/api/hooks/")
        && !path.starts_with("/api/relay/")
        && path != "/mcp"
        && !(path.starts_with("/oauth/") && path != "/oauth/authorize")
    {
        let ok = req.headers().get("x-pop").and_then(|v| v.to_str().ok()) == Some("1");
        if !ok {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Requête refusée (CSRF)." })),
            )
                .into_response();
        }
    }
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", "nosniff".parse().unwrap());
    h.insert("x-frame-options", "DENY".parse().unwrap());
    h.insert("referrer-policy", "same-origin".parse().unwrap());
    h.insert(
        "permissions-policy",
        "camera=(), microphone=(), geolocation=(), payment=()"
            .parse()
            .unwrap(),
    );
    h.insert("cross-origin-opener-policy", "same-origin".parse().unwrap());
    // Politique de contenu : scripts uniquement de POP, aucune intégration de POP dans un autre site.
    h.insert("content-security-policy", "default-src 'self'; script-src 'self' blob:; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; img-src 'self' data: blob: https:; media-src 'self' blob: https:; connect-src 'self' ws: wss:; frame-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'; object-src 'none'".parse().unwrap());
    if https_mode() {
        h.insert(
            "strict-transport-security",
            "max-age=63072000; includeSubDomains".parse().unwrap(),
        );
    }
    // Pas de mise en cache des réponses de l'API (données personnelles), sauf les vignettes relayées.
    if path.starts_with("/api/") && path != "/api/img" {
        h.insert("cache-control", "no-store".parse().unwrap());
    }
    // Fichiers de l'interface : ceux de /assets/ portent leur empreinte dans leur nom et ne changent jamais ; tout
    // le reste (index.html, pages, téléchargements) est revalidé à chaque visite. Sans cela, après une mise à jour,
    // un navigateur garde l'ancien index.html qui réclame des fichiers supprimés : page blanche.
    else if !h.contains_key("cache-control") {
        let v = if path.starts_with("/assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        h.insert("cache-control", v.parse().unwrap());
    }
    res
}

/// Production en HTTPS : `POP_SECURE_COOKIES=1`, ou une adresse publique en `https://`.
pub fn https_mode() -> bool {
    std::env::var("POP_SECURE_COOKIES")
        .map(|v| v == "1")
        .unwrap_or(false)
        || mail::public_url().starts_with("https://")
}

/// Rapport du soir (texte prêt à poster sur Discord).
async fn report(
    State(st): State<AppState>,
    u: auth::AuthUser,
    Path(mid): Path<i64>,
) -> Result<Json<Value>, error::AppError> {
    access::map_ctx(&st, &u, mid).await?;
    Ok(Json(json!({ "text": report_text(&st, mid).await? })))
}

pub async fn report_text(st: &AppState, mid: i64) -> Result<String, error::AppError> {
    let st = st.clone();
    let (name,): (String,) = sqlx::query_as("SELECT name FROM maps WHERE id = ?")
        .bind(mid)
        .fetch_one(&st.db)
        .await?;
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
    t.push_str(&format!(
        "Fait aujourd'hui ({}) : {}\n",
        done.len(),
        if done.is_empty() {
            "rien".into()
        } else {
            done.iter()
                .map(|(n, x)| format!("#{n} {x}"))
                .collect::<Vec<_>>()
                .join(" · ")
        }
    ));
    t.push_str(&format!("En cours : {doing} · Bloqué : {blocked}\n"));
    t.push_str(&format!(
        "Demain : {}\n",
        next.iter()
            .map(|(n, x, w)| format!("#{n} {x} ({})", w.clone().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join(" · ")
    ));
    if let Some((title, date)) = ms {
        t.push_str(&format!("Prochain objectif : {title}, {date}\n"));
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
        .route(
            "/api/studios/{sid}/members/{uid}/reset-link",
            post(account::admin_reset_link),
        )
        .route("/api/maps/{mid}/search", get(account::search))
        .route("/api/billing", get(stripe::status))
        .route("/api/billing/checkout", post(stripe::checkout))
        .route("/api/billing/portal", post(stripe::portal))
        .route("/api/admin/ai", get(aipop::admin_report))
        .route("/api/admin/users/{id}/credits", post(aipop::admin_grant))
        .route("/api/maps/{mid}/ai/plan", post(aiplan::propose))
        .route("/api/maps/{mid}/ai/plan/apply", post(aiplan::apply))
        .route(
            "/api/maps/{mid}/private-stats",
            get(privstats::get).post(privstats::import),
        )
        .route("/api/legal", get(legal::get))
        .route("/api/admin/legal", put(legal::save))
        .route("/api/img", get(legal::image))
        .route("/api/analytics/rising", get(analytics::rising_list))
        .route(
            "/api/admin/model",
            get(model::admin_status).post(model::admin_refresh),
        )
        .route("/api/admin/ia", get(model::admin_learning))
        .route("/api/ideas", get(ideas::list))
        .route("/api/ideas/niches", get(ideas::all_niches))
        .route(
            "/api/ideas/profile",
            axum::routing::put(ideas::save_settings),
        )
        .route("/api/ideas/{id}/state", post(ideas::set_state))
        .route("/api/ideas/{id}/start", post(ideas::start))
        .route("/api/analytics/status", get(analytics::status))
        .route("/api/analytics/refresh", post(analytics::refresh))
        .route("/api/analytics/trending", get(analytics::trending))
        .route("/api/analytics/launches", get(analytics::launches))
        .route("/api/analytics/market", get(analytics::market))
        .route("/api/analytics/islands/{code}", get(analytics::island))
        .route(
            "/api/analytics/watch",
            get(analytics::watched).post(analytics::watch),
        )
        .route(
            "/api/maps/{mid}/island",
            get(analytics::map_island).put(analytics::set_map_island),
        )
        .route("/api/studios/{sid}/tiktok", get(analytics::tiktok))
        .route(
            "/api/studios/{sid}/tiktok/refresh",
            post(analytics::tiktok_refresh),
        )
        .route("/api/me", get(auth::me).delete(manage::delete_me))
        .route("/api/me/export", get(manage::export_me))
        .route(
            "/api/studios/{sid}",
            patch(manage::rename_studio).delete(manage::delete_studio),
        )
        .route("/api/studios/{sid}/transfer", post(manage::transfer))
        .route("/api/studios/{sid}/relay/download", get(link::download))
        .route("/api/studios/{sid}/link-status", get(link::studio_status))
        .route("/api/studios/{sid}/skip-uefn", post(link::skip))
        .route("/api/relay/pair-studio", post(link::pair_studio))
        .route("/api/studios/{sid}/leave", post(manage::leave))
        .route("/api/studios/{sid}/export", get(manage::export_studio))
        .route(
            "/api/invitations/{id}",
            axum::routing::delete(manage::cancel_invite),
        )
        .route("/api/studios", post(studio::create_studio))
        .route(
            "/api/studios/{sid}/maps",
            post(studio::create_map).get(studio::maps_summary),
        )
        .route("/api/maps/{mid}/move", post(studio::move_map))
        .route("/api/studios/{sid}/members", get(studio::members))
        .route(
            "/api/studios/{sid}/members/{uid}",
            patch(studio::update_member).delete(studio::remove_member),
        )
        .route("/api/studios/{sid}/invitations", post(studio::invite))
        .route("/api/invitations/accept", post(studio::accept_invite))
        .route("/api/studios/{sid}/integrations", get(studio::integrations))
        .route(
            "/api/studios/{sid}/integrations/{kind}",
            put(studio::set_integration),
        )
        .route("/api/hooks/lore/{sid}", post(studio::lore_hook))
        .route("/api/hooks/stripe", post(stripe::webhook))
        .route("/api/hooks/discord/{sid}", post(integrations::discord_hook))
        .route("/api/hooks/github/{sid}", post(integrations::github_hook))
        .route(
            "/api/studios/{sid}/discord/send",
            post(integrations::discord_send),
        )
        .route(
            "/api/studios/{sid}/discord/register",
            post(integrations::discord_register),
        )
        .route(
            "/api/studios/{sid}/github/sync",
            post(integrations::github_sync),
        )
        .route(
            "/api/maps/{mid}",
            patch(studio::update_map).delete(manage::delete_map),
        )
        .route("/api/maps/{mid}/ws", get(ws::ws_handler))
        .route("/api/maps/{mid}/overview", get(plan::overview))
        .route("/api/waitlist", post(admin::join))
        .route("/api/waitlist/access", get(admin::check_access))
        .route("/api/auth/verify-email", post(verify::verify))
        .route(
            "/api/activation",
            get(activation::check).post(activation::activate),
        )
        .route("/api/me/verify-email/resend", post(verify::resend))
        .route("/api/me/security", get(security::overview))
        .route("/api/me/first-password", post(account::first_password))
        .route("/api/me/sessions/{id}", delete(security::revoke_session))
        .route(
            "/api/me/sessions/close-others",
            post(security::revoke_others),
        )
        .route("/api/admin/unlock", post(security::unlock_admin))
        .route("/api/admin/lock", post(security::lock_admin))
        .route("/api/admin/audit", get(security::admin_audit))
        .route("/api/admin/journal", get(admin::journal))
        .route("/api/admin/overview", get(admin::overview))
        .route(
            "/api/admin/announcement",
            get(announce::current)
                .post(announce::publish)
                .delete(announce::remove),
        )
        .route("/api/announcement/{id}/dismiss", post(announce::dismiss))
        .route("/api/admin/waitlist", get(admin::waitlist))
        .route("/api/admin/waitlist/{id}/approve", post(admin::approve))
        .route("/api/admin/waitlist/{id}/refuse", post(admin::refuse))
        .route("/api/admin/waitlist/{id}/reopen", post(admin::reopen))
        .route("/api/admin/waitlist/{id}/note", post(admin::note))
        .route("/api/admin/waitlist/{id}", delete(admin::remove))
        .route("/api/admin/invite", post(admin::invite))
        .route("/api/admin/users", get(admin::users))
        .route(
            "/api/admin/users/{id}",
            post(admin::update_user).delete(admin::delete_user),
        )
        .route("/api/admin/accounts", post(admin::create_account))
        .route("/api/admin/users/{id}/plan", post(admin::set_plan))
        .route(
            "/api/admin/users/{id}/password",
            post(admin::reset_password),
        )
        .route(
            "/api/admin/users/{id}/reset-link",
            post(admin::user_reset_link),
        )
        .route("/api/admin/users/{id}/activation", post(activation::resend))
        .route("/api/admin/users/{id}/unlock", post(admin::unlock_user))
        .route(
            "/api/admin/users/{id}/deletion",
            get(admin::deletion_preview),
        )
        .route("/api/maps/{mid}/guide", get(guide::guide))
        .route("/api/maps/{mid}/guide/ask", get(guide::ask))
        .route("/api/maps/{mid}/report", get(report))
        // Tâches
        .route(
            "/api/maps/{mid}/tasks",
            get(tasks::list).post(tasks::create),
        )
        .route("/api/maps/{mid}/people", get(tasks::people))
        .route("/api/maps/{mid}/locks", get(tasks::locks))
        .route(
            "/api/tasks/{tid}",
            get(tasks::detail)
                .patch(tasks::update)
                .delete(tasks::delete),
        )
        .route("/api/tasks/{tid}/status", post(tasks::set_status))
        .route("/api/tasks/{tid}/subtasks", post(tasks::add_subtask))
        .route("/api/tasks/{tid}/comments", post(tasks::comment))
        .route("/api/subtasks/{id}", patch(tasks::toggle_subtask))
        // Pages
        .route("/api/page-templates", get(pages::templates))
        .route("/api/maps/{mid}/pages/import", post(pages::import_md))
        .route(
            "/api/maps/{mid}/pages",
            get(pages::tree).post(pages::create),
        )
        .route(
            "/api/pages/{pid}",
            get(pages::get).patch(pages::update).delete(pages::delete),
        )
        .route("/api/pages/{pid}/versions", get(pages::versions))
        .route(
            "/api/pages/{pid}/versions/{vid}/restore",
            post(pages::restore),
        )
        .route("/api/pages/{pid}/proof", get(pages::proof))
        .route("/api/pages/{pid}/export.md", get(pages::export_md))
        .route(
            "/api/pages/{pid}/comments",
            get(pages::page_comments).post(pages::add_page_comment),
        )
        .route("/api/comments/{cid}/resolve", post(pages::resolve_comment))
        .route("/api/public/pages/{token}", get(pages::public_page))
        // Tableau blanc
        .route(
            "/api/maps/{mid}/boards",
            get(boards::list).post(boards::create),
        )
        .route(
            "/api/boards/{bid}",
            get(boards::get)
                .patch(boards::update)
                .delete(boards::delete),
        )
        .route("/api/boards/{bid}/duplicate", post(boards::duplicate))
        .route("/api/maps/{mid}/trash", get(pages::trash))
        .route("/api/pages/{pid}/restore", post(pages::restore_page))
        .route("/api/pages/{pid}/duplicate", post(pages::duplicate))
        .route("/api/boards/{bid}/convert", post(boards::convert))
        .route(
            "/api/boards/{bid}/frame-to-page",
            post(boards::frame_to_page),
        )
        // Planifier
        .route("/api/maps/{mid}/milestones", post(plan::add_milestone))
        .route(
            "/api/milestones/{id}",
            delete(plan::delete_milestone).patch(plan::update_milestone),
        )
        .route("/api/maps/{mid}/plan/preview", post(plan::preview))
        .route("/api/maps/{mid}/plan/apply", post(plan::apply))
        .route("/api/maps/{mid}/plan/timeline", get(plan::timeline))
        .route("/api/maps/{mid}/plan/move", post(plan::move_and_recalc))
        .route("/api/maps/{mid}/plan/recalc", post(plan::recalc))
        .route("/api/maps/{mid}/seasons", get(plan::seasons))
        // Lancer et suivre
        .route(
            "/api/maps/{mid}/discover",
            get(discover::get).post(discover::save),
        )
        .route("/api/maps/{mid}/checks", get(launch::checks))
        .route("/api/maps/{mid}/checks/{key}", put(launch::set_check))
        .route(
            "/api/maps/{mid}/launch-steps",
            get(launch::steps).post(launch::add_step),
        )
        .route("/api/launch-steps/{id}", patch(launch::toggle_step))
        .route(
            "/api/maps/{mid}/playtests",
            get(launch::exchange).post(launch::create_listing),
        )
        .route("/api/playtests/{lid}/feedback", post(launch::give_feedback))
        .route("/api/maps/{mid}/churn", get(launch::churn))
        // Rémunérer
        .route(
            "/api/maps/{mid}/revenues",
            get(money::revenues).put(money::set_revenue),
        )
        .route("/api/maps/{mid}/revenues/csv", post(money::import_csv))
        .route(
            "/api/maps/{mid}/share-rules",
            get(money::rules).put(money::set_rules),
        )
        .route("/api/maps/{mid}/statements", get(money::statements))
        .route(
            "/api/maps/{mid}/offers",
            get(money::offers).post(money::add_offer),
        )
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
        // Vérifications automatiques du niveau (#96) ; « /checks » est la checklist de la page Lancer (#130).
        .route("/api/maps/{mid}/level-checks", get(checks::get))
        .route("/api/maps/{mid}/agents", get(agents::list))
        .route("/api/maps/{mid}/agents/activity", get(agents::activite))
        .route("/api/maps/{mid}/fil", get(fil::route))
        .route("/api/maps/{mid}/questions", get(questions::route_liste))
        .route(
            "/api/maps/{mid}/faisabilite",
            get(faisabilite::route_lire).put(faisabilite::route_ecrire),
        )
        .route("/api/maps/{mid}/suggestions", get(suggestions::liste))
        .route(
            "/api/maps/{mid}/suggestions/{cle}",
            post(suggestions::route_demarrer),
        )
        .route(
            "/api/maps/{mid}/lancement",
            get(lancement::route_apercu).post(lancement::route_lancer),
        )
        .route(
            "/api/maps/{mid}/questions/{qid}",
            post(questions::route_repondre),
        )
        .route("/api/uefn-actions/{id}/undo", post(uefn::undo))
        .route("/api/approvals/{id}/{verdict}", post(uefn::decide))
        .route(
            "/api/maps/{mid}/filters",
            get(uefn::filters).post(uefn::add_filter),
        )
        .route("/api/filters/{id}", delete(uefn::delete_filter))
        .route("/api/maps/{mid}/verse/files", get(verse::files))
        .route("/api/maps/{mid}/verse/file", get(verse::file))
        .route("/api/maps/{mid}/verse/all", get(verse::all_contents))
        .route("/api/maps/{mid}/verse/versions", get(verse::versions))
        .route("/api/verse-versions/{id}", get(verse::version))
        .route("/api/verse-versions/{id}/revert", post(verse::revert))
        .route("/api/maps/{mid}/verse/symbols", get(verse::symbols))
        .route("/api/maps/{mid}/verse/send", post(verse::send))
        .route(
            "/api/studios/{sid}/library",
            get(library::list).post(library::create),
        )
        .route(
            "/api/library/{id}",
            get(library::get)
                .post(library::update)
                .delete(library::delete),
        )
        .route(
            "/api/studios/{sid}/skills",
            get(skills::list).post(skills::save),
        )
        .route("/api/studios/{sid}/skills/{name}", get(skills::get))
        .route("/api/skills/{id}", delete(skills::delete))
        .route("/api/me/tokens", get(mcp::tokens).post(mcp::create_token))
        .route("/api/tokens/{id}/scope", post(mcp::set_scope))
        .route("/api/tokens/{id}/role", post(mcp::set_role))
        .route("/api/agent-roles", get(mcp::roles))
        .route("/api/tokens/{id}", delete(mcp::delete_token))
        .route("/mcp", post(mcp::mcp))
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth::protected_resource),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(oauth::protected_resource),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth::authorization_server),
        )
        .route(
            "/.well-known/oauth-authorization-server/mcp",
            get(oauth::authorization_server),
        )
        .route("/oauth/register", post(oauth::register))
        .route("/oauth/authorize", get(oauth::authorize_start))
        .route("/oauth/token", post(oauth::token))
        .route("/oauth/revoke", post(oauth::revoke))
        .route(
            "/api/oauth/authorize",
            get(oauth::authorize_info).post(oauth::authorize_decide),
        )
}

/// Nomme ou retire un administrateur de la plateforme. Seul quelqu'un qui a accès à la machine du serveur (et à sa
/// base) peut le faire : c'est ce qui remplace l'ancien « premier compte créé depuis cet ordinateur ».
async fn admin_cli(db: &sqlx::SqlitePool, args: &[String]) -> anyhow::Result<()> {
    const USAGE: &str = "Usage : pop-server admin ajouter <e-mail> | retirer <e-mail> | liste";
    match (args.first().map(String::as_str), args.get(1)) {
        (Some(action @ ("ajouter" | "retirer")), Some(email)) => {
            let email = email.trim().to_lowercase();
            let flag = (action == "ajouter") as i64;
            let n = sqlx::query("UPDATE users SET is_admin = ? WHERE email = ?")
                .bind(flag)
                .bind(&email)
                .execute(db)
                .await?
                .rows_affected();
            if n == 0 {
                anyhow::bail!("Aucun compte avec l'adresse « {email} » : créez d'abord le compte dans POP, puis relancez.");
            }
            sqlx::query("INSERT INTO audit_log (user_id, action, detail, ip) SELECT id, ?, 'commande pop-server admin', '' FROM users WHERE email = ?")
                .bind(if flag == 1 { "admin_nommé" } else { "admin_retiré" })
                .bind(&email)
                .execute(db)
                .await?;
            println!(
                "{email} {} administrateur de la plateforme.",
                if flag == 1 {
                    "est maintenant"
                } else {
                    "n'est plus"
                }
            );
            let verified: Option<String> =
                sqlx::query_scalar("SELECT email_verified_at FROM users WHERE email = ?")
                    .bind(&email)
                    .fetch_one(db)
                    .await?;
            if flag == 1 && verified.is_none() {
                println!("Attention : ce compte n'a pas encore confirmé son adresse e-mail. Il aura les droits d'administration après avoir ouvert le lien reçu.");
            }
            if flag == 1 {
                println!("Pour ouvrir l'administration, ce compte doit activer la double authentification (Paramètres → Sécurité).");
            }
        }
        (Some("liste"), _) => {
            let rows: Vec<(String, String)> =
                sqlx::query_as("SELECT email, name FROM users WHERE is_admin = 1 ORDER BY email")
                    .fetch_all(db)
                    .await?;
            if rows.is_empty() {
                println!("Aucun administrateur nommé (hors POP_ADMIN_EMAILS).");
            }
            for (e, n) in rows {
                println!("{e}  ({n})");
            }
        }
        _ => anyhow::bail!(USAGE),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "pop_server=info".into()))
        .init();
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://pop-data.db".into());
    let opts = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let db = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;
    renumber_old_migrations(&db).await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    // `pop-server admin …` : gérer les administrateurs depuis la machine du serveur, puis s'arrêter.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("admin") {
        return admin_cli(&db, &args[1..]).await;
    }
    let st = AppState {
        db,
        hub: Arc::new(ws::Hub::default()),
        limiter: Arc::new(Mutex::new(HashMap::new())),
        board_lock: Arc::new(tokio::sync::Mutex::new(())),
        secure_cookies: https_mode(),
        dummy_hash: auth::hash_password("mot-de-passe-factice")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?,
        relays: Arc::new(uefn::RelayHub::default()),
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?,
    };
    // Compte de test local « kl » / « kl » (activé par start.sh et Lancer POP.bat, jamais dans l'image Docker).
    // En ligne (HTTPS), le compte « kl » n'est jamais créé, et s'il existe déjà dans la base il est bloqué :
    // sinon n'importe qui pourrait se connecter en administrateur avec « kl » / « kl ».
    if https_mode() {
        sqlx::query("UPDATE users SET password_hash = '!', is_admin = 0 WHERE email = 'kl'")
            .execute(&st.db)
            .await?;
        sqlx::query(
            "DELETE FROM sessions WHERE user_id IN (SELECT id FROM users WHERE email = 'kl')",
        )
        .execute(&st.db)
        .await
        .ok();
    }
    if !https_mode()
        && std::env::var("POP_DEV_ACCOUNT")
            .map(|v| v == "1")
            .unwrap_or(false)
    {
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE email = 'kl'")
            .fetch_optional(&st.db)
            .await?;
        if exists.is_none() {
            let h = auth::hash_password("kl").map_err(|e| anyhow::anyhow!("{e:?}"))?;
            sqlx::query("INSERT INTO users (email, name, password_hash, is_admin, plan) VALUES ('kl', 'KL', ?, 1, 'pro')").bind(h).execute(&st.db).await?;
        }
        sqlx::query("UPDATE users SET is_admin = 1, email_verified_at = COALESCE(email_verified_at, datetime('now')) WHERE email = 'kl'").execute(&st.db).await?;
        println!("Compte de test : identifiant « kl », mot de passe « kl »");
    }
    tokio::spawn(integrations::scheduler(st.clone()));
    tokio::spawn(admin::digest_loop(st.clone()));
    tokio::spawn(analytics::crawler(st.clone()));
    tokio::spawn(analytics::social_loop(st.clone()));
    tokio::spawn(manage::backup_loop(st.clone(), db_url.clone()));
    let web = std::env::var("POP_WEB_DIR").unwrap_or_else(|_| "../web/dist".into());
    let spa = ServeDir::new(&web).fallback(ServeFile::new(format!("{web}/index.html")));
    let app = api()
        .fallback_service(spa)
        .layer(middleware::from_fn(csrf))
        .layer(middleware::from_fn(cors_public))
        .layer(RequestBodyLimitLayer::new(16 * 1024 * 1024))
        .layer(tower_http::catch_panic::CatchPanicLayer::custom(|_| {
            tracing::error!("erreur interne (panic) pendant une requête");
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, [(axum::http::header::CONTENT_TYPE, "application/json")], r#"{"error":"Erreur interne du serveur POP. Réessayez ; si ça continue, regardez le terminal."}"#).into_response()
        }))
        .with_state(st);
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    // 127.0.0.1 par défaut ; POP_BIND=0.0.0.0 pour le réseau local ou un hébergement.
    let ip: std::net::IpAddr = std::env::var("POP_BIND")
        .ok()
        .and_then(|b| b.parse().ok())
        .unwrap_or([127, 0, 0, 1].into());
    let addr = SocketAddr::new(ip, port);
    println!("POP écoute sur http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

/// Les migrations de la branche « claude » (comptes, plans, idées, Discover, mentions légales, modèle, Starter/Pro,
/// paiements) portaient les numéros 12 à 20, déjà pris sur `main` par d'autres migrations. Elles sont maintenant
/// numérotées 15 à 23. Une base locale qui les a appliquées sous les anciens numéros est mise à jour ici, une seule
/// fois : on reconnaît l'ancienne numérotation à la migration 13 « personal ». Les bases de production ne sont pas
/// concernées (rien ne se passe si ce cas n'existe pas).
async fn renumber_old_migrations(db: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    let table: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(db)
    .await?;
    if table == 0 {
        return Ok(());
    }
    let old: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM _sqlx_migrations WHERE version = 13 AND description = 'personal'",
    )
    .fetch_one(db)
    .await?;
    if old == 0 {
        return Ok(());
    }
    let mut tx = db.begin().await?;
    // En deux temps pour ne jamais avoir deux lignes avec le même numéro.
    sqlx::query(
        "UPDATE _sqlx_migrations SET version = version + 1000 WHERE version BETWEEN 12 AND 20",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE _sqlx_migrations SET version = version - 997 WHERE version BETWEEN 1012 AND 1020",
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!("Migrations locales renumérotées (12-20 → 15-23).");
    Ok(())
}

#[cfg(test)]
mod routeur {
    /// Le routeur complet se construit : axum refuse (panique) une route déclarée deux fois (#130).
    #[test]
    fn routes_sans_doublon() {
        let _ = super::api();
    }
}
