//! POP Forge : application de bureau Windows de POP (jalon M4).
//!
//! Une fenêtre qui affiche l'interface POP du serveur choisi, comme un navigateur : connexion par le formulaire
//! habituel, session gardée par WebView2 d'un lancement à l'autre. Sans adresse enregistrée, Forge va sur
//! pop-uefn.com (#412) ; la page locale `ui/index.html` dit « POP est injoignable » s'il ne répond pas, et ne demande
//! l'adresse que par « Changer l'adresse de POP ». L'interface POP n'a aucun accès aux commandes de Forge
//! (`capabilities/default.json` : page locale seulement).
//!
//! UEFN Link tourne dans Forge (`relay.rs`) : son état s'affiche dans le titre de la fenêtre, le menu
//! « UEFN Link » relie un projet UEFN et ouvre la page d'état du relais.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agents_locaux;
mod relay;

use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use url::Host;

/// Réglages gardés dans `%APPDATA%\app.pop.forge\forge.json`.
#[derive(Serialize, Deserialize, Default)]
struct Settings {
    server: Option<String>,
}

/// Adresse de la page locale (différente selon le système), lue au démarrage.
struct Home(Mutex<Option<Url>>);

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|d| d.join("forge.json"))
        .map_err(|e| format!("Dossier de configuration introuvable : {e}"))
}

fn load(app: &AppHandle) -> Settings {
    settings_path(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let p = settings_path(app)?;
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(p.parent().unwrap_or(&p))?;
        std::fs::write(&p, serde_json::to_string_pretty(s).unwrap_or_default())
    };
    write().map_err(|e| format!("Impossible d'enregistrer l'adresse : {e}"))
}

/// Adresse saisie → adresse de POP : https:// par défaut, http(s) seulement, sans chemin ni paramètres.
fn normalize(input: &str) -> Result<Url, String> {
    let t = input.trim().trim_end_matches('/');
    if t.is_empty() {
        return Err("Indiquez l'adresse de POP.".into());
    }
    let with_scheme = if t.contains("://") {
        t.to_string()
    } else {
        format!("https://{t}")
    };
    let mut u = Url::parse(&with_scheme)
        .map_err(|_| format!("Adresse invalide : « {} ».", input.trim()))?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
        return Err("L'adresse doit commencer par https:// ou http://.".into());
    }
    // Sans préfixe, un POP du réseau local (adresse IP, localhost) est en http, un nom de domaine en https.
    let local = matches!(u.host(), Some(Host::Ipv4(_) | Host::Ipv6(_)))
        || u.host_str() == Some("localhost");
    if !t.contains("://") && local {
        let _ = u.set_scheme("http");
    }
    u.set_path("/");
    u.set_query(None);
    u.set_fragment(None);
    Ok(u)
}

/// Vérifie qu'un serveur POP répond : `/api/me` renvoie du JSON (401 sans session, 200 avec).
async fn check(u: &Url) -> Result<(), String> {
    let api = u.join("api/me").map_err(|e| e.to_string())?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let r =
        client.get(api).send().await.map_err(|_| {
            format!("POP ne répond pas à {u}. Vérifiez l'adresse et votre connexion.")
        })?;
    let json = r
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("application/json"))
        .unwrap_or(false);
    if json && (r.status() == 401 || r.status().is_success()) {
        Ok(())
    } else {
        Err(format!(
            "{u} répond, mais ce n'est pas un serveur POP (réponse {}).",
            r.status().as_u16()
        ))
    }
}

fn main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "Fenêtre principale introuvable.".into())
}

