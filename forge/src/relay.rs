//! UEFN Link dans Forge (#77) : le relais tourne dans le processus de Forge, avec la même configuration que
//! `pop-link.exe` (`%APPDATA%\pop-link\config.json`). Si un `pop-link.exe` est déjà ouvert (port 47800 pris),
//! Forge ne lance pas un second relais et le dit.
use std::{collections::HashSet, sync::Mutex, time::Duration};
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

/// Nouveaux projets UEFN (#153) : toutes les 30 s, Forge regarde les projets du PC. Un projet ni relié ni ignoré est
/// signalé à POP ; s'il n'y a qu'une map du même nom (dans un studio où l'on peut éditer), POP la relie et le lien
/// démarre à chaud. Sinon rien en silence : l'Accueil de Forge le propose. On ne redemande pas pour le même projet
/// tant que Forge tourne.
pub fn surveiller(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut demandes: HashSet<String> = HashSet::new();
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let actif = matches!(*app.state::<Relay>().etat.lock().unwrap(), Etat::Actif);
            let Ok(cfg) = pop_link::load() else { continue };
            if !actif {
                continue;
            }
            let liens = crate::liens_de(&cfg);
            let Some(jeton) = liens.first().map(|l| l.token.clone()) else {
                continue;
            };
            let appareil = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "POP Forge".into());
            for p in crate::tous_les_projets() {
                let chemin = p.to_string_lossy().to_string();
                let deja = liens
                    .iter()
                    .any(|l| pop_link::meme_dossier(&l.project, &chemin))
                    || cfg
                        .ignores
                        .iter()
                        .any(|i| pop_link::meme_dossier(i, &chemin))
                    || !demandes.insert(chemin.to_lowercase());
                if deja {
                    continue;
                }
                if let Ok(pop_link::Decouverte::Relie(l)) =
                    pop_link::decouvrir(&cfg.server, &jeton, &p, &appareil).await
                {
                    // Relu juste avant d'écrire : une liaison faite entre-temps dans Forge n'est pas perdue.
                    let c = pop_link::fusionner(pop_link::load().ok(), &cfg.server, l.clone());
                    if pop_link::save(&c).is_ok() {
                        if let Some((h, _)) = app.state::<Relay>().handle.lock().unwrap().as_ref() {
                            h.ajouter(&l);
                        }
                    }
                }
            }
        }
    });
}
