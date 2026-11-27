//! UEFN Link : relais local entre UEFN (sur le PC du créateur) et POP.
//!
//! Bibliothèque utilisée par `pop-link.exe` (`main.rs`, ligne de commande) et par POP Forge, qui fait tourner le
//! relais dans son propre processus : [`start`] démarre le relais et rend une [`Handle`] (état, journal, arrêt).
//!
//! `pop-link.exe` (téléchargeable sur le site) est recompilé sous Windows par le workflow « relais Windows » (#358).
mod digest;
mod files;
mod mcpdriver;
pub mod poste;

use anyhow::{anyhow, bail, Context, Result};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Config {
    pub server: String,
    pub token: String,
    pub project: String,
    pub map: String,
    #[serde(default)]
    pub digests_dir: Option<String>,
    #[serde(default)]
    pub uefn_mcp_url: Option<String>,
    /// Commande d'archivage Lore, ex. « lore commit -m {message} » (le message reste un seul argument).
    #[serde(default)]
    pub lore_cmd: Option<String>,
    /// Noms d'outils MCP imposés si la détection automatique se trompe : python, compile, push.
    #[serde(default)]
    pub tools: HashMap<String, String>,
    /// Liaison simplifiée : un projet UEFN = une map POP, chacun avec son jeton.
    #[serde(default)]
    pub links: Vec<ProjLink>,
    /// Projets que la personne a choisi d'ignorer (« Ignorer » dans Forge, #153) : jamais reliés ni proposés.
    #[serde(default)]
    pub ignores: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ProjLink {
    pub project: String,
    pub map: String,
    pub token: String,
}

/// Adresse de POP et code intégrés à la fin de pop-link.exe quand il est téléchargé depuis POP.
pub fn embedded() -> Option<Value> {
    let exe = std::fs::read(std::env::current_exe().ok()?).ok()?;
    let tail = &exe[exe.len().saturating_sub(4096)..];
    let txt = String::from_utf8_lossy(tail);
    let i = txt.rfind("#POPLINK#")?;
    let j = txt[i..].find("#END#")? + i;
    serde_json::from_str(&txt[i + 9..j]).ok()
}

/// Projet ouvert dans UEFN, partagé entre les projets reliés (vérifié toutes les 15 s).
#[derive(Default)]
struct OpenCache {
    at: Option<std::time::Instant>,
    open: Option<String>,
}
type Open = Arc<tokio::sync::Mutex<OpenCache>>;

fn newest_mtime(dir: &std::path::Path, depth: usize) -> std::time::SystemTime {
    let mut best = std::fs::metadata(dir)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH);
    if depth == 0 {
        return best;
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if ["Intermediate", "DerivedDataCache", ".git", "Binaries"].contains(&n.as_str()) {
                continue;
            }
            let p = e.path();
            let t = if p.is_dir() {
                newest_mtime(&p, depth - 1)
            } else {
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH)
            };
            if t > best {
                best = t
            }
        }
    }
    best
}

pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("POP_LINK_CONFIG") {
        return PathBuf::from(p);
    }
    let base = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support"))
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|_| PathBuf::from("."))
    };
    base.join("pop-link").join("config.json")
}

pub fn load() -> Result<Config> {
    let p = config_path();
    let s = std::fs::read_to_string(&p).with_context(|| format!("Pas encore appairé ({}). Lancez d'abord : pop-link pair <adresse POP> <code> <dossier du projet>", p.display()))?;
    Ok(serde_json::from_str(&s)?)
}

pub fn save(c: &Config) -> Result<()> {
    let p = config_path();
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, serde_json::to_string_pretty(c)?)?;
    Ok(())
}

// ---------- État partagé (page locale, Forge) ----------

/// État d'un projet relié, tel que l'affichent la page locale et POP Forge.
#[derive(Default, Clone, Serialize)]
pub struct Ui {
    /// Liaison avec POP : « connexion… », « connecté », « hors ligne ».
    pub relay: String,
    /// UEFN : « ? », « connecté », « fermé ».
    pub uefn: String,
    pub project: String,
    pub map: String,
    pub mcp_url: String,
    /// Le plus récent d'abord, 100 au plus : (heure, action, résultat).
    pub journal: Vec<(String, String, String)>,
    pub tools: Vec<String>,
}

type Shared = Arc<Mutex<Ui>>;
/// États des projets reliés, partagés avec la page locale : la liste grandit quand un projet est relié à chaud (#153).
type Uis = Arc<Mutex<Vec<Shared>>>;
/// Dossiers des projets reliés, partagés par toutes les liaisons (pour savoir lequel est ouvert dans UEFN).
type Projets = Arc<Mutex<Vec<String>>>;

fn log(ui: &Shared, action: &str, res: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hms = format!(
        "{:02}:{:02}:{:02}",
        (now / 3600 + 1) % 24,
        now / 60 % 60,
        now % 60
    );
    let mut u = ui.lock().unwrap();
    u.journal.insert(
        0,
        (hms, action.to_string(), res.chars().take(200).collect()),
    );
    u.journal.truncate(100);
    println!("[{}] {action} → {res}", u.journal[0].0);
}

// ---------- Pilote : le vrai UEFN via son serveur MCP ----------

/// Script Python qui affiche les dossiers du projet ouvert dans UEFN et le chemin du monde (nom de la map).
const OPEN_PROJECT_PY: &str = "import unreal\nprint(unreal.Paths.convert_relative_path_to_full(unreal.Paths.project_dir()))\nprint(unreal.Paths.convert_relative_path_to_full(unreal.Paths.project_plugins_dir()))\ntry:\n    print(unreal.EditorLevelLibrary.get_editor_world().get_path_name())\nexcept Exception: pass";

/// Parmi `projects`, celui qui est ouvert dans UEFN (serveur MCP démarré), sinon `None`.
pub async fn open_project(projects: &[PathBuf]) -> Option<PathBuf> {
    let first = projects.first()?;
    let mut m = mcpdriver::Mcp::new(mcpdriver::find_url(None, first), HashMap::new());
    if !m.alive().await {
        return None;
    }
    let out = m.python(OPEN_PROJECT_PY).await.ok()?.to_lowercase();
    projects
        .iter()
        .find(|p| {
            p.file_name()
                .map(|n| out.contains(&n.to_string_lossy().to_lowercase()))
                .unwrap_or(false)
        })
        .cloned()
}

