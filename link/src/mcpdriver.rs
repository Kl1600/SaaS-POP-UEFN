//! Pilote du vrai UEFN via son serveur MCP local (http://127.0.0.1:8000/mcp par défaut).
//! Epic ne publie pas la liste exacte des outils : on la découvre au démarrage et on choisit par mots-clés.
//! Le fichier de configuration peut imposer les noms (`tools`) si la détection se trompe.
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct Mcp {
    pub url: String,
    http: reqwest::Client,
    session: Option<String>,
    pub tools: Vec<Value>,
    pub overrides: HashMap<String, String>,
    next_id: u64,
}

impl Mcp {
    pub fn new(url: String, overrides: HashMap<String, String>) -> Self {
        Self { url, http: reqwest::Client::builder().timeout(std::time::Duration::from_secs(180)).build().unwrap(), session: None, tools: vec![], overrides, next_id: 1 }
    }

    async fn rpc(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        let mut req = self.http.post(&self.url)
            .header("accept", "application/json, text/event-stream")
            .header("content-type", "application/json")
            .json(&json!({ "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params }));
        if let Some(s) = &self.session {
            req = req.header("mcp-session-id", s);
        }
        let res = req.send().await?;
        if let Some(s) = res.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()) {
            self.session = Some(s.to_string());
        }
        let ctype = res.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
        let text = res.text().await?;
        // Réponse JSON directe ou flux SSE (lignes « data: »).
        let v: Value = if ctype.contains("event-stream") {
            text.lines().filter_map(|l| l.strip_prefix("data:")).filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok()).find(|v| v.get("id").is_some()).ok_or_else(|| anyhow!("réponse MCP vide"))?
        } else {
            serde_json::from_str(&text)?
        };
        if let Some(e) = v.get("error") {
            bail!("{}", e["message"].as_str().unwrap_or("erreur MCP"));
        }
        Ok(v["result"].clone())
    }

    pub async fn connect(&mut self) -> Result<()> {
        self.session = None;
        self.rpc("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "pop-link", "version": "0.1.0" } })).await?;
        let _ = self.http.post(&self.url).header("content-type", "application/json").header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", self.session.clone().unwrap_or_default())
            .json(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).send().await;
        let r = self.rpc("tools/list", json!({})).await?;
        self.tools = r["tools"].as_array().cloned().unwrap_or_default();
        Ok(())
    }

    pub async fn alive(&mut self) -> bool {
        if self.tools.is_empty() {
            return self.connect().await.is_ok();
        }
        if self.rpc("ping", json!({})).await.is_ok() {
            return true;
        }
        self.connect().await.is_ok()
    }

    /// Choisit l'outil pour un rôle (« python », « compile », « push »…).
    fn pick(&self, role: &str, words: &[&[&str]]) -> Option<Value> {
        if let Some(n) = self.overrides.get(role) {
            return self.tools.iter().find(|t| t["name"] == n.as_str()).cloned();
        }
        // Les outils de lecture ne doivent jamais servir à écrire, et inversement.
        let (avoid, verb): (&[&str], &[&str]) = match role {
            "props_set" => (&["get", "list", "read", "describe"], &["set", "update", "write", "modify", "change"]),
            "props_get" => (&["set", "update", "write", "modify"], &[]),
            _ => (&[], &[]),
        };
        self.tools.iter().find(|t| {
            let n = t["name"].as_str().unwrap_or("").to_lowercase();
            words.iter().all(|alts| alts.iter().any(|w| n.contains(w)))
                && !avoid.iter().any(|w| n.contains(w))
                && (verb.is_empty() || verb.iter().any(|w| n.contains(w)))
        }).cloned()
    }

    async fn call_tool(&mut self, tool: &Value, args: Value) -> Result<String> {
        let r = self.rpc("tools/call", json!({ "name": tool["name"], "arguments": args })).await?;
        let txt: String = r["content"].as_array().cloned().unwrap_or_default().iter().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n");
        if r["isError"].as_bool() == Some(true) {
            bail!("{txt}");
        }
        Ok(txt)
    }

    /// Exécute un script Python dans l'éditeur (Python Editor Scripting doit être activé).
    pub async fn python(&mut self, code: &str) -> Result<String> {
        let t = self.pick("python", &[&["python"], &["exec", "run"]]).ok_or_else(|| anyhow!("Aucun outil Python dans UEFN : activez Python Editor Scripting et les toolsets MCP."))?;
        let key = first_string_prop(&t).unwrap_or_else(|| "code".into());
        self.call_tool(&t, json!({ key: code })).await
    }

    pub async fn list_actors(&mut self) -> Result<Vec<Value>> {
        let out = self.python(PY_LIST).await?;
        let line = out.lines().rev().find(|l| l.trim_start().starts_with('[')).ok_or_else(|| anyhow!("liste d'acteurs illisible"))?;
        Ok(serde_json::from_str(line.trim())?)
    }

    pub async fn patch(&mut self, patches: &Value) -> Result<()> {
        // Options d'appareils : outil « Device » d'UEFN quand il existe, sinon Python.
        let mut patches = patches.clone();
        if let Some(t) = self.pick("props_set", &[&["propert", "editable", "option"], &["set", "update", "edit", "write"]]) {
            let mut names: HashMap<String, String> = HashMap::new();
            if let Ok(list) = self.list_actors().await { for a in list { names.insert(a["id"].as_str().unwrap_or("").into(), a["name"].as_str().unwrap_or("").into()); } }
            for p in patches.as_array_mut().into_iter().flatten() {
                let id = p["id"].as_str().unwrap_or("").to_string();
                let name = names.get(&id).cloned().unwrap_or_default();
                for key in ["options", "props"] {
                    if let Some(m) = p["changes"][key].as_object().cloned() {
                        for (k, v) in &m {
                            self.call_tool(&t, Self::fill_args(&t, &id, &name, Some((k, v)))).await.map_err(|e| anyhow!("option {k} : {e}"))?;
                        }
                        p["changes"].as_object_mut().unwrap().remove(key);
                    }
                }
            }
        }
        let patches = &patches;
        let code = format!("import json\nPATCHES = json.loads({})\n{}", serde_json::to_string(&serde_json::to_string(patches)?)?, PY_PATCH);
        let out = self.python(&code).await?;
        if let Some(f) = out.lines().find_map(|l| l.strip_prefix("FAILED=")) {
            bail!("UEFN a refusé ces options : {}", f.replace('|', " ; "));
        }
        Ok(())
    }

    pub async fn duplicate(&mut self, ids: &[String]) -> Result<Vec<String>> {
        let code = format!("IDS = {}\n{}", serde_json::to_string(ids)?, PY_DUP);
        let out = self.python(&code).await?;
        let line = out.lines().rev().find(|l| l.trim_start().starts_with('[')).unwrap_or("[]");
        Ok(serde_json::from_str(line.trim()).unwrap_or_default())
    }

    pub async fn delete(&mut self, ids: &[String]) -> Result<()> {
        let code = format!("IDS = {}\n{}", serde_json::to_string(ids)?, PY_DEL);
        self.python(&code).await.map(|_| ())
    }

    pub async fn save_level(&mut self) -> Result<()> {
        self.python("import unreal\nunreal.EditorLevelLibrary.save_current_level()\nprint('ok')").await.map(|_| ())
    }

    pub async fn compile(&mut self) -> Result<Vec<Value>> {
        let t = self.pick("compile", &[&["verse"], &["compile", "build"]]).ok_or_else(|| anyhow!("Outil de compilation Verse introuvable dans UEFN."))?;
        let out = self.call_tool(&t, json!({})).await.or_else(|e| Ok::<String, anyhow::Error>(e.to_string()))?;
        Ok(parse_diagnostics(&out))
    }

    /// Annule la dernière transaction POP de l'éditeur (suppression), puis vérifie que les acteurs sont revenus.
    pub async fn restore(&mut self, ids: &[String]) -> Result<()> {
        let code = format!("IDS = {}\n{}", serde_json::to_string(ids)?, PY_RESTORE);
        let out = self.python(&code).await?;
        if !out.contains("restored") {
            bail!("UEFN n'a pas pu annuler la suppression (une autre action a eu lieu depuis) : utilisez Ctrl+Z dans UEFN.");
        }
        Ok(())
    }

    /// Remplit les arguments d'un outil d'appareil d'après son schéma (acteur, propriété, valeur).
    fn fill_args(tool: &Value, id: &str, name: &str, prop: Option<(&str, &Value)>) -> Value {
        let mut args = json!({});
        let Some(props) = tool["inputSchema"]["properties"].as_object() else { return args };
        for (k, sch) in props {
            let lk = k.to_lowercase();
            if lk.contains("value") {
                if let Some((_, v)) = prop { args[k] = if sch["type"] == "string" && !v.is_string() { json!(v.to_string()) } else { v.clone() } }
            } else if lk.contains("prop") || lk.contains("field") || lk == "name" && prop.is_some() && props.keys().any(|x| x.to_lowercase().contains("actor") || x.to_lowercase().contains("device")) {
                if let Some((p, _)) = prop { args[k] = json!(p) }
            } else if lk.contains("path") || lk.contains("actor") || lk.contains("device") || lk.contains("id") || lk.contains("target") {
                args[k] = json!(if lk.contains("label") || lk.contains("name") { name } else { id });
            } else if lk.contains("label") || lk == "name" {
                args[k] = json!(name);
            }
        }
        args
    }

    /// Options réelles d'un appareil (outil « Device » d'UEFN MCP).
    pub async fn device_props(&mut self, id: &str, name: &str) -> Result<Value> {
        let t = self.pick("props_get", &[&["propert", "editable", "option"], &["get", "list", "read", "describe"]]).ok_or_else(|| anyhow!("UEFN n'expose pas d'outil pour lire les options des appareils (voir pop-link diagnostic)."))?;
        let out = self.call_tool(&t, Self::fill_args(&t, id, name, None)).await?;
        let start = out.find('{').ok_or_else(|| anyhow!("réponse illisible : {}", out.chars().take(200).collect::<String>()))?;
        let v: Value = serde_json::from_str(out[start..].trim()).or_else(|_| serde_json::from_str(&out[start..=out.rfind('}').unwrap_or(out.len() - 1)]))?;
        // Formes courantes : {props:{…}}, {properties:[{name,value}]}, ou directement {nom: valeur}.
        let obj = v.get("properties").or(v.get("props")).or(v.get("editables")).cloned().unwrap_or(v);
        Ok(match obj {
            Value::Array(list) => Value::Object(list.iter().filter_map(|p| Some((p["name"].as_str().or(p["key"].as_str())?.to_string(), p.get("value").cloned().unwrap_or(Value::Null)))).collect()),
            Value::Object(m) => Value::Object(m.into_iter().filter(|(_, v)| !v.is_object() && !v.is_array()).collect()),
            other => json!({ "valeur": other }),
        })
    }

    /// Plan vu du dessus dessiné à partir de l'emprise réelle des maillages et appareils du niveau.
    pub async fn topview(&mut self) -> Result<(String, Value)> {
        let out = self.python(PY_BOUNDS).await?;
        let line = out.lines().rev().find(|l| l.trim_start().starts_with('[')).ok_or_else(|| anyhow!("emprises illisibles"))?;
        let items: Vec<Value> = serde_json::from_str(line.trim())?;
        Ok(plan_svg(&items))
    }

    /// Outils retenus pour chaque rôle (affiché dans le diagnostic et dans POP).
    pub fn roles(&self) -> Value {
        let name = |t: Option<Value>| t.and_then(|t| t["name"].as_str().map(String::from)).unwrap_or_else(|| "introuvable".into());
        json!({
            "python": name(self.pick("python", &[&["python"], &["exec", "run"]])),
            "compile": name(self.pick("compile", &[&["verse"], &["compile", "build"]])),
            "push": name(self.pick("push", &[&["session"], &["push", "update", "verse", "apply"]])),
            "props_get": name(self.pick("props_get", &[&["propert", "editable", "option"], &["get", "list", "read", "describe"]])),
            "props_set": name(self.pick("props_set", &[&["propert", "editable", "option"], &["set", "update", "edit", "write"]])),
        })
    }

    pub async fn push_session(&mut self) -> Result<()> {
        let t = self.pick("push", &[&["session"], &["push", "update", "verse", "apply"]]).ok_or_else(|| anyhow!("Outil de session introuvable : lancez une session de test dans UEFN."))?;
        self.call_tool(&t, json!({})).await.map(|_| ())
    }
}

/// Adresse du serveur MCP : configuration, sinon le .mcp.json généré par UEFN (« Generate Client Config »), sinon le port par défaut.
pub fn find_url(configured: Option<&str>, project: &std::path::Path) -> String {
    if let Some(u) = configured { return u.to_string() }
    let mut candidates = vec![project.join(".mcp.json")];
    for base in ["C:/Program Files/Epic Games/Fortnite/FortniteGame", "D:/Epic Games/Fortnite/FortniteGame", "C:/Program Files (x86)/Epic Games/Fortnite/FortniteGame"] {
        candidates.push(std::path::PathBuf::from(base).join(".mcp.json"));
    }
    for c in candidates {
        let Ok(s) = std::fs::read_to_string(&c) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&s) else { continue };
        if let Some(servers) = v["mcpServers"].as_object() {
            for (_, sv) in servers {
                if let Some(u) = sv["url"].as_str().or(sv["serverUrl"].as_str()) { return u.to_string() }
            }
        }
    }
    "http://127.0.0.1:8000/mcp".into()
}

fn plan_svg(items: &[Value]) -> (String, Value) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for it in items {
        let (cx, cy, ex, ey) = (it["x"].as_f64().unwrap_or(0.0), it["y"].as_f64().unwrap_or(0.0), it["ex"].as_f64().unwrap_or(0.0), it["ey"].as_f64().unwrap_or(0.0));
        x0 = x0.min(cx - ex); y0 = y0.min(cy - ey); x1 = x1.max(cx + ex); y1 = y1.max(cy + ey);
    }
    if items.is_empty() || x1 <= x0 { x0 = -5000.0; y0 = -5000.0; x1 = 5000.0; y1 = 5000.0 }
    let m = ((x1 - x0).max(y1 - y0)) * 0.05;
    let (x0, y0, x1, y1) = (x0 - m, y0 - m, x1 + m, y1 + m);
    let w = 1200.0; let h = (w * (y1 - y0) / (x1 - x0)).round().max(200.0);
    let sx = |x: f64| (x - x0) / (x1 - x0) * w; let sy = |y: f64| (y - y0) / (y1 - y0) * h;
    let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="100%" height="100%" fill="#2e3b2c"/>"##);
    let mut sorted: Vec<&Value> = items.iter().collect();
    sorted.sort_by(|a, b| (b["ex"].as_f64().unwrap_or(0.0) * b["ey"].as_f64().unwrap_or(0.0)).partial_cmp(&(a["ex"].as_f64().unwrap_or(0.0) * a["ey"].as_f64().unwrap_or(0.0))).unwrap_or(std::cmp::Ordering::Equal));
    for it in sorted {
        let (cx, cy, ex, ey) = (it["x"].as_f64().unwrap_or(0.0), it["y"].as_f64().unwrap_or(0.0), it["ex"].as_f64().unwrap_or(0.0), it["ey"].as_f64().unwrap_or(0.0));
        let z = it["top"].as_f64().unwrap_or(0.0);
        let shade = (90.0 + (z / 30.0).clamp(-40.0, 120.0)) as u8;
        let fill = if it["kind"] == "landscape" { "#3d5236".to_string() } else { format!("rgb({shade},{shade},{})", shade.saturating_sub(8)) };
        svg += &format!(r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{fill}" fill-opacity="0.85" stroke="#000" stroke-opacity="0.25"/>"##, sx(cx - ex), sy(cy - ey), (sx(cx + ex) - sx(cx - ex)).max(1.0), (sy(cy + ey) - sy(cy - ey)).max(1.0));
    }
    svg += "</svg>";
    use base64::Engine;
    (format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(svg)), json!({ "x0": x0, "y0": y0, "x1": x1, "y1": y1 }))
}

