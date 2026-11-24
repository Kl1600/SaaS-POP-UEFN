//! Connexion des IA par OAuth (#285), selon la spécification d'autorisation de MCP (version 2025-06-18) :
//! - métadonnées de la ressource protégée (RFC 9728) et du serveur d'autorisation (RFC 8414) ;
//! - enregistrement dynamique des clients (RFC 7591), clients publics sans secret ;
//! - code d'autorisation + PKCE S256 obligatoire, paramètre `resource` (RFC 8707), `iss` dans la réponse (RFC 9207) ;
//! - jetons d'accès d'une heure, jetons de rafraîchissement à usage unique (rotation), révocation (RFC 7009).
//!
//! Un jeton d'accès OAuth est une ligne de `api_tokens` : l'IA apparaît dans Paramètres → IA connectées comme une IA
//! connectée par clé, avec la même lecture seule et la même révocation. Les clés classiques restent valables.
use crate::{
    auth::{random_token, sha256_hex, AuthUser},
    error::{bad, ApiResult},
    security::ClientIp,
    AppState,
};
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use data_encoding::BASE64URL_NOPAD;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const ACCESS_TTL_SECS: i64 = 3600;
const REFRESH_TTL_DAYS: i64 = 60;
const CODE_TTL_MINUTES: i64 = 10;

fn base() -> String {
    crate::mail::public_url()
}

/// Adresse du serveur MCP, telle que l'utilisateur la colle dans son IA.
pub fn resource_url() -> String {
    format!("{}/mcp", base())
}

/// En-tête `WWW-Authenticate` de la réponse 401 du serveur MCP : il dit à l'IA où trouver comment se connecter.
pub fn www_authenticate() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\"",
        base()
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("Bearer"))
}

// ---------- Découverte ----------

/// RFC 9728 : la ressource protégée (le serveur MCP) et son serveur d'autorisation (POP lui-même).
pub async fn protected_resource() -> Json<Value> {
    Json(json!({
        "resource": resource_url(),
        "authorization_servers": [base()],
        "scopes_supported": ["mcp", "mcp:read"],
        "bearer_methods_supported": ["header"],
        "resource_name": "POP",
    }))
}

/// RFC 8414 : où s'autoriser, où prendre les jetons, où s'enregistrer.
pub async fn authorization_server() -> Json<Value> {
    let b = base();
    Json(json!({
        "issuer": b,
        "authorization_endpoint": format!("{b}/oauth/authorize"),
        "token_endpoint": format!("{b}/oauth/token"),
        "registration_endpoint": format!("{b}/oauth/register"),
        "revocation_endpoint": format!("{b}/oauth/revoke"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "revocation_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": ["mcp", "mcp:read"],
        "authorization_response_iss_parameter_supported": true,
        "service_documentation": format!("{b}/aide"),
    }))
}

// ---------- Enregistrement dynamique ----------

/// Adresse de retour acceptée : https, http seulement en boucle locale (RFC 8252), ou schéma propre à une
/// application (cursor://, vscode://…). Jamais de fragment ni de schéma dangereux.
pub fn redirect_ok(u: &str) -> bool {
    if u.len() > 500 || u.contains('#') || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let Some((scheme, rest)) = u.split_once(':') else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    if scheme.is_empty()
        || !scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    {
        return false;
    }
    match scheme.as_str() {
        "https" => rest.starts_with("//") && rest.len() > 2,
        "http" => {
            let host = rest
                .strip_prefix("//")
                .unwrap_or("")
                .split(['/', '?'])
                .next()
                .unwrap_or("");
            let host = match host.strip_prefix('[') {
                Some(h) => h.split(']').next().unwrap_or(""),
                None => host.split(':').next().unwrap_or(""),
            };
            matches!(host, "localhost" | "127.0.0.1" | "::1")
        }
        "javascript" | "data" | "file" | "vbscript" | "about" | "blob" | "ftp" | "ws" | "wss" => {
            false
        }
        _ => !rest.is_empty(),
    }
}

#[derive(Deserialize)]
pub struct RegisterIn {
    #[serde(default)]
    redirect_uris: Vec<String>,
    client_name: Option<String>,
    token_endpoint_auth_method: Option<String>,
}

fn oauth_error(status: StatusCode, error: &str, desc: &str) -> Response {
    (
        status,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
        ],
        Json(json!({ "error": error, "error_description": desc })),
    )
        .into_response()
}