/// Autorise la page `/forge` du serveur POP choisi à appeler les commandes du relais (et seulement elles).
fn autoriser(app: &AppHandle, u: &Url) {
    let origin = u.origin().ascii_serialization();
    let cap = tauri::ipc::CapabilityBuilder::new("pop-forge-distant")
        .window("main")
        .remote(format!("{origin}/*"))
        .permission("allow-projets")
        .permission("allow-etat-relais")
        .permission("allow-relier")
        .permission("allow-importer")
        .permission("allow-projet-ouvert")
        .permission("allow-liaison")
        .permission("allow-fenetre")
        .permission("allow-choisir-dossier")
        .permission("allow-action-forge")
        // Capture de l'écran du jeu pour les scénarios de test (#350).
        .permission("allow-capture-jeu")
        // Assistant de premier lancement (#299).
        .permission("allow-bilan-poste")
        .permission("allow-activer-mcp-auto")
        // Lancer un agent IA local (#95).
        .permission("allow-outils-ia")
        .permission("allow-lancer-agent")
        // Ouvrir UEFN sur le projet d'une map (#402).
        .permission("allow-ouvrir-uefn");
    let _ = app.add_capability(cap);
}

/// Garde de navigation : Forge n'affiche que sa page locale et l'écran /forge du serveur POP choisi. Toute autre page
/// (le site POP, un lien externe) s'ouvre dans le navigateur de l'utilisateur.
fn autorise(app: &AppHandle, url: &Url) -> bool {
    if url.scheme() == "tauri"
        || url.host_str() == Some("tauri.localhost")
        || url.scheme() == "about"
    {
        return true;
    }
    let pop = load(app).server.and_then(|s| normalize(&s).ok());
    let meme = pop.map(|p| p.origin() == url.origin()).unwrap_or(false);
    if meme
        && (url.path() == "/forge"
            || url.path().starts_with("/forge/")
            || url.path().starts_with("/api/"))
    {
        return true;
    }
    if matches!(url.scheme(), "http" | "https") {
        let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
    }
    false
}

/// Lien d'ouverture reçu avant que la page de Forge soit chargée (#297) : requête `map=…&tache=…`.
#[derive(Default)]
struct Cible(Mutex<Option<String>>);

/// `pop-forge://map/12` ou `pop-forge://map/12/tache/3` → `map=12` ou `map=12&tache=3`. Rien d'autre n'est accepté.
fn cible_de(lien: &str) -> Option<String> {
    let reste = lien.strip_prefix("pop-forge://")?.trim_end_matches('/');
    let parts: Vec<&str> = reste.split('/').collect();
    let num = |s: &str| {
        (!s.is_empty() && s.len() <= 12 && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.to_string())
    };
    match parts.as_slice() {
        ["map", m] => Some(format!("map={}", num(m)?)),
        ["map", m, "tache", n] => Some(format!("map={}&tache={}", num(m)?, num(n)?)),
        _ => None,
    }
}

/// Un lien `pop-forge://` arrive (lancement ou Forge déjà ouvert) : on montre la fenêtre et on va sur la map.
fn recevoir_lien(app: &AppHandle, lien: &str) {
    let Some(q) = cible_de(lien) else { return };
    if let Ok(w) = main_window(app) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        // Page de POP déjà chargée : on y va directement ; sinon `ouvrir` s'en servira.
        if let Ok(mut u) = w.url() {
            if u.path() == "/forge" || u.path().starts_with("/forge/") {
                u.set_path("/forge");
                u.set_query(Some(&q));
                let _ = w.navigate(u);
                return;
            }
        }
    }
    *app.state::<Cible>().0.lock().unwrap() = Some(q);
}

/// Ouvre l'interface de Forge servie par POP (`/forge`), sur la map d'un lien reçu s'il y en a un.
fn ouvrir(app: &AppHandle, u: &Url) -> Result<(), String> {
    autoriser(app, u);
    let mut forge = u.join("forge").map_err(|e| e.to_string())?;
    if let Some(q) = app.state::<Cible>().0.lock().unwrap().take() {
        forge.set_query(Some(&q));
    }
    main_window(app)?.navigate(forge).map_err(|e| e.to_string())
}

/// POP officiel : Forge s'y connecte sans rien demander quand aucune autre adresse n'est enregistrée (#412).
const POP_DEFAUT: &str = "https://pop-uefn.com";

/// Résultat pour la page locale : adresse utilisée (ou à préremplir), erreur à afficher, et si c'est le POP officiel.
#[derive(Serialize)]
struct Start {
    address: String,
    error: Option<String>,
    defaut: bool,
}

