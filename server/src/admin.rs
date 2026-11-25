//! Administration de la plateforme POP : liste d'attente de la bêta, utilisateurs, vue générale.
//! Réservé aux comptes `is_admin` (nommés par `pop-server admin ajouter <e-mail>`, le compte de test « kl »),
//! ou dont l'adresse est dans POP_ADMIN_EMAILS. Les modérateurs (#65) ont une partie : liste d'attente, dépannage
//! des comptes ordinaires (déblocage, lien de mot de passe) et consultation des comptes.
use crate::{
    auth::{random_token, rate_limit, sha256_hex, AuthUser},
    error::{bad, ApiResult, AppError},
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Administrateur de la plateforme : nommé (`is_admin`) ou listé dans POP_ADMIN_EMAILS, et **adresse vérifiée** (#11) :
/// sans cela, n'importe qui pourrait s'inscrire avec l'adresse d'un administrateur.
pub async fn is_admin(st: &AppState, uid: i64) -> Result<bool, AppError> {
    let a: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT is_admin, email, email_verified_at IS NOT NULL FROM users WHERE id = ?",
    )
    .bind(uid)
    .fetch_optional(&st.db)
    .await?;
    let Some((flag, email, verified)) = a else {
        return Ok(false);
    };
    if verified == 0 {
        return Ok(false);
    }
    let env = std::env::var("POP_ADMIN_EMAILS").unwrap_or_default();
    Ok(flag == 1
        || env
            .split(',')
            .map(|e| e.trim().to_lowercase())
            .any(|e| !e.is_empty() && e == email.to_lowercase()))
}
/// Adresse listée dans POP_ADMIN_EMAILS (administrateur défini sur le serveur, non modifiable dans POP).
pub fn env_admin(email: &str) -> bool {
    std::env::var("POP_ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .any(|e| !e.trim().is_empty() && e.trim().eq_ignore_ascii_case(email.trim()))
}

/// Administrateurs actifs (nommés ou du serveur, adresse vérifiée, compte non désactivé), hors `sauf`.
async fn autres_admins(st: &AppState, sauf: i64) -> Result<i64, AppError> {
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT id, email, is_admin FROM users WHERE id <> ? AND disabled = 0 AND email_verified_at IS NOT NULL",
    )
    .bind(sauf)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .iter()
        .filter(|(_, e, a)| *a == 1 || env_admin(e))
        .count() as i64)
}

/// Rôle dans l'équipe POP. L'ordre compte : un administrateur peut tout ce que peut un modérateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Staff {
    None,
    Moderator,
    Admin,
}

/// Le rôle du compte : administrateur, modérateur (`is_moderator`, adresse vérifiée, compte actif) ou rien.
pub async fn staff(st: &AppState, uid: i64) -> Result<Staff, AppError> {
    if is_admin(st, uid).await? {
        return Ok(Staff::Admin);
    }
    let m: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE id = ? AND is_moderator = 1 AND disabled = 0 AND email_verified_at IS NOT NULL",
    )
    .bind(uid)
    .fetch_one(&st.db)
    .await?;
    Ok(if m > 0 { Staff::Moderator } else { Staff::None })
}

async fn require(st: &AppState, u: &AuthUser) -> Result<(), AppError> {
    crate::security::require_admin(st, u).await
}
/// Routes ouvertes aussi aux modérateurs (#65). Renvoie le rôle de l'appelant.
async fn require_mod(st: &AppState, u: &AuthUser) -> Result<Staff, AppError> {
    crate::security::require_staff(st, u, Staff::Moderator).await
}
async fn log(st: &AppState, u: &AuthUser, action: &str, detail: &str) {
    crate::security::audit(st, Some(u.id), action, detail, "").await;
}

/// Adresse d'une demande de la liste d'attente, pour le journal (#66) : on y lit qui est visé, pas un numéro.
async fn demande(st: &AppState, id: i64) -> String {
    sqlx::query_scalar::<_, String>("SELECT email FROM waitlist WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("demande n° {id}"))
}

// ---------- Public : demande d'accès ----------

#[derive(Deserialize)]
pub struct WaitIn {
    email: String,
    name: String,
    studio: Option<String>,
    team: Option<String>,
    experience: Option<String>,
    links: Option<String>,
    message: Option<String>,
    /// Champ piège, caché aux humains : rempli, la demande vient d'un robot.
    #[serde(default)]
    pop_hp: String,
}

/// Demandes d'accès acceptées par adresse IP et par heure.
const WAIT_PER_IP_HOUR: u32 = 5;

pub async fn join(
    State(st): State<AppState>,
    crate::security::ClientIp(ip): crate::security::ClientIp,
    Json(b): Json<WaitIn>,
) -> ApiResult<Value> {
    if !crate::auth::hit(
        &st,
        &format!("wait:ip:{ip}"),
        WAIT_PER_IP_HOUR,
        std::time::Duration::from_secs(3600),
    ) {
        return Err(AppError::TooManyMsg(
            "Trop de demandes d’accès depuis votre connexion : réessayez dans une heure.".into(),
        ));
    }
    // Un robot reçoit la même réponse qu'un humain, mais rien n'est enregistré.
    if !b.pop_hp.trim().is_empty() {
        return Ok(Json(json!({ "ok": true })));
    }
    rate_limit(&st, &format!("wait:{}", b.email))?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') || !email.contains('.') || email.len() > 200 {
        return Err(bad("Adresse e-mail invalide."));
    }
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(bad("Indiquez votre nom (80 caractères au plus)."));
    }
    let cut = |v: &Option<String>, n: usize| {
        v.as_deref()
            .unwrap_or("")
            .trim()
            .chars()
            .take(n)
            .collect::<String>()
    };
    let has_account: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&st.db)
        .await?;
    // Même réponse qu'une adresse inconnue : on ne révèle pas qui a un compte (#17). La personne est prévenue par e-mail.
    if has_account > 0 {
        if crate::mail::configured() {
            let to = email.clone();
            tokio::spawn(async move {
                let body = format!("Bonjour,\n\nQuelqu'un (sans doute vous) a demandé l'accès à la bêta de POP avec cette adresse. Vous avez déjà un compte POP : connectez-vous ici :\n{}/connexion\n\nMot de passe oublié ? Utilisez « Mot de passe oublié » sur cette page.\n\nSi vous n'avez rien demandé, ignorez ce message.\n\nL’équipe POP", crate::mail::public_url());
                if let Err(e) =
                    crate::mail::send(&to, "POP · vous avez déjà un compte", &body).await
                {
                    tracing::warn!("e-mail « compte existant » : {e:#}");
                }
            });
        }
        return Ok(Json(json!({ "ok": true })));
    }
    let existing: Option<String> =
        sqlx::query_scalar("SELECT status FROM waitlist WHERE email = ?")
            .bind(&email)
            .fetch_optional(&st.db)
            .await?;
    if let Some(s) = existing {
        return Ok(Json(json!({ "ok": true, "already": true, "status": s })));
    }
    sqlx::query("INSERT INTO waitlist (email, name, studio, team, experience, links, message) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&email).bind(name).bind(cut(&b.studio, 80)).bind(cut(&b.team, 40)).bind(cut(&b.experience, 40)).bind(cut(&b.links, 400)).bind(cut(&b.message, 1500))
        .execute(&st.db).await?;
    // Les administrateurs sont prévenus par le résumé horaire (`digest_loop`).
    Ok(Json(json!({ "ok": true })))
}

/// Demandes pas encore signalées, si la plus ancienne attend depuis une heure : au plus un résumé par heure.
/// Les marque comme signalées et renvoie (nom, e-mail, studio) de chacune.
pub async fn take_digest(db: &sqlx::SqlitePool) -> Result<Vec<(String, String, String)>, AppError> {
    let due: Option<i64> = sqlx::query_scalar("SELECT MAX(id) FROM waitlist WHERE admin_notified_at IS NULL HAVING MIN(created_at) <= datetime('now', '-1 hour')")
        .fetch_optional(db).await?.flatten();
    let Some(last) = due else { return Ok(vec![]) };
    let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT name, email, studio FROM waitlist WHERE admin_notified_at IS NULL AND id <= ? ORDER BY id")
        .bind(last).fetch_all(db).await?;
    sqlx::query("UPDATE waitlist SET admin_notified_at = datetime('now') WHERE admin_notified_at IS NULL AND id <= ?").bind(last).execute(db).await?;
    Ok(rows)
}

/// Envoie le résumé des nouvelles demandes d'accès : une notification et un e-mail par admin.
pub async fn send_digest(st: &AppState) -> Result<usize, AppError> {
    let rows = take_digest(&st.db).await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let n = rows.len();
    let title = if n == 1 {
        "1 nouvelle demande d’accès à la bêta".to_string()
    } else {
        format!("{n} nouvelles demandes d’accès à la bêta")
    };
    let mut body = format!(
        "{title} de POP :

"
    );
    for (name, email, studio) in rows.iter().take(50) {
        body += &format!(
            "- {name} ({email}){}
",
            if studio.is_empty() {
                String::new()
            } else {
                format!(", studio {studio}")
            }
        );
    }
    if n > 50 {
        body += &format!(
            "… et {} autres.
",
            n - 50
        )
    }
    body += &format!(
        "
Pour les traiter :
{}/admin",
        crate::mail::public_url()
    );
    let admins: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, email FROM users WHERE is_admin = 1 AND disabled = 0")
            .fetch_all(&st.db)
            .await?;
    for (id, mail) in &admins {
        let _ = sqlx::query(
            "INSERT INTO notifications (user_id, map_id, text, link) VALUES (?, NULL, ?, '/admin')",
        )
        .bind(id)
        .bind(&title)
        .execute(&st.db)
        .await;
        if crate::mail::configured() && mail.contains('@') {
            if let Err(e) = crate::mail::send(mail, &format!("POP : {title}"), &body).await {
                tracing::warn!("résumé de la liste d’attente non envoyé à un admin : {e}")
            }
        }
    }
    tracing::info!(
        "résumé de la liste d’attente : {n} demande(s), {} admin(s)",
        admins.len()
    );
    Ok(n)
}