/// RFC 7591 : une IA s'enregistre elle-même (nom et adresses de retour) et reçoit un identifiant de client.
pub async fn register(
    State(st): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(b): Json<RegisterIn>,
) -> Response {
    if crate::auth::hit(
        &st,
        &format!("oauth:register:{ip}"),
        20,
        std::time::Duration::from_secs(3600),
    ) {
    } else {
        return oauth_error(
            StatusCode::TOO_MANY_REQUESTS,
            "invalid_client_metadata",
            "Trop d'enregistrements depuis cette adresse, réessayez plus tard.",
        );
    }
    if b.redirect_uris.is_empty()
        || b.redirect_uris.len() > 10
        || !b.redirect_uris.iter().all(|u| redirect_ok(u))
    {
        return oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_redirect_uri",
            "Adresses de retour invalides : https, http en boucle locale ou schéma d'application.",
        );
    }
    if let Some(m) = &b.token_endpoint_auth_method {
        if m != "none" {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_client_metadata",
                "POP n'accepte que des clients publics (token_endpoint_auth_method = none) avec PKCE.",
            );
        }
    }
    let name: String = b
        .client_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("IA")
        .chars()
        .filter(|c| !c.is_control())
        .take(60)
        .collect();
    let client_id = format!("popc_{}", &random_token()[..32]);
    let uris = serde_json::to_string(&b.redirect_uris).unwrap_or_else(|_| "[]".into());
    if sqlx::query("INSERT INTO oauth_clients (client_id, name, redirect_uris) VALUES (?, ?, ?)")
        .bind(&client_id)
        .bind(&name)
        .bind(&uris)
        .execute(&st.db)
        .await
        .is_err()
    {
        return oauth_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "Erreur interne.",
        );
    }
    (
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({
            "client_id": client_id,
            "client_id_issued_at": chrono::Utc::now().timestamp(),
            "client_name": name,
            "redirect_uris": b.redirect_uris,
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
        })),
    )
        .into_response()
}

// ---------- Autorisation ----------

#[derive(Deserialize, Clone)]
pub struct AuthorizeQ {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    state: Option<String>,
    scope: Option<String>,
    resource: Option<String>,
}

struct Checked {
    client_name: String,
    redirect_uri: String,
}

fn resource_ok(r: &Option<String>) -> bool {
    match r.as_deref() {
        None => true,
        Some(r) => {
            let r = r.trim_end_matches('/');
            r == resource_url() || r == base()
        }
    }
}

/// Vérifie le client et l'adresse de retour. Une erreur ici ne renvoie jamais vers l'adresse de retour
/// (elle n'est pas sûre) : elle s'affiche à l'utilisateur.
async fn check_client(st: &AppState, q: &AuthorizeQ) -> Result<Checked, String> {
    let cid = q.client_id.as_deref().unwrap_or("");
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT name, redirect_uris FROM oauth_clients WHERE client_id = ?")
            .bind(cid)
            .fetch_optional(&st.db)
            .await
            .map_err(|_| "Erreur interne.".to_string())?;
    let Some((name, uris)) = row else {
        return Err(
            "Cette IA n'est pas enregistrée auprès de POP. Reconnectez-la depuis l'IA.".into(),
        );
    };
    let uris: Vec<String> = serde_json::from_str(&uris).unwrap_or_default();
    let redirect = match q.redirect_uri.as_deref() {
        Some(r) if uris.iter().any(|u| u == r) => r.to_string(),
        None if uris.len() == 1 => uris[0].clone(),
        _ => {
            return Err(
                "L'adresse de retour de cette IA ne correspond pas à son enregistrement.".into(),
            )
        }
    };
    Ok(Checked {
        client_name: name,
        redirect_uri: redirect,
    })
}

/// Le reste de la demande : une erreur ici peut être renvoyée à l'IA par son adresse de retour.
fn check_request(q: &AuthorizeQ) -> Result<(), (&'static str, &'static str)> {
    if q.response_type.as_deref() != Some("code") {
        return Err((
            "unsupported_response_type",
            "Seul response_type=code est accepté.",
        ));
    }
    let ch = q.code_challenge.as_deref().unwrap_or("");
    if ch.len() < 43 || ch.len() > 128 {
        return Err(("invalid_request", "PKCE obligatoire (code_challenge)."));
    }
    if q.code_challenge_method.as_deref() != Some("S256") {
        return Err((
            "invalid_request",
            "PKCE : seule la méthode S256 est acceptée.",
        ));
    }
    if !resource_ok(&q.resource) {
        return Err((
            "invalid_target",
            "Ressource inconnue : utilisez l'adresse du serveur MCP de POP.",
        ));
    }
    Ok(())
}