/// Au chargement de la page locale : ouvre POP (l'adresse enregistrée, sinon pop-uefn.com) s'il répond, sinon rend
/// l'erreur pour l'écran « POP est injoignable ». `change` : on veut seulement l'adresse pour la modifier.
#[tauri::command]
async fn start(app: AppHandle, change: bool) -> Result<Start, String> {
    let enregistree = load(&app).server;
    let defaut = enregistree.is_none();
    let server = enregistree.unwrap_or_else(|| POP_DEFAUT.to_string());
    if change {
        return Ok(Start {
            address: server,
            error: None,
            defaut,
        });
    }
    let u = normalize(&server)?;
    match check(&u).await {
        Ok(()) => {
            ouvrir(&app, &u)?;
            Ok(Start {
                address: server,
                error: None,
                defaut,
            })
        }
        Err(e) => Ok(Start {
            address: server,
            error: Some(e),
            defaut,
        }),
    }
}

/// « Se connecter » : vérifie l'adresse, l'enregistre et ouvre POP.
#[tauri::command]
async fn connect(app: AppHandle, address: String) -> Result<(), String> {
    let u = normalize(&address)?;
    check(&u).await?;
    let s = u.as_str().trim_end_matches('/').to_string();
    // Revenir à pop-uefn.com efface l'adresse enregistrée : Forge suit de nouveau le POP officiel (#412).
    let server = (s != POP_DEFAUT).then_some(s);
    save(&app, &Settings { server })?;
    ouvrir(&app, &u)
}