fn first_string_prop(tool: &Value) -> Option<String> {
    let props = tool["inputSchema"]["properties"].as_object()?;
    for pref in ["code", "script", "python", "source", "command"] {
        if props.contains_key(pref) {
            return Some(pref.into());
        }
    }
    props.iter().find(|(_, v)| v["type"] == "string").map(|(k, _)| k.clone())
}

/// Lignes du type `C:/Projet/Content/a.verse(12,5, 12,20): Script error 3512: message`.
pub fn parse_diagnostics(out: &str) -> Vec<Value> {
    let mut d = vec![];
    for l in out.lines() {
        let Some(i) = l.find(".verse(") else { continue };
        let head = &l[..i + 6];
        // Retire le préfixe du journal (« LogSolarisIde: Error: ») : le chemin commence à la lettre de lecteur.
        let start = head.char_indices().find(|&(k, c)| c.is_ascii_alphabetic() && head[k + 1..].starts_with(":/") || c.is_ascii_alphabetic() && head[k + 1..].starts_with(":\\")).map(|x| x.0).unwrap_or_else(|| head.rfind(": ").map(|k| k + 2).unwrap_or(0));
        let path_full = &head[start..];
        let rest = &l[i + 7..];
        let Some(close) = rest.find(')') else { continue };
        let nums: Vec<usize> = rest[..close].split(|c| c == ',' || c == ' ').filter_map(|x| x.trim().parse().ok()).collect();
        let msg = rest[close + 1..].trim_start_matches(':').trim();
        let severity = if msg.to_lowercase().contains("warning") { "warning" } else { "error" };
        let code = msg.split_whitespace().find_map(|w| w.trim_end_matches(':').parse::<u32>().ok()).map(|c| c.to_string()).unwrap_or_default();
        let message = msg.splitn(2, ':').nth(1).map(|m| m.trim()).filter(|m| !m.is_empty()).unwrap_or(msg);
        let path = path_full.replace('\\', "/");
        let path = path.find("/Content/").map(|k| path[k + 1..].to_string()).unwrap_or(path);
        d.push(json!({ "raw_path": path_full.replace('\\', "/"), "path": path, "line": nums.first().copied().unwrap_or(1), "col": nums.get(1).copied().unwrap_or(1), "severity": severity, "code": code, "message": message }));
    }
    d
}