fn with_params(uri: &str, params: &[(&str, &str)]) -> String {
    let qs = serde_urlencoded::to_string(params).unwrap_or_default();
    format!("{uri}{}{qs}", if uri.contains('?') { '&' } else { '?' })
}

fn error_page(msg: &str) -> Response {
    let esc = msg
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        format!("<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>POP · connexion impossible</title>\
<body style=\"font-family:system-ui,sans-serif;max-width:520px;margin:80px auto;padding:0 20px;color:#111\">\
<h1 style=\"font-size:22px;font-weight:600\">Connexion de l'IA impossible</h1><p>{esc}</p>\
<p><a href=\"/\" style=\"color:#ED753E\">Retour à POP</a></p></body></html>"),
    )
        .into_response()
}

/// Point d'entrée ouvert par l'IA dans le navigateur : on vérifie la demande puis on affiche la page « Autoriser »
/// de l'application (qui demande d'abord de se connecter si besoin).
pub async fn authorize_start(
    State(st): State<AppState>,
    Query(q): Query<AuthorizeQ>,
    raw: axum::extract::RawQuery,
) -> Response {
    let c = match check_client(&st, &q).await {
        Ok(c) => c,
        Err(m) => return error_page(&m),
    };
    if let Err((e, d)) = check_request(&q) {
        let mut p = vec![("error", e), ("error_description", d)];
        let state = q.state.clone().unwrap_or_default();
        if q.state.is_some() {
            p.push(("state", &state));
        }
        let b = base();
        p.push(("iss", &b));
        return Redirect::to(&with_params(&c.redirect_uri, &p)).into_response();
    }
    Redirect::to(&format!("/autoriser?{}", raw.0.unwrap_or_default())).into_response()
}

/// Ce que montre la page « Autoriser » : le nom de l'IA et où elle sera renvoyée.
pub async fn authorize_info(
    State(st): State<AppState>,
    _u: AuthUser,
    Query(q): Query<AuthorizeQ>,
) -> ApiResult<Value> {
    let c = check_client(&st, &q).await.map_err(bad)?;
    check_request(&q).map_err(|(_, d)| bad(d))?;
    let host = c
        .redirect_uri
        .split_once("://")
        .map(|(_, r)| r.split(['/', '?']).next().unwrap_or(r).to_string())
        .unwrap_or_else(|| c.redirect_uri.clone());
    Ok(Json(json!({
        "client": c.client_name,
        "redirect_host": host,
        "read_only": q.scope.as_deref().map(|s| s.split(' ').all(|x| x == "mcp:read")).unwrap_or(false),
    })))
}

#[derive(Deserialize)]
pub struct DecideIn {
    #[serde(flatten)]
    q: AuthorizeQ,
    approve: bool,
    #[serde(default)]
    read_only: bool,
}

/// L'utilisateur connecté clique sur « Autoriser » ou « Refuser » : on renvoie l'adresse où envoyer le navigateur.
pub async fn authorize_decide(
    State(st): State<AppState>,
    u: AuthUser,
    ClientIp(ip): ClientIp,
    Json(b): Json<DecideIn>,
) -> ApiResult<Value> {
    let c = check_client(&st, &b.q).await.map_err(bad)?;
    check_request(&b.q).map_err(|(_, d)| bad(d))?;
    let state = b.q.state.clone().unwrap_or_default();
    let iss = base();
    if !b.approve {
        let mut p = vec![
            ("error", "access_denied"),
            ("error_description", "Connexion refusée par l'utilisateur."),
        ];
        if b.q.state.is_some() {
            p.push(("state", &state));
        }
        p.push(("iss", &iss));
        return Ok(Json(
            json!({ "redirect": with_params(&c.redirect_uri, &p) }),
        ));
    }
    let code = random_token();
    sqlx::query("INSERT INTO oauth_codes (code_hash, client_id, user_id, redirect_uri, code_challenge, scope, expires_at) VALUES (?, ?, ?, ?, ?, ?, datetime('now', ?))")
        .bind(sha256_hex(&code))
        .bind(b.q.client_id.as_deref().unwrap_or(""))
        .bind(u.id)
        .bind(&c.redirect_uri)
        .bind(b.q.code_challenge.as_deref().unwrap_or(""))
        .bind(if b.read_only { "read" } else { "write" })
        .bind(format!("+{CODE_TTL_MINUTES} minutes"))
        .execute(&st.db)
        .await?;
    crate::security::audit(&st, Some(u.id), "oauth_autorisation", &c.client_name, &ip).await;
    let mut p = vec![("code", code.as_str())];
    if b.q.state.is_some() {
        p.push(("state", &state));
    }
    p.push(("iss", &iss));
    Ok(Json(
        json!({ "redirect": with_params(&c.redirect_uri, &p) }),
    ))
}