/// Projets UEFN : ceux que trouve pop-link (`Documents\Fortnite Projects`, aussi sous OneDrive), ceux du vrai dossier
/// Documents de Windows quand il a été déplacé (autre disque, autre nom : test de Kl1600 du 24/11), ceux qu'UEFN a
/// ouverts récemment, et ceux déjà reliés.
fn tous_les_projets() -> Vec<PathBuf> {
    let mut out = pop_link::find_projects();
    let mut ajouter = |p: PathBuf| {
        if est_un_projet(&p) && !out.contains(&p) {
            out.push(p);
        }
    };
    for p in projets_recents_uefn() {
        ajouter(p);
    }
    if let Ok(c) = pop_link::load() {
        for l in liens_de(&c) {
            ajouter(PathBuf::from(l.project));
        }
    }
    if let Some(docs) = dirs::document_dir() {
        if let Ok(rd) = std::fs::read_dir(docs.join("Fortnite Projects")) {
            for e in rd.flatten() {
                let p = e.path();
                if est_un_projet(&p) && !out.contains(&p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Projets cités dans les réglages d'UEFN (projets récents) : chemins `….uefnproject` trouvés dans les fichiers `.ini`
/// de `%LOCALAPPDATA%\UnrealEditorFortnite\Saved\Config`. Sans ces fichiers, rien (la recherche reste complète
/// avec le dossier Documents et « Parcourir… »).
fn projets_recents_uefn() -> Vec<PathBuf> {
    let Some(base) = dirs::data_local_dir() else {
        return vec![];
    };
    let mut out = vec![];
    let mut pile = vec![base
        .join("UnrealEditorFortnite")
        .join("Saved")
        .join("Config")];
    while let Some(d) = pile.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                pile.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ini")) {
                if let Ok(txt) = std::fs::read_to_string(&p) {
                    out.extend(chemins_uefnproject(&txt));
                }
            }
        }
    }
    out
}

/// Dossiers des projets cités dans un texte (`C:/…/MonIle/MonIle.uefnproject`, guillemets ou non).
fn chemins_uefnproject(txt: &str) -> Vec<PathBuf> {
    let mut out = vec![];
    for morceau in txt.split(['"', '=', ',', '(', ')', '\n']) {
        let m = morceau.trim();
        if m.to_lowercase().ends_with(".uefnproject") && m.len() > 14 {
            if let Some(dossier) =
                std::path::Path::new(&m.replace('/', std::path::MAIN_SEPARATOR_STR)).parent()
            {
                let d = dossier.to_path_buf();
                if !out.contains(&d) {
                    out.push(d);
                }
            }
        }
    }
    out
}

/// Projets reliés dans la configuration du relais : liste de liaisons, ou ancien appairage d'un seul projet.
fn liens_de(c: &pop_link::Config) -> Vec<pop_link::ProjLink> {
    if !c.links.is_empty() {
        c.links.clone()
    } else if !c.project.is_empty() {
        vec![pop_link::ProjLink {
            project: c.project.clone(),
            map: c.map.clone(),
            token: c.token.clone(),
        }]
    } else {
        vec![]
    }
}

/// Un dossier de projet UEFN contient un fichier `.uefnproject`.
fn est_un_projet(p: &std::path::Path) -> bool {
    std::fs::read_dir(p)
        .map(|r| {
            r.flatten()
                .any(|f| f.path().extension().is_some_and(|x| x == "uefnproject"))
        })
        .unwrap_or(false)
}

/// Projets UEFN trouvés sur ce PC.
#[tauri::command]
fn projets() -> Vec<String> {
    tous_les_projets()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

/// Projet ouvert dans UEFN (parmi ceux trouvés sur ce PC), s'il y en a un.
#[tauri::command]
async fn projet_ouvert() -> Option<String> {
    pop_link::open_project(&tous_les_projets())
        .await
        .map(|p| p.to_string_lossy().to_string())
}

/// Liaisons du relais : serveur et, pour chaque projet UEFN relié, sa map POP (plusieurs maps, retour de Kl1600).
#[tauri::command]
fn liaison() -> Option<serde_json::Value> {
    let c = pop_link::load().ok()?;
    let liens = liens_de(&c);
    let premier = liens.first().cloned().unwrap_or_default();
    Some(serde_json::json!({
        "projet": premier.project, "map": premier.map, "serveur": c.server,
        "liens": liens.iter().map(|l| serde_json::json!({ "projet": l.project, "map": l.map })).collect::<Vec<_>>(),
    }))
}

/// « Parcourir… » : choisir le dossier d'un projet UEFN dans l'explorateur de Windows.
#[tauri::command]
async fn choisir_dossier(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Dossier du projet UEFN")
        .pick_folder(move |d| {
            let _ = tx.send(d);
        });
    rx.await
        .ok()
        .flatten()
        .and_then(|d| d.into_path().ok())
        .map(|p| p.to_string_lossy().to_string())
}

/// État du relais en quelques mots.
#[tauri::command]
fn etat_relais(app: AppHandle) -> String {
    relay::libelle(&app)
}

/// Relie un projet UEFN à une map POP avec le code de l'Organiseur, puis redémarre le relais.
#[tauri::command]
async fn relier(app: AppHandle, code: String, projet: String) -> Result<String, String> {
    let server = load(&app)
        .server
        .ok_or("Indiquez d'abord l'adresse de POP.")?;
    let (code, projet) = (code.trim().to_string(), projet.trim().to_string());
    if code.is_empty() || projet.is_empty() {
        return Err("Indiquez le code et le projet UEFN.".into());
    }
    if !std::path::Path::new(&projet).is_dir() {
        return Err(format!("Dossier introuvable : {projet}"));
    }
    if !est_un_projet(std::path::Path::new(&projet)) {
        return Err(format!("{projet} ne contient pas de projet UEFN (fichier .uefnproject) : vérifiez le chemin du dossier."));
    }
    let device = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "POP Forge".into());
    // Plusieurs projets reliés à la fois (une map POP par projet) : la nouvelle liaison s'ajoute aux autres et
    // remplace celle du même projet ou de la même map.
    let avant = pop_link::load().ok();
    pop_link::pair(&server, &code, &projet, &device)
        .await
        .map_err(|e| format!("Liaison refusée : {e:#}"))?;
    if let (Some(avant), Ok(mut c)) = (avant, pop_link::load()) {
        if avant.server.trim_end_matches('/') == c.server.trim_end_matches('/') {
            let nouveau = pop_link::ProjLink {
                project: c.project.clone(),
                map: c.map.clone(),
                token: c.token.clone(),
            };
            let mut liens: Vec<pop_link::ProjLink> = liens_de(&avant)
                .into_iter()
                .filter(|l| l.project != nouveau.project && l.map != nouveau.map)
                .collect();
            liens.push(nouveau);
            c.links = liens;
            c.digests_dir = c.digests_dir.or(avant.digests_dir);
            c.uefn_mcp_url = c.uefn_mcp_url.or(avant.uefn_mcp_url);
            c.lore_cmd = c.lore_cmd.or(avant.lore_cmd);
            if c.tools.is_empty() {
                c.tools = avant.tools;
            }
            pop_link::save(&c).map_err(|e| format!("Configuration non enregistrée : {e:#}"))?;
        }
    }
    relay::demarrer(&app).await;
    Ok(relay::libelle(&app))
}

/// « Créer les maps et relier » (#404, remplace le pop-link.exe « déjà configuré ») : avec un code de studio du
/// site, POP crée une map par projet choisi (même nom) et les relie tous. Les liaisons déjà faites sont gardées.
#[tauri::command]
async fn importer(app: AppHandle, code: String, projets: Vec<String>) -> Result<String, String> {
    let server = load(&app)
        .server
        .ok_or("Indiquez d'abord l'adresse de POP.")?;
    let dossiers: Vec<std::path::PathBuf> = projets
        .iter()
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_dir() && est_un_projet(p))
        .collect();
    if dossiers.is_empty() {
        return Err("Aucun projet UEFN à relier.".into());
    }
    let device = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "POP Forge".into());
    let nouveaux = pop_link::pair_studio(&server, &code, &dossiers, &device)
        .await
        .map_err(|e| format!("Liaison refusée : {e:#}"))?;
    let premier = nouveaux.first().cloned().ok_or("Aucun projet relié.")?;
    let n = nouveaux.len();
    let mut c = match pop_link::load() {
        Ok(c) if c.server.trim_end_matches('/') == server.trim_end_matches('/') => c,
        _ => pop_link::Config {
            server: server.clone(),
            token: premier.token.clone(),
            project: premier.project.clone(),
            map: premier.map.clone(),
            ..Default::default()
        },
    };
    let mut liens: Vec<pop_link::ProjLink> = liens_de(&c)
        .into_iter()
        .filter(|l| {
            !nouveaux
                .iter()
                .any(|x| x.project == l.project || x.map == l.map)
        })
        .collect();
    liens.extend(nouveaux);
    c.links = liens;
    pop_link::save(&c).map_err(|e| format!("Configuration non enregistrée : {e:#}"))?;
    relay::demarrer(&app).await;
    Ok(format!("{n} projet(s) relié(s)"))
}

