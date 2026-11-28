//! Vérifications du poste pour l'assistant de premier lancement de Forge (#299) : UEFN installé, serveur MCP qui
//! répond, toolsets et Python, démarrage automatique du serveur MCP. Rien n'est modifié sauf par
//! `activer_demarrage_auto`, qui garde une copie du fichier de réglages d'UEFN.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const EXE: &str = "UnrealEditorFortnite-Win64-Shipping.exe";
/// À afficher quand POP ne trouve pas le réglage dans les fichiers d'UEFN.
pub const REGLAGE_MANUEL: &str = "dans UEFN : Édition → Préférences de l'éditeur → Agentic Workflows → Model Context Protocol, cochez le démarrage automatique (Auto Start Server)";

/// Exécutable d'UEFN, d'après les manifestes de l'Epic Games Launcher, sinon le dossier d'installation par défaut.
/// UEFN y est déclaré sous `Fortnite_Studio` (constaté avec UEFN 43.12, #402, #415) ; `Fortnite` reste en secours.
pub fn uefn_installe() -> Option<PathBuf> {
    let manifestes =
        PathBuf::from(std::env::var("PROGRAMDATA").unwrap_or_else(|_| r"C:\ProgramData".into()))
            .join(r"Epic\EpicGamesLauncher\Data\Manifests");
    let mut candidats: Vec<(u8, PathBuf)> = std::fs::read_dir(&manifestes)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "item"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| exe_du_manifeste(&t))
        .collect();
    candidats.sort_by_key(|c| c.0);
    candidats
        .into_iter()
        .map(|c| c.1)
        .chain(std::iter::once(
            PathBuf::from(r"C:\Program Files\Epic Games\Fortnite\FortniteGame\Binaries\Win64")
                .join(EXE),
        ))
        .find(|p| p.is_file())
}