// ---------- Jetons ----------

/// Type d'IA (pour le logo dans POP) d'après le nom donné par le client.
pub fn kind_of(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.contains("claude code") || n.contains("claude-code") {
        "claude_code"
    } else if n.contains("claude") {
        "claude_desktop"
    } else if n.contains("cursor") {
        "cursor"
    } else if n.contains("windsurf") || n.contains("codeium") {
        "windsurf"
    } else if n.contains("visual studio code")
        || n.contains("vscode")
        || n.contains("vs code")
        || n.contains("copilot")
    {
        "vscode"
    } else if n.contains("codex") || n.contains("openai") || n.contains("chatgpt") {
        "codex"
    } else if n.contains("gemini") {
        "gemini"
    } else {
        "autre"
    }
}

fn parse_body(headers: &HeaderMap, body: &Bytes) -> HashMap<String, String> {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("application/json"))
        .unwrap_or(false);
    if json {
        serde_json::from_slice::<HashMap<String, Value>>(body)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
            .collect()
    } else {
        serde_urlencoded::from_bytes(body).unwrap_or_default()
    }
}

fn pkce_ok(verifier: &str, challenge: &str) -> bool {
    let len_ok = (43..=128).contains(&verifier.len());
    let chars_ok = verifier
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._~".contains(c));
    len_ok && chars_ok && BASE64URL_NOPAD.encode(&Sha256::digest(verifier.as_bytes())) == challenge
}

fn token_response(access: &str, refresh: &str, scope: &str) -> Response {
    (
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
        ],
        Json(json!({
            "access_token": access,
            "token_type": "Bearer",
            "expires_in": ACCESS_TTL_SECS,
            "refresh_token": refresh,
            "scope": if scope == "read" { "mcp:read" } else { "mcp" },
        })),
    )
        .into_response()
}

async fn new_refresh(st: &AppState, api_token_id: i64, client_id: &str) -> anyhow::Result<String> {
    let r = format!("popr_{}", random_token());
    sqlx::query("INSERT INTO oauth_refresh (token_hash, api_token_id, client_id, expires_at) VALUES (?, ?, ?, datetime('now', ?))")
        .bind(sha256_hex(&r))
        .bind(api_token_id)
        .bind(client_id)
        .bind(format!("+{REFRESH_TTL_DAYS} days"))
        .execute(&st.db)
        .await?;
    Ok(r)
}

/// Point de jetons : échange d'un code (avec PKCE) ou d'un jeton de rafraîchissement.
pub async fn token(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let p = parse_body(&headers, &body);
    let get = |k: &str| p.get(k).map(String::as_str).unwrap_or("");
    let res = match get("grant_type") {
        "authorization_code" => exchange_code(&st, &p).await,
        "refresh_token" => refresh(&st, get("refresh_token"), get("client_id")).await,
        _ => Err(("unsupported_grant_type", "grant_type inconnu.")),
    };
    match res {
        Ok(r) => r,
        Err((e, d)) => oauth_error(StatusCode::BAD_REQUEST, e, d),
    }
}

type TokenResult = Result<Response, (&'static str, &'static str)>;