/// Barre de titre propre à Forge (#78, retour de Kl1600 du 24/11) : la fenêtre n'a plus le cadre ni le menu de
/// Windows ; la page dessine sa barre et appelle cette commande pour réduire, agrandir, fermer ou déplacer.
/// « Ouvrir dans UEFN » (#402) : lance UEFN sur le projet relié à une map, sans passer par le launcher Epic. Seul un
/// projet déjà relié est accepté. Si UEFN est ouvert, rien n'est lancé ni fermé.
#[tauri::command]
async fn ouvrir_uefn(projet: String) -> Result<String, String> {
    let relie = pop_link::load()
        .ok()
        .map(|c| {
            liens_de(&c)
                .into_iter()
                .any(|l| l.project.eq_ignore_ascii_case(&projet))
        })
        .unwrap_or(false);
    if !relie {
        return Err("Ce dossier n'est relié à aucune map.".into());
    }
    let p = PathBuf::from(&projet);
    if let Some(ouvert) = pop_link::open_project(&tous_les_projets()).await {
        let meme = ouvert.to_string_lossy().eq_ignore_ascii_case(&projet);
        return if meme {
            Ok("UEFN est déjà ouvert sur ce projet.".into())
        } else {
            Err(format!(
                "UEFN est ouvert sur un autre projet ({}) : fermez-le d'abord. Rien n'a été fermé.",
                ouvert
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            ))
        };
    }
    tokio::task::spawn_blocking(move || pop_link::poste::ouvrir_uefn(&p))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Outils d'IA installés sur ce PC, lançables ou non depuis Forge (#95).
#[tauri::command]
fn outils_ia() -> Vec<agents_locaux::Outil> {
    agents_locaux::outils()
}

/// Lance l'IA de l'utilisateur sur une tâche POP (#95), avec la clé de l'agent dans `POP_TOKEN` seulement.
#[tauri::command]
fn lancer_agent(
    app: AppHandle,
    outil: String,
    token: String,
    map: String,
    tache: i64,
    titre: String,
) -> Result<(), String> {
    if !token.starts_with("pop_")
        || token.len() > 200
        || !token[4..].bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Err("Clé d'agent invalide.".into());
    }
    let serveur = load(&app)
        .server
        .ok_or("Forge n'est relié à aucun serveur POP.")?;
    let o = agents_locaux::outils()
        .into_iter()
        .find(|o| o.id == outil)
        .ok_or("Outil inconnu.")?;
    if !o.lancable {
        return Err(o.raison.unwrap_or_else(|| "Outil indisponible.".into()));
    }
    let exe = PathBuf::from(o.chemin.ok_or("Outil introuvable.")?);
    // Config MCP sans clé (l'en-tête lit POP_TOKEN), gardée avec les données de Forge.
    let dossier_forge = dirs::data_local_dir()
        .ok_or("Dossier de données introuvable.")?
        .join("POP Forge");
    std::fs::create_dir_all(&dossier_forge).map_err(|e| e.to_string())?;
    let config = dossier_forge.join("mcp-pop.json");
    std::fs::write(&config, agents_locaux::config_claude(&serveur)).map_err(|e| e.to_string())?;
    // Dossier de travail : le projet UEFN relié à cette map, sinon le dossier de l'utilisateur.
    let dossier = pop_link::load()
        .ok()
        .and_then(|c| {
            liens_de(&c)
                .into_iter()
                .find(|l| l.map.eq_ignore_ascii_case(&map))
                .map(|l| PathBuf::from(l.project))
        })
        .filter(|p| p.is_dir())
        .or_else(dirs::home_dir)
        .ok_or("Dossier de travail introuvable.")?;
    let consigne = agents_locaux::consigne(&map, tache, &titre);
    let args = agents_locaux::arguments(&outil, &serveur, &config, &consigne)
        .ok_or("Outil non pris en charge.")?;
    agents_locaux::lancer(&exe, &args, &dossier, &token)
}