struct Link {
    open: Open,
    all_projects: Projets,
    cfg: Config,
    mcp: mcpdriver::Mcp,
    scan: files::Scanner,
    uefn: bool,
    last_actors: String,
    ui: Shared,
}

impl Link {
    /// UEFN est « connecté » pour ce projet seulement si c'est le projet ouvert dans l'éditeur.
    async fn uefn_here(&mut self) -> bool {
        if !self.mcp.alive().await {
            return false;
        }
        let all = self.all_projects.lock().unwrap().clone();
        if all.len() <= 1 {
            return true;
        }
        let mut c = self.open.lock().await;
        if c.at
            .map(|t| t.elapsed() > Duration::from_secs(15))
            .unwrap_or(true)
        {
            let names: Vec<String> = all
                .iter()
                .map(|p| {
                    PathBuf::from(p)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_lowercase())
                        .unwrap_or_default()
                })
                .collect();
            let out = self
                .mcp
                .python(OPEN_PROJECT_PY)
                .await
                .unwrap_or_default()
                .to_lowercase();
            let by_python = all
                .iter()
                .zip(&names)
                .find(|(_, n)| !n.is_empty() && out.contains(n.as_str()))
                .map(|(p, _)| p.clone());
            // Sinon : le projet modifié le plus récemment (UEFN écrit dans le projet ouvert).
            c.open = by_python.or_else(|| {
                all.iter()
                    .max_by_key(|p| newest_mtime(&PathBuf::from(p), 3))
                    .cloned()
            });
            c.at = Some(std::time::Instant::now());
        }
        c.open.as_deref() == Some(self.cfg.project.as_str())
    }

    fn need_uefn(&self) -> Result<()> {
        if !self.uefn {
            bail!("UEFN est fermé ou son serveur MCP n'est pas démarré.");
        }
        Ok(())
    }

    async fn exec(&mut self, action: &str, a: &Value) -> Result<Value> {
        let ids: Vec<String> = a["ids"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect();
        let mut out = json!({});
        match action {
            "write_file" => self.scan.write(
                a["path"].as_str().unwrap_or(""),
                a["content"]
                    .as_str()
                    .ok_or_else(|| anyhow!("contenu manquant"))?,
            )?,
            "delete_file" => self.scan.delete(a["path"].as_str().unwrap_or(""))?,
            "rename_file" => self.scan.rename(
                a["path"].as_str().unwrap_or(""),
                a["to"].as_str().unwrap_or(""),
            )?,
            "actor_update" | "actor_update_raw" | "actor_organize" => {
                self.need_uefn()?;
                self.mcp.patch(&a["patches"]).await?;
                out["actors"] = json!(self.mcp.list_actors().await?);
            }
            "actor_spawn" => {
                self.need_uefn()?;
                out["created_ids"] = json!([self.mcp.spawn(a).await?]);
                out["actors"] = json!(self.mcp.list_actors().await?);
            }

            "actor_blockout" | "actor_scatter" => {
                // Blockout (#116) et dispersion (#118) : tout ou rien ; si un élément échoue, ceux déjà posés sont retirés.
                self.need_uefn()?;
                let mut ids = vec![];
                for it in a["items"].as_array().cloned().unwrap_or_default() {
                    match self.mcp.spawn(&it).await {
                        Ok(id) => ids.push(id),
                        Err(e) => {
                            if !ids.is_empty() {
                                let _ = self.mcp.delete(&ids).await;
                            }
                            let quoi = if action == "actor_blockout" {
                                "Blockout"
                            } else {
                                "Dispersion"
                            };
                            bail!(
                                "{quoi} arrêté(e) à l'élément {} : {e:#} (rien n'est resté posé)",
                                ids.len() + 1
                            );
                        }
                    }
                }
                out["created_ids"] = json!(ids);
                out["actors"] = json!(self.mcp.list_actors().await?);
            }
            "mesh_spawn" => {
                self.need_uefn()?;
                out["created_ids"] = json!([self.mcp.spawn_mesh(a).await?]);
                out["actors"] = json!(self.mcp.list_actors().await?);
            }
            "island_settings" => {
                self.need_uefn()?;
                let (id, list) = self.mcp.island_settings().await?;
                out["id"] = json!(id);
                out["settings"] = list;
            }
            "asset_import" => {
                self.need_uefn()?;
                let (file, temp) = fichier_a_importer(&self.cfg.server, a).await?;
                let r = self
                    .mcp
                    .import(
                        &file.to_string_lossy(),
                        a["folder"].as_str().unwrap_or(""),
                        &self.cfg.project,
                    )
                    .await;
                if temp {
                    std::fs::remove_file(&file).ok();
                }
                out["imported"] = json!(r?);
            }
            "asset_delete" => {
                self.need_uefn()?;
                let paths: Vec<String> = a["paths"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect();
                self.mcp.delete_assets(&paths).await?;
            }
            "actor_select" => {
                self.need_uefn()?;
                out["selected"] = json!(self.mcp.select(&ids).await?);
            }
            "actor_duplicate" => {
                self.need_uefn()?;
                out["created_ids"] = json!(self.mcp.duplicate(&ids).await?);
                out["actors"] = json!(self.mcp.list_actors().await?);
            }
            "actor_delete" => {
                self.need_uefn()?;
                // La sauvegarde repart vers POP, qui la garde dans l'action d'annulation.
                out["saved"] = json!(self.mcp.delete(&ids).await?);
                out["actors"] = json!(self.mcp.list_actors().await?);
            }
            "actor_restore" => {
                self.need_uefn()?;
                let expected = a["actors"].as_array().map(|x| x.len()).unwrap_or(ids.len());
                out["created_ids"] = json!(
                    self.mcp
                        .restore(a["saved"].as_str().unwrap_or(""), expected)
                        .await?
                );
                out["actors"] = json!(self.mcp.list_actors().await?);
            }
            "actor_props" => {
                self.need_uefn()?;
                out["props"] = self
                    .mcp
                    .device_props(
                        a["id"].as_str().unwrap_or(""),
                        a["name"].as_str().unwrap_or(""),
                    )
                    .await?;
            }
            "device_catalog" => {
                self.need_uefn()?;
                out["catalog"] = self.mcp.device_catalog().await?;
            }
            "refresh_actors" => {
                self.need_uefn()?;
                out["actors"] = json!(self.mcp.list_actors().await?);
            }
            "compile" => {
                self.need_uefn()?;
                let mut d = self.mcp.compile().await?;
                // Chemins absolus d'UEFN → chemins relatifs au projet, comme dans POP.
                let files: Vec<String> = self.scan.read_all().into_iter().map(|(p, _)| p).collect();
                let root = self.cfg.project.replace('\\', "/").to_lowercase();
                for x in d.iter_mut() {
                    let raw = x["raw_path"]
                        .as_str()
                        .unwrap_or(x["path"].as_str().unwrap_or(""))
                        .replace('\\', "/");
                    let low = raw.to_lowercase();
                    let rel = if low.starts_with(&root) {
                        raw[root.len()..].trim_start_matches('/').to_string()
                    } else {
                        raw.clone()
                    };
                    let best = files
                        .iter()
                        .find(|f| f.eq_ignore_ascii_case(&rel))
                        .or_else(|| {
                            files
                                .iter()
                                .filter(|f| low.ends_with(&f.to_lowercase()))
                                .max_by_key(|f| f.len())
                        })
                        .or_else(|| {
                            files
                                .iter()
                                .find(|f| f.rsplit('/').next() == raw.rsplit('/').next())
                        });
                    x["path"] = json!(best.cloned().unwrap_or(rel));
                }
                out["diagnostics"] = json!(d);
            }
            "push_session" => {
                self.need_uefn()?;
                self.mcp
                    .push_session(a["verse_only"].as_bool().unwrap_or(false))
                    .await?;
            }
            "start_session" | "stop_session" => {
                self.need_uefn()?;
                let role = if action == "start_session" {
                    "session_start"
                } else {
                    "session_stop"
                };
                out["result"] = self.mcp.session(role, json!({})).await?;
            }
            "session_output" => {
                self.need_uefn()?;
                // GetVerseOutput { pattern, maxResults, cursor } : Print et erreurs Verse de la session.
                let mut q =
                    json!({ "maxResults": a["max_results"].as_u64().unwrap_or(100).min(500) });
                if let Some(p) = a["pattern"].as_str().filter(|p| !p.is_empty()) {
                    q["pattern"] = json!(p);
                }
                if !a["cursor"].is_null() {
                    q["cursor"] = a["cursor"].clone();
                }
                out["output"] = self.mcp.session("session_output", q).await?;
            }
            "save_level" => {
                self.need_uefn()?;
                self.mcp.save_level().await?;
            }
            "lore_checkin" => {
                let msg = a["message"].as_str().unwrap_or("").to_string();
                // Sans commande imposée : archivage par UEFN lui-même (enregistre tout, puis check-in).
                let Some(cmd) = &self.cfg.lore_cmd else {
                    self.need_uefn()?;
                    if !self.mcp.can_checkin() {
                        bail!("Archivage impossible : UEFN n'expose pas CheckIn. Ajoutez « lore_cmd » dans {} (ex. \"lore commit -m {{message}}\").", config_path().display());
                    }
                    self.mcp.checkin(&msg).await?;
                    return Ok(out);
                };
                let parts: Vec<String> = cmd
                    .split_whitespace()
                    .map(|w| {
                        if w == "{message}" {
                            msg.clone()
                        } else {
                            w.to_string()
                        }
                    })
                    .collect();
                let st = commande(&parts[0])
                    .args(&parts[1..])
                    .current_dir(&self.cfg.project)
                    .output()?;
                if !st.status.success() {
                    bail!("Lore : {}", String::from_utf8_lossy(&st.stderr));
                }
            }
            "capture_island" => {
                // Photo de l'île (#306) : envoyée directement au stockage objet par le lien signé du serveur POP.
                self.need_uefn()?;
                let url = a["put_url"].as_str().unwrap_or("");
                if !url.starts_with("https://") {
                    bail!("lien d'envoi de la capture invalide");
                }
                let (w, h) = (
                    a["width"].as_u64().unwrap_or(1920) as u32,
                    a["height"].as_u64().unwrap_or(1080) as u32,
                );
                let png = self
                    .mcp
                    .capture(&a["location"], &a["rotation"], w, h)
                    .await?;
                let n = png.len();
                let r = reqwest::Client::new()
                    .put(url)
                    .header("content-type", "image/png")
                    .body(png)
                    .send()
                    .await?;
                if !r.status().is_success() {
                    bail!(
                        "envoi de la capture refusé par le stockage ({})",
                        r.status()
                    );
                }
                out["bytes"] = json!(n);
                out["width"] = json!(w);
                out["height"] = json!(h);
            }
            "render_topview" => {
                self.need_uefn()?;
                let (img, b) = self.mcp.topview().await?;
                out["image"] = json!(img);
                out["bounds"] = b;
            }
            _ => bail!("action inconnue : {action}"),
        }
        Ok(out)
    }
}

/// Version du protocole entre le relais et le serveur (#158). À augmenter, avec `RELAY_PROTO` dans
/// `server/src/uefn.rs`, chaque fois que le relais apprend une action : un relais plus ancien est alors signalé
/// « périmé » dans POP. 1 = relais d'avant #158, qui n'annonce rien.
pub const PROTO: u32 = 2;

/// Commande externe lancée sans fenêtre (#410) : depuis POP Forge (application fenêtrée), Windows ouvrirait sinon une
/// console noire à chaque appel, ce qui ressemble à un virus.
pub fn commande(prog: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(prog);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Application qui fait tourner le relais, annoncée au serveur dans `hello` (#404) : « pop-link » par défaut,
/// « forge » quand POP Forge l'a dit avec `set_app`. POP sait ainsi qu'un ancien pop-link.exe est encore utilisé.
static APP: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

/// À appeler une fois, avant `start`, par l'application qui embarque le relais (POP Forge).
pub fn set_app(name: &'static str) {
    let _ = APP.set(name);
}

pub fn app_name() -> &'static str {
    APP.get().copied().unwrap_or("pop-link")
}

/// Types de fichiers qu'on importe dans UEFN (#90).
pub const IMPORT_EXT: &[&str] = &[
    "fbx", "obj", "png", "jpg", "jpeg", "tga", "wav", "ogg", "mp3",
];
const IMPORT_MAX: u64 = 200 * 1024 * 1024;

/// Fichier à importer : `local_path` (fichier de ce PC) ou `url` du serveur POP lui-même (jamais d'un autre site),
/// téléchargé dans un dossier temporaire. Rend (chemin, fichier temporaire à effacer après).
async fn fichier_a_importer(server: &str, a: &Value) -> Result<(PathBuf, bool)> {
    let ext_ok = |n: &str| {
        let e = n.rsplit('.').next().unwrap_or("").to_lowercase();
        n.contains('.') && IMPORT_EXT.contains(&e.as_str())
    };
    if let Some(p) = a["local_path"].as_str().filter(|p| !p.is_empty()) {
        let path = PathBuf::from(p);
        if !ext_ok(p) {
            bail!(
                "Type de fichier non importé ({p}) : {}.",
                IMPORT_EXT.join(", ")
            );
        }
        let m = std::fs::metadata(&path).with_context(|| format!("Fichier introuvable : {p}"))?;
        if !m.is_file() || m.len() > IMPORT_MAX {
            bail!("Fichier absent ou trop gros (200 Mo au plus) : {p}");
        }
        return Ok((path, false));
    }
    let url = a["url"].as_str().unwrap_or("");
    let base = server.trim_end_matches('/');
    if url.is_empty() || !url.starts_with(&format!("{base}/")) {
        bail!("Import refusé : le fichier doit venir de ce PC ou du serveur POP ({base}).");
    }
    let name = a["filename"]
        .as_str()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| url.rsplit('/').next().unwrap_or(""));
    let name: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || "._- ".contains(*c))
        .collect();
    if !ext_ok(&name) {
        bail!(
            "Type de fichier non importé ({name}) : {}.",
            IMPORT_EXT.join(", ")
        );
    }
    let r = reqwest::get(url).await.context("POP injoignable")?;
    if !r.status().is_success() {
        bail!("Téléchargement refusé par POP ({}).", r.status());
    }
    if r.content_length().unwrap_or(0) > IMPORT_MAX {
        bail!("Fichier trop gros (200 Mo au plus).");
    }
    let bytes = r.bytes().await?;
    if bytes.len() as u64 > IMPORT_MAX {
        bail!("Fichier trop gros (200 Mo au plus).");
    }
    let dir = std::env::temp_dir().join("pop-link-import");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    std::fs::write(&path, &bytes)?;
    Ok((path, true))
}

