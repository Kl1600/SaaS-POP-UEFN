//! UEFN Link dans Forge (#77) : le relais tourne dans le processus de Forge, avec la même configuration que
//! `pop-link.exe` (`%APPDATA%\pop-link\config.json`). Si un `pop-link.exe` est déjà ouvert (port 47800 pris),
//! Forge ne lance pas un second relais et le dit.
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager};

/// Port de la page d'état du relais, le même que `pop-link.exe`.
pub const PORT: u16 = 47800;

#[derive(Clone)]
enum Etat {
    NonRelie,
    Ailleurs,
    Erreur(String),
    Actif,
}

pub struct Relay {
    etat: Mutex<Etat>,
    handle: Mutex<Option<(pop_link::Handle, tokio::task::JoinHandle<()>)>>,
}

impl Default for Relay {
    fn default() -> Self {
        Relay {
            etat: Mutex::new(Etat::NonRelie),
            handle: Mutex::new(None),
        }
    }
}

/// Réserve le port de la page d'état ; quelques essais, le temps qu'un relais arrêté le libère.
async fn reserver() -> Option<std::net::TcpListener> {
    for _ in 0..5 {
        if let Ok(l) = std::net::TcpListener::bind(("127.0.0.1", PORT)) {
            return Some(l);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    None
}

/// (Re)démarre le relais d'après la configuration de pop-link.
pub async fn demarrer(app: &AppHandle) {
    // POP sait que ce relais est celui de Forge, pas un ancien pop-link.exe (#404).
    pop_link::set_app("forge");
    let r = app.state::<Relay>();
    let ancien = r.handle.lock().unwrap().take();
    if let Some((h, page)) = ancien {
        h.stop();
        page.abort();
    }
    let etat = match pop_link::load() {
        Err(_) => Etat::NonRelie,
        Ok(cfg) => match reserver().await {
            None => Etat::Ailleurs,
            Some(l) => {
                // `start` et `serve_status` lancent des tâches tokio : on les appelle dans le runtime de Tauri.
                let res = tauri::async_runtime::spawn(async move {
                    pop_link::start(cfg).map(|h| {
                        let page = h.serve_status(l);
                        (h, page)
                    })
                })
                .await;
                match res {
                    Ok(Ok((h, page))) => {
                        *r.handle.lock().unwrap() = Some((h, page));
                        Etat::Actif
                    }
                    Ok(Err(e)) => Etat::Erreur(format!("{e:#}")),
                    Err(e) => Etat::Erreur(e.to_string()),
                }
            }
        },
    };
    *r.etat.lock().unwrap() = etat;
}

/// État en quelques mots, pour le titre de la fenêtre et la page locale.
pub fn libelle(app: &AppHandle) -> String {
    let r = app.state::<Relay>();
    let etat = r.etat.lock().unwrap().clone();
    match etat {
        Etat::NonRelie => "UEFN non relié".into(),
        Etat::Ailleurs => "relais déjà ouvert dans pop-link.exe".into(),
        Etat::Erreur(e) => format!("relais en erreur : {e}"),
        Etat::Actif => {
            let st = r
                .handle
                .lock()
                .unwrap()
                .as_ref()
                .map(|(h, _)| h.status())
                .unwrap_or_default();
            if st.iter().any(|u| u.uefn == "connecté") {
                "UEFN connecté".into()
            } else if st.iter().all(|u| u.relay != "connecté") {
                "relais hors ligne (POP injoignable)".into()
            } else {
                "UEFN fermé".into()
            }
        }
    }
}

/// Met l'état dans le titre de la fenêtre, toutes les 2 s.
pub fn suivre(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut avant = String::new();
        loop {
            let l = libelle(&app);
            if l != avant {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_title(&format!("POP Forge · {l}"));
                }
                avant = l;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}
