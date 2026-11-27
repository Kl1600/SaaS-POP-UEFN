//! Lancer l'IA de l'utilisateur sur une tâche POP depuis Forge (#95). POP n'appelle aucune IA : Forge démarre l'outil
//! installé (Claude Code, Codex), avec son abonnement, dans une console à lui. La clé POP de l'agent passe par la
//! variable d'environnement `POP_TOKEN` du processus lancé : aucun fichier ne la contient.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone)]
pub struct Outil {
    pub id: &'static str,
    pub nom: &'static str,
    /// Chemin trouvé sur ce PC.
    pub chemin: Option<String>,
    /// Lançable depuis Forge (sinon `raison` dit pourquoi).
    pub lancable: bool,
    pub raison: Option<String>,
}

/// Cherche un exécutable dans le PATH, puis dans des dossiers connus.
fn trouver(noms: &[&str], autres: &[PathBuf]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(autres.iter().cloned())
        .flat_map(|d| noms.iter().map(move |n| d.join(n)))
        .find(|p| p.is_file())
}

fn env_dir(var: &str, suite: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(|v| PathBuf::from(v).join(suite))
}

/// Outils d'IA en ligne de commande et leur état sur ce PC (Cursor, l'app Claude et ChatGPT : « Mon IA », #420).
pub fn outils() -> Vec<Outil> {
    let npm: Vec<PathBuf> = [env_dir("APPDATA", "npm")].into_iter().flatten().collect();
    let claude = trouver(
        &["claude.exe", "claude.cmd"],
        &[
            npm.clone(),
            [env_dir("USERPROFILE", ".local/bin")]
                .into_iter()
                .flatten()
                .collect(),
        ]
        .concat(),
    );
    let codex = trouver(&["codex.exe", "codex.cmd"], &npm);
    let absent = |n: &str| Some(format!("{n} n'est pas installé sur ce PC."));
    vec![
        Outil {
            id: "claude_code",
            nom: "Claude Code",
            lancable: claude.is_some(),
            raison: if claude.is_none() {
                absent("Claude Code")
            } else {
                None
            },
            chemin: claude.map(|p| p.to_string_lossy().into()),
        },
        Outil {
            id: "codex",
            nom: "Codex",
            lancable: codex.is_some(),
            raison: if codex.is_none() {
                absent("Codex (en ligne de commande)")
            } else {
                None
            },
            chemin: codex.map(|p| p.to_string_lossy().into()),
        },
    ]
}

/// Texte sûr pour la ligne de commande et le presse-papiers : lettres, chiffres et ponctuation simple, 120 au plus.
fn propre(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || " .,:;!?'()-#/".contains(*c))
        .take(120)
        .collect()
}

const ETAPES: &str = "lis-la avec get_task, passe-la en cours, avance en notant ta progression avec save_progress, puis termine-la. Reponds en francais.";

/// Consigne de départ d'un agent lancé par Forge (elle passe par la ligne de commande).
pub fn consigne(map: &str, tache: Option<i64>, titre: &str) -> String {
    match tache {
        Some(tache) => format!(
            "Tu travailles sur POP, map {}. Prends la tache #{tache} ({}) : {ETAPES}",
            propre(map),
            propre(titre)
        ),
        None => format!("Tu travailles sur POP, map {}. {ATTENDRE}", propre(map)),
    }
}

/// Sans tâche (#455) : l'humain donne les instructions depuis le terminal de l'agent dans Forge.
const ATTENDRE: &str = "L'humain va te donner ses instructions depuis Forge : lis tes messages avec read_messages, reponds avec send_message, et relis-les entre deux actions. Reponds en francais.";

/// Consigne pour « Mon IA » (#420) : l'IA est déjà branchée à POP, peut-être sur une autre map ; elle choisit d'abord
/// la map avec use_map.
pub fn consigne_mon_ia(map: &str, map_id: i64, tache: Option<i64>, titre: &str) -> String {
    let debut = format!(
        "Dans POP, choisis la map {} avec use_map({map_id}) si ce n'est pas deja ta map de travail.",
        propre(map)
    );
    match tache {
        Some(tache) => format!(
            "{debut} Prends la tache #{tache} ({}) : {ETAPES}",
            propre(titre)
        ),
        None => format!("{debut} {ATTENDRE}"),
    }
}

/// IA que l'utilisateur a branchée à POP par MCP (#420) : Forge ouvre son app si elle est installée, sinon son site.
#[derive(Serialize, Clone)]
pub struct AppIa {
    pub id: &'static str,
    pub nom: &'static str,
    pub app: Option<String>,
    pub site: Option<&'static str>,
}

pub fn apps_ia() -> Vec<AppIa> {
    let app = |chemins: &[Option<PathBuf>]| {
        chemins
            .iter()
            .flatten()
            .find(|p| p.is_file())
            .map(|p| p.to_string_lossy().into())
    };
    vec![
        AppIa {
            id: "claude_desktop",
            nom: "App Claude",
            app: app(&[env_dir("LOCALAPPDATA", "AnthropicClaude/claude.exe")]),
            site: Some("https://claude.ai/new"),
        },
        AppIa {
            id: "chatgpt",
            nom: "ChatGPT",
            app: app(&[env_dir("LOCALAPPDATA", "Programs/ChatGPT/ChatGPT.exe")]),
            site: Some("https://chatgpt.com/"),
        },
        AppIa {
            id: "cursor",
            nom: "Cursor",
            app: app(&[env_dir("LOCALAPPDATA", "Programs/cursor/Cursor.exe")]),
            site: None,
        },
    ]
}