// ---------- Boucle principale ----------

/// Relais démarré par [`start`] : un état par projet relié, l'ajout d'un projet à chaud (#153) et l'arrêt.
pub struct Handle {
    uis: Uis,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    base: Config,
    open: Open,
    projets: Projets,
}

impl Handle {
    /// Copie de l'état de chaque projet relié (relais, UEFN, journal…).
    pub fn status(&self) -> Vec<Ui> {
        self.uis
            .lock()
            .unwrap()
            .iter()
            .map(|u| u.lock().unwrap().clone())
            .collect()
    }

    /// Dossiers des projets reliés par ce relais.
    pub fn projets(&self) -> Vec<String> {
        self.projets.lock().unwrap().clone()
    }

    /// Relie un projet de plus sans redémarrer le relais (#153). Faux si son dossier n'existe pas ou s'il est déjà
    /// relié (même dossier).
    pub fn ajouter(&self, l: &ProjLink) -> bool {
        let root = PathBuf::from(&l.project);
        if !root.is_dir() || self.projets().iter().any(|p| meme_dossier(p, &l.project)) {
            return false;
        }
        self.projets.lock().unwrap().push(l.project.clone());
        let mut c = self.base.clone();
        c.project = l.project.clone();
        c.map = l.map.clone();
        c.token = l.token.clone();
        let mcp_url = mcpdriver::find_url(c.uefn_mcp_url.as_deref(), &root);
        let ui: Shared = Arc::new(Mutex::new(Ui {
            relay: "connexion…".into(),
            uefn: "?".into(),
            project: c.project.clone(),
            map: c.map.clone(),
            mcp_url: mcp_url.clone(),
            ..Default::default()
        }));
        self.uis.lock().unwrap().push(ui.clone());
        println!("  · {} → map « {} »", c.project, c.map);
        let mut link = Link {
            open: self.open.clone(),
            all_projects: self.projets.clone(),
            scan: files::Scanner::new(root),
            mcp: mcpdriver::Mcp::new(mcp_url, c.tools.clone()),
            cfg: c,
            uefn: false,
            last_actors: String::new(),
            ui,
        };
        self.tasks.lock().unwrap().push(tokio::spawn(async move {
            let mut backoff = 1;
            loop {
                match session(&mut link).await {
                    Ok(()) => backoff = 1,
                    Err(e) => {
                        link.ui.lock().unwrap().relay = "hors ligne".into();
                        log(&link.ui, "connexion", &format!("{e:#}"));
                    }
                }
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(30);
            }
        }));
        true
    }