/// Exécutable d'UEFN d'après un manifeste `.item` du launcher, avec sa priorité (0 : `Fortnite_Studio`).
fn exe_du_manifeste(manifeste: &str) -> Option<(u8, PathBuf)> {
    let v: Value = serde_json::from_str(manifeste).ok()?;
    let rang = match v["AppName"].as_str()? {
        "Fortnite_Studio" => 0,
        "Fortnite" => 1,
        _ => return None,
    };
    let dossier = PathBuf::from(v["InstallLocation"].as_str()?);
    let exe = match v["LaunchExecutable"].as_str() {
        Some(l) if rang == 0 && l.contains("UnrealEditorFortnite") => {
            dossier.join(l.replace('/', "\\"))
        }
        _ => dossier.join(r"FortniteGame\Binaries\Win64").join(EXE),
    };
    Some((rang, exe))
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

/// Démarrage automatique vu par l'assistant (#528). Unreal n'écrit pas un réglage égal à sa valeur par défaut : une
/// clé absente veut dire « actif (par défaut) ». Seul `False` écrit dans un `.ini` le désactive ; et un serveur MCP qui
/// répond prouve qu'il marche.
pub fn demarrage_auto_actif(reglage: Option<bool>, mcp_repond: bool) -> bool {
    mcp_repond || reglage != Some(false)
}

/// UEFN est-il ouvert ? (UE réécrit ses réglages en se fermant : on ne les change que s'il est fermé.)
pub fn uefn_ouvert() -> bool {
    #[cfg(windows)]
    {
        // CSV : en tableau, tasklist coupe les noms à 25 caractères (« UnrealEditorFortnite-Win6 »), et UEFN ouvert
        // passait pour fermé (#410).
        crate::commande("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {EXE}"), "/NH", "/FO", "CSV"])
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

/// Section des réglages d'un projet dans `VK_Projects\<projectId>.ini` (constaté avec UEFN 43.12, #402).
const SECTION_PROJET: &str = "[/Script/ValkyrieEditor.ValkyriePerProjectUserConfig]";

/// `bindings.projectId` du `.uefnproject` d'un dossier de projet UEFN.
pub fn projet_id(projet: &Path) -> Option<String> {
    std::fs::read_dir(projet)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "uefnproject"))
        .find_map(|p| {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
            v["bindings"]["projectId"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

/// Espace Creator propriétaire du projet (#143) : début de `bindings.projectVersePath` du `.uefnproject`
/// (`/klmaps@fortnite.com/VaultRush` → `klmaps@fortnite.com`) ; `None` pour un projet jamais publié.
pub fn espace_creator(projet: &Path) -> Option<String> {
    std::fs::read_dir(projet)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "uefnproject"))
        .find_map(|p| {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
            let chemin = v["bindings"]["projectVersePath"].as_str()?;
            chemin
                .trim_start_matches('/')
                .split('/')
                .next()
                .filter(|e| e.contains('@'))
                .map(str::to_lowercase)
        })
}

/// Identifiant Lore du projet (`.lore/id`), s'il est suivi par Lore (#143).
pub fn lore_id(projet: &Path) -> Option<String> {
    let id = std::fs::read_to_string(projet.join(".lore").join("id")).ok()?;
    let id = id.trim();
    (!id.is_empty() && id.len() <= 100).then(|| id.to_string())
}

/// Date et heure UTC au format d'UEFN : `2026.11.26-15.21.51`.
pub fn horodatage(secs: u64) -> String {
    let (j, s) = ((secs / 86_400) as i64, secs % 86_400);
    // Jours depuis 1970 → date civile (algorithme de Howard Hinnant).
    let z = j + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let jour = doy - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let an = yoe + ere * 400 + i64::from(mois <= 2);
    format!(
        "{an:04}.{mois:02}.{jour:02}-{:02}.{:02}.{:02}",
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

/// Met `LastOpenedTime` à `quand` dans la section du projet ; ajoute la ligne (ou la section) si elle manque.
/// Aucune autre ligne n'est touchée.
pub fn maj_dernier_lancement(ini: &str, quand: &str) -> String {
    let fin = if ini.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lignes: Vec<String> = ini.lines().map(str::to_string).collect();
    let debut = lignes
        .iter()
        .position(|l| l.trim().eq_ignore_ascii_case(SECTION_PROJET));
    let nouvelle = format!("LastOpenedTime={quand}");
    match debut {
        None => {
            if lignes.last().is_some_and(|l| !l.trim().is_empty()) {
                lignes.push(String::new());
            }
            lignes.push(SECTION_PROJET.to_string());
            lignes.push(nouvelle);
        }
        Some(d) => {
            let fin_section = lignes[d + 1..]
                .iter()
                .position(|l| l.trim_start().starts_with('['))
                .map_or(lignes.len(), |i| d + 1 + i);
            match lignes[d + 1..fin_section]
                .iter()
                .position(|l| l.trim_start().starts_with("LastOpenedTime="))
            {
                Some(i) => lignes[d + 1 + i] = nouvelle,
                None => lignes.insert(d + 1, nouvelle),
            }
        }
    }
    lignes.join(fin) + fin
}

/// Ouvre UEFN sur un projet sans passer par le launcher Epic (#402) : le projet est présélectionné dans l'écran de
/// choix d'UEFN (il reste à cliquer « Lancer », aucun moyen de l'éviter n'est connu). Ne fait rien si UEFN est ouvert.
pub fn ouvrir_uefn(projet: &Path) -> Result<String> {
    if uefn_ouvert() {
        bail!("UEFN est déjà ouvert : rien n'a été lancé.");
    }
    let Some(exe) = uefn_installe() else {
        bail!(
            "UEFN n'est pas installé sur ce PC (Epic Games Launcher → Unreal Editor for Fortnite)."
        );
    };
    if !projet.is_dir() {
        bail!("Dossier du projet introuvable : {}.", projet.display());
    }
    let Some(id) = projet_id(projet) else {
        bail!("Ce dossier n'est pas un projet UEFN (aucun fichier .uefnproject avec son identifiant).");
    };
    if id.contains(['/', '\\', '.']) {
        bail!("Identifiant de projet invalide.");
    }
    let dossier = PathBuf::from(std::env::var("LOCALAPPDATA").context("LOCALAPPDATA introuvable")?)
        .join(r"UnrealEditorFortnite\Saved\Config\WindowsEditor\VK_Projects");
    std::fs::create_dir_all(&dossier)?;
    let ini = dossier.join(format!("{id}.ini"));
    let avant = std::fs::read_to_string(&ini).unwrap_or_default();
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&ini, maj_dernier_lancement(&avant, &horodatage(secs)))?;
    std::process::Command::new(&exe)
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .spawn()
        .with_context(|| format!("lancement d'UEFN ({})", exe.display()))?;
    Ok("UEFN démarre avec ce projet présélectionné : cliquez sur « Lancer » dans UEFN.".into())
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
        "demarrage_auto": demarrage_auto_actif(auto.as_ref().map(|a| a.1), mcp),
        // Réglage absent des fichiers : UEFN utilise sa valeur par défaut (activé).
        "demarrage_auto_defaut": auto.is_none(),
        "reglages_uefn": auto.map(|a| a.0.to_string_lossy().to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifeste_uefn() {
        // UEFN 43.12 (#402, #415) : entrée Fortnite_Studio avec LaunchExecutable.
        let studio = r#"{"AppName":"Fortnite_Studio","InstallLocation":"E:\\Fortnite","LaunchExecutable":"FortniteGame/Binaries/Win64/UnrealEditorFortnite-Win64-Shipping.exe"}"#;
        let (rang, exe) = exe_du_manifeste(studio).unwrap();
        assert_eq!(rang, 0);
        assert!(exe.to_string_lossy().starts_with(r"E:\Fortnite"));
        assert!(exe.to_string_lossy().ends_with(EXE));
        let jeu = r#"{"AppName":"Fortnite","InstallLocation":"D:\\Jeux\\Fortnite","LaunchExecutable":"FortniteGame/Binaries/Win64/FortniteLauncher.exe"}"#;
        let (rang, exe) = exe_du_manifeste(jeu).unwrap();
        assert_eq!(rang, 1);
        assert!(
            exe.to_string_lossy().ends_with(EXE),
            "le lanceur du jeu n'est pas UEFN : {exe:?}"
        );
        assert_eq!(
            exe_du_manifeste(r#"{"AppName":"Rocket","InstallLocation":"D:\\R"}"#),
            None
        );
    }

    #[test]
    fn reglage_dans_la_section_mcp() {
        let ini = "[/Script/Autre.Reglages]\nbAutoStart=True\n\n[/Script/AgenticWorkflows.McpServerSettings]\nPort=8000\nbAutoStartServer=False\n";
        assert_eq!(ligne_demarrage_auto(ini), Some((5, false)));
        // #528 : clé absente = valeur par défaut (actif) ; True = actif ; False = à activer, sauf si le serveur répond.
        assert!(demarrage_auto_actif(None, false));
        assert!(demarrage_auto_actif(Some(true), false));
        assert!(!demarrage_auto_actif(Some(false), false));
        assert!(demarrage_auto_actif(Some(false), true));
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

    #[test]
    fn espace_et_lore_du_projet() {
        let d = std::env::temp_dir().join(format!("pop-espace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".lore")).unwrap();
        assert_eq!(espace_creator(&d), None, "pas de .uefnproject");
        std::fs::write(
            d.join("V.uefnproject"),
            r#"{"bindings":{"projectId":"x","projectVersePath":"/KLmaps@fortnite.com/VaultRush"}}"#,
        )
        .unwrap();
        assert_eq!(espace_creator(&d).as_deref(), Some("klmaps@fortnite.com"));
        std::fs::write(
            d.join("V.uefnproject"),
            r#"{"bindings":{"projectVersePath":""}}"#,
        )
        .unwrap();
        assert_eq!(espace_creator(&d), None, "jamais publié");
        assert_eq!(lore_id(&d), None);
        std::fs::write(d.join(".lore").join("id"), "abc-123\n").unwrap();
        assert_eq!(lore_id(&d).as_deref(), Some("abc-123"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn horodatage_uefn() {
        assert_eq!(horodatage(0), "1970.01.01-00.00.00");
        // 26/11/2026 15:21:51 UTC (exemple de #402).
        assert_eq!(horodatage(1_795_706_511), "2026.11.26-15.21.51");
        assert_eq!(horodatage(951_782_400), "2000.02.29-00.00.00");
    }

    #[test]
    fn dernier_lancement_sans_toucher_le_reste() {
        let s = SECTION_PROJET;
        let ini = format!("[Autre]\r\nX=1\r\n{s}\r\nLastOpenedTime=2026.01.01-00.00.00\r\nFavori=True\r\n[Suite]\r\nY=2\r\n");
        let r = maj_dernier_lancement(&ini, "2026.11.26-15.21.51");
        assert_eq!(r, ini.replace("2026.01.01-00.00.00", "2026.11.26-15.21.51"));
        // Ligne absente : ajoutée en tête de la section, le reste intact.
        let r = maj_dernier_lancement(&format!("{s}\nFavori=True\n"), "T");
        assert_eq!(r, format!("{s}\nLastOpenedTime=T\nFavori=True\n"));
        // Fichier vide (projet jamais ouvert) : la seule section.
        assert_eq!(
            maj_dernier_lancement("", "T"),
            format!("{s}\nLastOpenedTime=T\n")
        );
    }

    #[test]
    fn identifiant_du_projet() {
        let d = std::env::temp_dir().join(format!("pop-test-uefn-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("GrassCutter.uefnproject"),
            r#"{"title":"GrassCutter","bindings":{"projectId":"a1b2c3d4e5"}}"#,
        )
        .unwrap();
        assert_eq!(projet_id(&d).as_deref(), Some("a1b2c3d4e5"));
        std::fs::remove_dir_all(&d).unwrap();
        assert_eq!(projet_id(Path::new("/dossier/absent")), None);
    }
}