/// Assistant de premier lancement (#299) : vérifications du poste (UEFN, serveur MCP, toolsets, Python, démarrage
/// automatique) pour le projet ouvert dans UEFN, sinon le premier projet relié.
#[tauri::command]
async fn bilan_poste() -> serde_json::Value {
    let cfg = pop_link::load().ok();
    let ouvert = pop_link::open_project(&tous_les_projets()).await;
    let projet = ouvert
        .clone()
        .or_else(|| cfg.as_ref().map(|c| PathBuf::from(&c.project)));
    let mut b = pop_link::poste::bilan(
        projet.as_deref(),
        cfg.as_ref().and_then(|c| c.uefn_mcp_url.as_deref()),
    )
    .await;
    b["projet_ouvert"] = serde_json::json!(ouvert.map(|p| p.to_string_lossy().to_string()));
    b
}

/// Active le démarrage automatique du serveur MCP d'UEFN, avec l'accord de l'utilisateur (#299).
#[tauri::command]
fn activer_mcp_auto() -> Result<String, String> {
    pop_link::poste::activer_demarrage_auto()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Photo de l'écran du jeu pour un scénario de test (#350) : la fenêtre du client Fortnite (session de test) si elle
/// est ouverte, sinon l'écran principal. Rend une image PNG en base64.
#[tauri::command]
async fn capture_jeu() -> Result<String, String> {
    tokio::task::spawn_blocking(capture_png)
        .await
        .map_err(|e| e.to_string())?
        .map(|png| {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(png)
        })
}

#[cfg(windows)]
fn capture_png() -> Result<Vec<u8>, String> {
    use xcap::{Monitor, Window};
    let jeu = Window::all().ok().and_then(|ws| {
        ws.into_iter().find(|w| {
            let app = w.app_name().unwrap_or_default().to_lowercase();
            let titre = w.title().unwrap_or_default().to_lowercase();
            // Le client de jeu, pas l'éditeur (« Unreal Editor for Fortnite »).
            (app.contains("fortniteclient") || titre == "fortnite")
                && !titre.contains("unreal editor")
                && !w.is_minimized().unwrap_or(true)
        })
    });
    let img = match jeu {
        Some(w) => w
            .capture_image()
            .map_err(|e| format!("capture de la fenêtre du jeu : {e}"))?,
        None => {
            let m = Monitor::all()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|m| m.is_primary().unwrap_or(false))
                .ok_or("écran principal introuvable")?;
            m.capture_image()
                .map_err(|e| format!("capture de l'écran : {e}"))?
        }
    };
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, xcap::image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png.into_inner())
}

