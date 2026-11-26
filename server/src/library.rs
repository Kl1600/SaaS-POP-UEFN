//! Bibliothèque Verse du studio (#85) : modules prêts (économie, équipes, minuteur, HUD, sauvegarde…) partagés par
//! l'équipe. Les humains et les IA connectées (MCP : `search_library`, `get_library_module`) la consultent avant
//! d'écrire du neuf. Lecture : tout membre du studio, invité compris ; ajout, modification, suppression : membre et plus.
use crate::{
    access::{studio_role, Role},
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

pub const CATEGORIES: &[&str] = &[
    "economie",
    "equipes",
    "minuteur",
    "hud",
    "sauvegarde",
    "interface",
    "jeu",
    "outils",
    "autre",
];
const MAX_CONTENU: usize = 200_000;

/// Modules de l'équipe POP (#87, #88) : visibles par tous les studios, en lecture seule, avec un identifiant négatif
/// (-1, -2…) pour ne jamais se mêler aux modules d'un studio. Le code est dans `server/library/`.
/// (nom, catégorie, notice, appareils requis, code)
pub const POP: &[(&str, &str, &str, &[&str], &str)] = &[
    ("Économie (POP)", "economie",
     "Un solde de pièces par joueur. Un bouton fait gagner Gain pièces ; un autre achète un objet : s'il reste assez de pièces, Prix est retiré et le distributeur donne l'objet, sinon le message dit combien il manque. Depuis votre Verse : Ajouter(Joueur, 25), Solde(Joueur) et if (Depenser[Joueur, 50]). Les soldes repartent à zéro à chaque partie (sauvegarde : module Sauvegarde).",
     &["button_device (gain, facultatif)", "button_device (achat, facultatif)", "item_granter_device (objet acheté)", "hud_message_device (solde à l'écran)"],
     include_str!("../library/economie.verse")),
    ("Équipes (POP)", "equipes",
     "Chaque joueur qui arrive rejoint l'équipe la moins nombreuse ; au début de la partie, tous les joueurs présents sont répartis. Le nombre d'équipes se règle dans les Réglages de l'île. Option Actif : désactiver pour garder les équipes de l'île. Depuis votre Verse : NombreDans(Equipe) et Equilibrer().",
     &[],
     include_str!("../library/equipes.verse")),
    ("Minuteur (POP)", "minuteur",
     "Compte à rebours en mm:ss affiché à tous les joueurs, lancé par un bouton ou dès le début (DemarrerSeul). À zéro, il envoie FinEvent à votre Verse et, si TerminerLaPartie est coché, termine la partie. Depuis votre Verse : Lancer(), Arreter() et FinEvent.Await().",
     &["button_device (départ, facultatif)", "hud_message_device (affichage)", "end_game_device (fin de partie, facultatif)"],
     include_str!("../library/minuteur.verse")),
    ("HUD (POP)", "hud",
     "Un panneau à l'écran de chaque joueur : un titre et des lignes nommées (par défaut « Score » et « Pièces »), sur un fond sombre. Réglez le titre, les lignes et la position dans les options de l'appareil. Depuis votre Verse : Definir(Joueur, \"Score\", \"120\"), DefinirNombre(Joueur, \"Pièces\", 35), Montrer(Joueur) et Cacher(Joueur). Avec le module Économie : appelez DefinirNombre après chaque achat ou gain.",
     &[],
     include_str!("../library/hud.verse")),
    ("Sauvegarde (POP)", "sauvegarde",
     "Garde les données de chaque joueur d'une partie à l'autre (données persistantes de Verse) : pièces, score, niveau, record, nombre de parties et des compteurs libres par nom. Un joueur qui revient peut voir « Bon retour ! » avec son niveau et son record. Depuis votre Verse : Donnees(Joueur).Pieces, AjouterPieces(Joueur, 10), AjouterScore, DefinirNiveau(Joueur, 3), ProposerRecord(Joueur, 1200), Compteur(Joueur, \"coffres\"), AjouterAuCompteur(Joueur, \"coffres\", 1) et Effacer(Joueur). Les données ne se gardent que sur une île publiée ou en session de test ; ne retirez jamais un champ d'une île publiée.",
     &["hud_message_device (message « Bon retour », facultatif)"],
     include_str!("../library/sauvegarde.verse")),
];

fn pop_fiche(i: usize, avec_code: bool) -> Value {
    let (nom, cat, notice, appareils, code) = POP[i];
    let mut v = json!({ "id": -(i as i64) - 1, "name": nom, "category": cat, "description": notice, "devices": appareils, "version": 1, "author": "Équipe POP", "official": true });
    if avec_code {
        v["content"] = json!(code);
    }
    v
}

#[derive(Deserialize, Default)]
pub struct SearchQ {
    pub q: Option<String>,
    pub category: Option<String>,
}

/// Modules d'un studio (sans le code), filtrés par texte (nom, description) et catégorie.
pub async fn search(st: &AppState, studio_id: i64, q: &SearchQ) -> Result<Vec<Value>, AppError> {
    let texte = format!("%{}%", q.q.as_deref().unwrap_or("").trim().to_lowercase());
    let cat = q.category.as_deref().unwrap_or("").trim().to_string();
    let rows: Vec<(i64, String, String, String, String, i64, Option<String>, String)> = sqlx::query_as(
        "SELECT v.id, v.name, v.category, v.description, v.devices, v.version, u.name, v.updated_at FROM verse_modules v LEFT JOIN users u ON u.id = v.author_id
         WHERE v.studio_id = ? AND (lower(v.name) LIKE ? OR lower(v.description) LIKE ?) AND (? = '' OR v.category = ?)
         ORDER BY v.category, v.name LIMIT 200",
    )
    .bind(studio_id).bind(&texte).bind(&texte).bind(&cat).bind(&cat)
    .fetch_all(&st.db).await?;
    let texte = texte.trim_matches('%').to_string();
    let mut out: Vec<Value> = (0..POP.len())
        .filter(|&i| {
            (cat.is_empty() || POP[i].1 == cat)
                && (texte.is_empty()
                    || POP[i].0.to_lowercase().contains(&texte)
                    || POP[i].2.to_lowercase().contains(&texte))
        })
        .map(|i| pop_fiche(i, false))
        .collect();
    out.extend(rows
        .into_iter()
        .map(|r| json!({ "id": r.0, "name": r.1, "category": r.2, "description": r.3, "devices": serde_json::from_str::<Value>(&r.4).unwrap_or(json!([])), "version": r.5, "author": r.6, "updated_at": r.7 })));
    Ok(out)
}

/// Un module complet (avec le code), s'il appartient au studio donné.
pub async fn module(st: &AppState, studio_id: i64, id: i64) -> Result<Value, AppError> {
    if id < 0 {
        let i = (-id - 1) as usize;
        return if i < POP.len() {
            Ok(pop_fiche(i, true))
        } else {
            Err(AppError::NotFound)
        };
    }
    let r: Option<(i64, String, String, String, String, String, i64, Option<String>, String)> = sqlx::query_as(
        "SELECT v.id, v.name, v.category, v.description, v.content, v.devices, v.version, u.name, v.updated_at FROM verse_modules v LEFT JOIN users u ON u.id = v.author_id WHERE v.id = ? AND v.studio_id = ?",
    )
    .bind(id).bind(studio_id).fetch_optional(&st.db).await?;
    let r = r.ok_or(AppError::NotFound)?;
    Ok(
        json!({ "id": r.0, "name": r.1, "category": r.2, "description": r.3, "content": r.4, "devices": serde_json::from_str::<Value>(&r.5).unwrap_or(json!([])), "version": r.6, "author": r.7, "updated_at": r.8 }),
    )
}

async fn studio_of(st: &AppState, id: i64) -> Result<i64, AppError> {
    sqlx::query_scalar("SELECT studio_id FROM verse_modules WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn list(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Query(q): Query<SearchQ>,
) -> ApiResult<Value> {
    studio_role(&st, &u, sid).await?;
    Ok(Json(json!(search(&st, sid, &q).await?)))
}

pub async fn get(State(st): State<AppState>, u: AuthUser, Path(id): Path<i64>) -> ApiResult<Value> {
    if id < 0 {
        return Ok(Json(module(&st, 0, id).await?));
    }
    let sid = studio_of(&st, id).await?;
    studio_role(&st, &u, sid).await?;
    Ok(Json(module(&st, sid, id).await?))
}

#[derive(Deserialize)]
pub struct ModuleIn {
    name: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    description: String,
    content: String,
    #[serde(default)]
    devices: Vec<String>,
}

fn valider(b: &ModuleIn) -> Result<String, AppError> {
    check_len("Nom", &b.name, 1, 80)?;
    check_len("Description", &b.description, 0, 1000)?;
    if b.content.trim().is_empty() {
        return Err(bad("Le code du module est vide."));
    }
    if b.content.len() > MAX_CONTENU {
        return Err(bad("Module trop long (200 000 caractères au plus)."));
    }
    if b.devices.len() > 50 || b.devices.iter().any(|d| d.chars().count() > 120) {
        return Err(bad("Liste d'appareils requis trop longue."));
    }
    let cat = b.category.as_deref().unwrap_or("autre");
    if !CATEGORIES.contains(&cat) {
        return Err(bad(format!(
            "Catégorie inconnue : {cat}. Catégories : {}.",
            CATEGORIES.join(", ")
        )));
    }
    Ok(cat.to_string())
}

fn doublon(e: sqlx::Error) -> AppError {
    match &e {
        sqlx::Error::Database(d) if d.message().contains("UNIQUE") => {
            bad("Un module de ce nom existe déjà dans la bibliothèque du studio.")
        }
        _ => e.into(),
    }
}

pub async fn create(
    State(st): State<AppState>,
    u: AuthUser,
    Path(sid): Path<i64>,
    Json(b): Json<ModuleIn>,
) -> ApiResult<Value> {
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let cat = valider(&b)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO verse_modules (studio_id, name, category, description, content, devices, author_id) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id")
        .bind(sid).bind(b.name.trim()).bind(&cat).bind(b.description.trim()).bind(&b.content).bind(json!(b.devices).to_string()).bind(u.id)
        .fetch_one(&st.db).await.map_err(doublon)?;
    Ok(Json(json!({ "id": id })))
}

pub async fn update(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<ModuleIn>,
) -> ApiResult<Value> {
    let sid = studio_of(&st, id).await?;
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    let cat = valider(&b)?;
    let version: i64 = sqlx::query_scalar("UPDATE verse_modules SET name = ?, category = ?, description = ?, content = ?, devices = ?, author_id = ?, version = version + 1, updated_at = datetime('now') WHERE id = ? RETURNING version")
        .bind(b.name.trim()).bind(&cat).bind(b.description.trim()).bind(&b.content).bind(json!(b.devices).to_string()).bind(u.id).bind(id)
        .fetch_one(&st.db).await.map_err(doublon)?;
    Ok(Json(json!({ "id": id, "version": version })))
}

pub async fn delete(
    State(st): State<AppState>,
    u: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Value> {
    let sid = studio_of(&st, id).await?;
    if studio_role(&st, &u, sid).await? < Role::Member {
        return Err(AppError::Forbidden);
    }
    sqlx::query("DELETE FROM verse_modules WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn membre(st: &AppState, email: &str, studio: i64, role: &str) -> AuthUser {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO users (email, name, password_hash) VALUES (?, ?, 'x') RETURNING id",
        )
        .bind(email)
        .bind(email)
        .fetch_one(&st.db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO memberships (studio_id, user_id, role) VALUES (?, ?, ?)")
            .bind(studio)
            .bind(id)
            .bind(role)
            .execute(&st.db)
            .await
            .unwrap();
        AuthUser {
            id,
            email: email.into(),
            name: email.into(),
            sid: String::new(),
        }
    }
    async fn studio(st: &AppState, nom: &str) -> i64 {
        sqlx::query_scalar("INSERT INTO studios (name) VALUES (?) RETURNING id")
            .bind(nom)
            .fetch_one(&st.db)
            .await
            .unwrap()
    }
    fn module_in(nom: &str) -> Json<ModuleIn> {
        Json(serde_json::from_value(json!({ "name": nom, "category": "minuteur", "description": "Compte à rebours de manche", "content": "minuteur := class(creative_device):\n    OnBegin<override>()<suspends>:void = {}", "devices": ["timer_device"] })).unwrap())
    }

    #[tokio::test]
    async fn droits_et_recherche() {
        let st = crate::test_state().await;
        let (s1, s2) = (studio(&st, "Studio A").await, studio(&st, "Studio B").await);
        let m = membre(&st, "m@a.fr", s1, "member").await;
        let invite = membre(&st, "v@a.fr", s1, "viewer").await;
        let autre = membre(&st, "o@b.fr", s2, "owner").await;

        // Un membre ajoute ; un invité ne peut pas.
        let Json(r) = create(
            State(st.clone()),
            m.clone(),
            Path(s1),
            module_in("Minuteur de manche"),
        )
        .await
        .unwrap();
        let id = r["id"].as_i64().unwrap();
        assert!(matches!(
            create(
                State(st.clone()),
                invite.clone(),
                Path(s1),
                module_in("Autre")
            )
            .await,
            Err(AppError::Forbidden)
        ));
        assert!(matches!(
            update(State(st.clone()), invite.clone(), Path(id), module_in("X")).await,
            Err(AppError::Forbidden)
        ));

        // L'invité lit et cherche.
        let Json(l) = list(
            State(st.clone()),
            invite.clone(),
            Path(s1),
            Query(SearchQ {
                q: Some("rebours".into()),
                category: None,
            }),
        )
        .await
        .unwrap();
        let l: Vec<&Value> = l
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["official"] != true)
            .collect();
        assert_eq!(l.len(), 1, "module du studio (les modules POP sont à part)");
        assert!(
            l[0].get("content").is_none(),
            "la liste ne renvoie pas le code"
        );
        let Json(g) = get(State(st.clone()), invite.clone(), Path(id))
            .await
            .unwrap();
        assert!(g["content"].as_str().unwrap().contains("creative_device"));
        assert_eq!(g["devices"][0], "timer_device");

        // Un autre studio ne voit rien.
        assert!(list(
            State(st.clone()),
            autre.clone(),
            Path(s1),
            Query(SearchQ::default())
        )
        .await
        .is_err());
        assert!(get(State(st.clone()), autre.clone(), Path(id))
            .await
            .is_err());
        let Json(vide) = list(
            State(st.clone()),
            autre.clone(),
            Path(s2),
            Query(SearchQ::default()),
        )
        .await
        .unwrap();
        assert!(
            vide.as_array()
                .unwrap()
                .iter()
                .all(|m| m["official"] == true),
            "un autre studio ne voit que les modules de l'équipe POP"
        );

        // Modification : la version monte ; nom en double refusé ; catégorie inconnue refusée.
        let Json(u) = update(
            State(st.clone()),
            m.clone(),
            Path(id),
            module_in("Minuteur de manche"),
        )
        .await
        .unwrap();
        assert_eq!(u["version"], 2);
        assert!(create(
            State(st.clone()),
            m.clone(),
            Path(s1),
            module_in("Minuteur de manche")
        )
        .await
        .is_err());
        let Json(mauvais) = module_in("Y");
        let mauvais = ModuleIn {
            category: Some("violet".into()),
            ..mauvais
        };
        assert!(
            create(State(st.clone()), m.clone(), Path(s1), Json(mauvais))
                .await
                .is_err()
        );

        // Filtre par catégorie, puis suppression.
        let Json(c) = list(
            State(st.clone()),
            m.clone(),
            Path(s1),
            Query(SearchQ {
                q: None,
                category: Some("interface".into()),
            }),
        )
        .await
        .unwrap();
        assert_eq!(c.as_array().unwrap().len(), 0);
        delete(State(st.clone()), m.clone(), Path(id))
            .await
            .unwrap();
        assert!(get(State(st.clone()), m.clone(), Path(id)).await.is_err());
    }

    #[tokio::test]
    async fn modules_de_l_equipe_pop() {
        let st = crate::test_state().await;
        let sid: i64 = sqlx::query_scalar("INSERT INTO studios (name) VALUES ('S') RETURNING id")
            .fetch_one(&st.db)
            .await
            .unwrap();
        let l = search(&st, sid, &SearchQ::default()).await.unwrap();
        assert!(
            l.len() >= 3
                && l.iter()
                    .take(3)
                    .all(|m| m["official"] == true && m.get("content").is_none()),
            "{l:?}"
        );
        let eco = search(
            &st,
            sid,
            &SearchQ {
                q: Some("pièces".into()),
                category: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(eco[0]["name"], "Économie (POP)");
        // Lot 2 (#88) : HUD et Sauvegarde, chacun dans sa catégorie, avec sa notice et son code.
        for (cat, nom, classe) in [
            ("hud", "HUD (POP)", "hud_pop := class"),
            ("sauvegarde", "Sauvegarde (POP)", "sauvegarde_pop := class"),
        ] {
            let l = search(
                &st,
                sid,
                &SearchQ {
                    q: None,
                    category: Some(cat.into()),
                },
            )
            .await
            .unwrap();
            assert_eq!(l[0]["name"], nom);
            assert!(l[0]["description"].as_str().unwrap().len() > 100);
            let id = l[0]["id"].as_i64().unwrap();
            assert!(module(&st, sid, id).await.unwrap()["content"]
                .as_str()
                .unwrap()
                .contains(classe));
        }
        let m = module(&st, sid, -3).await.unwrap();
        assert!(m["content"]
            .as_str()
            .unwrap()
            .contains("minuteur_pop := class(creative_device)"));
        assert!(module(&st, sid, -99).await.is_err());
        for (_, _, notice, _, code) in POP {
            assert!(
                notice.len() > 80
                    && code.contains("creative_device")
                    && code.contains("using { /Fortnite.com/Devices }")
            );
        }
    }
}
