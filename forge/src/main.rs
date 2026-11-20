//! POP Forge : application de bureau Windows de POP (jalon M4).
//!
//! Une fenêtre qui affiche l'interface POP du serveur choisi, comme un navigateur : connexion par le formulaire
//! habituel, session gardée par WebView2 d'un lancement à l'autre. Au premier lancement (ou si POP ne répond pas),
//! la page locale `ui/index.html` demande l'adresse. L'interface POP n'a aucun accès aux commandes de Forge
//! (`capabilities/default.json` : page locale seulement).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex, time::Duration};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Manager, Url, WebviewWindow,
};
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

/// Résultat pour la page locale : adresse à préremplir et erreur à afficher.
#[derive(Serialize)]
struct Start {
    address: String,
    error: Option<String>,
}

/// Au chargement de la page locale : ouvre POP si l'adresse enregistrée répond, sinon rend l'erreur.
#[tauri::command]
async fn start(app: AppHandle, change: bool) -> Result<Start, String> {
    let Some(server) = load(&app).server else {
        return Ok(Start {
            address: String::new(),
            error: None,
        });
    };
    if change {
        return Ok(Start {
            address: server,
            error: None,
        });
    }
    let u = normalize(&server)?;
    match check(&u).await {
        Ok(()) => {
            main_window(&app)?.navigate(u).map_err(|e| e.to_string())?;
            Ok(Start {
                address: server,
                error: None,
            })
        }
        Err(e) => Ok(Start {
            address: server,
            error: Some(e),
        }),
    }
}

/// « Se connecter » : vérifie l'adresse, l'enregistre et ouvre POP.
#[tauri::command]
async fn connect(app: AppHandle, address: String) -> Result<(), String> {
    let u = normalize(&address)?;
    check(&u).await?;
    let s = u.as_str().trim_end_matches('/').to_string();
    save(&app, &Settings { server: Some(s) })?;
    main_window(&app)?.navigate(u).map_err(|e| e.to_string())
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let change = MenuItem::with_id(
        app,
        "changer",
        "Changer l'adresse de POP…",
        true,
        None::<&str>,
    )?;
    let reload = MenuItem::with_id(app, "recharger", "Recharger", true, Some("F5"))?;
    let quit = PredefinedMenuItem::quit(app, Some("Quitter"))?;
    let forge = Submenu::with_items(
        app,
        "POP Forge",
        true,
        &[
            &change,
            &reload,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    Menu::with_items(app, &[&forge])
}

fn on_menu(app: &AppHandle, id: &str) {
    let Ok(w) = main_window(app) else { return };
    match id {
        "changer" => {
            let home = app.state::<Home>().0.lock().unwrap().clone();
            if let Some(mut h) = home {
                h.set_query(Some("changer=1"));
                let _ = w.navigate(h);
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
        .manage(Home(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![start, connect])
        .menu(menu)
        .on_menu_event(|app, e| on_menu(app, e.id().as_ref()))
        .setup(|app| {
            if let Some(w) = app.get_webview_window("main") {
                *app.state::<Home>().0.lock().unwrap() = w.url().ok();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("POP Forge n'a pas pu démarrer");
}

#[cfg(test)]
mod tests {
    use super::{check, normalize};
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