/// Config MCP de Claude Code : l'en-tête lit la clé dans `POP_TOKEN` (aucune clé dans le fichier).
pub fn config_claude(serveur: &str) -> String {
    serde_json::json!({ "mcpServers": { "pop": {
        "type": "http",
        "url": format!("{}/mcp", serveur.trim_end_matches('/')),
        "headers": { "Authorization": "Bearer ${POP_TOKEN}" }
    } } })
    .to_string()
}

/// Arguments de l'outil pour démarrer sur la tâche.
pub fn arguments(outil: &str, serveur: &str, config: &Path, consigne: &str) -> Option<Vec<String>> {
    let mcp = format!("{}/mcp", serveur.trim_end_matches('/'));
    match outil {
        // La consigne d'abord (#413) : `--mcp-config` accepte plusieurs fichiers et prendrait la consigne pour l'un d'eux.
        "claude_code" => Some(vec![
            consigne.into(),
            "--mcp-config".into(),
            config.to_string_lossy().into(),
        ]),
        // Codex lit `-c clé=valeur` ; une valeur qui n'est pas du TOML est prise telle quelle.
        "codex" => Some(vec![
            "-c".into(),
            format!("mcp_servers.pop.url={mcp}"),
            "-c".into(),
            "mcp_servers.pop.bearer_token_env_var=POP_TOKEN".into(),
            consigne.into(),
        ]),
        _ => None,
    }
}

/// Démarre l'outil dans une nouvelle console Windows, dans le dossier du projet UEFN s'il est connu.
#[cfg(windows)]
pub fn lancer(exe: &Path, args: &[String], dossier: &Path, token: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const NOUVELLE_CONSOLE: u32 = 0x0000_0010;
    // cmd /k "…" garde la console ouverte à la fin ; les guillemets extérieurs sont retirés par cmd.
    let mut ligne = format!("\"{}\"", exe.display());
    for a in args {
        if a.contains('"') {
            return Err("Argument invalide.".into());
        }
        ligne.push_str(&format!(" \"{a}\""));
    }
    std::process::Command::new("cmd")
        .raw_arg(format!("/k \"{ligne}\""))
        .current_dir(dossier)
        .env("POP_TOKEN", token)
        .creation_flags(NOUVELLE_CONSOLE)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Lancement impossible : {e}"))
}

#[cfg(not(windows))]
pub fn lancer(_: &Path, _: &[String], _: &Path, _: &str) -> Result<(), String> {
    Err("Le lancement d'un agent n'est disponible que sous Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consigne_sans_caracteres_dangereux() {
        let c = consigne(
            "Vault\"Rush & co",
            Some(3),
            "Ajouter | le minuteur %PATH% <x>",
        );
        assert!(!c.contains(['"', '&', '|', '%', '<', '>', '^']), "{c}");
        assert!(c.contains("#3"));
    }

    #[test]
    fn consigne_pour_mon_ia() {
        let c = consigne_mon_ia("VaultRush", 12, Some(3), "Ajouter le minuteur");
        assert!(
            c.starts_with("Dans POP, choisis la map VaultRush avec use_map(12)"),
            "{c}"
        );
        assert!(
            c.contains("Prends la tache #3 (Ajouter le minuteur)") && c.contains("get_task"),
            "{c}"
        );
    }

    /// #455 : sans tâche, l'IA attend les instructions de l'humain dans son terminal.
    #[test]
    fn consigne_sans_tache() {
        for c in [
            consigne("VaultRush", None, ""),
            consigne_mon_ia("VaultRush", 12, None, ""),
        ] {
            assert!(
                !c.contains("tache #") && c.contains("read_messages") && c.contains("VaultRush"),
                "{c}"
            );
        }
    }

    #[test]
    fn aucune_cle_dans_la_config() {
        let c = config_claude("https://pop-uefn.com/");
        assert!(c.contains("https://pop-uefn.com/mcp") && c.contains("${POP_TOKEN}"));
        let a = arguments("codex", "https://pop-uefn.com", Path::new("x.json"), "go").unwrap();
        assert!(a.contains(&"mcp_servers.pop.bearer_token_env_var=POP_TOKEN".to_string()));
        assert!(arguments("cursor", "https://x", Path::new("x"), "go").is_none());
        assert!(
            outils().iter().all(|o| o.id != "cursor"),
            "Cursor passe par « Mon IA »"
        );
    }

    #[test]
    fn consigne_avant_mcp_config() {
        // #413 : après `--mcp-config`, Claude Code lisait la consigne comme un second fichier de configuration.
        let a = arguments(
            "claude_code",
            "https://pop-uefn.com",
            Path::new("pop.json"),
            "go",
        )
        .unwrap();
        assert_eq!(a, ["go", "--mcp-config", "pop.json"]);
    }
}