async fn exchange_code(st: &AppState, p: &HashMap<String, String>) -> TokenResult {
    let get = |k: &str| p.get(k).map(String::as_str).unwrap_or("");
    let ise = ("server_error", "Erreur interne.");
    if !resource_ok(&p.get("resource").cloned()) {
        return Err(("invalid_target", "Ressource inconnue."));
    }
    // Le code est consommé tout de suite, même si la suite échoue : un code ne sert qu'une fois.
    let row: Option<(String, i64, String, String, String)> = sqlx::query_as(
        "UPDATE oauth_codes SET used = 1 WHERE code_hash = ? AND used = 0 AND expires_at > datetime('now')
         RETURNING client_id, user_id, redirect_uri, code_challenge, scope",
    )
    .bind(sha256_hex(get("code")))
    .fetch_optional(&st.db)
    .await
    .map_err(|_| ise)?;
    let Some((cid, uid, redirect, challenge, scope)) = row else {
        return Err(("invalid_grant", "Code invalide, expiré ou déjà utilisé."));
    };
    if get("client_id") != cid {
        return Err(("invalid_grant", "Ce code a été donné à un autre client."));
    }
    if get("redirect_uri") != redirect && !get("redirect_uri").is_empty() {
        return Err((
            "invalid_grant",
            "redirect_uri différent de celui de l'autorisation.",
        ));
    }
    if !pkce_ok(get("code_verifier"), &challenge) {
        return Err(("invalid_grant", "Vérification PKCE échouée."));
    }
    let disabled: Option<i64> = sqlx::query_scalar("SELECT disabled FROM users WHERE id = ?")
        .bind(uid)
        .fetch_optional(&st.db)
        .await
        .map_err(|_| ise)?;
    if disabled != Some(0) {
        return Err(("invalid_grant", "Compte désactivé."));
    }
    let name: String = sqlx::query_scalar("SELECT name FROM oauth_clients WHERE client_id = ?")
        .bind(&cid)
        .fetch_one(&st.db)
        .await
        .map_err(|_| ise)?;
    let access = format!("pop_{}", random_token());
    let id: i64 = sqlx::query_scalar("INSERT INTO api_tokens (user_id, name, token_hash, kind, scope, expires_at, oauth_client) VALUES (?, ?, ?, ?, ?, datetime('now', ?), ?) RETURNING id")
        .bind(uid)
        .bind(&name)
        .bind(sha256_hex(&access))
        .bind(kind_of(&name))
        .bind(&scope)
        .bind(format!("+{ACCESS_TTL_SECS} seconds"))
        .bind(&cid)
        .fetch_one(&st.db)
        .await
        .map_err(|_| ise)?;
    let r = new_refresh(st, id, &cid).await.map_err(|_| ise)?;
    Ok(token_response(&access, &r, &scope))
}

async fn refresh(st: &AppState, token: &str, client_id: &str) -> TokenResult {
    let ise = ("server_error", "Erreur interne.");
    let row: Option<(i64, String, Option<String>, bool)> = sqlx::query_as(
        "SELECT api_token_id, client_id, used_at, expires_at > datetime('now') FROM oauth_refresh WHERE token_hash = ?",
    )
    .bind(sha256_hex(token))
    .fetch_optional(&st.db)
    .await
    .map_err(|_| ise)?;
    let Some((tid, cid, used, alive)) = row else {
        return Err((
            "invalid_grant",
            "Jeton de rafraîchissement inconnu ou révoqué.",
        ));
    };
    if used.is_some() {
        // Un jeton déjà utilisé qui revient : il a pu être volé. On coupe la connexion.
        let _ = sqlx::query("DELETE FROM api_tokens WHERE id = ?")
            .bind(tid)
            .execute(&st.db)
            .await;
        return Err((
            "invalid_grant",
            "Jeton de rafraîchissement déjà utilisé : connexion révoquée.",
        ));
    }
    if !alive || (!client_id.is_empty() && client_id != cid) {
        return Err((
            "invalid_grant",
            "Jeton de rafraîchissement expiré ou d'un autre client.",
        ));
    }
    let n = sqlx::query("UPDATE oauth_refresh SET used_at = datetime('now') WHERE token_hash = ? AND used_at IS NULL")
        .bind(sha256_hex(token))
        .execute(&st.db)
        .await
        .map_err(|_| ise)?
        .rows_affected();
    if n == 0 {
        return Err(("invalid_grant", "Jeton de rafraîchissement déjà utilisé."));
    }
    let access = format!("pop_{}", random_token());
    let scope: Option<String> = sqlx::query_scalar(
        "UPDATE api_tokens SET token_hash = ?, expires_at = datetime('now', ?) WHERE id = ? AND user_id IN (SELECT id FROM users WHERE disabled = 0) RETURNING scope",
    )
    .bind(sha256_hex(&access))
    .bind(format!("+{ACCESS_TTL_SECS} seconds"))
    .bind(tid)
    .fetch_optional(&st.db)
    .await
    .map_err(|_| ise)?;
    let Some(scope) = scope else {
        return Err(("invalid_grant", "Connexion révoquée."));
    };
    // Les anciens jetons de rafraîchissement de cette connexion ne servent plus.
    let _ = sqlx::query("DELETE FROM oauth_refresh WHERE api_token_id = ? AND used_at IS NOT NULL AND used_at < datetime('now', '-1 day')")
        .bind(tid)
        .execute(&st.db)
        .await;
    let r = new_refresh(st, tid, &cid).await.map_err(|_| ise)?;
    Ok(token_response(&access, &r, &scope))
}