const PY_RESTORE: &str = r#"import unreal
unreal.SystemLibrary.execute_console_command(None, 'TRANSACTION UNDO')
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
have = set(a.get_path_name() for a in sub.get_all_level_actors())
print('restored' if all(i in have for i in IDS) else 'missing')
"#;

const PY_BOUNDS: &str = r#"import unreal, json
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
out = []
for a in sub.get_all_level_actors():
    cls = a.get_class().get_name()
    if cls in ('WorldSettings', 'Brush', 'DefaultPhysicsVolume', 'LevelBounds', 'SkyLight', 'DirectionalLight', 'ExponentialHeightFog', 'SkyAtmosphere', 'VolumetricCloud'): continue
    try:
        o, e = a.get_actor_bounds(False)
    except Exception:
        continue
    if e.x <= 1 and e.y <= 1: continue
    if e.x > 200000 or e.y > 200000: continue
    kind = 'landscape' if 'Landscape' in cls else 'actor'
    out.append({'x': o.x, 'y': o.y, 'ex': e.x, 'ey': e.y, 'top': o.z + e.z, 'kind': kind})
print(json.dumps(out[:6000]))
"#;

const PY_LIST: &str = r#"import unreal, json
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
out = []
for a in sub.get_all_level_actors():
    t = a.get_actor_transform(); l = t.translation; r = t.rotation.rotator(); s = t.scale3d
    cls = a.get_class().get_name()
    kind = 'verse_device' if 'verse' in cls.lower() else ('device' if 'device' in cls.lower() else ('mesh' if 'StaticMesh' in cls else 'prop'))
    tags = [str(x) for x in a.tags]
    try:
        for comp in a.get_components_by_class(unreal.ActorComponent):
            if 'TagMarkup' in comp.get_class().get_name():
                for t in comp.get_editor_property('tags') or []:
                    n = t.get_name() if hasattr(t, 'get_name') else str(t)
                    if n and n not in tags: tags.append(n)
    except Exception:
        pass
    out.append({'id': a.get_path_name(), 'name': a.get_actor_label(), 'type': kind, 'class': cls, 'folder': str(a.get_folder_path()),
                'hidden': bool(a.is_temporarily_hidden_in_editor()), 'tags': tags,
                'transform': {'location': [l.x, l.y, l.z], 'rotation': [r.pitch, r.yaw, r.roll], 'scale': [s.x, s.y, s.z]},
                'options': {}, 'props': {}, 'events': []})
