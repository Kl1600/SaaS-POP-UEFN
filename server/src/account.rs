//! Compte : nom, mot de passe, mot de passe oublié, notifications, recherche globale.
use crate::{
    access::{map_ctx, studio_role, Role},
    auth::{hash_password, random_token, sha256_hex, AuthUser},
    error::{bad, check_len, ApiResult, AppError},
    mail, AppState,
};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Crée une notification pour un membre et la pousse en direct.
pub async fn notify_user(st: &AppState, uid: i64, mid: i64, text: &str, link: &str) {
    if sqlx::query("INSERT INTO notifications (user_id, map_id, text, link) VALUES (?, ?, ?, ?)")
        .bind(uid)
        .bind(mid)
        .bind(text.chars().take(300).collect::<String>())
        .bind(link)
        .execute(&st.db)
        .await
        .is_ok()
    {
        crate::access::notify(st, mid, json!({ "t": "notif", "uid": uid }));
    }
}

/// Mentions « @Prénom » ou « @Prénom Nom » dans un texte → membres du studio de la map.
pub async fn mentioned(st: &AppState, mid: i64, text: &str) -> Vec<i64> {
    let members: Vec<(i64, String)> = sqlx::query_as("SELECT u.id, u.name FROM memberships m JOIN users u ON u.id = m.user_id JOIN maps p ON p.studio_id = m.studio_id WHERE p.id = ?").bind(mid).fetch_all(&st.db).await.unwrap_or_default();
    let low = text.to_lowercase();
    members
        .into_iter()
        .filter(|(_, n)| {
            let n = n.to_lowercase();
            low.contains(&format!("@{n}"))
                || n.split_whitespace()
                    .next()
                    .map(|f| low.contains(&format!("@{f}")))
                    .unwrap_or(false)
        })
        .map(|m| m.0)
        .collect()
}

pub async fn notifications(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    let rows: Vec<(i64, Option<i64>, String, String, Option<String>, String)> = sqlx::query_as("SELECT id, map_id, text, link, read_at, at FROM notifications WHERE user_id = ? ORDER BY id DESC LIMIT 60").bind(u.id).fetch_all(&st.db).await?;
    let unread: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read_at IS NULL",
    )
    .bind(u.id)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(
        json!({ "unread": unread, "items": rows.into_iter().map(|(id, mid, text, link, read, at)| json!({ "id": id, "map_id": mid, "text": text, "link": link, "read": read.is_some(), "at": at })).collect::<Vec<_>>() }),
    ))
}