/// Photo du jour (#68) : comptes actifs aujourd'hui et comptes par abonnement, gardés pour les courbes.
pub async fn photo_du_jour(st: &AppState) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO admin_stats_day (day, active, starter, pro, trials)
         SELECT date('now'),
                (SELECT COUNT(*) FROM users WHERE date(last_seen) = date('now')),
                (SELECT COUNT(*) FROM users WHERE plan = 'starter'),
                (SELECT COUNT(*) FROM users WHERE plan = 'pro'),
                (SELECT COUNT(*) FROM users WHERE trial_until > datetime('now'))
         ON CONFLICT(day) DO UPDATE SET active = MAX(active, excluded.active), starter = excluded.starter,
                pro = excluded.pro, trials = excluded.trials",
    )
    .execute(&st.db)
    .await?;
    Ok(())
}

/// Courbes de la vue générale (#68), sur `jours` jours : inscriptions, comptes actifs, demandes d'accès, et
/// abonnements. Un compte est actif un jour s'il s'est connecté ou a agi (tâche, page, UEFN) ce jour-là ; la photo
/// quotidienne complète ce qu'on sait de lui.
async fn courbes(st: &AppState, jours: i64) -> Result<Value, AppError> {
    let depuis = format!("-{} days", jours - 1);
    let parjour = |sql: String| {
        let (st, depuis) = (st.clone(), depuis.clone());
        async move {
            sqlx::query_as::<_, (String, i64)>(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(depuis)
                .fetch_all(&st.db)
                .await
                .map(|v| v.into_iter().collect::<std::collections::HashMap<_, _>>())
        }
    };
    let inscriptions = parjour("SELECT date(created_at), COUNT(*) FROM users WHERE date(created_at) >= date('now', ?) GROUP BY 1".into()).await?;
    let demandes = parjour("SELECT date(created_at), COUNT(*) FROM waitlist WHERE date(created_at) >= date('now', ?) GROUP BY 1".into()).await?;
    let actifs = parjour(
        "SELECT d, COUNT(DISTINCT u) FROM (
            SELECT date(created_at) d, user_id u FROM sessions
            UNION SELECT date(last_seen), user_id FROM sessions WHERE last_seen IS NOT NULL
            UNION SELECT date(at), user_id FROM task_events WHERE user_id IS NOT NULL
            UNION SELECT date(at), user_id FROM page_versions WHERE user_id IS NOT NULL
            UNION SELECT date(at), user_id FROM uefn_actions WHERE user_id IS NOT NULL
        ) WHERE d >= date('now', ?) GROUP BY d"
            .into(),
    )
    .await?;
    let photos: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT day, active, starter, pro, trials FROM admin_stats_day WHERE day >= date('now', ?) ORDER BY day",
    )
    .bind(&depuis)
    .fetch_all(&st.db)
    .await?;
    let jours_liste: Vec<String> = sqlx::query_scalar(
        "WITH RECURSIVE j(d) AS (SELECT date('now', ?) UNION ALL SELECT date(d, '+1 day') FROM j WHERE d < date('now')) SELECT d FROM j",
    )
    .bind(&depuis)
    .fetch_all(&st.db)
    .await?;
    let photo = |d: &str| photos.iter().find(|p| p.0 == d);
    let un = |m: &std::collections::HashMap<String, i64>, d: &str| m.get(d).copied().unwrap_or(0);
    let plans: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM users WHERE plan = 'starter'), (SELECT COUNT(*) FROM users WHERE plan = 'pro'),
                (SELECT COUNT(*) FROM users WHERE trial_until > datetime('now'))",
    )
    .fetch_one(&st.db)
    .await?;
    Ok(json!({
        "days": jours,
        "daily": jours_liste.iter().map(|d| json!({
            "day": d, "signups": un(&inscriptions, d), "requests": un(&demandes, d),
            "active": un(&actifs, d).max(photo(d).map(|p| p.1).unwrap_or(0)),
        })).collect::<Vec<_>>(),
        "plans": { "starter": plans.0, "pro": plans.1, "trials": plans.2 },
        "plans_history": photos.iter().map(|p| json!({ "day": p.0, "starter": p.2, "pro": p.3, "trials": p.4 })).collect::<Vec<_>>(),
    }))
}

/// Toutes les minutes : envoie le résumé des demandes d'accès quand il est dû, et met à jour la photo du jour (#68).
pub async fn digest_loop(st: AppState) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        if let Err(e) = photo_du_jour(&st).await {
            tracing::warn!("statistiques du jour : {e:?}")
        }
        if let Err(e) = send_digest(&st).await {
            tracing::warn!("résumé de la liste d’attente : {e:?}")
        }
    }
}

#[derive(Deserialize)]
pub struct TokQ {
    token: String,
}

/// Vérifie un lien d'accès (page d'inscription).
pub async fn check_access(State(st): State<AppState>, Query(q): Query<TokQ>) -> ApiResult<Value> {
    let r: Option<(String, String)> = sqlx::query_as("SELECT email, name FROM waitlist WHERE token_hash = ? AND status = 'approved' AND decided_at > datetime('now', '-30 days')")
        .bind(sha256_hex(&q.token)).fetch_optional(&st.db).await?;
    let (email, name) =
        r.ok_or_else(|| bad("Lien d’accès invalide, déjà utilisé ou expiré (30 jours)."))?;
    Ok(Json(json!({ "email": email, "name": name })))
}

/// Autorise une inscription : premier compte, invitation d'un studio, lien d'accès validé, ou inscriptions ouvertes.
pub async fn signup_allowed(
    st: &AppState,
    email: &str,
    ip: &str,
    invite: Option<&str>,
    access: Option<&str>,
) -> Result<Option<i64>, AppError> {
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&st.db)
        .await?;
    if std::env::var("POP_OPEN_SIGNUP")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return Ok(None);
    }
    // Une installation vide accepte son premier compte depuis l'ordinateur du serveur (sans droit d'administration :
    // voir `pop-server admin`), ou pour une adresse listée dans POP_ADMIN_EMAILS.
    let env_admin = std::env::var("POP_ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .any(|e| !e.trim().is_empty() && e.trim().eq_ignore_ascii_case(email));
    if env_admin {
        // Une adresse admin qui arrive avec un lien d'accès validé : la ligne de liste d'attente passe « inscrit » (#39).
        if let Some(t) = access.filter(|t| !t.is_empty()) {
            let w: Option<(i64, String)> = sqlx::query_as("SELECT id, email FROM waitlist WHERE token_hash = ? AND status = 'approved' AND decided_at > datetime('now', '-30 days')").bind(sha256_hex(t)).fetch_optional(&st.db).await?;
            if let Some((wid, _)) = w.filter(|w| w.1.eq_ignore_ascii_case(email)) {
                return Ok(Some(wid));
            }
        }
        return Ok(None);
    }
    if users == 0 && crate::security::is_loopback(ip) {
        return Ok(None);
    }
    if let Some(t) = invite.filter(|t| !t.is_empty()) {
        let ok: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invitations WHERE token_hash = ? AND accepted_at IS NULL AND created_at > datetime('now', '-7 days')").bind(sha256_hex(t)).fetch_one(&st.db).await?;
        if ok > 0 {
            return Ok(None);
        }
        return Err(bad("Invitation invalide ou expirée."));
    }
    if let Some(t) = access.filter(|t| !t.is_empty()) {
        let r: Option<(i64, String)> = sqlx::query_as("SELECT id, email FROM waitlist WHERE token_hash = ? AND status = 'approved' AND decided_at > datetime('now', '-30 days')").bind(sha256_hex(t)).fetch_optional(&st.db).await?;
        let (wid, wmail) =
            r.ok_or_else(|| bad("Lien d’accès invalide, déjà utilisé ou expiré."))?;
        if !wmail.eq_ignore_ascii_case(email) {
            return Err(bad(
                "Ce lien d’accès est réservé à une autre adresse e-mail.",
            ));
        }
        return Ok(Some(wid));
    }
    Err(AppError::Forbidden)
}

pub async fn mark_registered(st: &AppState, wid: i64, uid: i64) {
    let _ = sqlx::query("UPDATE waitlist SET status = 'registered', token_hash = NULL, registered_user = ? WHERE id = ?").bind(uid).bind(wid).execute(&st.db).await;
}

// ---------- Administration ----------

#[derive(Deserialize, Default)]
pub struct OverviewQ {
    days: Option<i64>,
}