/// RFC 7009 : l'IA révoque un jeton (d'accès ou de rafraîchissement). Réponse 200 dans tous les cas.
pub async fn revoke(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let p = parse_body(&headers, &body);
    let t = p.get("token").map(String::as_str).unwrap_or("");
    if !t.is_empty() {
        let h = sha256_hex(t);
        let _ = sqlx::query("DELETE FROM api_tokens WHERE oauth_client IS NOT NULL AND (token_hash = ? OR id IN (SELECT api_token_id FROM oauth_refresh WHERE token_hash = ?))")
            .bind(&h)
            .bind(&h)
            .execute(&st.db)
            .await;
    }
    StatusCode::OK.into_response()
}

/// Ménage : codes et jetons expirés (appelé de temps en temps).
pub async fn cleanup(st: &AppState) {
    let _ = sqlx::query("DELETE FROM oauth_codes WHERE expires_at < datetime('now', '-1 day')")
        .execute(&st.db)
        .await;
    let _ = sqlx::query("DELETE FROM oauth_refresh WHERE expires_at < datetime('now')")
        .execute(&st.db)
        .await;
    // Une connexion OAuth dont plus aucun jeton de rafraîchissement n'est valable et dont le jeton d'accès a expiré.
    let _ = sqlx::query("DELETE FROM api_tokens WHERE oauth_client IS NOT NULL AND expires_at < datetime('now') AND id NOT IN (SELECT api_token_id FROM oauth_refresh WHERE used_at IS NULL)")
        .execute(&st.db)
        .await;
    let _ = sqlx::query("DELETE FROM oauth_clients WHERE created_at < datetime('now', '-1 day') AND client_id NOT IN (SELECT oauth_client FROM api_tokens WHERE oauth_client IS NOT NULL) AND client_id NOT IN (SELECT client_id FROM oauth_codes)")
        .execute(&st.db)
        .await;
}

/// CORS pour les points publics (découverte, jetons, MCP) : utilisés par des IA dans le navigateur, sans cookie.
pub fn is_public_path(p: &str) -> bool {
    p == "/mcp"
        || p.starts_with("/.well-known/oauth-")
        || p == "/oauth/token"
        || p == "/oauth/register"
        || p == "/oauth/revoke"
}

#[cfg(test)]
mod tests_oauth {
    use super::*;
    use crate::mcp::{mcp, McpQ};

    #[test]
    fn adresses_de_retour() {
        assert!(redirect_ok("https://claude.ai/api/mcp/auth_callback"));
        assert!(redirect_ok("http://localhost:33418/callback"));
        assert!(redirect_ok("http://127.0.0.1:5000/cb"));
        assert!(redirect_ok("http://[::1]:8000/cb"));
        assert!(redirect_ok(
            "cursor://anysphere.cursor-retrieval/oauth/callback"
        ));
        assert!(!redirect_ok("http://evil.com/cb"));
        assert!(!redirect_ok("http://localhost.evil.com/cb"));
        assert!(!redirect_ok("javascript:alert(1)"));
        assert!(!redirect_ok("https://a.fr/cb#x"));
        assert!(!redirect_ok("data:text/html,x"));
        assert_eq!(kind_of("Claude Code (pop)"), "claude_code");
        assert_eq!(kind_of("Claude"), "claude_desktop");
        assert_eq!(kind_of("ChatGPT"), "codex");
        assert_eq!(kind_of("Visual Studio Code"), "vscode");
    }