pub async fn read_notifications(State(st): State<AppState>, u: AuthUser) -> ApiResult<Value> {
    sqlx::query(
        "UPDATE notifications SET read_at = datetime('now') WHERE user_id = ? AND read_at IS NULL",
    )
    .bind(u.id)
    .execute(&st.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ProfileIn {
    name: String,
}

pub async fn update_profile(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<ProfileIn>,
) -> ApiResult<Value> {
    check_len("Nom", &b.name, 1, 80)?;
    sqlx::query("UPDATE users SET name = ? WHERE id = ?")
        .bind(b.name.trim())
        .bind(u.id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PasswordIn {
    current: String,
    new: String,
}

pub async fn change_password(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<PasswordIn>,
) -> ApiResult<Value> {
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(u.id)
        .fetch_one(&st.db)
        .await?;
    if !crate::auth::verify_password(&b.current, &hash) {
        return Err(bad("Mot de passe actuel incorrect."));
    }
    let tokens = set_password(&st, u.id, &b.new).await?;
    Ok(Json(json!({ "ok": true, "tokens_revoked": tokens })))
}

#[derive(Deserialize)]
pub struct FirstPasswordIn {
    new: String,
    #[serde(default)]
    accept_terms: bool,
}

/// Première connexion d'un compte créé par l'administrateur : choisir son propre mot de passe.
/// Garde la session en cours, ferme les autres.
pub async fn first_password(
    State(st): State<AppState>,
    u: AuthUser,
    Json(b): Json<FirstPasswordIn>,
) -> ApiResult<Value> {
    let must: i64 = sqlx::query_scalar("SELECT must_change_password FROM users WHERE id = ?")
        .bind(u.id)
        .fetch_one(&st.db)
        .await?;
    if must == 0 {
        return Err(bad(
            "Changez votre mot de passe depuis Paramètres → Compte.",
        ));
    }
    if !b.accept_terms {
        return Err(bad("Acceptez les conditions d’utilisation et la politique de confidentialité pour continuer."));
    }
    crate::security::password_policy(&b.new, &u.email)?;
    let old: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(u.id)
        .fetch_one(&st.db)
        .await?;
    if crate::auth::verify_password(&b.new, &old) {
        return Err(bad(
            "Choisissez un mot de passe différent du mot de passe provisoire.",
        ));
    }
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 0, terms_at = datetime('now') WHERE id = ?").bind(hash_password(&b.new)?).bind(u.id).execute(&st.db).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash != ?")
        .bind(u.id)
        .bind(&u.sid)
        .execute(&st.db)
        .await?;
    crate::security::audit(
        &st,
        Some(u.id),
        "mot_de_passe_choisi",
        "première connexion",
        "",
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

async fn set_password(st: &AppState, uid: i64, pw: &str) -> Result<u64, AppError> {
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
        .bind(uid)
        .fetch_one(&st.db)
        .await?;
    crate::security::password_policy(pw, &email)?;
    crate::security::audit(st, Some(uid), "mot_de_passe_changé", "", "").await;
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 0 WHERE id = ?")
        .bind(hash_password(pw)?)
        .bind(uid)
        .execute(&st.db)
        .await?;
    // Déconnecte les autres appareils et révoque les jetons IA : un mot de passe changé après un vol ne doit
    // laisser aucun accès ouvert (#13). Renvoie le nombre de jetons révoqués.
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(uid)
        .execute(&st.db)
        .await?;
    let tokens = sqlx::query("DELETE FROM api_tokens WHERE user_id = ?")
        .bind(uid)
        .execute(&st.db)
        .await?
        .rows_affected();
    Ok(tokens)
}

/// Lien de nouveau mot de passe (chemin relatif) : 2 heures, une seule fois.
pub(crate) async fn new_reset(st: &AppState, uid: i64) -> Result<String, AppError> {
    let token = random_token();
    sqlx::query("INSERT INTO password_resets (token_hash, user_id, expires_at) VALUES (?, ?, datetime('now', '+2 hours'))").bind(sha256_hex(&token)).bind(uid).execute(&st.db).await?;
    Ok(format!("/reinitialiser/{token}"))
}

#[derive(Deserialize)]
pub struct ForgotIn {
    email: String,
}

/// Mot de passe oublié : e-mail avec lien (même réponse que l'adresse existe ou non).
pub async fn forgot(State(st): State<AppState>, Json(b): Json<ForgotIn>) -> ApiResult<Value> {
    crate::auth::rate_limit(&st, &format!("reset:{}", b.email))?;
    if !mail::configured() {
        return Ok(Json(json!({ "mail": false })));
    }
    let row: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
        .bind(b.email.trim().to_lowercase())
        .fetch_optional(&st.db)
        .await?;
    if let Some(uid) = row {
        let link = format!("{}{}", mail::public_url(), new_reset(&st, uid).await?);
        let to = b.email.trim().to_lowercase();
        tokio::spawn(async move {
            if let Err(e) = mail::send(&to, "POP · réinitialiser votre mot de passe", &format!("Bonjour,\n\nPour choisir un nouveau mot de passe, ouvrez ce lien (valable 2 heures) :\n{link}\n\nSi vous n'avez rien demandé, ignorez ce message.")).await {
                tracing::warn!("e-mail de réinitialisation : {e:#}");
            }
        });
    }
    Ok(Json(json!({ "mail": true })))
}

/// Un admin génère un lien de réinitialisation pour un membre (quand l'e-mail n'est pas configuré).
pub async fn admin_reset_link(
    State(st): State<AppState>,
    u: AuthUser,
    Path((sid, uid)): Path<(i64, i64)>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Admin {
        return Err(AppError::Forbidden);
    }
    let target: Option<String> =
        sqlx::query_scalar("SELECT role FROM memberships WHERE studio_id = ? AND user_id = ?")
            .bind(sid)
            .bind(uid)
            .fetch_optional(&st.db)
            .await?;
    match target.as_deref() {
        None => return Err(AppError::NotFound),
        Some("owner") if studio_role(&st, &u, sid).await? != Role::Owner => {
            return Err(AppError::Forbidden)
        }
        _ => {}
    }
    Ok(Json(json!({ "link": new_reset(&st, uid).await? })))
}

#[derive(Deserialize)]
pub struct ResetIn {
    token: String,
    password: String,
}

pub async fn reset(State(st): State<AppState>, Json(b): Json<ResetIn>) -> ApiResult<Value> {
    let row: Option<i64> = sqlx::query_scalar("SELECT user_id FROM password_resets WHERE token_hash = ? AND used_at IS NULL AND expires_at > datetime('now')").bind(sha256_hex(&b.token)).fetch_optional(&st.db).await?;
    let uid = row.ok_or_else(|| bad("Lien expiré ou déjà utilisé."))?;
    let tokens = set_password(&st, uid, &b.password).await?;
    sqlx::query("UPDATE password_resets SET used_at = datetime('now') WHERE token_hash = ?")
        .bind(sha256_hex(&b.token))
        .execute(&st.db)
        .await?;
    // Un nouveau mot de passe choisi par lien lève le blocage après trop d'essais (#38).
    sqlx::query("UPDATE users SET failed_logins = 0, locked_until = NULL WHERE id = ?")
        .bind(uid)
        .execute(&st.db)
        .await?;
    // Le lien de réinitialisation est arrivé à cette adresse : elle est donc vérifiée.
    crate::verify::mark_verified(&st, uid).await?;
    Ok(Json(json!({ "ok": true, "tokens_revoked": tokens })))
}

#[derive(Deserialize)]
pub struct SearchQ {
    q: String,
}

/// Minuscules sans accents, un caractère pour un caractère (les positions restent alignées sur le texte d'origine).
fn fold(s: &str) -> Vec<char> {
    s.chars()
        .map(|c| {
            let l = c.to_lowercase().next().unwrap_or(c);
            match l {
                'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' | 'ā' => 'a',
                'ç' | 'ć' | 'č' => 'c',
                'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' => 'e',
                'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
                'ñ' | 'ń' => 'n',
                'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' | 'œ' => 'o',
                'ù' | 'ú' | 'û' | 'ü' | 'ū' => 'u',
                'ý' | 'ÿ' => 'y',
                'ß' => 's',
                'æ' => 'a',
                '’' | '‘' => '\'',
                _ => l,
            }
        })
        .collect()
}
pub fn fold_str(s: &str) -> String {
    fold(s).into_iter().collect()
}

struct Q {
    words: Vec<String>,
}
impl Q {
    /// Tous les mots doivent apparaître (dans le titre ou le contenu). Renvoie un score, ou None.
    fn score(&self, title: &str, body: &str) -> Option<i64> {
        let t = fold_str(title);
        let b = fold_str(body);
        let mut sc = 0;
        for w in &self.words {
            if t.contains(w.as_str()) {
                sc += if t.starts_with(w.as_str()) || t.contains(&format!(" {w}")) {
                    14
                } else {
                    10
                }
            } else if b.contains(w.as_str()) {
                sc += 3
            } else {
                return None;
            }
        }
        if t == self.words.join(" ") {
            sc += 20
        }
        Some(sc)
    }
}

/// Extrait autour du premier mot trouvé.
fn snippet(text: &str, q: &Q) -> String {
    let chars: Vec<char> = text.chars().collect();
    let f = fold(text);
    let w: Vec<char> = q
        .words
        .first()
        .map(|w| w.chars().collect())
        .unwrap_or_default();
    let pos = if w.is_empty() {
        None
    } else {
        f.windows(w.len()).position(|x| x == w.as_slice())
    };
    let Some(i) = pos else {
        return chars.iter().take(90).collect();
    };
    let start = i.saturating_sub(30);
    let s: String = chars[start..].iter().take(110).collect();
    let s = s.replace('\n', " ");
    if start > 0 {
        format!("…{s}")
    } else {
        s
    }
}

fn block_text(blocks: &str) -> String {
    fn walk(v: &Value, out: &mut String) {
        match v {
            Value::String(s) => {
                out.push_str(s);
                out.push('\n')
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => o
                .iter()
                .filter(|(k, _)| {
                    !["id", "type", "kind", "url", "src", "color"].contains(&k.as_str())
                })
                .for_each(|(_, x)| walk(x, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    if let Ok(v) = serde_json::from_str::<Value>(blocks) {
        walk(&v, &mut out)
    }
    out
}

/// Recherche globale dans la map : tâches, pages, tableaux, acteurs, jalons, membres et actions.
/// Insensible aux majuscules et aux accents ; plusieurs mots = tous doivent être présents.
pub async fn search(
    State(st): State<AppState>,
    u: AuthUser,
    Path(mid): Path<i64>,
    Query(q): Query<SearchQ>,
) -> ApiResult<Value> {
    let ctx = map_ctx(&st, &u, mid).await?;
    let raw = q.q.trim().trim_start_matches('#').to_string();
    if raw.chars().count() < 2 && raw.parse::<i64>().is_err() {
        return Ok(Json(json!([])));
    }
    let query = Q {
        words: fold_str(&raw)
            .split_whitespace()
            .map(String::from)
            .collect(),
    };
    if query.words.is_empty() {
        return Ok(Json(json!([])));
    }
    let num = raw.parse::<i64>().ok();
    let mut out: Vec<(i64, Value)> = vec![];

    // Tâches (titre, description, étiquette, personne assignée).
    let tasks: Vec<(i64, i64, String, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT t.id, t.number, t.title, t.description, t.status, t.tag, u.name FROM tasks t LEFT JOIN users u ON u.id = t.assignee_id WHERE t.map_id = ? ORDER BY t.number DESC LIMIT 3000",
    ).bind(mid).fetch_all(&st.db).await?;
    for (id, n, t, d, s, tag, who) in tasks {
        let body = format!("{d}\n{tag}\n{}", who.clone().unwrap_or_default());
        let sc = if num == Some(n) {
            Some(100)
        } else {
            query.score(&t, &body)
        };
        if let Some(sc) = sc {
            let detail = [
                Some(crate::tasks::status_label(&s).to_string()),
                who.map(|w| format!("assignée à {w}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
            out.push((sc + 2, json!({ "kind": "Tâche", "title": format!("#{n} {t}"), "detail": detail, "link": format!("/taches?t={id}") })));
        }
    }
    // Pages.
    let pages: Vec<(i64, String, String, String, i64)> = sqlx::query_as("SELECT id, title, blocks, share, created_by FROM pages WHERE map_id = ? AND deleted_at IS NULL ORDER BY updated_at DESC LIMIT 2000").bind(mid).fetch_all(&st.db).await?;
    for (id, t, blocks, _share, _by) in pages
        .into_iter()
        .filter(|p| p.3 != "private" || p.4 == ctx.user.id)
    {
        let text = block_text(&blocks);
        if let Some(sc) = query.score(&t, &text) {
            let in_title = query.score(&t, "").is_some();
            out.push((sc + 3, json!({ "kind": "Page", "title": if t.is_empty() { "Sans titre".into() } else { t }, "detail": if in_title { String::new() } else { snippet(&text, &query) }, "link": format!("/pages/{id}") })));
        }
    }
    // Tableaux blancs (titre et textes des objets).
    let boards: Vec<(i64, String, String)> = sqlx::query_as("SELECT id, title, objects FROM boards WHERE map_id = ? AND deleted_at IS NULL ORDER BY updated_at DESC LIMIT 500").bind(mid).fetch_all(&st.db).await?;
    for (id, t, objs) in boards {
        let text: String = serde_json::from_str::<Value>(&objs)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
            .values()
            .filter_map(|o| o["text"].as_str().map(String::from))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(sc) = query.score(&t, &text) {
            out.push((sc, json!({ "kind": "Tableau", "title": t, "detail": if query.score(&t, "").is_some() { String::new() } else { snippet(&text, &query) }, "link": format!("/tableau/{id}") })));
        }
    }
    // Acteurs UEFN.
    let actors: Option<String> =
        sqlx::query_scalar("SELECT actors FROM uefn_state WHERE map_id = ?")
            .bind(mid)
            .fetch_optional(&st.db)
            .await?;
    let actors: Vec<Value> = actors
        .and_then(|a| serde_json::from_str(&a).ok())
        .unwrap_or_default();
    let mut na = 0;
    for a in &actors {
        let name = a["name"].as_str().unwrap_or("");
        if let Some(sc) = query.score(name, a["folder"].as_str().unwrap_or("")) {
            out.push((sc - 2, json!({ "kind": "Acteur", "title": name, "detail": a["folder"], "link": format!("/organiseur?q={}", urlencode(name)) })));
            na += 1;
            if na >= 8 {
                break;
            }
        }
    }
    // Jalons.
    let ms: Vec<(String, String)> =
        sqlx::query_as("SELECT title, date FROM milestones WHERE map_id = ?")
            .bind(mid)
            .fetch_all(&st.db)
            .await?;
    for (t, d) in ms {
        if let Some(sc) = query.score(&t, "") {
            out.push((
                sc,
                json!({ "kind": "Jalon", "title": t, "detail": d, "link": "/planifier" }),
            ))
        }
    }
    // Membres : un membre trouvé ajoute aussi ses tâches et ses dernières actions.
    let people: Vec<(i64, String, String)> = sqlx::query_as("SELECT u.id, u.name, m.role FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.studio_id = ?").bind(ctx.studio_id).fetch_all(&st.db).await?;
    let mut found_people: Vec<i64> = vec![];
    for (id, n, r) in &people {
        if let Some(sc) = query.score(n, "") {
            let (open, done): (i64, i64) = sqlx::query_as("SELECT COALESCE(SUM(status != 'done'), 0), COALESCE(SUM(status = 'done'), 0) FROM tasks WHERE map_id = ? AND assignee_id = ?").bind(mid).bind(id).fetch_one(&st.db).await?;
            out.push((sc + 8, json!({ "kind": "Membre", "title": n, "detail": format!("{r} · {open} tâche{} en cours ou à faire · {done} terminée{}", if open > 1 { "s" } else { "" }, if done > 1 { "s" } else { "" }), "link": "/taches?vue=personnes" })));
            found_people.push(*id);
        }
    }
    // Actions du journal (par texte ou par auteur).
    let acts: Vec<(i64, Option<i64>, Option<String>, String, String, String, String)> = sqlx::query_as(
        "SELECT a.id, a.user_id, u.name, a.kind, a.title, a.detail, a.at FROM activity a LEFT JOIN users u ON u.id = a.user_id WHERE a.map_id = ? ORDER BY a.id DESC LIMIT 600",
    ).bind(mid).fetch_all(&st.db).await?;
    let mut nact = 0;
    for (_, uid, who, kind, t, d, at) in acts {
        let by_person = uid.map(|x| found_people.contains(&x)).unwrap_or(false);
        let sc = if by_person {
            Some(4)
        } else {
            query.score(&t, &d).map(|s| s - 4)
        };
        let Some(sc) = sc else { continue };
        let link = match kind.as_str() {
            "task" | "comment" => "/taches",
            "page" => "/pages",
            "board" => "/tableau",
            "checkin" => "/organiseur",
            "plan" => "/planifier",
            "money" => "/remunerer",
            "playtest" => "/lancer",
            "team" => "/equipe",
            _ => "/",
        };
        out.push((sc, json!({ "kind": "Action", "title": format!("{t}{}", if d.is_empty() { String::new() } else { format!(" · {d}") }), "detail": format!("{} · {}", who.unwrap_or_else(|| "Système".into()), at.get(..16).unwrap_or(&at).replace('T', " ")), "link": link, "at": at })));
        nact += 1;
        if nact >= 8 {
            break;
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(Json(json!(out
        .into_iter()
        .take(30)
        .map(|x| x.1)
        .collect::<Vec<_>>())))
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_./".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
