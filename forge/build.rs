// Liste explicite des commandes de Forge : chacune a sa permission (allow-…), donnée par les capacités.
// La page locale a toutes les commandes ; la page /forge de POP n'a que celles du relais (main.rs, `autoriser`).
fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "start",
            "connect",
            "projets",
            "etat_relais",
            "relier",
            "importer",
            "maj_verifier",
            "maj_installer",
            "ignorer",
            "projet_ouvert",
            "liaison",
            "fenetre",
            "action_forge",
            "choisir_dossier",
            "capture_jeu",
            "bilan_poste",
            "activer_mcp_auto",
            "outils_ia",
            "lancer_agent",
            "ouvrir_uefn",
            "apps_ia",
            "consigne_mon_ia",
            "ouvrir_ia",
        ]),
    ))
    .expect("échec de la préparation de POP Forge");
}
