//! Mises à jour automatiques de Forge (#79) : module officiel de Tauri. Forge demande au POP auquel il est relié
//! (`/api/forge/latest`) s'il existe une version plus récente ; l'installateur téléchargé n'est installé que si sa
//! signature correspond à la clé publique compilée dans Forge (`tauri.conf.json`).
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
pub struct Nouvelle {
    version: String,
    notes: Option<String>,
}

/// Adresse de vérification : celle du POP enregistré dans Forge.
fn adresse(app: &AppHandle) -> Result<url::Url, String> {
    let serveur = crate::load(app)
        .server
        .ok_or("Forge n'est relié à aucun POP.")?;
    url::Url::parse(&format!(
        "{}/api/forge/latest",
        serveur.trim_end_matches('/')
    ))
    .map_err(|e| format!("Adresse de POP invalide : {e}"))
}

async fn chercher(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    app.updater_builder()
        .endpoints(vec![adresse(app)?])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| format!("Vérification impossible : {e}"))
}

/// Une version plus récente est-elle publiée ? `None` sinon (ou si POP ne répond pas : rien à montrer).
#[tauri::command]
pub async fn maj_verifier(app: AppHandle) -> Option<Nouvelle> {
    chercher(&app).await.ok().flatten().map(|u| Nouvelle {
        version: u.version.clone(),
        notes: u.body.clone(),
    })
}

/// Version installée de Forge (#437), affichée dans Forge et dans le diagnostic.
#[tauri::command]
pub fn forge_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// « Vérifier les mises à jour » (#437) : comme `maj_verifier`, mais distingue « à jour » (`None`) d'une erreur.
#[tauri::command]
pub async fn maj_etat(app: AppHandle) -> Result<Option<Nouvelle>, String> {
    Ok(chercher(&app).await?.map(|u| Nouvelle {
        version: u.version.clone(),
        notes: u.body.clone(),
    }))
}

/// Télécharge, vérifie la signature, installe, puis relance Forge. Une signature qui ne correspond pas est refusée.
#[tauri::command]
pub async fn maj_installer(app: AppHandle) -> Result<(), String> {
    let u = chercher(&app).await?.ok_or("Forge est déjà à jour.")?;
    u.download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| {
            let t = e.to_string();
            if t.to_lowercase().contains("signature") {
                format!("Mise à jour refusée : sa signature ne correspond pas à celle de POP. Forge reste en version {}. ({t})", app.package_info().version)
            } else {
                format!("Mise à jour impossible : {t}")
            }
        })?;
    app.restart()
}