pub async fn overview(
    State(st): State<AppState>,
    u: AuthUser,
    Query(q): Query<OverviewQ>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    let jours = if q.days == Some(90) { 90 } else { 30 };
    photo_du_jour(&st).await?;
    let stats = courbes(&st, jours).await?;
    let one = |q: &'static str| {
        let st = st.clone();
        async move {
            sqlx::query_scalar::<_, i64>(q)
                .fetch_one(&st.db)
                .await
                .unwrap_or(0)
        }
    };
    let users = one("SELECT COUNT(*) FROM users").await;
    let users_7d =
        one("SELECT COUNT(*) FROM users WHERE created_at > datetime('now', '-7 days')").await;
    let active_7d =
        one("SELECT COUNT(*) FROM users WHERE last_seen > datetime('now', '-7 days')").await;
    let studios = one("SELECT COUNT(*) FROM studios").await;
    let maps = one("SELECT COUNT(*) FROM maps").await;
    let tasks = one("SELECT COUNT(*) FROM tasks").await;
    let tasks_done_7d =
        one("SELECT COUNT(*) FROM tasks WHERE done_at > datetime('now', '-7 days')").await;
    let pending = one("SELECT COUNT(*) FROM waitlist WHERE status = 'pending'").await;
    let approved = one("SELECT COUNT(*) FROM waitlist WHERE status = 'approved'").await;
    let registered = one("SELECT COUNT(*) FROM waitlist WHERE status = 'registered'").await;
    let tokens =
        one("SELECT COUNT(*) FROM api_tokens WHERE last_used > datetime('now', '-7 days')").await;
    let linked_maps: Vec<i64> = sqlx::query_scalar("SELECT id FROM maps")
        .fetch_all(&st.db)
        .await
        .unwrap_or_default();
    let relays = linked_maps
        .iter()
        .filter(|m| st.relays.status(**m) != "offline")
        .count();
    let signups: Vec<(String, i64)> = sqlx::query_as("SELECT date(created_at), COUNT(*) FROM waitlist WHERE created_at > datetime('now', '-30 days') GROUP BY 1 ORDER BY 1").fetch_all(&st.db).await?;
    Ok(Json(json!({
        "users": users, "users_7d": users_7d, "active_7d": active_7d, "studios": studios, "maps": maps, "tasks": tasks, "tasks_done_7d": tasks_done_7d,
        "waitlist": { "pending": pending, "approved": approved, "registered": registered }, "ai_tokens_active": tokens, "relays_online": relays,
        "mail": crate::mail::configured(), "open_signup": std::env::var("POP_OPEN_SIGNUP").map(|v| v == "1").unwrap_or(false),
        "requests_per_day": signups.into_iter().map(|(d, n)| json!({ "day": d, "count": n })).collect::<Vec<_>>(),
        "stats": stats,
    })))
}

#[derive(Deserialize)]
pub struct ListQ {
    status: Option<String>,
}

pub async fn waitlist(
    State(st): State<AppState>,
    u: AuthUser,
    Query(q): Query<ListQ>,
) -> ApiResult<Value> {
    require_mod(&st, &u).await?;
    let status = q.status.unwrap_or_else(|| "pending".into());
    let rows: Vec<(i64, String, String, String, String, String, String, String, String, String, String, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT w.id, w.email, w.name, w.studio, w.team, w.experience, w.links, w.message, w.status, w.note, w.created_at, w.decided_at, d.name,
                EXISTS (SELECT 1 FROM account_activations a JOIN users au ON au.id = a.user_id WHERE a.waitlist_id = w.id AND au.activation_pending = 1)
         FROM waitlist w LEFT JOIN users d ON d.id = w.decided_by WHERE (? = 'all' OR w.status = ?) ORDER BY w.created_at DESC LIMIT 500",
    ).bind(&status).bind(&status).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|r| json!({
        "id": r.0, "email": r.1, "name": r.2, "studio": r.3, "team": r.4, "experience": r.5, "links": r.6, "message": r.7, "status": r.8, "note": r.9,
        "created_at": r.10, "decided_at": r.11, "decided_by": r.12, "activation_pending": r.13 == 1,
    })).collect::<Vec<_>>())))
}

async fn grant(st: &AppState, admin: &AuthUser, wid: i64) -> Result<Value, AppError> {
    let (email, name): (String, String) =
        sqlx::query_as("SELECT email, name FROM waitlist WHERE id = ?")
            .bind(wid)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    log(st, admin, "admin_accès_accordé", &email).await;
    let token = random_token();
    sqlx::query("UPDATE waitlist SET status = 'approved', token_hash = ?, decided_at = datetime('now'), decided_by = ? WHERE id = ?").bind(sha256_hex(&token)).bind(admin.id).bind(wid).execute(&st.db).await?;
    let link = format!("{}/inscription?acces={token}", crate::mail::public_url());
    let mut mailed = false;
    if crate::mail::configured() {
        let first = name.split_whitespace().next().unwrap_or(&name);
        let body = format!("Bonjour {first},\n\nBonne nouvelle : votre accès à la bêta de POP est validé.\nCréez votre compte avec ce lien (valable 30 jours) :\n\n{link}\n\nÀ très vite,\nL’équipe POP");
        mailed = crate::mail::send(&email, "Votre accès à la bêta de POP", &body)
            .await
            .is_ok();
    }
    Ok(json!({ "link": link, "mailed": mailed, "email": email }))
}

pub async fn approve(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require_mod(&st, &u).await?;
    Ok(Json(grant(&st, &u, id).await?))
}

pub async fn refuse(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require_mod(&st, &u).await?;
    log(&st, &u, "admin_demande_refusée", &demande(&st, id).await).await;
    sqlx::query("UPDATE waitlist SET status = 'refused', token_hash = NULL, decided_at = datetime('now'), decided_by = ? WHERE id = ? AND status != 'registered'").bind(u.id).bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn reopen(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require_mod(&st, &u).await?;
    log(
        &st,
        &u,
        "admin_demande_remise_en_attente",
        &demande(&st, id).await,
    )
    .await;
    sqlx::query("UPDATE waitlist SET status = 'pending', token_hash = NULL, decided_at = NULL, decided_by = NULL WHERE id = ? AND status != 'registered'").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    log(&st, &u, "admin_demande_supprimée", &demande(&st, id).await).await;
    sqlx::query("DELETE FROM waitlist WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct NoteIn {
    note: String,
}
pub async fn note(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<NoteIn>,
) -> ApiResult<Value> {
    require_mod(&st, &u).await?;
    log(&st, &u, "admin_demande_note", &demande(&st, id).await).await;
    sqlx::query("UPDATE waitlist SET note = ? WHERE id = ?")
        .bind(b.note.chars().take(1000).collect::<String>())
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct InviteIn {
    email: String,
    name: String,
}
/// Donner un accès directement, sans demande préalable.
pub async fn invite(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<InviteIn>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    let email = b.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(bad("Adresse e-mail invalide."));
    }
    let has: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&st.db)
        .await?;
    if has > 0 {
        return Err(AppError::Conflict(
            "Cette personne a déjà un compte.".into(),
        ));
    }
    let name = if b.name.trim().is_empty() {
        email.split('@').next().unwrap_or("").to_string()
    } else {
        b.name.trim().chars().take(80).collect()
    };
    let id: i64 = sqlx::query_scalar("INSERT INTO waitlist (email, name, message) VALUES (?, ?, 'Invité par un administrateur') ON CONFLICT(email) DO UPDATE SET name = excluded.name RETURNING id")
        .bind(&email).bind(&name).fetch_one(&st.db).await?;
    Ok(Json(grant(&st, &u, id).await?))
}

#[derive(Deserialize)]
pub struct JournalQ {
    who: Option<i64>,
    #[serde(rename = "type")]
    kind: Option<String>,
    page: Option<i64>,
}

/// Types d'actions du journal (#66) : préfixes des actions enregistrées par `security::audit`.
const JOURNAL_TYPES: &[(&str, &str, &[&str])] = &[
    (
        "attente",
        "Liste d’attente",
        &["admin\\_demande\\_%", "admin\\_accès\\_%"],
    ),
    (
        "comptes",
        "Comptes",
        &[
            "admin\\_compte\\_%",
            "admin\\_lien\\_%",
            "admin\\_mot\\_de\\_passe\\_%",
            "admin\\_plan\\_%",
            "admin\\_crédits\\_%",
            "admin\\_nommé",
            "admin\\_retiré",
        ],
    ),
    ("annonces", "Annonces", &["annonce\\_%"]),
    (
        "securite",
        "Accès admin",
        &["admin\\_déverrouillé", "admin\\_confirmation\\_%"],
    ),
];

/// Journal des actions des admins et modérateurs (#66) : le plus récent en haut, 50 par page, filtres par auteur et par
/// type. Réservé aux administrateurs.
pub async fn journal(
    State(st): State<AppState>,
    u: AuthUser,
    Query(q): Query<JournalQ>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    let motifs: Vec<&str> = match q.kind.as_deref().filter(|k| !k.is_empty()) {
        Some(k) => JOURNAL_TYPES
            .iter()
            .find(|t| t.0 == k)
            .map(|t| t.2.to_vec())
            .ok_or_else(|| bad("Type d’action inconnu."))?,
        None => JOURNAL_TYPES
            .iter()
            .flat_map(|t| t.2.iter().copied())
            .collect(),
    };
    let cond = (0..motifs.len())
        .map(|i| format!("a.action LIKE ?{} ESCAPE '\\'", i + 3))
        .collect::<Vec<_>>()
        .join(" OR ");
    let page = q.page.unwrap_or(1).max(1);
    let sql = format!(
        "SELECT a.id, a.action, a.detail, a.at, a.user_id, u.name, u.email FROM audit_log a LEFT JOIN users u ON u.id = a.user_id
         WHERE ({cond}) AND (?1 IS NULL OR a.user_id = ?1) ORDER BY a.id DESC LIMIT 51 OFFSET ?2"
    );
    let mut req = sqlx::query_as::<
        _,
        (
            i64,
            String,
            String,
            String,
            Option<i64>,
            Option<String>,
            Option<String>,
        ),
    >(sqlx::AssertSqlSafe(sql.as_str()))
    .bind(q.who)
    .bind((page - 1) * 50);
    for m in &motifs {
        req = req.bind(*m);
    }
    let mut rows = req.fetch_all(&st.db).await?;
    let suite = rows.len() > 50;
    rows.truncate(50);
    let libelle = |a: &str| {
        match a {
            "admin_déverrouillé" => return "Administration déverrouillée".to_string(),
            "admin_confirmation_échouée" => {
                return "Mot de passe d’administration refusé".to_string()
            }
            "admin_nommé" => return "Administrateur nommé".to_string(),
            "admin_retiré" => return "Administrateur retiré".to_string(),
            "admin_crédits_ia" => return "Crédits IA donnés".to_string(),
            _ => {}
        }
        let s = a.trim_start_matches("admin_").replace('_', " ");
        let mut c = s.chars();
        c.next()
            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    };
    let type_de = |a: &str| {
        JOURNAL_TYPES
            .iter()
            .find(|t| {
                t.2.iter().any(|m| {
                    let m = m.replace('\\', "");
                    match m.strip_suffix('%') {
                        Some(p) => a.starts_with(p),
                        None => a == m,
                    }
                })
            })
            .map(|t| t.0)
            .unwrap_or("")
    };
    let auteurs: Vec<(i64, String)> = sqlx::query_as(
        "SELECT DISTINCT u.id, u.name FROM audit_log a JOIN users u ON u.id = a.user_id WHERE a.action LIKE 'admin\\_%' ESCAPE '\\' OR a.action LIKE 'annonce\\_%' ESCAPE '\\' ORDER BY u.name",
    )
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "rows": rows.into_iter().map(|r| json!({ "id": r.0, "action": r.1, "label": libelle(&r.1), "type": type_de(&r.1), "target": r.2, "at": r.3, "who_id": r.4, "who": r.5, "who_email": r.6 })).collect::<Vec<_>>(),
        "page": page, "more": suite,
        "authors": auteurs.into_iter().map(|(id, n)| json!({ "id": id, "name": n })).collect::<Vec<_>>(),
        "types": JOURNAL_TYPES.iter().map(|t| json!({ "key": t.0, "label": t.1 })).collect::<Vec<_>>(),
    })))
}

