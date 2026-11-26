//! Vérifications du poste pour l'assistant de premier lancement de Forge (#299) : UEFN installé, serveur MCP qui
//! répond, toolsets et Python, démarrage automatique du serveur MCP. Rien n'est modifié sauf par
//! `activer_demarrage_auto`, qui garde une copie du fichier de réglages d'UEFN.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const EXE: &str = "UnrealEditorFortnite-Win64-Shipping.exe";
/// À afficher quand POP ne trouve pas le réglage dans les fichiers d'UEFN.
pub const REGLAGE_MANUEL: &str = "dans UEFN : Édition → Préférences de l'éditeur → Agentic Workflows → Model Context Protocol, cochez le démarrage automatique (Auto Start Server)";

/// Exécutable d'UEFN, d'après les manifestes de l'Epic Games Launcher (installation de Fortnite), sinon le dossier
/// d'installation par défaut.
pub fn uefn_installe() -> Option<PathBuf> {
    let manifestes =
        PathBuf::from(std::env::var("PROGRAMDATA").unwrap_or_else(|_| r"C:\ProgramData".into()))
            .join(r"Epic\EpicGamesLauncher\Data\Manifests");
    let mut dossiers: Vec<PathBuf> = std::fs::read_dir(&manifestes)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "item"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| installation_fortnite(&t))
        .collect();
    dossiers.push(PathBuf::from(r"C:\Program Files\Epic Games\Fortnite"));
    dossiers
        .into_iter()
        .map(|d| d.join(r"FortniteGame\Binaries\Win64").join(EXE))
        .find(|p| p.is_file())
}

/// Dossier d'installation de Fortnite dans un manifeste `.item` du launcher.
fn installation_fortnite(manifeste: &str) -> Option<PathBuf> {
    let v: Value = serde_json::from_str(manifeste).ok()?;
    if v["AppName"].as_str() != Some("Fortnite") {
        return None;
    }
    v["InstallLocation"].as_str().map(PathBuf::from)
}

fn dossier_config() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var("LOCALAPPDATA").ok()?)
        .join(r"UnrealEditorFortnite\Saved\Config");
    d.is_dir().then_some(d)
}

fn fichiers_ini(d: &Path, out: &mut Vec<PathBuf>, prof: u8) {
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() && prof < 3 {
            fichiers_ini(&p, out, prof + 1)
        } else if p.extension().is_some_and(|x| x == "ini") {
            out.push(p)
        }
    }
}

/// Réglage de démarrage automatique dans un `.ini` d'UEFN : la ligne d'une clé « …AutoStart… » dans une section
/// du serveur MCP. Rend (numéro de ligne, valeur).
pub fn ligne_demarrage_auto(ini: &str) -> Option<(usize, bool)> {
    let mut section_mcp = false;
    for (i, l) in ini.lines().enumerate() {
        let t = l.trim();
        if t.starts_with('[') {
            let s = t.to_lowercase();
            section_mcp = s.contains("mcp") || s.contains("modelcontext");
            continue;
        }
        if !section_mcp {
            continue;
        }
        if let Some((k, v)) = t.split_once('=') {
            let k = k.trim().to_lowercase().replace('_', "");
            if k.contains("autostart") {
                return Some((i, v.trim().eq_ignore_ascii_case("true")));
            }
        }
    }
    None
}

/// État du démarrage automatique du serveur MCP : (fichier, activé), ou rien si le réglage n'est dans aucun fichier.
pub fn demarrage_auto() -> Option<(PathBuf, bool)> {
    let mut ini = Vec::new();
    fichiers_ini(&dossier_config()?, &mut ini, 0);
    ini.into_iter().find_map(|p| {
        let t = std::fs::read_to_string(&p).ok()?;
        ligne_demarrage_auto(&t).map(|(_, on)| (p, on))
    })
}

/// UEFN est-il ouvert ? (UE réécrit ses réglages en se fermant : on ne les change que s'il est fermé.)
pub fn uefn_ouvert() -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {EXE}"), "/NH"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(EXE))
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    false
}

/// Active le démarrage automatique du serveur MCP (accord de l'utilisateur dans Forge). UEFN doit être fermé.
/// Copie le fichier en `….ini.pop-sauvegarde` avant de le changer.
pub fn activer_demarrage_auto() -> Result<PathBuf> {
    if uefn_ouvert() {
        bail!("Fermez UEFN, puis recommencez : UEFN réécrit ses réglages en se fermant.");
    }
    let Some((p, on)) = demarrage_auto() else {
        bail!("Réglage introuvable dans les fichiers d'UEFN. À faire une fois {REGLAGE_MANUEL}.");
    };
    if on {
        return Ok(p);
    }
    let t = std::fs::read_to_string(&p)?;
    let (n, _) = ligne_demarrage_auto(&t).context("réglage disparu")?;
    std::fs::copy(&p, p.with_extension("ini.pop-sauvegarde")).context("copie de sauvegarde")?;
    let fin = if t.contains("\r\n") { "\r\n" } else { "\n" };
    let lignes: Vec<String> = t
        .lines()
        .enumerate()
        .map(|(i, l)| {
            if i == n {
                format!("{}=True", l.split_once('=').map(|x| x.0).unwrap_or(l))
            } else {
                l.to_string()
            }
        })
        .collect();
    std::fs::write(&p, lignes.join(fin) + fin)?;
    Ok(p)
}

/// Bilan pour l'assistant : chaque vérification avec son résultat (`null` : pas vérifiable maintenant).
pub async fn bilan(projet: Option<&Path>, mcp_url: Option<&str>) -> Value {
    let uefn = uefn_installe();
    let auto = demarrage_auto();
    let racine = projet
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let url = crate::mcpdriver::find_url(mcp_url, &racine);
    let mut m = crate::mcpdriver::Mcp::new(url, Default::default());
    let (mcp, toolsets, python) = match m.connect_with(true).await {
        Ok(()) => {
            let n = m.toolsets.len();
            let py = m.list_actors().await.is_ok();
            (true, Some(n), Some(py))
        }
        Err(_) => (false, None, None),
    };
    json!({
        "windows": cfg!(windows),
        "uefn": uefn.map(|p| p.to_string_lossy().to_string()),
        "uefn_ouvert": uefn_ouvert() || mcp,
        "mcp": mcp,
        "toolsets": toolsets,
        "python": python,
        "demarrage_auto": auto.as_ref().map(|a| a.1),
        "reglages_uefn": auto.map(|a| a.0.to_string_lossy().to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifeste_fortnite() {
        let m = r#"{"AppName":"Fortnite","InstallLocation":"D:\\Jeux\\Fortnite"}"#;
        assert_eq!(
            installation_fortnite(m),
            Some(PathBuf::from(r"D:\Jeux\Fortnite"))
        );
        assert_eq!(
            installation_fortnite(r#"{"AppName":"Rocket","InstallLocation":"D:\\R"}"#),
            None
        );
    }

    #[test]
    fn reglage_dans_la_section_mcp() {
        let ini = "[/Script/Autre.Reglages]\nbAutoStart=True\n\n[/Script/AgenticWorkflows.McpServerSettings]\nPort=8000\nbAutoStartServer=False\n";
        assert_eq!(ligne_demarrage_auto(ini), Some((5, false)));
        assert_eq!(
            ligne_demarrage_auto("[/Script/X]\nbAutoStartServer=True\n"),
            None,
            "hors section MCP"
        );
        assert_eq!(
            ligne_demarrage_auto("[/Script/ModelContextProtocol.Settings]\nAuto_Start=true\n"),
            Some((1, true))
        );
    }
}