    /// Arrête le relais : coupe la liaison avec POP et ne pilote plus UEFN.
    pub fn stop(&self) {
        for t in self.tasks.lock().unwrap().iter() {
            t.abort();
        }
    }

    /// Sert la page d'état locale sur un port déjà réservé par l'appelant (POP Forge le réserve avant de
    /// démarrer, pour ne pas lancer un second relais à côté d'un `pop-link.exe` ouvert).
    pub fn serve_status(&self, l: std::net::TcpListener) -> tokio::task::JoinHandle<()> {
        let uis = self.uis.clone();
        tokio::spawn(async move {
            l.set_nonblocking(true).ok();
            if let Ok(l) = tokio::net::TcpListener::from_std(l) {
                let _ = axum::serve(l, status_router(uis)).await;
            }
        })
    }

    /// Vrai quand plus aucune liaison ne tourne (après [`Handle::stop`]).
    pub fn stopped(&self) -> bool {
        self.tasks.lock().unwrap().iter().all(|t| t.is_finished())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Même dossier de projet, sans tenir compte de la casse ni du sens des barres (Windows).
pub fn meme_dossier(a: &str, b: &str) -> bool {
    let n = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    n(a) == n(b)
}

/// Démarre le relais pour chaque projet de la configuration (à appeler dans un runtime tokio).
/// Ne sert pas la page d'état locale : c'est le rôle de [`run`] (ligne de commande).
pub fn start(cfg: Config) -> Result<Handle> {
    // Liste des projets reliés : liaison simplifiée (plusieurs) ou appairage classique (un seul).
    let links: Vec<ProjLink> = if cfg.links.is_empty() {
        vec![ProjLink {
            project: cfg.project.clone(),
            map: cfg.map.clone(),
            token: cfg.token.clone(),
        }]
    } else {
        cfg.links.clone()
    };
    println!(
        "UEFN Link · {} projet(s) relié(s) à {}",
        links.len(),
        cfg.server
    );
    let h = Handle {
        uis: Default::default(),
        tasks: Default::default(),
        base: cfg.clone(),
        open: Default::default(),
        projets: Default::default(),
    };
    for l in &links {
        if !h.ajouter(l) && !PathBuf::from(&l.project).is_dir() {
            println!("  ! dossier introuvable, ignoré : {}", l.project);
        }
    }
    if h.uis.lock().unwrap().is_empty() {
        bail!(
            "Aucun projet UEFN valide dans la configuration ({}).",
            config_path().display()
        );
    }
    Ok(h)
}

/// `pop-link run` : relais et page d'état locale, jusqu'à la fermeture de la fenêtre.
pub async fn run(port: u16) -> Result<()> {
    let h = start(load()?)?;
    println!("Page d'état : http://127.0.0.1:{port}  (laissez cette fenêtre ouverte)");
    local_page(h.uis.clone(), port).await;
    futures::future::pending::<()>().await;
    Ok(())
}

async fn session(link: &mut Link) -> Result<()> {
    let ws_url = link
        .cfg
        .server
        .replacen("http", "ws", 1)
        .trim_end_matches('/')
        .to_string()
        + "/api/relay/ws";
    let mut req = ws_url.into_client_request()?;
    req.headers_mut().insert(
        "authorization",
        format!("Bearer {}", link.cfg.token).parse()?,
    );
    let (ws, _) = tokio_tungstenite::connect_async(req)
        .await
        .context("POP injoignable")?;
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
    let writer = tokio::spawn(async move {
        while let Some(v) = rx.recv().await {
            if sink
                .send(Message::Text(v.to_string().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });
    link.ui.lock().unwrap().relay = "connecté".into();
    log(&link.ui, "connexion", "relié à POP");
    link.uefn = link.uefn_here().await;
    link.ui.lock().unwrap().uefn = if link.uefn {
        "connecté".into()
    } else {
        "fermé".into()
    };
    link.ui.lock().unwrap().tools = link
        .mcp
        .tools
        .iter()
        .filter_map(|t| t["name"].as_str().map(String::from))
        .collect();
    tx.send(json!({ "t": "tools", "url": link.mcp.url.clone(), "tools": link.mcp.tools.iter().map(|t| json!({ "name": t["name"], "description": t["description"] })).collect::<Vec<_>>(), "roles": link.mcp.roles() })).ok();
    let project_name = PathBuf::from(&link.cfg.project)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // `proto` (#158) : le serveur sait si ce relais connaît ses actions, sinon POP affiche « relais périmé ».
    tx.send(json!({ "t": "hello", "project": project_name, "uefn": if link.uefn { "connected" } else { "closed" }, "proto": PROTO, "app": app_name() }))?;
    sync(link, &tx, true).await;
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    let mut n: u64 = 0;
    loop {
        tokio::select! {
            msg = stream.next() => {
                let Some(msg) = msg else { break };
                let Message::Text(t) = msg? else { continue };
                let v: Value = serde_json::from_str(&t)?;
                if v["t"] == "cmd" {
                    let action = v["action"].as_str().unwrap_or("").to_string();
                    let r = link.exec(&action, &v["args"]).await;
                    let reply = match &r {
                        Ok(d) => json!({ "t": "result", "id": v["id"], "ok": true, "data": d }),
                        Err(e) => json!({ "t": "result", "id": v["id"], "ok": false, "error": format!("{e:#}") }),
                    };
                    log(&link.ui, &action, &match &r { Ok(_) => "ok".to_string(), Err(e) => format!("erreur : {e:#}") });
                    if let Some(a) = reply["data"]["actors"].as_array() { link.last_actors = json!(a).to_string(); }
                    tx.send(reply)?;
                    if action.ends_with("_file") { sync(link, &tx, false).await; }
                }
            }
            _ = tick.tick() => {
                n += 1;
                if n % 2 == 0 {
                    let alive = link.uefn_here().await;
                    if alive != link.uefn {
                        link.uefn = alive;
                        link.ui.lock().unwrap().uefn = if alive { "connecté".into() } else { "fermé".into() };
                        log(&link.ui, "UEFN", if alive { "connecté" } else { "fermé" });
                        tx.send(json!({ "t": "status", "uefn": if alive { "connected" } else { "closed" } }))?;
                        if alive {
                            link.last_actors.clear();
                            link.ui.lock().unwrap().tools = link.mcp.tools.iter().filter_map(|t| t["name"].as_str().map(String::from)).collect();
                            tx.send(json!({ "t": "tools", "url": link.mcp.url.clone(), "tools": link.mcp.tools.iter().map(|t| json!({ "name": t["name"], "description": t["description"] })).collect::<Vec<_>>(), "roles": link.mcp.roles() })).ok();
                        }
                    }
                }
                sync(link, &tx, false).await;
                if link.uefn && (n % 10 == 0 || link.last_actors.is_empty()) {
                    match link.mcp.list_actors().await {
                        Ok(a) => {
                        let s = json!(a).to_string();
                        if s != link.last_actors { link.last_actors = s; tx.send(json!({ "t": "actors", "actors": a }))?; }
                        }
                        Err(e) => if link.last_actors.is_empty() { log(&link.ui, "acteurs", &format!("lecture impossible : {e:#}")); link.last_actors = "[]".into(); },
                    }
                }
            }
        }
    }
    writer.abort();
    bail!("connexion fermée par POP")
}

/// Envoie à POP les fichiers .verse modifiés, les digests et l'état des verrous.
async fn sync(link: &mut Link, tx: &mpsc::UnboundedSender<Value>, full: bool) {
    let changed = link.scan.changed(full);
    if full || !changed.is_empty() {
        let all = full || changed.len() > 1 && changed.len() == link.scan.read_all().len();
        tx.send(json!({ "t": "files", "full": all, "files": changed.iter().map(|(p, c)| json!({"path": p, "content": c})).collect::<Vec<_>>() })).ok();
        if !full {
            log(
                &link.ui,
                "fichiers",
                &format!("{} fichier(s) envoyés", changed.len()),
            );
        }
    }
    // Digests générés par UEFN : %LOCALAPPDATA%\UnrealEditorFortnite\Saved\VerseProject\FortniteGame
    let extra = link
        .cfg
        .digests_dir
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("LOCALAPPDATA")
                .ok()
                .map(|l| {
                    PathBuf::from(l)
                        .join("UnrealEditorFortnite")
                        .join("Saved")
                        .join("VerseProject")
                        .join("FortniteGame")
                })
                .filter(|p| p.is_dir())
        });
    if let Some(d) = link.scan.digests(extra.as_deref()) {
        let mut syms = vec![];
        for (name, c) in &d {
            digest::parse(c, name, &mut syms);
        }
        let tags = digest::tags(&link.scan.read_all());
        log(
            &link.ui,
            "digests",
            &format!("{} symboles, {} tags", syms.len(), tags.len()),
        );
        tx.send(json!({ "t": "digests", "symbols": syms, "tags": tags }))
            .ok();
    } else if !changed.is_empty() {
        tx.send(json!({ "t": "digests_tags", "tags": digest::tags(&link.scan.read_all()) }))
            .ok();
    }
}

// ---------- Page d'état locale ----------

fn status_router(uis: Uis) -> axum::Router {
    use axum::{routing::get, Router};
    Router::new().route(
        "/",
        get(move || {
            let list: Vec<Ui> = uis
                .lock()
                .unwrap()
                .iter()
                .map(|u| u.lock().unwrap().clone())
                .collect();
            async move { axum::response::Html(list.iter().map(page).collect::<Vec<_>>().join("")) }
        }),
    )
}

async fn local_page(uis: Uis, port: u16) {
    match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => {
            let _ = axum::serve(l, status_router(uis)).await;
        }
        Err(e) => println!(
            "Page d'état indisponible ({e}) : un autre pop-link est peut-être déjà ouvert."
        ),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn page(u: &Ui) -> String {
    let rows: String = u
        .journal
        .iter()
        .map(|(h, a, r)| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(h),
                esc(a),
                esc(r)
            )
        })
        .collect();
    let uefn = if u.uefn.is_empty() { "?" } else { &u.uefn };
    let tools = if u.tools.is_empty() {
        "<span class=muted>aucun (UEFN fermé ou serveur MCP arrêté)</span>".to_string()
    } else {
        u.tools
            .iter()
            .map(|t| format!("<code>{}</code>", esc(t)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!(
        r#"<!doctype html><html lang="fr"><meta charset="utf-8"><meta http-equiv="refresh" content="3"><title>UEFN Link</title>
<style>body{{font-family:system-ui,sans-serif;background:#000;color:#fafafa;margin:0;padding:32px}}.card{{background:#0f0f0f;border:1px solid #222;border-radius:16px;padding:20px;max-width:900px;margin-bottom:16px}}
h1{{font-weight:500;margin:0 0 8px}}.muted{{color:#a3a3a3}}table{{width:100%;border-collapse:collapse;font-size:14px}}td{{padding:6px;border-top:1px solid #222;vertical-align:top}}code{{background:#1a1a1a;border-radius:6px;padding:1px 6px;margin:2px;display:inline-block;font-size:12px}}</style>
<div class="card"><h1>UEFN Link</h1><p class="muted">Projet : {}<br>Map POP : {}<br>UEFN MCP : {}</p>
<p>Relais : <b>{}</b> · UEFN : <b>{}</b></p><p class="muted" style="margin-bottom:4px">Outils UEFN détectés :</p><div>{}</div></div>
<div class="card"><h2 style="font-weight:500;margin-top:0">Journal</h2><table>{}</table></div>"#,
        esc(&u.project),
        esc(&u.map),
        esc(&u.mcp_url),
        esc(&u.relay),
        esc(uefn),
        tools,
        rows
    )
}

// ---------- Appairage et diagnostic (ligne de commande, Forge) ----------

pub async fn pair(server: &str, code: &str, project: &str, name: &str) -> Result<()> {
    let server = server.trim_end_matches('/').to_string();
    let r: Value = reqwest::Client::new()
        .post(format!("{server}/api/relay/pair"))
        .json(&json!({ "code": code, "name": name }))
        .send()
        .await?
        .json()
        .await?;
    let token = r["token"]
        .as_str()
        .ok_or_else(|| anyhow!("{}", r["error"].as_str().unwrap_or("appairage refusé")))?;
    let project = std::path::absolute(project)?.to_string_lossy().to_string();
    let map = r["map"].as_str().unwrap_or("").to_string();
    // #153 : on ajoute ce projet aux autres au lieu d'effacer la configuration, après l'avoir sauvegardée.
    let avant = load().ok();
    if avant.is_some() {
        let p = config_path();
        std::fs::copy(&p, p.with_extension("json.avant-pair.bak"))
            .context("Sauvegarde de la configuration impossible")?;
    }
    save(&fusionner(
        avant,
        &server,
        ProjLink {
            project: project.clone(),
            map: map.clone(),
            token: token.into(),
        },
    ))?;
    println!(
        "Appairé à la map « {map} ». Projet : {project}
Configuration : {}
Lancez maintenant : pop-link run",
        config_path().display()
    );
    Ok(())
}

/// Configuration après l'ajout d'un lien (#153) : les autres projets reliés au même serveur sont gardés ; le lien du
/// même projet ou de la même map est remplacé. Un autre serveur repart d'une configuration neuve.
pub fn fusionner(avant: Option<Config>, server: &str, nouveau: ProjLink) -> Config {
    let meme_serveur = |c: &Config| c.server.trim_end_matches('/') == server.trim_end_matches('/');
    let mut c = match avant {
        Some(c) if meme_serveur(&c) => c,
        _ => Config {
            server: server.to_string(),
            ..Default::default()
        },
    };
    let mut liens: Vec<ProjLink> = if c.links.is_empty() && !c.project.is_empty() {
        vec![ProjLink {
            project: c.project.clone(),
            map: c.map.clone(),
            token: c.token.clone(),
        }]
    } else {
        c.links.clone()
    };
    liens.retain(|l| !meme_dossier(&l.project, &nouveau.project) && l.map != nouveau.map);
    c.ignores.retain(|p| !meme_dossier(p, &nouveau.project));
    c.project = nouveau.project.clone();
    c.map = nouveau.map.clone();
    c.token = nouveau.token.clone();
    liens.push(nouveau);
    c.links = liens;
    c
}

/// Réponse de POP à un projet découvert sur le PC (#153).
pub enum Decouverte {
    /// Une seule map du même nom, où la personne peut éditer : POP a créé le lien.
    Relie(ProjLink),
    /// Aucune map, ou plusieurs : rien n'est relié, Forge demande à la personne.
    ADemander,
}

/// Signale à POP un nouveau projet du PC (`/api/relay/discover`), avec le jeton d'un projet déjà relié (qui
/// identifie la personne).
pub async fn decouvrir(
    server: &str,
    token: &str,
    projet: &Path,
    appareil: &str,
) -> Result<Decouverte> {
    let nom = projet
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let r: Value = reqwest::Client::new()
        .post(format!(
            "{}/api/relay/discover",
            server.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .json(&json!({ "project": nom, "device": appareil }))
        .send()
        .await
        .context("POP injoignable")?
        .json()
        .await?;
    Ok(match (r["map"].as_str(), r["token"].as_str()) {
        (Some(map), Some(t)) => Decouverte::Relie(ProjLink {
            project: std::path::absolute(projet)?.to_string_lossy().to_string(),
            map: map.into(),
            token: t.into(),
        }),
        _ => Decouverte::ADemander,
    })
}

pub fn ask(q: &str) -> String {
    use std::io::Write;
    print!("{q} ");
    std::io::stdout().flush().ok();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    s.trim().trim_matches('"').to_string()
}

/// Projets UEFN du dossier « Documents/Fortnite Projects ».
pub fn find_projects() -> Vec<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let mut out = vec![];
    for base in [
        PathBuf::from(&home)
            .join("Documents")
            .join("Fortnite Projects"),
        PathBuf::from(&home)
            .join("OneDrive")
            .join("Documents")
            .join("Fortnite Projects"),
    ] {
        if let Ok(rd) = std::fs::read_dir(&base) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir()
                    && std::fs::read_dir(&p)
                        .map(|r| {
                            r.flatten().any(|f| {
                                f.path()
                                    .extension()
                                    .map(|x| x == "uefnproject")
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false)
                {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// pop-link.exe téléchargé depuis POP : tout est automatique (adresse et code intégrés).
pub async fn auto_pair(emb: &Value) -> Result<()> {
    let server = emb["server"]
        .as_str()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    let code = emb["code"].as_str().unwrap_or("").to_string();
    let projects = find_projects();
    let projects = if projects.is_empty() {
        println!("Aucun projet trouvé dans Documents\\Fortnite Projects.");
        vec![PathBuf::from(ask(
            "Dossier du projet UEFN (glissez le dossier ici) :",
        ))]
    } else {
        projects
    };
    println!("Projets UEFN trouvés :");
    for p in &projects {
        println!("  · {}", p.display())
    }
    let device = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC UEFN".into());
    let links = pair_studio(&server, &code, &projects, &device).await?;
    let n = links.len();
    let first = links
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("aucun projet relié"))?;
    save(&Config {
        server,
        token: first.token,
        project: first.project,
        map: first.map,
        links,
        ..Default::default()
    })?;
    println!("\nC'est relié : {n} projet(s) synchronisé(s) avec POP.\n");
    Ok(())
}

/// Relie plusieurs projets d'un coup à un studio avec un code de studio (`/api/relay/pair-studio`) : POP crée la
/// map de chaque projet qui n'en a pas (même nom) et rend un jeton par projet. Utilisé par `auto_pair` et par Forge
/// (« Créer les maps et relier », #404).
pub async fn pair_studio(
    server: &str,
    code: &str,
    projects: &[PathBuf],
    device: &str,
) -> Result<Vec<ProjLink>> {
    let server = server.trim_end_matches('/');
    let body = json!({ "code": code.trim(), "device": device, "projects": projects.iter().map(|p| json!({ "name": p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), "creator_space": poste::espace_creator(p), "lore_id": poste::lore_id(p) })).collect::<Vec<_>>() });
    let r: Value = reqwest::Client::new()
        .post(format!("{server}/api/relay/pair-studio"))
        .json(&body)
        .send()
        .await
        .context("POP injoignable")?
        .json()
        .await?;
    let list = r["links"]
        .as_array()
        .ok_or_else(|| anyhow!("{}", r["error"].as_str().unwrap_or("liaison refusée")))?;
    let mut links = vec![];
    // Par nom de projet, pas par ordre (#153) : POP saute un projet dont plusieurs maps portent le nom.
    for l in list {
        let nom = l["project"].as_str().unwrap_or("");
        let Some(p) = projects.iter().find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(nom))
        }) else {
            continue;
        };
        links.push(ProjLink {
            project: std::path::absolute(p)?.to_string_lossy().to_string(),
            map: nom.into(),
            token: l["token"].as_str().unwrap_or("").into(),
        });
    }
    Ok(links)
}

/// Premier lancement par double-clic : questions simples dans la console.
pub async fn guided_pair() -> Result<()> {
    println!("UEFN Link · première configuration\n");
    let server =
        ask("Adresse de POP (ex. https://pop.mon-studio.fr ou http://192.168.1.20:8080) :");
    let code = ask("Code d'appairage (POP → Organiseur → « Obtenir un code ») :");
    let projects = find_projects();
    let project = if projects.is_empty() {
        ask("Dossier du projet UEFN (glissez le dossier ici) :")
    } else {
        for (i, p) in projects.iter().enumerate() {
            println!("  {}. {}", i + 1, p.display())
        }
        let c = ask("Numéro du projet (ou chemin complet) :");
        c.parse::<usize>()
            .ok()
            .and_then(|n| projects.get(n.wrapping_sub(1)))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(c)
    };
    let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC UEFN".into());
    pair(&server, &code, &project, &name).await
}

pub async fn diagnostic() -> Result<()> {
    let cfg = load().ok();
    let root = cfg
        .as_ref()
        .map(|c| PathBuf::from(&c.project))
        .unwrap_or_else(|| PathBuf::from("."));
    let url = mcpdriver::find_url(cfg.as_ref().and_then(|c| c.uefn_mcp_url.as_deref()), &root);
    println!("Serveur MCP d'UEFN : {url}");
    let mut m = mcpdriver::Mcp::new(
        url,
        cfg.as_ref().map(|c| c.tools.clone()).unwrap_or_default(),
    );
    m.connect_with(true).await.context("UEFN injoignable : ouvrez le projet dans UEFN, puis Édition → Préférences de l'éditeur → Agentic Workflows → Model Context Protocol → Start Server")?;
    if !m.toolsets.is_empty() {
        println!("{} toolsets :", m.toolsets.len());
        for (n, d) in &m.toolsets {
            println!("  · {n} : {}", d.chars().take(90).collect::<String>())
        }
        println!();
    }
    println!("{} outils :", m.tools.len());
    let one_line = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(110)
            .collect::<String>()
    };
    for t in &m.tools {
        println!(
            "  - {} : {}",
            t["name"].as_str().unwrap_or(""),
            one_line(t["description"].as_str().unwrap_or(""))
        )
    }
    println!("\nRôles choisis : {}", m.roles());
    match m.list_actors().await {
        Ok(a) => println!("Python OK : {} acteurs dans le niveau ouvert.", a.len()),
        Err(e) => println!("Python : échec ({e:#}). Activez « Enable Project Python Scripts » dans les paramètres du projet."),
    }
    let out = std::env::temp_dir().join("pop-link-diagnostic.json");
    std::fs::write(&out, serde_json::to_string_pretty(&m.tools)?)?;
    println!("\nDétail complet des outils : {}", out.display());
    Ok(())
}

pub fn pause() {
    if cfg!(windows) {
        ask("\nAppuyez sur Entrée pour fermer.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(project: &str) -> Config {
        Config {
            // Port 1 : aucun POP n'y répond, la liaison échoue tout de suite.
            server: "http://127.0.0.1:1".into(),
            token: "pop_test".into(),
            project: project.into(),
            map: "Map de test".into(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn start_status_stop() {
        let dir = std::env::temp_dir().join(format!("pop-link-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let h = start(cfg(&dir.to_string_lossy())).unwrap();
        let st = h.status();
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].map, "Map de test");
        // POP injoignable : le relais passe « hors ligne » et le note dans le journal.
        let mut ok = false;
        for _ in 0..50 {
            let s = &h.status()[0];
            if s.relay == "hors ligne" && s.journal.iter().any(|(_, a, _)| a == "connexion") {
                ok = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            ok,
            "le relais devrait être hors ligne avec une ligne de journal"
        );
        assert!(!h.stopped());
        h.stop();
        for _ in 0..50 {
            if h.stopped() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(h.stopped());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn import_sur() {
        let pop = "http://127.0.0.1:8080";
        // Un autre site : refusé, rien n'est téléchargé.
        let e = fichier_a_importer(pop, &json!({ "url": "https://exemple.com/x.fbx" })).await;
        assert!(format!("{:#}", e.unwrap_err()).contains("ce PC ou du serveur POP"));
        let e = fichier_a_importer(
            pop,
            &json!({ "url": "http://127.0.0.1:8080.exemple.com/x.fbx" }),
        )
        .await;
        assert!(e.is_err());
        // Type non prévu : refusé.
        let e = fichier_a_importer(pop, &json!({ "local_path": "C:/x/script.exe" })).await;
        assert!(format!("{:#}", e.unwrap_err()).contains("Type de fichier"));
        // Fichier local valide : accepté tel quel (pas de copie temporaire).
        let f = std::env::temp_dir().join(format!("pop-import-{}.png", std::process::id()));
        std::fs::write(&f, b"png").unwrap();
        let (p, temp) = fichier_a_importer(pop, &json!({ "local_path": f.to_string_lossy() }))
            .await
            .unwrap();
        assert_eq!(p, f);
        assert!(!temp);
        std::fs::remove_file(&f).ok();
    }

    #[tokio::test]
    async fn start_refuse_sans_projet() {
        let e = start(cfg("/dossier/qui/n/existe/pas")).err().unwrap();
        assert!(format!("{e}").contains("Aucun projet UEFN valide"));
    }

    fn lien(p: &str, m: &str) -> ProjLink {
        ProjLink {
            project: p.into(),
            map: m.into(),
            token: format!("jeton-{m}"),
        }
    }

    /// #153 : `pop-link pair` ajoute un projet sans effacer les autres ; même projet ou même map → remplacé.
    #[test]
    fn fusionner_sans_perte() {
        let mut avant = cfg("C:/P/A");
        avant.server = "https://pop.test/".into();
        avant.links = (0..10)
            .map(|i| lien(&format!("C:/P/P{i}"), &format!("M{i}")))
            .collect();
        avant.ignores = vec![r"C:\P\Neuf".into()];
        let c = fusionner(
            Some(avant.clone()),
            "https://pop.test",
            lien("C:/P/Neuf", "Neuf"),
        );
        assert_eq!(c.links.len(), 11, "un projet de plus, les 10 autres gardés");
        assert!(c.ignores.is_empty(), "un projet relié n'est plus ignoré");
        let c = fusionner(Some(c), "https://pop.test", lien(r"c:\p\neuf\", "Neuf2"));
        assert_eq!(c.links.len(), 11, "même dossier : remplacé");
        assert_eq!(c.links.last().unwrap().map, "Neuf2");
        let c = fusionner(Some(avant), "https://autre.test", lien("C:/P/X", "X"));
        assert_eq!(c.links.len(), 1, "autre serveur : configuration neuve");
    }

    /// #153 : un projet relié à chaud apparaît dans l'état du relais, sans redémarrer ; pas deux fois le même.
    #[tokio::test]
    async fn ajouter_a_chaud() {
        let d = std::env::temp_dir().join(format!("pop-chaud-{}", std::process::id()));
        let (a, b) = (d.join("A"), d.join("B"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let h = start(cfg(&a.to_string_lossy())).unwrap();
        assert_eq!(h.status().len(), 1);
        assert!(h.ajouter(&lien(&b.to_string_lossy(), "B")));
        assert_eq!(h.status().len(), 2);
        assert_eq!(h.projets().len(), 2);
        assert!(!h.ajouter(&lien(&b.to_string_lossy(), "B")), "déjà relié");
        assert!(
            !h.ajouter(&lien(&d.join("absent").to_string_lossy(), "X")),
            "dossier absent"
        );
        h.stop();
        let _ = std::fs::remove_dir_all(&d);
    }
}