    async fn body(r: Response) -> (StatusCode, Value) {
        let s = r.status();
        let b = axum::body::to_bytes(r.into_body(), usize::MAX)
            .await
            .unwrap();
        (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }

    fn form(p: &[(&str, &str)]) -> (HeaderMap, Bytes) {
        let mut h = HeaderMap::new();
        h.insert(
            header::CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        (h, Bytes::from(serde_urlencoded::to_string(p).unwrap()))
    }

    async fn ping(st: &AppState, token: &str) -> (StatusCode, HeaderMap) {
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        let r = mcp(
            State(st.clone()),
            h,
            Query(McpQ { token: None }),
            Json(json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" })),
        )
        .await;
        (r.status(), r.headers().clone())
    }

    fn param(url: &str, k: &str) -> Option<String> {
        let q = url.split_once('?')?.1;
        serde_urlencoded::from_str::<Vec<(String, String)>>(q)
            .ok()?
            .into_iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v)
    }

    struct Ctx {
        st: AppState,
        user: AuthUser,
        cid: String,
        verifier: String,
    }

    async fn ctx() -> Ctx {
        let st = crate::test_state().await;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO users (email, name, password_hash) VALUES ('o@a.fr', 'Olga', 'x') RETURNING id",
        )
        .fetch_one(&st.db)
        .await
        .unwrap();
        let (s, v) = body(
            register(
                State(st.clone()),
                ClientIp("1.2.3.4".into()),
                Json(RegisterIn {
                    redirect_uris: vec!["http://localhost:4567/callback".into()],
                    client_name: Some("Claude Code".into()),
                    token_endpoint_auth_method: Some("none".into()),
                }),
            )
            .await,
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);
        Ctx {
            st,
            user: AuthUser {
                id,
                email: "o@a.fr".into(),
                name: "Olga".into(),
                sid: String::new(),
            },
            cid: v["client_id"].as_str().unwrap().into(),
            verifier: "v".repeat(20) + &random_token(),
        }
    }

    fn q(c: &Ctx) -> AuthorizeQ {
        AuthorizeQ {
            response_type: Some("code".into()),
            client_id: Some(c.cid.clone()),
            redirect_uri: Some("http://localhost:4567/callback".into()),
            code_challenge: Some(BASE64URL_NOPAD.encode(&Sha256::digest(c.verifier.as_bytes()))),
            code_challenge_method: Some("S256".into()),
            state: Some("etat-42".into()),
            scope: Some("mcp".into()),
            resource: Some(resource_url()),
        }
    }

    async fn code(c: &Ctx, read_only: bool) -> String {
        let r = authorize_decide(
            State(c.st.clone()),
            c.user.clone(),
            ClientIp("1.2.3.4".into()),
            Json(DecideIn {
                q: q(c),
                approve: true,
                read_only,
            }),
        )
        .await
        .unwrap();
        let url = r.0["redirect"].as_str().unwrap().to_string();
        assert!(url.starts_with("http://localhost:4567/callback?"));
        assert_eq!(param(&url, "state").as_deref(), Some("etat-42"));
        assert_eq!(param(&url, "iss"), Some(base()));
        param(&url, "code").unwrap()
    }

    async fn exchange(c: &Ctx, code: &str, verifier: &str) -> (StatusCode, Value) {
        let (h, b) = form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", "http://localhost:4567/callback"),
            ("client_id", &c.cid),
            ("code_verifier", verifier),
            ("resource", &resource_url()),
        ]);
        body(token(State(c.st.clone()), h, b).await).await
    }

    async fn refresh_with(c: &Ctx, r: &str) -> (StatusCode, Value) {
        let (h, b) = form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", r),
            ("client_id", &c.cid),
        ]);
        body(token(State(c.st.clone()), h, b).await).await
    }

    #[tokio::test]
    async fn flux_complet_avec_rotation() {
        let c = ctx().await;
        // Sans jeton : 401 avec l'adresse des métadonnées.
        let (s, h) = ping(&c.st, "").await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert!(h[header::WWW_AUTHENTICATE]
            .to_str()
            .unwrap()
            .contains("/.well-known/oauth-protected-resource/mcp"));
        // Mauvais vérificateur PKCE : refusé, et le code ne sert plus.
        let k = code(&c, false).await;
        assert_eq!(
            exchange(&c, &k, &("x".repeat(50))).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            exchange(&c, &k, &c.verifier).await.0,
            StatusCode::BAD_REQUEST
        );
        // Bon flux.
        let k = code(&c, false).await;
        let (s, v) = exchange(&c, &k, &c.verifier).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let a1 = v["access_token"].as_str().unwrap().to_string();
        let r1 = v["refresh_token"].as_str().unwrap().to_string();
        assert_eq!(v["token_type"], "Bearer");
        assert_eq!(ping(&c.st, &a1).await.0, StatusCode::OK);
        // Le code ne sert qu'une fois.
        assert_eq!(
            exchange(&c, &k, &c.verifier).await.0,
            StatusCode::BAD_REQUEST
        );
        // L'IA apparaît comme une IA du compte, avec le bon type.
        let (kind, scope): (String, String) =
            sqlx::query_as("SELECT kind, scope FROM api_tokens WHERE oauth_client = ?")
                .bind(&c.cid)
                .fetch_one(&c.st.db)
                .await
                .unwrap();
        assert_eq!((kind.as_str(), scope.as_str()), ("claude_code", "write"));
        // Rotation : nouveau jeton d'accès, l'ancien ne marche plus.
        let (s, v) = refresh_with(&c, &r1).await;
        assert_eq!(s, StatusCode::OK);
        let a2 = v["access_token"].as_str().unwrap().to_string();
        let r2 = v["refresh_token"].as_str().unwrap().to_string();
        assert_ne!(r1, r2);
        assert_eq!(ping(&c.st, &a1).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(ping(&c.st, &a2).await.0, StatusCode::OK);
        // Réutiliser l'ancien jeton de rafraîchissement coupe la connexion.
        assert_eq!(refresh_with(&c, &r1).await.0, StatusCode::BAD_REQUEST);
        assert_eq!(ping(&c.st, &a2).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(refresh_with(&c, &r2).await.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn lecture_seule_expiration_revocation_refus() {
        let c = ctx().await;
        let k = code(&c, true).await;
        let (_, v) = exchange(&c, &k, &c.verifier).await;
        assert_eq!(v["scope"], "mcp:read");
        let a = v["access_token"].as_str().unwrap().to_string();
        let r = v["refresh_token"].as_str().unwrap().to_string();
        // Jeton d'accès expiré : refusé.
        sqlx::query("UPDATE api_tokens SET expires_at = datetime('now', '-1 minute') WHERE oauth_client = ?")
            .bind(&c.cid)
            .execute(&c.st.db)
            .await
            .unwrap();
        assert_eq!(ping(&c.st, &a).await.0, StatusCode::UNAUTHORIZED);
        // Le rafraîchissement redonne un jeton, toujours en lecture seule.
        let (_, v) = refresh_with(&c, &r).await;
        assert_eq!(v["scope"], "mcp:read");
        let a = v["access_token"].as_str().unwrap().to_string();
        let r = v["refresh_token"].as_str().unwrap().to_string();
        assert_eq!(ping(&c.st, &a).await.0, StatusCode::OK);
        // Révocation par l'IA (RFC 7009) avec le jeton de rafraîchissement.
        let (h, b) = form(&[("token", &r)]);
        assert_eq!(
            revoke(State(c.st.clone()), h, b).await.status(),
            StatusCode::OK
        );
        assert_eq!(ping(&c.st, &a).await.0, StatusCode::UNAUTHORIZED);
        // Refus : l'IA reçoit access_denied et l'état.
        let d = authorize_decide(
            State(c.st.clone()),
            c.user.clone(),
            ClientIp("1.2.3.4".into()),
            Json(DecideIn {
                q: q(&c),
                approve: false,
                read_only: false,
            }),
        )
        .await
        .unwrap();
        let url = d.0["redirect"].as_str().unwrap();
        assert_eq!(param(url, "error").as_deref(), Some("access_denied"));
        assert_eq!(param(url, "state").as_deref(), Some("etat-42"));
        // Demande sans PKCE, autre ressource ou adresse de retour non enregistrée : refusée.
        let mut sans = q(&c);
        sans.code_challenge = None;
        assert!(
            authorize_info(State(c.st.clone()), c.user.clone(), Query(sans))
                .await
                .is_err()
        );
        let mut autre = q(&c);
        autre.resource = Some("https://ailleurs.fr/mcp".into());
        assert!(
            authorize_info(State(c.st.clone()), c.user.clone(), Query(autre))
                .await
                .is_err()
        );
        let mut retour = q(&c);
        retour.redirect_uri = Some("http://localhost:9999/vol".into());
        assert!(
            authorize_info(State(c.st.clone()), c.user.clone(), Query(retour))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn la_revocation_dans_pop_coupe_le_rafraichissement() {
        let c = ctx().await;
        let k = code(&c, false).await;
        let (_, v) = exchange(&c, &k, &c.verifier).await;
        let r = v["refresh_token"].as_str().unwrap().to_string();
        // L'utilisateur clique sur la corbeille dans Paramètres → IA connectées.
        sqlx::query("DELETE FROM api_tokens WHERE oauth_client = ?")
            .bind(&c.cid)
            .execute(&c.st.db)
            .await
            .unwrap();
        assert_eq!(refresh_with(&c, &r).await.0, StatusCode::BAD_REQUEST);
    }
}