#[cfg(not(windows))]
fn capture_png() -> Result<Vec<u8>, String> {
    Err("La capture de l'écran du jeu n'est disponible que sous Windows.".into())
}

#[tauri::command]
fn fenetre(window: WebviewWindow, action: String) -> Result<(), String> {
    match action.as_str() {
        "reduire" => window.minimize(),
        "agrandir" => {
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        "fermer" => window.close(),
        "deplacer" => window.start_dragging(),
        _ => return Err(format!("Action de fenêtre inconnue : {action}")),
    }
    .map_err(|e| e.to_string())
}

/// Ce que proposait l'ancien menu de Windows, maintenant dans le menu du profil de Forge : changer l'adresse de
/// POP, relier un projet, page d'état du relais, accueil, recharger.
#[tauri::command]
fn action_forge(app: AppHandle, id: String) -> Result<(), String> {
    if !["changer", "relier", "etat", "accueil", "recharger"].contains(&id.as_str()) {
        return Err(format!("Action inconnue : {id}"));
    }
    on_menu(&app, &id);
    Ok(())
}

fn on_menu(app: &AppHandle, id: &str) {
    let Ok(w) = main_window(app) else { return };
    match id {
        "changer" | "relier" => {
            let home = app.state::<Home>().0.lock().unwrap().clone();
            if let Some(mut h) = home {
                h.set_query(Some(if id == "changer" {
                    "changer=1"
                } else {
                    "relier=1"
                }));
                let _ = w.navigate(h);
            }
        }
        "etat" => {
            if let Some(e) = app.get_webview_window("etat") {
                let _ = e.set_focus();
            } else if let Ok(u) = format!("http://127.0.0.1:{}/", relay::PORT).parse() {
                let _ = WebviewWindowBuilder::new(app, "etat", WebviewUrl::External(u))
                    .title("UEFN Link · état du relais")
                    .inner_size(960.0, 720.0)
                    .build();
            }
        }
        "accueil" => {
            let server = load(app).server.and_then(|s| normalize(&s).ok());
            if let Some(u) = server {
                let _ = ouvrir(app, &u);
            }
        }
        "recharger" => {
            let _ = w.reload();
        }
        _ => {}
    }
}

fn main() {
    tauri::Builder::default()
        // En premier : un second lancement (clic sur un lien pop-forge://) passe son lien à la fenêtre ouverte.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Ok(w) = main_window(app) {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Home(Mutex::new(None)))
        .manage(relay::Relay::default())
        .manage(Cible::default())
        .invoke_handler(tauri::generate_handler![
            capture_jeu,
            bilan_poste,
            outils_ia,
            lancer_agent,
            ouvrir_uefn,
            activer_mcp_auto,
            start,
            connect,
            projets,
            etat_relais,
            relier,
            importer,
            projet_ouvert,
            liaison,
            fenetre,
            action_forge,
            choisir_dossier
        ])
        .setup(|app| {
            let h = app.handle().clone();
            let w = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("POP Forge")
                .decorations(false)
                .inner_size(1400.0, 900.0)
                .min_inner_size(900.0, 600.0)
                .center()
                .on_navigation(move |u| autorise(&h, u))
                .build()?;
            *app.state::<Home>().0.lock().unwrap() = w.url().ok();
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                // L'installateur enregistre pop-forge:// ; on le refait au lancement (copie portable, mise à jour).
                #[cfg(windows)]
                let _ = app.deep_link().register_all();
                if let Ok(Some(liens)) = app.deep_link().get_current() {
                    if let Some(l) = liens.first() {
                        recevoir_lien(app.handle(), l.as_str());
                    }
                }
                let h = app.handle().clone();
                app.deep_link().on_open_url(move |e| {
                    if let Some(l) = e.urls().first() {
                        recevoir_lien(&h, l.as_str());
                    }
                });
            }
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move { relay::demarrer(&h).await });
            relay::suivre(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("POP Forge n'a pas pu démarrer");
}

#[cfg(test)]
mod tests {
    use super::{check, cible_de, normalize};

    #[test]
    fn liens_pop_forge() {
        assert_eq!(cible_de("pop-forge://map/12").as_deref(), Some("map=12"));
        assert_eq!(
            cible_de("pop-forge://map/12/tache/3/").as_deref(),
            Some("map=12&tache=3")
        );
        assert_eq!(cible_de("pop-forge://map/12&x=1"), None);
        assert_eq!(cible_de("pop-forge://map/../reglages"), None);
        assert_eq!(cible_de("https://map/12"), None);
    }

    use std::io::{Read, Write};

    /// Petit serveur d'une seule réponse sur un port libre ; rend son adresse.
    fn serveur(reponse: &'static str) -> tauri::Url {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let _ = s.write_all(reponse.as_bytes());
            }
        });
        normalize(&format!("http://127.0.0.1:{port}")).unwrap()
    }

    #[test]
    fn verification_du_serveur() {
        let pop = serveur("HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}");
        assert!(tauri::async_runtime::block_on(check(&pop)).is_ok());
        let autre = serveur("HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok");
        let e = tauri::async_runtime::block_on(check(&autre)).unwrap_err();
        assert!(e.contains("pas un serveur POP"), "{e}");
        let ferme = normalize("http://127.0.0.1:1").unwrap();
        let e = tauri::async_runtime::block_on(check(&ferme)).unwrap_err();
        assert!(e.contains("ne répond pas"), "{e}");
    }

    #[test]
    fn projets_recents() {
        let ini = "[/Script/X]\nRecentlyOpenedProjectFiles=(ProjectName=\"C:/Users/ben/Documents/Fortnite Projects/VaultRush/VaultRush.uefnproject\",LastOpenTime=1)\nAutre=D:/UEFN/Grass Cutter/Grass Cutter.uefnproject\n";
        let p = super::chemins_uefnproject(ini);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(p[0].to_string_lossy().ends_with("VaultRush"));
        assert!(p[1].to_string_lossy().ends_with("Grass Cutter"));
    }

    #[test]
    fn adresses() {
        assert_eq!(
            normalize("pop.exemple.fr").unwrap().as_str(),
            "https://pop.exemple.fr/"
        );
        assert_eq!(
            normalize(" http://192.168.1.20:8080/organiseur?x=1 ")
                .unwrap()
                .as_str(),
            "http://192.168.1.20:8080/"
        );
        assert_eq!(
            normalize("192.168.1.20:8080").unwrap().as_str(),
            "http://192.168.1.20:8080/"
        );
        assert_eq!(
            normalize("localhost:8080").unwrap().as_str(),
            "http://localhost:8080/"
        );
        assert!(normalize("").is_err());
        assert!(normalize("ftp://pop.exemple.fr").is_err());
        assert!(normalize("file:///C:/x").is_err());
    }
}