print(json.dumps(out))
"#;

const PY_PATCH: &str = r#"import unreal
FAILED = []
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
by_id = {a.get_path_name(): a for a in sub.get_all_level_actors()}
with unreal.ScopedEditorTransaction('POP'):
    for p in PATCHES:
        a = by_id.get(p['id'])
        if a is None: continue
        c = p['changes']
        if 'name' in c: a.set_actor_label(c['name'])
        if 'folder' in c: a.set_folder_path(c['folder'])
        if 'hidden' in c: a.set_is_temporarily_hidden_in_editor(bool(c['hidden']))
        if 'transform' in c:
            t = c['transform']
            a.set_actor_location(unreal.Vector(*t['location']), False, False)
            a.set_actor_rotation(unreal.Rotator(*t['rotation']), False)
            a.set_actor_scale3d(unreal.Vector(*t['scale']))
        if 'tags' in c: a.tags = [unreal.Name(x) for x in c['tags']]
        if 'events' in c: FAILED.append('liens d\'événements : réglez-les dans les options de l\'appareil (« Quand … envoyer un événement à »)')
        for k, v in list(c.get('options', {}).items()) + list(c.get('props', {}).items()):
            try: a.set_editor_property(k, v)
            except Exception as e: FAILED.append(k + ' : ' + str(e))
print('FAILED=' + '|'.join(FAILED) if FAILED else 'ok')
"#;

const PY_DUP: &str = r#"import unreal, json
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
src = [a for a in sub.get_all_level_actors() if a.get_path_name() in IDS]
new = sub.duplicate_actors(src, None, unreal.Vector(300, 0, 0))
print(json.dumps([a.get_path_name() for a in new]))
"#;

const PY_DEL: &str = r#"import unreal
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
sub.destroy_actors([a for a in sub.get_all_level_actors() if a.get_path_name() in IDS])
print('ok')
"#;

#[cfg(test)]
mod t {
    #[test]
    fn diags() {
        let d = super::parse_diagnostics("C:/P/MyIsland/Content/vault.verse(12,5, 12,20): Script error 3512: This invocation calls a function that has the 'no_rollback' effect");
        assert_eq!(d[0]["path"], "Content/vault.verse");
        assert_eq!(d[0]["line"], 12);
        assert_eq!(d[0]["code"], "3512");
    }
}