pub async fn users(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    require_mod(&st, &u).await?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(i64, String, String, String, Option<String>, i64, i64, i64, i64, i64, Option<String>, i64, String, Option<String>, i64, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.email, u.name, u.created_at, u.last_seen, u.is_admin, u.disabled, u.is_moderator,
                (SELECT COUNT(*) FROM memberships m WHERE m.user_id = u.id),
                (SELECT COUNT(*) FROM tasks t WHERE t.assignee_id = u.id AND t.status = 'done'), u.username, u.must_change_password, u.plan,
                CASE WHEN u.locked_until > datetime('now') THEN u.locked_until END,
                u.activation_pending, (SELECT MAX(a.created_at) FROM account_activations a WHERE a.user_id = u.id)
         FROM users u ORDER BY u.created_at DESC LIMIT 2000",
    ).fetch_all(&st.db).await?;
    Ok(Json(json!(rows.into_iter().map(|r| json!({
        "id": r.0, "email": r.1, "name": r.2, "created_at": r.3, "last_seen": r.4, "is_admin": r.5 == 1 || env_admin(&r.1), "env_admin": env_admin(&r.1), "disabled": r.6 == 1, "is_moderator": r.7 == 1,
        "studios": r.8, "tasks_done": r.9, "username": r.10, "must_change": r.11 == 1, "plan": r.12, "locked_until": r.13, "me": r.0 == u.id,
        "activation_pending": r.14 == 1, "activation_sent_at": r.15,
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct UserPatch {
    is_admin: Option<bool>,
    is_moderator: Option<bool>,
    disabled: Option<bool>,
}
pub async fn update_user(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<UserPatch>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    if id == u.id {
        return Err(bad("Vous ne pouvez pas modifier votre propre compte ici."));
    }
    let (email, admin): (String, i64) =
        sqlx::query_as("SELECT email, is_admin FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    if b.is_admin == Some(false) && env_admin(&email) {
        return Err(bad("Administrateur défini sur le serveur (POP_ADMIN_EMAILS) : ce droit ne se retire pas ici."));
    }
    let perd = (b.is_admin == Some(false) && admin == 1)
        || (b.disabled == Some(true) && (admin == 1 || env_admin(&email)));
    if perd && autres_admins(&st, id).await? == 0 {
        return Err(bad("C'est le dernier administrateur actif : nommez-en un autre avant de lui retirer ce droit ou de le désactiver."));
    }
    log(
        &st,
        &u,
        "admin_compte_modifié",
        &format!(
            "utilisateur {id} : admin={:?} modérateur={:?} désactivé={:?}",
            b.is_admin, b.is_moderator, b.disabled
        ),
    )
    .await;
    if let Some(a) = b.is_admin {
        sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
            .bind(a as i64)
            .bind(id)
            .execute(&st.db)
            .await?;
    }
    if let Some(m) = b.is_moderator {
        sqlx::query("UPDATE users SET is_moderator = ? WHERE id = ?")
            .bind(m as i64)
            .bind(id)
            .execute(&st.db)
            .await?;
    }
    if let Some(d) = b.disabled {
        sqlx::query("UPDATE users SET disabled = ? WHERE id = ?")
            .bind(d as i64)
            .bind(id)
            .execute(&st.db)
            .await?;
        if d {
            // Un compte désactivé perd ses sessions et ses jetons IA (#13).
            sqlx::query("DELETE FROM sessions WHERE user_id = ?")
                .bind(id)
                .execute(&st.db)
                .await?;
            sqlx::query("DELETE FROM api_tokens WHERE user_id = ?")
                .bind(id)
                .execute(&st.db)
                .await?;
        }
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PlanIn {
    plan: String,
}
/// Change l'abonnement d'un compte (bêta : pas encore de paiement).
pub async fn set_plan(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<PlanIn>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    if !crate::billing::PLANS.contains(&b.plan.as_str()) {
        return Err(bad("Plan inconnu."));
    }
    let n = sqlx::query("UPDATE users SET plan = ? WHERE id = ?")
        .bind(&b.plan)
        .bind(id)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    log(
        &st,
        &u,
        "admin_plan_modifié",
        &format!("utilisateur {id} : {}", b.plan),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

// ---------- Comptes créés par l'administrateur (bêta) ----------

/// Mot de passe provisoire lisible, sans caractères ambigus (ex. « k7mq-x2pa-9hrt »).
fn temp_password() -> String {
    use rand::Rng;
    const A: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut r = rand::rngs::OsRng;
    (0..3)
        .map(|_| {
            (0..4)
                .map(|_| A[r.gen_range(0..A.len())] as char)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Identifiant : 3 à 32 caractères, lettres sans accent, chiffres, point, tiret, tiret bas.
fn check_username(raw: &str) -> Result<String, AppError> {
    let u = raw.trim().to_lowercase();
    let n = u.chars().count();
    if !(3..=32).contains(&n) {
        return Err(bad("L’identifiant doit faire entre 3 et 32 caractères."));
    }
    if !u
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
    {
        return Err(bad("L’identifiant ne peut contenir que des lettres sans accent, des chiffres, des points et des tirets."));
    }
    Ok(u)
}

#[derive(Deserialize)]
pub struct AccountIn {
    username: String,
    name: String,
    email: String,
    waitlist_id: Option<i64>,
    plan: Option<String>,
}

/// Crée le compte d'un bêta-testeur, sans mot de passe (#42) : la personne reçoit un lien d'activation par e-mail
/// et choisit elle-même son mot de passe. Sans SMTP, l'admin reçoit le lien à copier.
pub async fn create_account(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<AccountIn>,
) -> ApiResult<Value> {
    let role = require_mod(&st, &u).await?;
    let username = check_username(&b.username)?;
    let email = b.email.trim().to_lowercase();
    // Un modérateur crée un compte seulement en acceptant une demande de la liste d'attente, pour l'adresse de
    // cette demande, et sans choisir le plan (#65) ; la création libre reste aux administrateurs.
    if role < Staff::Admin {
        let asked: Option<String> = match b.waitlist_id {
            Some(w) => {
                sqlx::query_scalar(
                    "SELECT email FROM waitlist WHERE id = ? AND status != 'registered'",
                )
                .bind(w)
                .fetch_optional(&st.db)
                .await?
            }
            None => None,
        };
        if asked.map(|a| a.trim().to_lowercase()) != Some(email.clone()) {
            return Err(AppError::Forbidden);
        }
    }
    if !email.contains('@') || email.len() > 200 {
        return Err(bad("Adresse e-mail invalide."));
    }
    let name: String = if b.name.trim().is_empty() {
        username.clone()
    } else {
        b.name.trim().chars().take(80).collect()
    };
    let taken: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username = ?1 OR email = ?1")
            .bind(&username)
            .fetch_one(&st.db)
            .await?;
    if taken > 0 {
        return Err(AppError::Conflict("Cet identifiant est déjà pris.".into()));
    }
    let has: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&st.db)
        .await?;
    if has > 0 {
        return Err(AppError::Conflict(
            "Un compte existe déjà avec cette adresse e-mail.".into(),
        ));
    }
    let plan = if role < Staff::Admin {
        "pro"
    } else {
        b.plan.as_deref().unwrap_or("pro")
    };
    if !crate::billing::PLANS.contains(&plan) {
        return Err(bad("Plan inconnu."));
    }
    // Mot de passe inconnu de tous (aléatoire) : le compte ne s'ouvre qu'avec le lien d'activation.
    let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, username, must_change_password, activation_pending, plan) VALUES (?, ?, ?, ?, 0, 1, ?) RETURNING id")
        .bind(&email).bind(&name).bind(crate::auth::hash_password(&crate::auth::random_token())?).bind(&username).bind(plan).fetch_one(&st.db).await?;
    // Relie la demande de la liste d'attente (celle choisie, sinon celle de la même adresse).
    let wid: Option<i64> = match b.waitlist_id {
        Some(w) => Some(w),
        None => {
            sqlx::query_scalar("SELECT id FROM waitlist WHERE email = ?")
                .bind(&email)
                .fetch_optional(&st.db)
                .await?
        }
    };
    if let Some(w) = wid {
        // Demande acceptée ; elle passe à « inscrit » à l'activation, pas à la création.
        sqlx::query("UPDATE waitlist SET status = 'approved', token_hash = NULL, decided_at = COALESCE(decided_at, datetime('now')), decided_by = COALESCE(decided_by, ?) WHERE id = ?").bind(u.id).bind(w).execute(&st.db).await?;
    }
    let sent = crate::activation::issue(&st, id, wid).await?;
    log(
        &st,
        &u,
        "admin_compte_créé",
        &format!("{username} ({email})"),
    )
    .await;
    Ok(Json(
        json!({ "id": id, "username": username, "name": name, "email": email, "mailed": sent["mailed"], "link": sent.get("link") }),
    ))
}

/// Nouveau mot de passe provisoire pour un utilisateur (ses sessions sont fermées).
pub async fn reset_password(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require(&st, &u).await?;
    if id == u.id {
        return Err(bad("Changez votre propre mot de passe dans Paramètres."));
    }
    let (email, username): (String, Option<String>) =
        sqlx::query_as("SELECT email, username FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    let password = temp_password();
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 1, failed_logins = 0, locked_until = NULL WHERE id = ?").bind(crate::auth::hash_password(&password)?).bind(id).execute(&st.db).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    log(
        &st,
        &u,
        "admin_mot_de_passe_réinitialisé",
        &username.clone().unwrap_or_else(|| email.clone()),
    )
    .await;
    Ok(Json(
        json!({ "username": username.unwrap_or(email), "password": password, "url": crate::mail::public_url() + "/connexion" }),
    ))
}

// ---------- Dépannage d'un compte (#38) ----------

/// Compte visé par une action de dépannage : verrou admin, jamais son propre compte. Renvoie (e-mail, nom).
/// Compte visé par une action de dépannage. `min` = le rôle demandé pour cette action. Un modérateur n'agit que
/// sur les comptes ordinaires : jamais sur un administrateur ou un autre modérateur (sinon un lien de mot de passe
/// lui donnerait leurs droits).
async fn target(
    st: &AppState,
    u: &AuthUser,
    id: i64,
    min: Staff,
) -> Result<(String, String), AppError> {
    let me = crate::security::require_staff(st, u, min).await?;
    if me < Staff::Admin && staff(st, id).await? > Staff::None {
        return Err(AppError::Forbidden);
    }
    if id == u.id {
        return Err(bad(
            "Vous ne pouvez pas faire cette action sur votre propre compte.",
        ));
    }
    sqlx::query_as("SELECT email, name FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)
}

/// Lien de nouveau mot de passe pour n'importe quel compte (2 heures, une seule fois) : envoyé par e-mail si le
/// SMTP est configuré, toujours renvoyé pour être copié.
pub async fn user_reset_link(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (email, name) = target(&st, &u, id, Staff::Moderator).await?;
    let link = format!(
        "{}{}",
        crate::mail::public_url(),
        crate::account::new_reset(&st, id).await?
    );
    log(&st, &u, "admin_lien_mot_de_passe", &email).await;
    let mut mailed = false;
    if crate::mail::configured() {
        let first = name.split_whitespace().next().unwrap_or(&name);
        let body = format!("Bonjour {first},\n\nUn administrateur de POP vous envoie un lien pour choisir un nouveau mot de passe (valable 2 heures, une seule fois) :\n\n{link}\n\nSi vous n’avez rien demandé, ignorez ce message.\n\nL’équipe POP");
        mailed = crate::mail::send(&email, "POP · choisir un nouveau mot de passe", &body)
            .await
            .is_ok();
    }
    Ok(Json(
        json!({ "link": link, "mailed": mailed, "email": email }),
    ))
}

/// Déconnecter partout (#39) : supprime les sessions et les jetons IA du compte, sans le désactiver.
pub async fn logout_everywhere(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (email, _) = target(&st, &u, id, Staff::Moderator).await?;
    let s = sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(id)
        .execute(&st.db)
        .await?
        .rows_affected();
    let j = sqlx::query("DELETE FROM api_tokens WHERE user_id = ?")
        .bind(id)
        .execute(&st.db)
        .await?
        .rows_affected();
    log(
        &st,
        &u,
        "admin_deconnecte_partout",
        &format!("{email} : {s} session(s), {j} jeton(s) IA"),
    )
    .await;
    Ok(Json(json!({ "sessions": s, "tokens": j })))
}

/// Fiche d'un compte (#39) : studios et rôle, dernière visite, double authentification, blocage, désactivation,
/// et les 20 derniers événements du journal de sécurité qui le concernent.
pub async fn user_card(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    require_mod(&st, &u).await?;
    let r: Option<(String, String, Option<String>, i64, i64, i64, i64, Option<String>, String, String)> = sqlx::query_as(
        "SELECT email, name, last_seen, totp_enabled, disabled, is_admin, is_moderator,
                CASE WHEN locked_until > datetime('now') THEN locked_until END, created_at, plan FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&st.db)
    .await?;
    let r = r.ok_or(AppError::NotFound)?;
    let studios: Vec<(i64, String, i64, String)> = sqlx::query_as(
        "SELECT s.id, s.name, s.personal, m.role FROM memberships m JOIN studios s ON s.id = m.studio_id WHERE m.user_id = ? ORDER BY s.personal DESC, s.name",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    let sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE user_id = ? AND expires_at > datetime('now')",
    )
    .bind(id)
    .fetch_one(&st.db)
    .await?;
    let evenements: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT action, detail, ip, at FROM audit_log WHERE user_id = ? OR (detail LIKE '%' || ? || '%') ORDER BY id DESC LIMIT 20",
    )
    .bind(id)
    .bind(&r.0)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "id": id, "email": r.0, "name": r.1, "last_seen": r.2, "two_factor": r.3 == 1, "disabled": r.4 == 1,
        "is_admin": r.5 == 1 || env_admin(&r.0), "env_admin": env_admin(&r.0), "is_moderator": r.6 == 1, "locked_until": r.7,
        "created_at": r.8, "plan": r.9, "active_sessions": sessions,
        "studios": studios.into_iter().map(|(sid, n, p, role)| json!({ "id": sid, "name": if p == 1 { "Espace perso".to_string() } else { n }, "role": role })).collect::<Vec<_>>(),
        "events": evenements.into_iter().map(|(a, d, ip, at)| json!({ "action": a, "detail": d, "ip": ip, "at": at })).collect::<Vec<_>>(),
    })))
}

/// Lève le blocage après trop de mots de passe faux.
pub async fn unlock_user(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (email, _) = target(&st, &u, id, Staff::Moderator).await?;
    sqlx::query("UPDATE users SET failed_logins = 0, locked_until = NULL WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    log(&st, &u, "admin_compte_débloqué", &email).await;
    Ok(Json(json!({ "ok": true })))
}

fn plan_json(p: &crate::manage::DeletionPlan) -> Value {
    json!({
        "delete": p.delete.iter().map(|d| d.1.clone()).collect::<Vec<_>>(),
        "transfer": p.transfer.iter().map(|t| json!({ "studio": t.1, "to": t.3 })).collect::<Vec<_>>(),
    })
}

/// Ce que deviendront les studios du compte s'il est supprimé (affiché avant la confirmation).
pub async fn deletion_preview(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let (email, name) = target(&st, &u, id, Staff::Admin).await?;
    let plan = crate::manage::deletion_plan(&st.db, id).await?;
    let mut v = plan_json(&plan);
    v["email"] = json!(email);
    v["name"] = json!(name);
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct DeleteUserIn {
    confirm: String,
}

/// Supprime un compte : l'administrateur retape l'adresse e-mail. Les studios partagés dont il était seul
/// propriétaire sont transférés, les autres supprimés (même logique que la suppression par la personne).
pub async fn delete_user(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<DeleteUserIn>,
) -> ApiResult<Value> {
    let (email, _) = target(&st, &u, id, Staff::Admin).await?;
    if !b.confirm.trim().eq_ignore_ascii_case(&email) {
        return Err(bad("L’adresse tapée ne correspond pas à celle du compte."));
    }
    let plan = crate::manage::deletion_plan(&st.db, id).await?;
    let names = |v: Vec<String>| {
        if v.is_empty() {
            "aucun".to_string()
        } else {
            v.join(", ")
        }
    };
    let detail = format!(
        "{email} · studios supprimés : {} · transférés : {}",
        names(plan.delete.iter().map(|d| d.1.clone()).collect()),
        names(
            plan.transfer
                .iter()
                .map(|t| format!("{} à {}", t.1, t.3))
                .collect()
        ),
    );
    crate::manage::remove_account(&st, id, &plan).await?;
    log(&st, &u, "admin_compte_supprimé", &detail).await;
    Ok(Json(plan_json(&plan)))
}

#[cfg(test)]
mod tests {
    use super::take_digest;

    async fn db() -> sqlx::SqlitePool {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        db
    }
    async fn ask(db: &sqlx::SqlitePool, i: usize, age: &str) {
        sqlx::query(
            "INSERT INTO waitlist (email, name, created_at) VALUES (?, ?, datetime('now', ?))",
        )
        .bind(format!("p{i}@exemple.com"))
        .bind(format!("P{i}"))
        .bind(age)
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn vingt_demandes_un_seul_resume() {
        let db = db().await;
        for i in 0..20 {
            ask(&db, i, &format!("-{} minutes", 59 - i)).await
        }
        assert!(
            take_digest(&db).await.unwrap().is_empty(),
            "la plus ancienne n'attend pas encore depuis une heure"
        );
        sqlx::query("UPDATE waitlist SET created_at = datetime(created_at, '-2 minutes')")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(
            take_digest(&db).await.unwrap().len(),
            20,
            "un résumé couvre les 20 demandes"
        );
        assert!(
            take_digest(&db).await.unwrap().is_empty(),
            "rien n'est signalé deux fois"
        );
        ask(&db, 99, "-1 minutes").await;
        assert!(
            take_digest(&db).await.unwrap().is_empty(),
            "la demande suivante attend le prochain résumé"
        );
    }
}

#[cfg(test)]
mod depannage_tests {
    use super::*;
    use crate::{auth::hash_password, error::AppError};
    use axum::{
        extract::{Path, State},
        http::HeaderMap,
        Json,
    };
    use axum_extra::extract::cookie::CookieJar;

    const PW: &str = "Ancien-secret-du-compte-42";
    const NEW_PW: &str = "Nouveau-secret-tout-neuf-77";

    async fn user(st: &AppState, email: &str, admin: bool) -> AuthUser {
        // Adresse vérifiée : sans elle, un admin n'a pas ses droits (#11).
        let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, is_admin, email_verified_at) VALUES (?, ?, ?, ?, datetime('now')) RETURNING id")
            .bind(email).bind(email.split('@').next().unwrap()).bind(hash_password(PW).unwrap()).bind(admin as i64)
            .fetch_one(&st.db).await.unwrap();
        let sid = format!("session-{id}");
        // Session d'admin déverrouillée (mot de passe confirmé il y a moins de 30 minutes).
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at, sudo_until) VALUES (?, ?, datetime('now', '+1 day'), datetime('now', '+10 minutes'))")
            .bind(&sid).bind(id).execute(&st.db).await.unwrap();
        AuthUser {
            id,
            email: email.into(),
            name: email.into(),
            sid,
        }
    }
    async fn login(st: &AppState, email: &str, pw: &str) -> Result<Value, AppError> {
        let b = serde_json::from_value(json!({ "email": email, "password": pw })).unwrap();
        crate::auth::login(
            State(st.clone()),
            crate::security::ClientIp("203.0.113.9".into()),
            HeaderMap::new(),
            CookieJar::new(),
            Json(b),
        )
        .await
        .map(|(_, Json(v))| v)
    }
    async fn audited(st: &AppState, action: &str, by: i64) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM audit_log WHERE action = ? AND user_id = ?",
        )
        .bind(action)
        .bind(by)
        .fetch_one(&st.db)
        .await
        .unwrap()
            > 0
    }
    /// #39 : déconnecter partout (compte actif, reconnexion possible), fiche du compte, dernier administrateur.
    #[tokio::test]
    async fn fiche_deconnexion_dernier_admin() {
        let st = crate::test_state().await;
        let admin = user(&st, "chef@exemple.com", true).await;
        let x = user(&st, "x@exemple.com", false).await;
        sqlx::query("INSERT INTO api_tokens (user_id, name, token_hash) VALUES (?, 'IA', 'h')")
            .bind(x.id)
            .execute(&st.db)
            .await
            .unwrap();
        let Json(r) = logout_everywhere(State(st.clone()), admin.clone(), Path(x.id))
            .await
            .unwrap();
        assert_eq!(
            (r["sessions"].as_u64(), r["tokens"].as_u64()),
            (Some(1), Some(1))
        );
        let actif: i64 = sqlx::query_scalar("SELECT disabled FROM users WHERE id = ?")
            .bind(x.id)
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(actif, 0, "le compte reste actif");
        assert!(
            login(&st, "x@exemple.com", PW).await.is_ok(),
            "reconnexion possible"
        );
        assert!(audited(&st, "admin_deconnecte_partout", admin.id).await);
        assert!(
            logout_everywhere(State(st.clone()), admin.clone(), Path(admin.id))
                .await
                .is_err(),
            "pas sur soi-même"
        );

        let Json(f) = user_card(State(st.clone()), admin.clone(), Path(x.id))
            .await
            .unwrap();
        assert_eq!(f["two_factor"], false);
        assert_eq!(f["disabled"], false);
        assert!(
            f["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["action"] == "admin_deconnecte_partout"),
            "{f}"
        );

        // Dernier administrateur actif : un admin désactivé ou à l'adresse non vérifiée ne compte pas.
        assert_eq!(
            autres_admins(&st, admin.id).await.unwrap(),
            0,
            "chef est le seul admin"
        );
        let second = user(&st, "second@exemple.com", true).await;
        assert_eq!(autres_admins(&st, admin.id).await.unwrap(), 1);
        sqlx::query("UPDATE users SET disabled = 1 WHERE id = ?")
            .bind(second.id)
            .execute(&st.db)
            .await
            .unwrap();
        assert_eq!(
            autres_admins(&st, admin.id).await.unwrap(),
            0,
            "un admin désactivé ne compte pas"
        );
        // Désactiver ou retirer un admin quand il en reste un autre : accepté.
        sqlx::query("UPDATE users SET disabled = 0 WHERE id = ?")
            .bind(second.id)
            .execute(&st.db)
            .await
            .unwrap();
        let patch = |a: Option<bool>, d: Option<bool>| {
            Json(UserPatch {
                is_admin: a,
                is_moderator: None,
                disabled: d,
            })
        };
        update_user(
            State(st.clone()),
            admin.clone(),
            Path(second.id),
            patch(Some(false), None),
        )
        .await
        .unwrap();
    }

    /// #39 : un admin du serveur (POP_ADMIN_EMAILS) apparaît admin, son droit ne se retire pas, et son inscription
    /// avec un lien d'accès passe la ligne de liste d'attente à « inscrit ».
    #[tokio::test]
    async fn admin_du_serveur() {
        assert!(!env_admin("serveur@exemple.com") || std::env::var("POP_ADMIN_EMAILS").is_ok());
        let st = crate::test_state().await;
        // Variable d'environnement propre à ce test (adresse unique, sans effet sur les autres tests).
        let adr = "serveur-39@exemple.com";
        let avant = std::env::var("POP_ADMIN_EMAILS").unwrap_or_default();
        std::env::set_var("POP_ADMIN_EMAILS", format!("{avant},{adr}"));
        let chef = user(&st, "chef39@exemple.com", true).await;
        let srv = user(&st, adr, false).await;
        let Json(l) = users(State(st.clone()), chef.clone()).await.unwrap();
        let ligne = l
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == srv.id)
            .unwrap()
            .clone();
        assert_eq!(
            (ligne["is_admin"].clone(), ligne["env_admin"].clone()),
            (json!(true), json!(true))
        );
        let e = update_user(
            State(st.clone()),
            chef.clone(),
            Path(srv.id),
            Json(UserPatch {
                is_admin: Some(false),
                is_moderator: None,
                disabled: None,
            }),
        )
        .await;
        assert!(e.is_err());
        // Lien d'accès validé pour une autre adresse admin du serveur.
        let adr2 = "serveur-39b@exemple.com";
        std::env::set_var("POP_ADMIN_EMAILS", format!("{avant},{adr},{adr2}"));
        sqlx::query("INSERT INTO waitlist (email, name, status, token_hash, decided_at) VALUES (?, 'S', 'approved', ?, datetime('now'))")
            .bind(adr2).bind(sha256_hex("jeton-39")).execute(&st.db).await.unwrap();
        let w = signup_allowed(&st, adr2, "203.0.113.1", None, Some("jeton-39"))
            .await
            .unwrap();
        assert!(w.is_some(), "le lien est lu pour l'adresse admin");
    }

    /// Les quatre actions de dépannage, sur le compte `id`, par `u`.
    async fn all(st: &AppState, u: &AuthUser, id: i64) -> Vec<Result<Value, AppError>> {
        let s = || State(st.clone());
        vec![
            user_reset_link(s(), u.clone(), Path(id)).await.map(|j| j.0),
            unlock_user(s(), u.clone(), Path(id)).await.map(|j| j.0),
            deletion_preview(s(), u.clone(), Path(id))
                .await
                .map(|j| j.0),
            delete_user(
                s(),
                u.clone(),
                Path(id),
                Json(DeleteUserIn {
                    confirm: "x@exemple.com".into(),
                }),
            )
            .await
            .map(|j| j.0),
        ]
    }

    /// #17 : une adresse qui a déjà un compte reçoit la même réponse qu'une adresse inconnue, et rien n'est enregistré.
    #[tokio::test]
    async fn demande_acces_ne_revele_pas_les_comptes() {
        let st = crate::test_state().await;
        user(&st, "deja@exemple.com", false).await;
        let ask = |email: &str| WaitIn {
            email: email.into(),
            name: "Test".into(),
            studio: None,
            team: None,
            experience: None,
            links: None,
            message: None,
            pop_hp: String::new(),
        };
        let ip = |n: u8| crate::security::ClientIp(format!("10.0.0.{n}"));
        let a = join(State(st.clone()), ip(1), Json(ask("deja@exemple.com")))
            .await
            .unwrap()
            .0;
        let b = join(State(st.clone()), ip(2), Json(ask("nouveau@exemple.com")))
            .await
            .unwrap()
            .0;
        assert_eq!(a, b);
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM waitlist WHERE email = 'deja@exemple.com'")
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn droits_et_propre_compte() {
        let st = crate::test_state().await;
        let admin = user(&st, "admin@exemple.com", true).await;
        let member = user(&st, "x@exemple.com", false).await;
        for r in all(&st, &member, admin.id).await {
            assert!(
                matches!(r, Err(AppError::Forbidden)),
                "un non-admin est refusé : {r:?}"
            )
        }
        // Admin sans le verrou (confirmation récente expirée).
        sqlx::query("UPDATE sessions SET sudo_until = NULL WHERE token_hash = ?")
            .bind(&admin.sid)
            .execute(&st.db)
            .await
            .unwrap();
        for r in all(&st, &admin, member.id).await {
            assert!(
                matches!(r, Err(AppError::Guard("sudo", _))),
                "le verrou admin est exigé : {r:?}"
            )
        }
        sqlx::query(
            "UPDATE sessions SET sudo_until = datetime('now', '+10 minutes') WHERE token_hash = ?",
        )
        .bind(&admin.sid)
        .execute(&st.db)
        .await
        .unwrap();
        for r in all(&st, &admin, admin.id).await {
            assert!(
                matches!(r, Err(AppError::BadRequest(_))),
                "son propre compte est refusé : {r:?}"
            )
        }
        assert!(matches!(
            unlock_user(State(st.clone()), admin.clone(), Path(9999)).await,
            Err(AppError::NotFound)
        ));
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(n, 2, "rien n'a été supprimé");
    }

    #[tokio::test]
    async fn lien_de_nouveau_mot_de_passe_hors_studio_et_blocage_leve() {
        let st = crate::test_state().await;
        let admin = user(&st, "admin@exemple.com", true).await;
        let m = user(&st, "membre@exemple.com", false).await;
        // Le membre est dans un studio dont l'admin ne fait pas partie, et il est bloqué.
        sqlx::query("INSERT INTO studios (id, name) VALUES (1, 'Studio du membre')")
            .execute(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (1, ?, 'owner')")
            .bind(m.id)
            .execute(&st.db)
            .await
            .unwrap();
        sqlx::query("UPDATE users SET locked_until = datetime('now', '+15 minutes') WHERE id = ?")
            .bind(m.id)
            .execute(&st.db)
            .await
            .unwrap();
        assert!(
            matches!(
                login(&st, "membre@exemple.com", PW).await,
                Err(AppError::Conflict(_))
            ),
            "compte bloqué"
        );
        let r = user_reset_link(State(st.clone()), admin.clone(), Path(m.id))
            .await
            .unwrap()
            .0;
        let link = r["link"].as_str().unwrap();
        assert!(link.contains("/reinitialiser/"));
        assert!(audited(&st, "admin_lien_mot_de_passe", admin.id).await);
        let token = link.rsplit('/').next().unwrap();
        let b = serde_json::from_value(json!({ "token": token, "password": NEW_PW })).unwrap();
        let _ = crate::account::reset(State(st.clone()), Json(b))
            .await
            .unwrap();
        // Le blocage est levé : connexion immédiate avec le nouveau mot de passe.
        login(&st, "membre@exemple.com", NEW_PW).await.unwrap();
        // Le lien ne sert qu'une fois.
        let b = serde_json::from_value(
            json!({ "token": token, "password": "Encore-un-autre-secret-88" }),
        )
        .unwrap();
        assert!(crate::account::reset(State(st.clone()), Json(b))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn debloquer_apres_cinq_essais() {
        let st = crate::test_state().await;
        let admin = user(&st, "admin@exemple.com", true).await;
        let m = user(&st, "membre@exemple.com", false).await;
        for _ in 0..5 {
            let _ = login(&st, "membre@exemple.com", "Mauvais-secret-000").await;
        }
        assert!(
            matches!(
                login(&st, "membre@exemple.com", PW).await,
                Err(AppError::Conflict(_))
            ),
            "bloqué après 5 essais"
        );
        let list = users(State(st.clone()), admin.clone()).await.unwrap().0;
        let row = list
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == m.id)
            .unwrap()
            .clone();
        assert!(
            row["locked_until"].is_string(),
            "la liste affiche le blocage"
        );
        let _ = unlock_user(State(st.clone()), admin.clone(), Path(m.id))
            .await
            .unwrap();
        assert!(audited(&st, "admin_compte_débloqué", admin.id).await);
        login(&st, "membre@exemple.com", PW).await.unwrap();
        let list = users(State(st.clone()), admin.clone()).await.unwrap().0;
        assert!(list
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == m.id)
            .unwrap()["locked_until"]
            .is_null());
    }

    #[tokio::test]
    async fn supprimer_un_compte_transfere_ou_supprime_ses_studios() {
        let st = crate::test_state().await;
        let admin = user(&st, "admin@exemple.com", true).await;
        let m = user(&st, "membre@exemple.com", false).await;
        let other = user(&st, "autre@exemple.com", false).await;
        sqlx::query("INSERT INTO studios (id, name) VALUES (1, 'Seul'), (2, 'Partagé')")
            .execute(&st.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (1, ?, 'owner'), (2, ?, 'owner'), (2, ?, 'member')")
            .bind(m.id).bind(m.id).bind(other.id).execute(&st.db).await.unwrap();
        let p = deletion_preview(State(st.clone()), admin.clone(), Path(m.id))
            .await
            .unwrap()
            .0;
        assert_eq!(p["delete"], json!(["Seul"]));
        assert_eq!(
            p["transfer"],
            json!([{ "studio": "Partagé", "to": "autre" }])
        );
        let wrong = delete_user(
            State(st.clone()),
            admin.clone(),
            Path(m.id),
            Json(DeleteUserIn {
                confirm: "autre@exemple.com".into(),
            }),
        )
        .await;
        assert!(
            matches!(wrong, Err(AppError::BadRequest(_))),
            "adresse à retaper"
        );
        let _ = delete_user(
            State(st.clone()),
            admin.clone(),
            Path(m.id),
            Json(DeleteUserIn {
                confirm: " Membre@Exemple.com ".into(),
            }),
        )
        .await
        .unwrap();
        assert!(
            matches!(
                login(&st, "membre@exemple.com", PW).await,
                Err(AppError::BadRequest(_))
            ),
            "plus de connexion possible"
        );
        let studios: Vec<String> = sqlx::query_scalar("SELECT name FROM studios ORDER BY id")
            .fetch_all(&st.db)
            .await
            .unwrap();
        assert_eq!(studios, vec!["Partagé".to_string()]);
        let role: String =
            sqlx::query_scalar("SELECT role FROM memberships WHERE studio_id = 2 AND user_id = ?")
                .bind(other.id)
                .fetch_one(&st.db)
                .await
                .unwrap();
        assert_eq!(role, "owner");
        assert!(audited(&st, "admin_compte_supprimé", admin.id).await);
    }

    /// Rôle modérateur (#65) : liste d'attente, dépannage des comptes ordinaires, consultation ; rien d'autre.
    #[tokio::test]
    async fn moderateur() {
        let st = crate::test_state().await;
        let s = || State(st.clone());
        let admin = user(&st, "admin@exemple.com", true).await;
        let moder = user(&st, "modo@exemple.com", false).await;
        let other = user(&st, "modo2@exemple.com", false).await;
        let member = user(&st, "x@exemple.com", false).await;
        // Seul un administrateur donne le rôle.
        let give = |who: &AuthUser, id: i64| {
            update_user(
                s(),
                who.clone(),
                Path(id),
                Json(serde_json::from_value(json!({ "is_moderator": true })).unwrap()),
            )
        };
        assert!(matches!(
            give(&member, moder.id).await,
            Err(AppError::Forbidden)
        ));
        let _ = give(&admin, moder.id).await.unwrap();
        let _ = give(&admin, other.id).await.unwrap();
        assert_eq!(staff(&st, moder.id).await.unwrap(), Staff::Moderator);
        assert_eq!(staff(&st, member.id).await.unwrap(), Staff::None);

        // Ce qu'il peut faire.
        let wid: i64 = sqlx::query_scalar(
            "INSERT INTO waitlist (email, name) VALUES ('w@exemple.com', 'W') RETURNING id",
        )
        .fetch_one(&st.db)
        .await
        .unwrap();
        let q = || {
            Query(ListQ {
                status: Some("all".into()),
            })
        };
        assert!(waitlist(s(), moder.clone(), q()).await.is_ok());
        let nb = || Json(serde_json::from_value(json!({ "note": "à rappeler" })).unwrap());
        assert!(note(s(), moder.clone(), Path(wid), nb()).await.is_ok());
        assert!(refuse(s(), moder.clone(), Path(wid)).await.is_ok());
        assert!(reopen(s(), moder.clone(), Path(wid)).await.is_ok());
        assert!(approve(s(), moder.clone(), Path(wid)).await.is_ok());
        // Accepter une demande crée le compte, pour l'adresse de la demande seulement.
        let acc = |email: &str, w: Option<i64>| {
            create_account(
                s(),
                moder.clone(),
                Json(serde_json::from_value(json!({ "username": format!("compte-{}", email.split('@').next().unwrap()), "name": "W", "email": email, "waitlist_id": w, "plan": "starter" })).unwrap()),
            )
        };
        assert!(matches!(
            acc("autre@exemple.com", Some(wid)).await,
            Err(AppError::Forbidden)
        ));
        assert!(matches!(
            acc("libre@exemple.com", None).await,
            Err(AppError::Forbidden)
        ));
        let made = acc("w@exemple.com", Some(wid)).await.unwrap();
        let plan: String = sqlx::query_scalar("SELECT plan FROM users WHERE id = ?")
            .bind(made.0["id"].as_i64().unwrap())
            .fetch_one(&st.db)
            .await
            .unwrap();
        assert_eq!(plan, "pro", "le modérateur ne choisit pas le plan");
        assert!(users(s(), moder.clone()).await.is_ok());
        assert!(unlock_user(s(), moder.clone(), Path(member.id))
            .await
            .is_ok());
        assert!(user_reset_link(s(), moder.clone(), Path(member.id))
            .await
            .is_ok());
        assert!(audited(&st, "admin_compte_débloqué", moder.id).await);

        // Ce qui lui est refusé.
        let forbidden = |r: Result<(), AppError>, what: &str| {
            assert!(
                matches!(r, Err(AppError::Forbidden)),
                "refusé au modérateur : {what} ({r:?})"
            )
        };
        for (id, who) in [(admin.id, "admin"), (other.id, "autre modérateur")] {
            forbidden(
                unlock_user(s(), moder.clone(), Path(id)).await.map(|_| ()),
                who,
            );
            forbidden(
                user_reset_link(s(), moder.clone(), Path(id))
                    .await
                    .map(|_| ()),
                who,
            );
        }
        forbidden(give(&moder, member.id).await.map(|_| ()), "donner le rôle");
        let off = Json(serde_json::from_value(json!({ "disabled": true })).unwrap());
        forbidden(
            update_user(s(), moder.clone(), Path(member.id), off)
                .await
                .map(|_| ()),
            "désactiver",
        );
        forbidden(
            deletion_preview(s(), moder.clone(), Path(member.id))
                .await
                .map(|_| ()),
            "supprimer",
        );
        forbidden(
            reset_password(s(), moder.clone(), Path(member.id))
                .await
                .map(|_| ()),
            "mot de passe provisoire",
        );
        forbidden(
            remove(s(), moder.clone(), Path(wid)).await.map(|_| ()),
            "supprimer une demande",
        );
        forbidden(
            overview(s(), moder.clone(), Query(Default::default()))
                .await
                .map(|_| ()),
            "vue générale",
        );
        forbidden(
            crate::security::admin_audit(s(), moder.clone())
                .await
                .map(|_| ()),
            "sécurité",
        );

        // Comme l'administrateur, le rôle exige une adresse vérifiée, et il se perd avec le compte désactivé.
        sqlx::query("UPDATE users SET email_verified_at = NULL WHERE id = ?")
            .bind(other.id)
            .execute(&st.db)
            .await
            .unwrap();
        assert_eq!(staff(&st, other.id).await.unwrap(), Staff::None);
        let rm = Json(serde_json::from_value(json!({ "is_moderator": false })).unwrap());
        let _ = update_user(s(), admin.clone(), Path(moder.id), rm)
            .await
            .unwrap();
        assert!(matches!(
            users(s(), moder.clone()).await,
            Err(AppError::Forbidden)
        ));
    }
}

#[cfg(test)]
mod journal_tests {
    use super::*;
    use axum::extract::{Path, Query, State};

    async fn user(st: &AppState, email: &str, admin: bool) -> AuthUser {
        let id: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, is_admin, email_verified_at) VALUES (?, ?, 'x', ?, datetime('now')) RETURNING id")
            .bind(email).bind(email.split('@').next().unwrap()).bind(admin as i64)
            .fetch_one(&st.db).await.unwrap();
        let sid = format!("session-{id}");
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at, sudo_until) VALUES (?, ?, datetime('now', '+1 day'), datetime('now', '+10 minutes'))")
            .bind(&sid).bind(id).execute(&st.db).await.unwrap();
        AuthUser {
            id,
            email: email.into(),
            name: email.split('@').next().unwrap().into(),
            sid,
        }
    }

    async fn journal_de(st: &AppState, u: &AuthUser, q: Value) -> Result<Value, AppError> {
        journal(
            State(st.clone()),
            u.clone(),
            Query(serde_json::from_value(q).unwrap()),
        )
        .await
        .map(|Json(v)| v)
    }

    /// #66 : une action d'admin apparaît avec auteur, action, compte visé et date ; filtres ; 403 pour un non-admin.
    #[tokio::test]
    async fn journal_des_admins() {
        let st = crate::test_state().await;
        let (kl, ben, simple) = (
            user(&st, "kl@pop.fr", true).await,
            user(&st, "ben@pop.fr", true).await,
            user(&st, "yanis@a.fr", false).await,
        );
        sqlx::query("UPDATE users SET locked_until = datetime('now', '+1 hour') WHERE id = ?")
            .bind(simple.id)
            .execute(&st.db)
            .await
            .unwrap();
        unlock_user(State(st.clone()), kl.clone(), Path(simple.id))
            .await
            .unwrap();
        let wid: i64 = sqlx::query_scalar(
            "INSERT INTO waitlist (email, name) VALUES ('nouveau@a.fr', 'Nouveau') RETURNING id",
        )
        .fetch_one(&st.db)
        .await
        .unwrap();
        refuse(State(st.clone()), ben.clone(), Path(wid))
            .await
            .unwrap();
        // Une connexion ratée (journal de sécurité) n'est pas une action d'admin.
        crate::security::audit(&st, Some(simple.id), "connexion_échouée", "", "1.2.3.4").await;

        let v = journal_de(&st, &kl, json!({})).await.unwrap();
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "{v}");
        assert_eq!(
            (
                rows[0]["who"].as_str(),
                rows[0]["label"].as_str(),
                rows[0]["target"].as_str(),
                rows[0]["type"].as_str()
            ),
            (
                Some("ben"),
                Some("Demande refusée"),
                Some("nouveau@a.fr"),
                Some("attente")
            )
        );
        assert_eq!(
            (
                rows[1]["who"].as_str(),
                rows[1]["label"].as_str(),
                rows[1]["target"].as_str()
            ),
            (Some("kl"), Some("Compte débloqué"), Some("yanis@a.fr"))
        );
        assert!(rows[1]["at"].as_str().unwrap().len() >= 19, "date et heure");
        // Filtres.
        let v = journal_de(&st, &kl, json!({ "who": kl.id })).await.unwrap();
        assert!(
            v["rows"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["who"] == "kl")
                && v["rows"].as_array().unwrap().len() == 1
        );
        let v = journal_de(&st, &kl, json!({ "type": "attente" }))
            .await
            .unwrap();
        assert!(
            v["rows"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["type"] == "attente")
                && v["rows"].as_array().unwrap().len() == 1
        );
        let v = journal_de(&st, &kl, json!({ "type": "comptes", "who": ben.id }))
            .await
            .unwrap();
        assert_eq!(v["rows"].as_array().unwrap().len(), 0);
        // 50 par page.
        for k in 0..60 {
            crate::security::audit(
                &st,
                Some(kl.id),
                "annonce_publiée",
                &format!("annonce {k}"),
                "",
            )
            .await;
        }
        let v = journal_de(&st, &kl, json!({})).await.unwrap();
        assert_eq!(
            (v["rows"].as_array().unwrap().len(), v["more"].as_bool()),
            (50, Some(true))
        );
        let v = journal_de(&st, &kl, json!({ "page": 2 })).await.unwrap();
        assert_eq!(
            (v["rows"].as_array().unwrap().len(), v["more"].as_bool()),
            (12, Some(false))
        );
        // Un compte non admin : 403.
        assert!(matches!(
            journal_de(&st, &simple, json!({})).await,
            Err(AppError::Forbidden)
        ));
    }
}

#[cfg(test)]
mod stats_tests {
    use super::*;
    use axum::extract::{Query, State};

    /// #68 : courbes sur 30 ou 90 jours, inscription du jour comptée, totaux par plan conformes aux comptes.
    #[tokio::test]
    async fn statistiques_par_jour() {
        let st = crate::test_state().await;
        let kl: i64 = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, is_admin, email_verified_at, plan) VALUES ('kl@pop.fr', 'KL', 'x', 1, datetime('now'), 'pro') RETURNING id").fetch_one(&st.db).await.unwrap();
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at, sudo_until, created_at) VALUES ('s', ?, datetime('now', '+1 day'), datetime('now', '+10 minutes'), datetime('now'))").bind(kl).execute(&st.db).await.unwrap();
        let u = AuthUser {
            id: kl,
            email: "kl@pop.fr".into(),
            name: "KL".into(),
            sid: "s".into(),
        };
        let vue = |jours: i64| {
            let (st, u) = (st.clone(), u.clone());
            async move {
                overview(State(st), u, Query(OverviewQ { days: Some(jours) }))
                    .await
                    .unwrap()
                    .0
            }
        };
        let v = vue(30).await;
        assert_eq!(v["stats"]["daily"].as_array().unwrap().len(), 30);
        assert_eq!(
            vue(90).await["stats"]["daily"].as_array().unwrap().len(),
            90
        );
        let aujourdhui = |v: &Value, k: &str| {
            v["stats"]["daily"].as_array().unwrap().last().unwrap()[k]
                .as_i64()
                .unwrap()
        };
        let avant = aujourdhui(&v, "signups");
        // Un compte de test créé : +1 inscription aujourd'hui ; plans comptés comme dans Utilisateurs.
        sqlx::query("INSERT INTO users (email, name, password_hash, plan, trial_until) VALUES ('t@a.fr', 'Test', 'x', 'starter', datetime('now', '+7 days'))").execute(&st.db).await.unwrap();
        let v = vue(30).await;
        assert_eq!(aujourdhui(&v, "signups"), avant + 1);
        assert!(
            aujourdhui(&v, "active") >= 1,
            "KL a une session aujourd'hui"
        );
        assert_eq!(
            v["stats"]["plans"],
            json!({ "starter": 1, "pro": 1, "trials": 1 })
        );
        let h = v["stats"]["plans_history"].as_array().unwrap();
        assert_eq!(h.last().unwrap()["starter"], 1, "photo du jour à jour");
    }
}
