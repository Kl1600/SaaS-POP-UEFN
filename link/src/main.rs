//! UEFN Link : relais local entre UEFN (sur le PC du créateur) et POP.
//!
//!   pop-link                      (double-clic : appairage guidé la première fois, puis connexion)
//!   pop-link pair <adresse POP> <code> <dossier du projet UEFN> [--name "PC de Yanis"]
//!   pop-link run [--port 47800]
//!   pop-link diagnostic           (vérifie UEFN MCP et liste ses outils)
mod digest;
mod files;
mod mcpdriver;

use anyhow::{anyhow, bail, Context, Result};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

#[derive(Serialize, Deserialize, Clone, Default)]
struct Config {
    server: String,
    token: String,
    project: String,
    map: String,
    #[serde(default)]
    digests_dir: Option<String>,
    #[serde(default)]
    uefn_mcp_url: Option<String>,
    /// Commande d'archivage Lore, ex. « lore commit -m {message} » (le message reste un seul argument).
    #[serde(default)]
    lore_cmd: Option<String>,
    /// Noms d'outils MCP imposés si la détection automatique se trompe : python, compile, push.
    #[serde(default)]
    tools: HashMap<String, String>,
    /// Liaison simplifiée : un projet UEFN = une map POP, chacun avec son jeton.
    #[serde(default)]
    links: Vec<ProjLink>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct ProjLink {
    project: String,
    map: String,
    token: String,
}

/// Adresse de POP et code intégrés à la fin de pop-link.exe quand il est téléchargé depuis POP.
fn embedded() -> Option<Value> {
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
    let mut best = std::fs::metadata(dir).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
    if depth == 0 { return best }
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if ["Intermediate", "DerivedDataCache", ".git", "Binaries"].contains(&n.as_str()) { continue }
            let p = e.path();
            let t = if p.is_dir() { newest_mtime(&p, depth - 1) } else { e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH) };
            if t > best { best = t }
        }
    }
    best
}

fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("POP_LINK_CONFIG") {
        return PathBuf::from(p);
    }
    let base = if cfg!(windows) {
        std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME").map(|h| PathBuf::from(h).join("Library/Application Support")).unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config"))).unwrap_or_else(|_| PathBuf::from("."))
    };
    base.join("pop-link").join("config.json")
}

fn load() -> Result<Config> {
    let p = config_path();
    let s = std::fs::read_to_string(&p).with_context(|| format!("Pas encore appairé ({}). Lancez d'abord : pop-link pair <adresse POP> <code> <dossier du projet>", p.display()))?;
    Ok(serde_json::from_str(&s)?)
}

fn save(c: &Config) -> Result<()> {
    let p = config_path();
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, serde_json::to_string_pretty(c)?)?;
    Ok(())
}

// ---------- État partagé (page locale) ----------

#[derive(Default, Clone, Serialize)]
struct Ui {
    relay: String,
    uefn: String,
    project: String,
    map: String,
    mcp_url: String,
    journal: Vec<(String, String, String)>, // (heure, action, résultat)
    tools: Vec<String>,
}

type Shared = Arc<Mutex<Ui>>;

fn log(ui: &Shared, action: &str, res: &str) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let hms = format!("{:02}:{:02}:{:02}", (now / 3600 + 1) % 24, now / 60 % 60, now % 60);
    let mut u = ui.lock().unwrap();
    u.journal.insert(0, (hms, action.to_string(), res.chars().take(200).collect()));
    u.journal.truncate(100);
    println!("[{}] {action} → {res}", u.journal[0].0);
}

// ---------- Pilote : le vrai UEFN via son serveur MCP ----------

struct Link {
    open: Open,
    all_projects: Vec<String>,
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
        if !self.mcp.alive().await { return false }
        if self.all_projects.len() <= 1 { return true }
        let mut c = self.open.lock().await;
        if c.at.map(|t| t.elapsed() > Duration::from_secs(15)).unwrap_or(true) {
            let names: Vec<String> = self.all_projects.iter().map(|p| PathBuf::from(p).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()).collect();
            let out = self.mcp.python("import unreal\nprint(unreal.Paths.convert_relative_path_to_full(unreal.Paths.project_dir()))\nprint(unreal.Paths.convert_relative_path_to_full(unreal.Paths.project_plugins_dir()))\ntry:\n    print(unreal.EditorLevelLibrary.get_editor_world().get_path_name())\nexcept Exception: pass").await.unwrap_or_default().to_lowercase();
            let by_python = self.all_projects.iter().zip(&names).find(|(_, n)| !n.is_empty() && out.contains(n.as_str())).map(|(p, _)| p.clone());
            // Sinon : le projet modifié le plus récemment (UEFN écrit dans le projet ouvert).
            c.open = by_python.or_else(|| self.all_projects.iter().max_by_key(|p| newest_mtime(&PathBuf::from(p), 3)).cloned());
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
        let ids: Vec<String> = a["ids"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_str().map(String::from)).collect();
        let mut out = json!({});
        match action {
            "write_file" => self.scan.write(a["path"].as_str().unwrap_or(""), a["content"].as_str().ok_or_else(|| anyhow!("contenu manquant"))?)?,
            "delete_file" => self.scan.delete(a["path"].as_str().unwrap_or(""))?,
            "rename_file" => self.scan.rename(a["path"].as_str().unwrap_or(""), a["to"].as_str().unwrap_or(""))?,
            "actor_update" | "actor_update_raw" => {
                self.need_uefn()?;
                self.mcp.patch(&a["patches"]).await?;
                out["actors"] = json!(self.mcp.list_actors().await?);
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
                out["created_ids"] = json!(self.mcp.restore(a["saved"].as_str().unwrap_or(""), expected).await?);
                out["actors"] = json!(self.mcp.list_actors().await?);
            }
            "actor_props" => {
                self.need_uefn()?;
                out["props"] = self.mcp.device_props(a["id"].as_str().unwrap_or(""), a["name"].as_str().unwrap_or("")).await?;
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
                    let raw = x["raw_path"].as_str().unwrap_or(x["path"].as_str().unwrap_or("")).replace('\\', "/");
                    let low = raw.to_lowercase();
                    let rel = if low.starts_with(&root) { raw[root.len()..].trim_start_matches('/').to_string() } else { raw.clone() };
                    let best = files.iter().find(|f| f.eq_ignore_ascii_case(&rel)).or_else(|| files.iter().filter(|f| low.ends_with(&f.to_lowercase())).max_by_key(|f| f.len()))
                        .or_else(|| files.iter().find(|f| f.rsplit('/').next() == raw.rsplit('/').next()));
                    x["path"] = json!(best.cloned().unwrap_or(rel));
                }
                out["diagnostics"] = json!(d);
            }
            "push_session" => {
                self.need_uefn()?;
                self.mcp.push_session(a["verse_only"].as_bool().unwrap_or(false)).await?;
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
                let parts: Vec<String> = cmd.split_whitespace().map(|w| if w == "{message}" { msg.clone() } else { w.to_string() }).collect();
                let st = std::process::Command::new(&parts[0]).args(&parts[1..]).current_dir(&self.cfg.project).output()?;
                if !st.status.success() {
                    bail!("Lore : {}", String::from_utf8_lossy(&st.stderr));
                }
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

// ---------- Boucle principale ----------

async fn run(port: u16) -> Result<()> {
    let cfg = load()?;
    // Liste des projets reliés : liaison simplifiée (plusieurs) ou appairage classique (un seul).
    let links: Vec<ProjLink> = if cfg.links.is_empty() { vec![ProjLink { project: cfg.project.clone(), map: cfg.map.clone(), token: cfg.token.clone() }] } else { cfg.links.clone() };
    let all: Vec<String> = links.iter().map(|l| l.project.clone()).collect();
    let open: Open = Default::default();
    let mut uis = vec![];
    println!("UEFN Link · {} projet(s) relié(s) à {}", links.len(), cfg.server);
    for l in &links {
        let root = PathBuf::from(&l.project);
        if !root.is_dir() {
            println!("  ! dossier introuvable, ignoré : {}", root.display());
            continue;
        }
        let mut c = cfg.clone();
        c.project = l.project.clone();
        c.map = l.map.clone();
        c.token = l.token.clone();
        let mcp_url = mcpdriver::find_url(c.uefn_mcp_url.as_deref(), &root);
        let ui: Shared = Arc::new(Mutex::new(Ui { relay: "connexion…".into(), uefn: "?".into(), project: c.project.clone(), map: c.map.clone(), mcp_url: mcp_url.clone(), ..Default::default() }));
        uis.push(ui.clone());
        println!("  · {} → map « {} »", c.project, c.map);
        let mut link = Link { open: open.clone(), all_projects: all.clone(), scan: files::Scanner::new(root), mcp: mcpdriver::Mcp::new(mcp_url, c.tools.clone()), cfg: c, uefn: false, last_actors: String::new(), ui: ui.clone() };
        tokio::spawn(async move {
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
        });
    }
    if uis.is_empty() {
        bail!("Aucun projet UEFN valide dans la configuration ({}).", config_path().display());
    }
    println!("Page d'état : http://127.0.0.1:{port}  (laissez cette fenêtre ouverte)");
    local_page(uis, port).await;
    futures::future::pending::<()>().await;
    Ok(())
}

async fn session(link: &mut Link) -> Result<()> {
    let ws_url = link.cfg.server.replacen("http", "ws", 1).trim_end_matches('/').to_string() + "/api/relay/ws";
    let mut req = ws_url.into_client_request()?;
    req.headers_mut().insert("authorization", format!("Bearer {}", link.cfg.token).parse()?);
    let (ws, _) = tokio_tungstenite::connect_async(req).await.context("POP injoignable")?;
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
    let writer = tokio::spawn(async move {
        while let Some(v) = rx.recv().await {
            if sink.send(Message::Text(v.to_string().into())).await.is_err() {
                break;
            }
        }
    });
    link.ui.lock().unwrap().relay = "connecté".into();
    log(&link.ui, "connexion", "relié à POP");
    link.uefn = link.uefn_here().await;
    link.ui.lock().unwrap().uefn = if link.uefn { "connecté".into() } else { "fermé".into() };
    link.ui.lock().unwrap().tools = link.mcp.tools.iter().filter_map(|t| t["name"].as_str().map(String::from)).collect();
    tx.send(json!({ "t": "tools", "url": link.mcp.url.clone(), "tools": link.mcp.tools.iter().map(|t| json!({ "name": t["name"], "description": t["description"] })).collect::<Vec<_>>(), "roles": link.mcp.roles() })).ok();
    let project_name = PathBuf::from(&link.cfg.project).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    tx.send(json!({ "t": "hello", "project": project_name, "uefn": if link.uefn { "connected" } else { "closed" } }))?;
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
            log(&link.ui, "fichiers", &format!("{} fichier(s) envoyés", changed.len()));
        }
    }
    // Digests générés par UEFN : %LOCALAPPDATA%\UnrealEditorFortnite\Saved\VerseProject\FortniteGame
    let extra = link.cfg.digests_dir.as_ref().map(PathBuf::from).or_else(|| std::env::var("LOCALAPPDATA").ok().map(|l| PathBuf::from(l).join("UnrealEditorFortnite").join("Saved").join("VerseProject").join("FortniteGame")).filter(|p| p.is_dir()));
    if let Some(d) = link.scan.digests(extra.as_deref()) {
        let mut syms = vec![];
        for (name, c) in &d {
            digest::parse(c, name, &mut syms);
        }
        let tags = digest::tags(&link.scan.read_all());
        log(&link.ui, "digests", &format!("{} symboles, {} tags", syms.len(), tags.len()));
        tx.send(json!({ "t": "digests", "symbols": syms, "tags": tags })).ok();
    } else if !changed.is_empty() {
        tx.send(json!({ "t": "digests_tags", "tags": digest::tags(&link.scan.read_all()) })).ok();
    }
}

// ---------- Page d'état locale ----------

async fn local_page(uis: Vec<Shared>, port: u16) {
    use axum::{routing::get, Router};
    let app = Router::new().route("/", get(move || {
        let list: Vec<Ui> = uis.iter().map(|u| u.lock().unwrap().clone()).collect();
        async move { axum::response::Html(list.iter().map(page).collect::<Vec<_>>().join("")) }
    }));
    match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => { let _ = axum::serve(l, app).await; }
        Err(e) => println!("Page d'état indisponible ({e}) : un autre pop-link est peut-être déjà ouvert."),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn page(u: &Ui) -> String {
    let rows: String = u.journal.iter().map(|(h, a, r)| format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", esc(h), esc(a), esc(r))).collect();
    let uefn = if u.uefn.is_empty() { "?" } else { &u.uefn };
    let tools = if u.tools.is_empty() { "<span class=muted>aucun (UEFN fermé ou serveur MCP arrêté)</span>".to_string() } else { u.tools.iter().map(|t| format!("<code>{}</code>", esc(t))).collect::<Vec<_>>().join(" ") };
    format!(r#"<!doctype html><html lang="fr"><meta charset="utf-8"><meta http-equiv="refresh" content="3"><title>UEFN Link</title>
<style>body{{font-family:system-ui,sans-serif;background:#000;color:#fafafa;margin:0;padding:32px}}.card{{background:#0f0f0f;border:1px solid #222;border-radius:16px;padding:20px;max-width:900px;margin-bottom:16px}}
h1{{font-weight:500;margin:0 0 8px}}.muted{{color:#a3a3a3}}table{{width:100%;border-collapse:collapse;font-size:14px}}td{{padding:6px;border-top:1px solid #222;vertical-align:top}}code{{background:#1a1a1a;border-radius:6px;padding:1px 6px;margin:2px;display:inline-block;font-size:12px}}</style>
<div class="card"><h1>UEFN Link</h1><p class="muted">Projet : {}<br>Map POP : {}<br>UEFN MCP : {}</p>
<p>Relais : <b>{}</b> · UEFN : <b>{}</b></p><p class="muted" style="margin-bottom:4px">Outils UEFN détectés :</p><div>{}</div></div>
<div class="card"><h2 style="font-weight:500;margin-top:0">Journal</h2><table>{}</table></div>"#,
        esc(&u.project), esc(&u.map), esc(&u.mcp_url), esc(&u.relay), esc(uefn), tools, rows)
}

// ---------- Ligne de commande ----------

async fn pair(server: &str, code: &str, project: &str, name: &str) -> Result<()> {
    let server = server.trim_end_matches('/').to_string();
    let r: Value = reqwest::Client::new().post(format!("{server}/api/relay/pair")).json(&json!({ "code": code, "name": name })).send().await?.json().await?;
    let token = r["token"].as_str().ok_or_else(|| anyhow!("{}", r["error"].as_str().unwrap_or("appairage refusé")))?;
    let project = std::path::absolute(project)?.to_string_lossy().to_string();
    save(&Config { server, token: token.into(), project: project.clone(), map: r["map"].as_str().unwrap_or("").into(), ..Default::default() })?;
    println!("Appairé à la map « {} ». Projet : {project}\nConfiguration : {}\nLancez maintenant : pop-link run", r["map"].as_str().unwrap_or(""), config_path().display());
    Ok(())
}

fn ask(q: &str) -> String {
    use std::io::Write;
    print!("{q} ");
    std::io::stdout().flush().ok();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    s.trim().trim_matches('"').to_string()
}

/// Projets UEFN du dossier « Documents/Fortnite Projects ».
fn find_projects() -> Vec<PathBuf> {
    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    let mut out = vec![];
    for base in [PathBuf::from(&home).join("Documents").join("Fortnite Projects"), PathBuf::from(&home).join("OneDrive").join("Documents").join("Fortnite Projects")] {
        if let Ok(rd) = std::fs::read_dir(&base) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() && std::fs::read_dir(&p).map(|r| r.flatten().any(|f| f.path().extension().map(|x| x == "uefnproject").unwrap_or(false))).unwrap_or(false) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// pop-link.exe téléchargé depuis POP : tout est automatique (adresse et code intégrés).
async fn auto_pair(emb: &Value) -> Result<()> {
    let server = emb["server"].as_str().unwrap_or("").trim_end_matches('/').to_string();
    let code = emb["code"].as_str().unwrap_or("").to_string();
    let projects = find_projects();
    let projects = if projects.is_empty() {
        println!("Aucun projet trouvé dans Documents\\Fortnite Projects.");
        vec![PathBuf::from(ask("Dossier du projet UEFN (glissez le dossier ici) :"))]
    } else { projects };
    println!("Projets UEFN trouvés :");
    for p in &projects { println!("  · {}", p.display()) }
    let device = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC UEFN".into());
    let body = json!({ "code": code, "device": device, "projects": projects.iter().map(|p| json!({ "name": p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default() })).collect::<Vec<_>>() });
    let r: Value = reqwest::Client::new().post(format!("{server}/api/relay/pair-studio")).json(&body).send().await.context("POP injoignable")?.json().await?;
    let list = r["links"].as_array().ok_or_else(|| anyhow!("{}", r["error"].as_str().unwrap_or("liaison refusée")))?;
    let mut links = vec![];
    for (l, p) in list.iter().zip(projects.iter()) {
        links.push(ProjLink { project: std::path::absolute(p)?.to_string_lossy().to_string(), map: l["project"].as_str().unwrap_or("").into(), token: l["token"].as_str().unwrap_or("").into() });
    }
    let first = links.first().cloned().ok_or_else(|| anyhow!("aucun projet relié"))?;
    save(&Config { server, token: first.token, project: first.project, map: first.map, links, ..Default::default() })?;
    println!("\nC'est relié : {} projet(s) synchronisé(s) avec POP.\n", list.len());
    Ok(())
}

/// Premier lancement par double-clic : questions simples dans la console.
async fn guided_pair() -> Result<()> {
    println!("UEFN Link · première configuration\n");
    let server = ask("Adresse de POP (ex. https://pop.mon-studio.fr ou http://192.168.1.20:8080) :");
    let code = ask("Code d'appairage (POP → Organiseur → « Obtenir un code ») :");
    let projects = find_projects();
    let project = if projects.is_empty() {
        ask("Dossier du projet UEFN (glissez le dossier ici) :")
    } else {
        for (i, p) in projects.iter().enumerate() { println!("  {}. {}", i + 1, p.display()) }
        let c = ask("Numéro du projet (ou chemin complet) :");
        c.parse::<usize>().ok().and_then(|n| projects.get(n.wrapping_sub(1))).map(|p| p.to_string_lossy().to_string()).unwrap_or(c)
    };
    let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC UEFN".into());
    pair(&server, &code, &project, &name).await
}

async fn diagnostic() -> Result<()> {
    let cfg = load().ok();
    let root = cfg.as_ref().map(|c| PathBuf::from(&c.project)).unwrap_or_else(|| PathBuf::from("."));
    let url = mcpdriver::find_url(cfg.as_ref().and_then(|c| c.uefn_mcp_url.as_deref()), &root);
    println!("Serveur MCP d'UEFN : {url}");
    let mut m = mcpdriver::Mcp::new(url, cfg.as_ref().map(|c| c.tools.clone()).unwrap_or_default());
    m.connect_with(true).await.context("UEFN injoignable : ouvrez le projet dans UEFN, puis Édition → Préférences de l'éditeur → Agentic Workflows → Model Context Protocol → Start Server")?;
    if !m.toolsets.is_empty() {
        println!("{} toolsets :", m.toolsets.len());
        for (n, d) in &m.toolsets { println!("  · {n} : {}", d.chars().take(90).collect::<String>()) }
        println!();
    }
    println!("{} outils :", m.tools.len());
    let one_line = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(110).collect::<String>();
    for t in &m.tools { println!("  - {} : {}", t["name"].as_str().unwrap_or(""), one_line(t["description"].as_str().unwrap_or(""))) }
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

fn pause() {
    if cfg!(windows) { ask("\nAppuyez sur Entrée pour fermer."); }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();
    let port = flag("--port").and_then(|p| p.parse().ok()).unwrap_or(47800);
    let r = match args.get(1).map(String::as_str) {
        Some("pair") if args.len() >= 5 => pair(&args[2], &args[3], &args[4], &flag("--name").unwrap_or_else(|| "UEFN Link".into())).await,
        Some("diagnostic") => { let r = diagnostic().await; if let Err(e) = &r { println!("{e:#}") } pause(); Ok(()) }
        Some("run") => run(port).await,
        None => {
            if load().is_err() {
                let r = match embedded() { Some(emb) => auto_pair(&emb).await, None => guided_pair().await };
                if let Err(e) = r { println!("\nLiaison impossible : {e:#}"); pause(); return; }
            }
            run(port).await
        }
        _ => {
            println!("UEFN Link\n  pop-link                 appairage guidé puis connexion\n  pop-link pair <adresse POP> <code> <dossier du projet UEFN> [--name \"PC de Yanis\"]\n  pop-link run [--port 47800]\n  pop-link diagnostic      vérifie UEFN MCP");
            Ok(())
        }
    };
    if let Err(e) = r { println!("Erreur : {e:#}"); pause(); }
}
