//! Pilote du vrai UEFN via son serveur MCP local (http://127.0.0.1:8000/mcp par défaut).
//!
//! UEFN n'expose que trois outils « méta » (`list_toolsets`, `describe_toolset`, `call_tool`) ;
//! les vrais outils sont rangés dans des toolsets et s'appellent par `call_tool`. On découvre au
//! démarrage les toolsets utiles à POP, puis on choisit chaque outil par son nom réel
//! (`execute_python`, `BuildAll`, `PushChanges`…). Pour un autre serveur MCP sans toolsets, on garde
//! la détection par mots-clés. Le fichier de configuration peut imposer les noms (`tools`).
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Toolsets décrits à chaque connexion (les autres ne le sont que par `pop-link diagnostic`).
const USEFUL_TOOLSETS: &[&str] = &[
    "python_scripting",
    "VerseToolset.VerseToolset",
    "SessionToolset",
    "DeviceToolset",
    "RevisionControl",
    "LevelToolset",
    "ActorToolset",
];

/// Outil attendu pour chaque rôle : (morceau du nom du toolset, nom de l'outil).
const KNOWN: &[(&str, &str, &str)] = &[
    ("python", "python_scripting", "execute_python"),
    ("compile", "verse", "BuildAll"),
    ("push", "session", "PushChanges"),
    ("session_status", "session", "GetSessionStatus"),
    ("session_start", "session", "StartSession"),
    ("session_stop", "session", "StopSession"),
    ("session_output", "session", "GetVerseOutput"),
    ("props_get", "device", "ListClassicDeviceOptions"),
    ("props_set", "device", "SetClassicDeviceOption"),
    ("verse_props_list", "device", "ListDeviceProperties"),
    ("verse_props_get", "device", "GetDeviceProperties"),
    ("verse_props_set", "device", "SetDeviceProperty"),
    ("checkin", "revision", "CheckIn"),
];

pub struct Mcp {
    pub url: String,
    http: reqwest::Client,
    session: Option<String>,
    /// Outils utilisables. Pour un outil de toolset : `name` complet, `tool` (nom court) et `toolset`.
    pub tools: Vec<Value>,
    /// Tous les toolsets annoncés par UEFN (nom, description).
    pub toolsets: Vec<(String, String)>,
    pub overrides: HashMap<String, String>,
    next_id: u64,
}

impl Mcp {
    pub fn new(url: String, overrides: HashMap<String, String>) -> Self {
        Self {
            url,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .build()
                .unwrap(),
            session: None,
            tools: vec![],
            toolsets: vec![],
            overrides,
            next_id: 1,
        }
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
        if let Some(s) = res
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            self.session = Some(s.to_string());
        }
        let ctype = res
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let text = res.text().await?;
        // Réponse JSON directe ou flux SSE (lignes « data: »).
        let v: Value = if ctype.contains("event-stream") {
            text.lines()
                .filter_map(|l| l.strip_prefix("data:"))
                .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
                .find(|v| v.get("id").is_some())
                .ok_or_else(|| anyhow!("réponse MCP vide"))?
        } else {
            serde_json::from_str(&text)?
        };
        if let Some(e) = v.get("error") {
            bail!("{}", e["message"].as_str().unwrap_or("erreur MCP"));
        }
        Ok(v["result"].clone())
    }

    pub async fn connect(&mut self) -> Result<()> {
        self.connect_with(false).await
    }

    /// `all` : décrit tous les toolsets (diagnostic), sinon seulement ceux dont POP se sert.
    pub async fn connect_with(&mut self, all: bool) -> Result<()> {
        // On repart de zéro : une connexion ratée (UEFN qui se ferme) ne doit pas laisser d'outils sans session,
        // sinon `alive` croit la connexion bonne à la réouverture et chaque appel est refusé (#4).
        self.session = None;
        self.tools.clear();
        self.toolsets.clear();
        self.rpc("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "pop-link", "version": "0.1.0" } })).await?;
        let _ = self
            .http
            .post(&self.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-session-id", self.session.clone().unwrap_or_default())
            .json(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .send()
            .await;
        let r = self.rpc("tools/list", json!({})).await?;
        let top = r["tools"].as_array().cloned().unwrap_or_default();
        let has = |n: &str| top.iter().any(|t| t["name"] == n);
        if !(has("call_tool") && has("list_toolsets") && has("describe_toolset")) {
            self.tools = top;
            self.toolsets.clear();
            return Ok(());
        }
        // Serveur à toolsets (UEFN) : on aplatit les outils des toolsets utiles.
        let mut tools: Vec<Value> = top
            .into_iter()
            .filter(|t| {
                !["call_tool", "list_toolsets", "describe_toolset"]
                    .contains(&t["name"].as_str().unwrap_or(""))
            })
            .collect();
        let listing = self.raw_call("list_toolsets", json!({})).await?;
        self.toolsets = parse_toolsets(&listing);
        let wanted: Vec<String> = self
            .toolsets
            .iter()
            .map(|(n, _)| n.clone())
            .filter(|n| {
                all || USEFUL_TOOLSETS.iter().any(|w| n.contains(w))
                    || self
                        .overrides
                        .values()
                        .any(|o| o.starts_with(&format!("{n}.")))
            })
            .collect();
        for ts in wanted {
            let Ok(txt) = self
                .raw_call("describe_toolset", json!({ "toolset_name": ts }))
                .await
            else {
                continue;
            };
            let Ok(d) = serde_json::from_str::<Value>(&txt) else {
                continue;
            };
            for t in d["tools"].as_array().into_iter().flatten() {
                let full = t["name"].as_str().unwrap_or("").to_string();
                let short = full.rsplit('.').next().unwrap_or(&full).to_string();
                tools.push(json!({ "name": full, "tool": short, "toolset": ts, "description": t["description"], "inputSchema": t.get("inputSchema").or(t.get("input_schema")).cloned().unwrap_or(json!({})) }));
            }
        }
        self.tools = tools;
        Ok(())
    }

    /// Appel direct d'un outil de premier niveau ; renvoie le texte (erreur si `isError`).
    async fn raw_call(&mut self, name: &str, args: Value) -> Result<String> {
        let r = match self
            .rpc("tools/call", json!({ "name": name, "arguments": args }))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                // Session MCP perdue (UEFN relancé) : la prochaine vérification refait la connexion complète.
                if e.to_string().contains("Mcp-Session-Id") {
                    self.tools.clear();
                    self.session = None;
                }
                return Err(e);
            }
        };
        let txt: String = r["content"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if r["isError"].as_bool() == Some(true) {
            bail!("{}", txt.trim());
        }
        Ok(txt)
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

    /// Outil réel d'UEFN pour un rôle (nom imposé par la configuration, sinon nom connu).
    fn known(&self, role: &str) -> Option<Value> {
        if let Some(n) = self.overrides.get(role) {
            return self
                .tools
                .iter()
                .find(|t| t["name"] == n.as_str() || t["tool"] == n.as_str())
                .cloned();
        }
        let (_, ts, tool) = KNOWN.iter().find(|(r, _, _)| *r == role)?;
        self.tools
            .iter()
            .find(|t| {
                t["tool"] == *tool
                    && t["toolset"]
                        .as_str()
                        .map(|s| s.to_lowercase().contains(ts))
                        .unwrap_or(false)
            })
            .cloned()
    }

    /// Choisit l'outil pour un rôle (« python », « compile », « push »…) : nom réel d'UEFN d'abord,
    /// puis mots-clés pour les autres serveurs MCP.
    fn pick(&self, role: &str, words: &[&[&str]]) -> Option<Value> {
        if self.overrides.contains_key(role) || KNOWN.iter().any(|(r, _, _)| *r == role) {
            if let Some(t) = self.known(role) {
                return Some(t);
            }
            if self.overrides.contains_key(role) || !self.toolsets.is_empty() {
                return None;
            }
        }
        // Les outils de lecture ne doivent jamais servir à écrire, et inversement.
        let (avoid, verb): (&[&str], &[&str]) = match role {
            "props_set" => (
                &["get", "list", "read", "describe"],
                &["set", "update", "write", "modify", "change"],
            ),
            "props_get" => (&["set", "update", "write", "modify"], &[]),
            _ => (&[], &[]),
        };
        self.tools
            .iter()
            .find(|t| {
                let n = t["name"].as_str().unwrap_or("").to_lowercase();
                words.iter().all(|alts| alts.iter().any(|w| n.contains(w)))
                    && !avoid.iter().any(|w| n.contains(w))
                    && (verb.is_empty() || verb.iter().any(|w| n.contains(w)))
            })
            .cloned()
    }

    /// Appelle un outil (de premier niveau, ou d'un toolset via `call_tool`).
    /// UEFN enveloppe les résultats dans `{"returnValue": …}` : on renvoie la valeur elle-même.
    async fn call_tool(&mut self, tool: &Value, args: Value) -> Result<String> {
        let txt = match tool["toolset"].as_str() {
            Some(ts) => {
                let (ts, short) = (
                    ts.to_string(),
                    tool["tool"].as_str().unwrap_or("").to_string(),
                );
                self.raw_call(
                    "call_tool",
                    json!({ "toolset_name": ts, "tool_name": short, "arguments": args }),
                )
                .await?
            }
            None => {
                self.raw_call(tool["name"].as_str().unwrap_or(""), args)
                    .await?
            }
        };
        Ok(unwrap_return(&txt))
    }

    /// Exécute un script Python dans l'éditeur (Python Editor Scripting doit être activé).
    pub async fn python(&mut self, code: &str) -> Result<String> {
        let t = self.pick("python", &[&["python"], &["exec", "run"]]).ok_or_else(|| anyhow!("Aucun outil Python dans UEFN : activez « Enable Project Python Scripts » et démarrez le serveur MCP."))?;
        // UEFN : `script: { content, language }` ; autres serveurs : un simple champ texte.
        let args = if t["inputSchema"]["properties"]["script"]["type"] == "object" {
            json!({ "script": { "content": code, "language": "Python" } })
        } else {
            json!({ first_string_prop(&t).unwrap_or_else(|| "code".into()): code })
        };
        self.call_tool(&t, args).await
    }

    /// Outils d'appareils d'UEFN (DeviceToolset) disponibles.
    fn device_tools(&self) -> bool {
        self.known("props_set").is_some() || self.known("verse_props_set").is_some()
    }

    pub async fn list_actors(&mut self) -> Result<Vec<Value>> {
        let out = self.python(PY_LIST).await?;
        let line = out
            .lines()
            .rev()
            .find(|l| l.trim_start().starts_with('['))
            .ok_or_else(|| anyhow!("liste d'acteurs illisible"))?;
        Ok(serde_json::from_str(line.trim())?)
    }

    pub async fn patch(&mut self, patches: &Value) -> Result<()> {
        // Options d'appareils : DeviceToolset d'UEFN quand il existe, sinon outil trouvé par mots-clés, sinon Python.
        let mut patches = patches.clone();
        if self.device_tools() {
            let mut classes: HashMap<String, String> = HashMap::new();
            if let Ok(list) = self.list_actors().await {
                for a in list {
                    classes.insert(
                        a["id"].as_str().unwrap_or("").into(),
                        a["class"].as_str().unwrap_or("").to_lowercase(),
                    );
                }
            }
            for p in patches.as_array_mut().into_iter().flatten() {
                let id = p["id"].as_str().unwrap_or("").to_string();
                for key in ["options", "props"] {
                    let Some(m) = p["changes"][key].as_object().cloned() else {
                        continue;
                    };
                    let verse = key == "props"
                        || classes
                            .get(&id)
                            .map(|c| c.contains("verse") || c.contains("scriptdevice"))
                            .unwrap_or(false);
                    for (k, v) in &m {
                        // Un appareil Verse a ses réglages @editable et aussi des options classiques
                        // (« Enabled at Game Start »…) : on tente le réglage Verse, puis l'option classique.
                        let mut err = None;
                        if verse {
                            if let Some(t) = self.known("verse_props_set") {
                                match self.call_tool(&t, json!({ "device": { "refPath": id }, "propertyName": k, "value": serde_json::to_string(v)? })).await {
                                    Ok(_) => continue,
                                    Err(e) => err = Some(e),
                                }
                            }
                        }
                        let t = self
                            .known("props_set")
                            .ok_or_else(|| anyhow!("UEFN n'expose pas SetClassicDeviceOption."))?;
                        let val = v
                            .as_str()
                            .map(String::from)
                            .unwrap_or_else(|| v.to_string());
                        if let Err(e) = self.call_tool(&t, json!({ "devicePath": { "refPath": id }, "optionName": k, "value": val })).await {
                            bail!("option {k} : {}", err.map(|x| format!("{x} ; {e}")).unwrap_or_else(|| e.to_string()));
                        }
                    }
                    p["changes"].as_object_mut().unwrap().remove(key);
                }
            }
        } else if let Some(t) = self.pick(
            "props_set",
            &[
                &["propert", "editable", "option"],
                &["set", "update", "edit", "write"],
            ],
        ) {
            let mut names: HashMap<String, String> = HashMap::new();
            if let Ok(list) = self.list_actors().await {
                for a in list {
                    names.insert(
                        a["id"].as_str().unwrap_or("").into(),
                        a["name"].as_str().unwrap_or("").into(),
                    );
                }
            }
            for p in patches.as_array_mut().into_iter().flatten() {
                let id = p["id"].as_str().unwrap_or("").to_string();
                let name = names.get(&id).cloned().unwrap_or_default();
                for key in ["options", "props"] {
                    if let Some(m) = p["changes"][key].as_object().cloned() {
                        for (k, v) in &m {
                            self.call_tool(&t, Self::fill_args(&t, &id, &name, Some((k, v))))
                                .await
                                .map_err(|e| anyhow!("option {k} : {e}"))?;
                        }
                        p["changes"].as_object_mut().unwrap().remove(key);
                    }
                }
            }
        }
        let patches = &patches;
        let code = format!(
            "import json\nPATCHES = json.loads({})\n{}",
            serde_json::to_string(&serde_json::to_string(patches)?)?,
            PY_PATCH
        );
        let out = self.python(&code).await?;
        if let Some(f) = out.lines().find_map(|l| l.strip_prefix("FAILED=")) {
            bail!("UEFN a refusé ces options : {}", f.replace('|', " ; "));
        }
        Ok(())
    }

    /// Pose un appareil et rend l'identifiant de l'acteur créé.
    pub async fn spawn(&mut self, args: &Value) -> Result<String> {
        let code = format!(
            "ARGS = {}\n{}",
            serde_json::to_string(&args.to_string())?,
            PY_SPAWN
        );
        let out = self.python(&code).await?;
        out.lines()
            .rev()
            .find_map(|l| l.trim().strip_prefix("SPAWNED=").map(String::from))
            .ok_or_else(|| anyhow!("UEFN n'a pas confirmé la pose : {}", out.trim()))
    }

    pub async fn duplicate(&mut self, ids: &[String]) -> Result<Vec<String>> {
        let code = format!("IDS = {}\n{}", serde_json::to_string(ids)?, PY_DUP);
        let out = self.python(&code).await?;
        let line = out
            .lines()
            .rev()
            .find(|l| l.trim_start().starts_with('['))
            .unwrap_or("[]");
        Ok(serde_json::from_str(line.trim()).unwrap_or_default())
    }

    /// Supprime des acteurs et renvoie leur sauvegarde (texte T3D, en base64) pour pouvoir les recréer.
    /// Une suppression faite par Python n'entre pas dans l'historique d'annulation d'UEFN : sans cette
    /// sauvegarde, elle serait définitive.
    pub async fn delete(&mut self, ids: &[String]) -> Result<String> {
        let code = format!("IDS = {}\n{}", serde_json::to_string(ids)?, PY_DEL);
        let out = self.python(&code).await?;
        let saved = out
            .lines()
            .find_map(|l| l.strip_prefix("T3D="))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if saved.is_empty() {
            bail!("UEFN n'a pas pu sauvegarder les acteurs avant suppression : rien n'a été supprimé.");
        }
        Ok(saved)
    }

    pub async fn save_level(&mut self) -> Result<()> {
        self.python("import unreal\nunreal.EditorLevelLibrary.save_current_level()\nprint('ok')")
            .await
            .map(|_| ())
    }

    pub async fn compile(&mut self) -> Result<Vec<Value>> {
        let t = self
            .pick("compile", &[&["verse"], &["compile", "build"]])
            .ok_or_else(|| anyhow!("Outil de compilation Verse introuvable dans UEFN."))?;
        let out = self
            .call_tool(&t, json!({}))
            .await
            .or_else(|e| Ok::<String, anyhow::Error>(e.to_string()))?;
        // UEFN (BuildAll) : liste JSON structurée ; sinon, lignes du journal de compilation.
        if let Ok(Value::Array(list)) = serde_json::from_str::<Value>(out.trim()) {
            return Ok(list.iter().map(build_diag).collect());
        }
        Ok(parse_diagnostics(&out))
    }

    /// Archive dans Lore (ou le système de version du projet) via UEFN : enregistre tout, puis un check-in.
    pub async fn checkin(&mut self, message: &str) -> Result<()> {
        let t = self.known("checkin").ok_or_else(|| {
            anyhow!("UEFN n'expose pas l'archivage (ValkyrieRevisionControl.CheckIn).")
        })?;
        self.call_tool(&t, json!({ "description": message }))
            .await
            .map(|_| ())
    }

    pub fn can_checkin(&self) -> bool {
        self.known("checkin").is_some()
    }

    /// Recrée des acteurs supprimés à partir de leur sauvegarde (mêmes réglages, position, dossier, nom).
    /// Les acteurs recréés ont un nouvel identifiant interne : on renvoie la liste des nouveaux.
    pub async fn restore(&mut self, saved: &str, expected: usize) -> Result<Vec<String>> {
        if saved.is_empty() {
            bail!("Pas de sauvegarde pour cette suppression (faite avant cette version de pop-link) : elle ne peut pas être annulée.");
        }
        let code = format!("SAVED = {}\n{}", serde_json::to_string(saved)?, PY_RESTORE);
        let out = self.python(&code).await?;
        let line = out
            .lines()
            .rev()
            .find(|l| l.trim_start().starts_with('['))
            .ok_or_else(|| {
                anyhow!(
                    "UEFN n'a pas pu recréer les acteurs : {}",
                    out.chars().take(200).collect::<String>()
                )
            })?;
        let ids: Vec<String> = serde_json::from_str(line.trim())?;
        if ids.len() < expected {
            bail!(
                "UEFN n'a recréé que {} acteur(s) sur {expected}.",
                ids.len()
            );
        }
        Ok(ids)
    }

    /// Remplit les arguments d'un outil d'appareil d'après son schéma (acteur, propriété, valeur).
    fn fill_args(tool: &Value, id: &str, name: &str, prop: Option<(&str, &Value)>) -> Value {
        let mut args = json!({});
        let Some(props) = tool["inputSchema"]["properties"].as_object() else {
            return args;
        };
        for (k, sch) in props {
            let lk = k.to_lowercase();
            if lk.contains("value") {
                if let Some((_, v)) = prop {
                    args[k] = if sch["type"] == "string" && !v.is_string() {
                        json!(v.to_string())
                    } else {
                        v.clone()
                    }
                }
            } else if lk.contains("prop")
                || lk.contains("field")
                || lk == "name"
                    && prop.is_some()
                    && props.keys().any(|x| {
                        x.to_lowercase().contains("actor") || x.to_lowercase().contains("device")
                    })
            {
                if let Some((p, _)) = prop {
                    args[k] = json!(p)
                }
            } else if lk.contains("path")
                || lk.contains("actor")
                || lk.contains("device")
                || lk.contains("id")
                || lk.contains("target")
            {
                args[k] = json!(if lk.contains("label") || lk.contains("name") {
                    name
                } else {
                    id
                });
            } else if lk.contains("label") || lk == "name" {
                args[k] = json!(name);
            }
        }
        args
    }

    /// Options réelles d'un appareil (outil « Device » d'UEFN MCP).
    pub async fn device_props(&mut self, id: &str, name: &str) -> Result<Value> {
        if self.device_tools() {
            return self.device_props_uefn(id).await;
        }
        let t = self.pick("props_get", &[&["propert", "editable", "option"], &["get", "list", "read", "describe"]]).ok_or_else(|| anyhow!("UEFN n'expose pas d'outil pour lire les options des appareils (voir pop-link diagnostic)."))?;
        let out = self
            .call_tool(&t, Self::fill_args(&t, id, name, None))
            .await?;
        let start = out.find('{').ok_or_else(|| {
            anyhow!(
                "réponse illisible : {}",
                out.chars().take(200).collect::<String>()
            )
        })?;
        let v: Value = serde_json::from_str(out[start..].trim()).or_else(|_| {
            serde_json::from_str(&out[start..=out.rfind('}').unwrap_or(out.len() - 1)])
        })?;
        // Formes courantes : {props:{…}}, {properties:[{name,value}]}, ou directement {nom: valeur}.
        let obj = v
            .get("properties")
            .or(v.get("props"))
            .or(v.get("editables"))
            .cloned()
            .unwrap_or(v);
        Ok(match obj {
            Value::Array(list) => Value::Object(
                list.iter()
                    .filter_map(|p| {
                        Some((
                            p["name"].as_str().or(p["key"].as_str())?.to_string(),
                            p.get("value").cloned().unwrap_or(Value::Null),
                        ))
                    })
                    .collect(),
            ),
            Value::Object(m) => Value::Object(
                m.into_iter()
                    .filter(|(_, v)| !v.is_object() && !v.is_array())
                    .collect(),
            ),
            other => json!({ "valeur": other }),
        })
    }

    /// Plan vu du dessus dessiné à partir de l'emprise réelle des maillages et appareils du niveau.
    pub async fn topview(&mut self) -> Result<(String, Value)> {
        let out = self.python(PY_BOUNDS).await?;
        let line = out
            .lines()
            .rev()
            .find(|l| l.trim_start().starts_with('['))
            .ok_or_else(|| anyhow!("emprises illisibles"))?;
        let items: Vec<Value> = serde_json::from_str(line.trim())?;
        Ok(plan_svg(&items))
    }

    /// Options d'un appareil via le DeviceToolset : options classiques, plus réglages @editable s'il s'agit d'un appareil Verse.
    async fn device_props_uefn(&mut self, id: &str) -> Result<Value> {
        let mut out = serde_json::Map::new();
        let mut ok = false;
        let mut first_err = None;
        if let Some(t) = self.known("props_get") {
            match self
                .call_tool(&t, json!({ "devicePath": { "refPath": id } }))
                .await
            {
                Ok(txt) => {
                    let list: Value = serde_json::from_str(txt.trim()).map_err(|_| {
                        anyhow!(
                            "réponse illisible : {}",
                            txt.chars().take(200).collect::<String>()
                        )
                    })?;
                    for o in list.as_array().into_iter().flatten() {
                        if let Some(n) = o["name"].as_str() {
                            out.insert(
                                n.to_string(),
                                o.get("currentValue").cloned().unwrap_or(Value::Null),
                            );
                        }
                    }
                    ok = true;
                }
                Err(e) => first_err = Some(e),
            }
        }
        if let (Some(list), Some(get)) = (
            self.known("verse_props_list"),
            self.known("verse_props_get"),
        ) {
            let dev = json!({ "refPath": id });
            if let Ok(schema) = self.call_tool(&list, json!({ "device": dev })).await {
                ok = true;
                // Schéma de propriétés JSON (vide quand l'appareil n'a aucun réglage @editable).
                let s: Value = serde_json::from_str(schema.trim()).unwrap_or(Value::Null);
                let props = s.get("properties").cloned().unwrap_or(s);
                let names: Vec<String> = match &props {
                    Value::Object(m) => m.keys().cloned().collect(),
                    Value::Array(a) => a
                        .iter()
                        .filter_map(|p| p["name"].as_str().map(String::from))
                        .collect(),
                    _ => vec![],
                };
                if !names.is_empty() {
                    let txt = self
                        .call_tool(&get, json!({ "device": dev, "propertyNames": names }))
                        .await?;
                    match serde_json::from_str::<Value>(txt.trim()) {
                        Ok(Value::Array(list)) => {
                            for p in list {
                                if let Some(n) = p["name"].as_str().or(p["propertyName"].as_str()) {
                                    out.insert(
                                        n.to_string(),
                                        p.get("value").cloned().unwrap_or(Value::Null),
                                    );
                                }
                            }
                        }
                        Ok(Value::Object(m)) => out.extend(
                            m.get("properties")
                                .and_then(|p| p.as_object())
                                .cloned()
                                .unwrap_or(m),
                        ),
                        _ => {
                            out.insert("valeur".into(), json!(txt));
                        }
                    }
                }
            }
        }
        if !ok {
            return Err(first_err.unwrap_or_else(|| anyhow!("UEFN n'expose pas d'outil pour lire les options des appareils (voir pop-link diagnostic).")));
        }
        Ok(Value::Object(out))
    }

    /// Outils retenus pour chaque rôle (affiché dans le diagnostic et dans POP).
    /// Outil de session d'UEFN (`SessionToolset`) : démarrer, arrêter, lire la sortie Verse (#82).
    /// Le résultat est rendu en JSON quand UEFN en renvoie, sinon en texte.
    pub async fn session(&mut self, role: &str, args: Value) -> Result<Value> {
        let t = self.known(role).ok_or_else(|| {
            anyhow!("Outil de session introuvable dans UEFN ({role}) : vérifiez avec « pop-link diagnostic ».")
        })?;
        let out = self.call_tool(&t, args).await?;
        Ok(serde_json::from_str(&out).unwrap_or(Value::String(out)))
    }

    pub fn roles(&self) -> Value {
        let name = |t: Option<Value>| {
            t.and_then(|t| t["name"].as_str().map(String::from))
                .unwrap_or_else(|| "introuvable".into())
        };
        json!({
            "python": name(self.pick("python", &[&["python"], &["exec", "run"]])),
            "compile": name(self.pick("compile", &[&["verse"], &["compile", "build"]])),
            "push": name(self.pick("push", &[&["session"], &["push", "update", "verse", "apply"]])),
            "props_get": name(self.pick("props_get", &[&["propert", "editable", "option"], &["get", "list", "read", "describe"]])),
            "props_set": name(self.pick("props_set", &[&["propert", "editable", "option"], &["set", "update", "edit", "write"]])),
            "checkin": name(self.known("checkin")),
        })
    }

    /// `verse_only` : seul le Verse a changé, UEFN saute la recuisson des assets (bien plus rapide).
    pub async fn push_session(&mut self, verse_only: bool) -> Result<()> {
        let t = self
            .pick(
                "push",
                &[&["session"], &["push", "update", "verse", "apply"]],
            )
            .ok_or_else(|| {
                anyhow!("Outil de session introuvable : lancez une session de test dans UEFN.")
            })?;
        if let Some(s) = self.known("session_status") {
            let st = self.call_tool(&s, json!({})).await.unwrap_or_default();
            if st.trim().eq_ignore_ascii_case("disconnected") {
                bail!("Aucune session de test en cours : lancez-en une dans UEFN (Lancer la session), puis renvoyez.");
            }
        }
        self.call_tool(&t, json!({ "bVerseOnly": verse_only }))
            .await
            .map(|_| ())
    }
}

/// Adresse du serveur MCP : configuration, sinon le .mcp.json généré par UEFN (« Generate Client Config »), sinon le port par défaut.
pub fn find_url(configured: Option<&str>, project: &std::path::Path) -> String {
    if let Some(u) = configured {
        return u.to_string();
    }
    let mut candidates = vec![project.join(".mcp.json")];
    for base in [
        "C:/Program Files/Epic Games/Fortnite/FortniteGame",
        "D:/Epic Games/Fortnite/FortniteGame",
        "C:/Program Files (x86)/Epic Games/Fortnite/FortniteGame",
    ] {
        candidates.push(std::path::PathBuf::from(base).join(".mcp.json"));
    }
    for c in candidates {
        let Ok(s) = std::fs::read_to_string(&c) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&s) else {
            continue;
        };
        if let Some(servers) = v["mcpServers"].as_object() {
            for (_, sv) in servers {
                if let Some(u) = sv["url"].as_str().or(sv["serverUrl"].as_str()) {
                    return u.to_string();
                }
            }
        }
    }
    "http://127.0.0.1:8000/mcp".into()
}

fn plan_svg(items: &[Value]) -> (String, Value) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for it in items {
        let (cx, cy, ex, ey) = (
            it["x"].as_f64().unwrap_or(0.0),
            it["y"].as_f64().unwrap_or(0.0),
            it["ex"].as_f64().unwrap_or(0.0),
            it["ey"].as_f64().unwrap_or(0.0),
        );
        x0 = x0.min(cx - ex);
        y0 = y0.min(cy - ey);
        x1 = x1.max(cx + ex);
        y1 = y1.max(cy + ey);
    }
    if items.is_empty() || x1 <= x0 {
        x0 = -5000.0;
        y0 = -5000.0;
        x1 = 5000.0;
        y1 = 5000.0
    }
    let m = ((x1 - x0).max(y1 - y0)) * 0.05;
    let (x0, y0, x1, y1) = (x0 - m, y0 - m, x1 + m, y1 + m);
    let w = 1200.0;
    let h = (w * (y1 - y0) / (x1 - x0)).round().max(200.0);
    let sx = |x: f64| (x - x0) / (x1 - x0) * w;
    let sy = |y: f64| (y - y0) / (y1 - y0) * h;
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="100%" height="100%" fill="#2e3b2c"/>"##
    );
    let mut sorted: Vec<&Value> = items.iter().collect();
    sorted.sort_by(|a, b| {
        (b["ex"].as_f64().unwrap_or(0.0) * b["ey"].as_f64().unwrap_or(0.0))
            .partial_cmp(&(a["ex"].as_f64().unwrap_or(0.0) * a["ey"].as_f64().unwrap_or(0.0)))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for it in sorted {
        let (cx, cy, ex, ey) = (
            it["x"].as_f64().unwrap_or(0.0),
            it["y"].as_f64().unwrap_or(0.0),
            it["ex"].as_f64().unwrap_or(0.0),
            it["ey"].as_f64().unwrap_or(0.0),
        );
        let z = it["top"].as_f64().unwrap_or(0.0);
        let shade = (90.0 + (z / 30.0).clamp(-40.0, 120.0)) as u8;
        let fill = if it["kind"] == "landscape" {
            "#3d5236".to_string()
        } else {
            format!("rgb({shade},{shade},{})", shade.saturating_sub(8))
        };
        svg += &format!(
            r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{fill}" fill-opacity="0.85" stroke="#000" stroke-opacity="0.25"/>"##,
            sx(cx - ex),
            sy(cy - ey),
            (sx(cx + ex) - sx(cx - ex)).max(1.0),
            (sy(cy + ey) - sy(cy - ey)).max(1.0)
        );
    }
    svg += "</svg>";
    use base64::Engine;
    (
        format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(svg)
        ),
        json!({ "x0": x0, "y0": y0, "x1": x1, "y1": y1 }),
    )
}

/// Liste « - Nom.Du.Toolset: description » renvoyée par `list_toolsets`.
fn parse_toolsets(txt: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![];
    for l in txt.lines() {
        if let Some(rest) = l.strip_prefix("- ") {
            if let Some((n, d)) = rest.split_once(':') {
                if !n.is_empty() && !n.contains(' ') {
                    out.push((n.to_string(), d.trim().to_string()));
                    continue;
                }
            }
        }
        // Description sur plusieurs lignes.
        if let Some(last) = out.last_mut() {
            last.1.push(' ');
            last.1.push_str(l.trim());
        }
    }
    out
}

/// `{"returnValue": x}` → x (texte brut si c'est une chaîne, JSON sinon).
fn unwrap_return(txt: &str) -> String {
    match serde_json::from_str::<Value>(txt.trim()) {
        Ok(Value::Object(m)) if m.len() == 1 && m.contains_key("returnValue") => {
            match &m["returnValue"] {
                Value::String(s) => s.clone(),
                v => v.to_string(),
            }
        }
        _ => txt.to_string(),
    }
}

/// Diagnostic de BuildAll : chemin Verse `/<racine>/sous/x.verse`, lignes et colonnes comptées à partir de 0.
fn build_diag(d: &Value) -> Value {
    let verse_path = d["filePath"].as_str().unwrap_or("").to_string();
    // La racine du paquet Verse du projet correspond au dossier Content.
    let rel = verse_path
        .trim_start_matches('/')
        .split_once('/')
        .map(|(_, r)| format!("Content/{r}"))
        .unwrap_or_else(|| verse_path.clone());
    let sp = &d["span"];
    let severity = d["severity"].as_str().unwrap_or("error").to_lowercase();
    json!({
        "raw_path": rel, "path": rel, "verse_path": verse_path,
        "line": sp["startLine"].as_u64().unwrap_or(0) + 1, "col": sp["startCharacter"].as_u64().unwrap_or(0) + 1,
        "end_line": sp["endLine"].as_u64().unwrap_or(0) + 1, "end_col": sp["endCharacter"].as_u64().unwrap_or(0) + 1,
        "severity": if severity.contains("warn") { "warning" } else if severity.contains("info") || severity.contains("note") { "info" } else { "error" },
        "code": d["code"].as_i64().map(|c| c.to_string()).or_else(|| d["code"].as_str().map(String::from)).unwrap_or_default(),
        "message": d["message"].as_str().unwrap_or(""),
    })
}

fn first_string_prop(tool: &Value) -> Option<String> {
    let props = tool["inputSchema"]["properties"].as_object()?;
    for pref in ["code", "script", "python", "source", "command"] {
        if props.contains_key(pref) {
            return Some(pref.into());
        }
    }
    props
        .iter()
        .find(|(_, v)| v["type"] == "string")
        .map(|(k, _)| k.clone())
}

/// Lignes du type `C:/Projet/Content/a.verse(12,5, 12,20): Script error 3512: message`.
pub fn parse_diagnostics(out: &str) -> Vec<Value> {
    let mut d = vec![];
    for l in out.lines() {
        let Some(i) = l.find(".verse(") else { continue };
        let head = &l[..i + 6];
        // Retire le préfixe du journal (« LogSolarisIde: Error: ») : le chemin commence à la lettre de lecteur.
        let start = head
            .char_indices()
            .find(|&(k, c)| {
                c.is_ascii_alphabetic() && head[k + 1..].starts_with(":/")
                    || c.is_ascii_alphabetic() && head[k + 1..].starts_with(":\\")
            })
            .map(|x| x.0)
            .unwrap_or_else(|| head.rfind(": ").map(|k| k + 2).unwrap_or(0));
        let path_full = &head[start..];
        let rest = &l[i + 7..];
        let Some(close) = rest.find(')') else {
            continue;
        };
        let nums: Vec<usize> = rest[..close]
            .split(|c| c == ',' || c == ' ')
            .filter_map(|x| x.trim().parse().ok())
            .collect();
        let msg = rest[close + 1..].trim_start_matches(':').trim();
        let severity = if msg.to_lowercase().contains("warning") {
            "warning"
        } else {
            "error"
        };
        let code = msg
            .split_whitespace()
            .find_map(|w| w.trim_end_matches(':').parse::<u32>().ok())
            .map(|c| c.to_string())
            .unwrap_or_default();
        let message = msg
            .splitn(2, ':')
            .nth(1)
            .map(|m| m.trim())
            .filter(|m| !m.is_empty())
            .unwrap_or(msg);
        let path = path_full.replace('\\', "/");
        let path = path
            .find("/Content/")
            .map(|k| path[k + 1..].to_string())
            .unwrap_or(path);
        d.push(json!({ "raw_path": path_full.replace('\\', "/"), "path": path, "line": nums.first().copied().unwrap_or(1), "col": nums.get(1).copied().unwrap_or(1), "severity": severity, "code": code, "message": message }));
    }
    d
}

const PY_RESTORE: &str = r#"import unreal, json, base64
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
sel = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem).get_selection_set()
world = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_editor_world()
prev = sub.get_selected_level_actors()
before = set(a.get_path_name() for a in sub.get_all_level_actors())
unreal.TypedElementCommonActions().paste_elements_from_string(sel, world, unreal.TypedElementPasteOptions(), base64.b64decode(SAVED).decode('utf-8'))
sub.set_selected_level_actors(prev)
print(json.dumps([a.get_path_name() for a in sub.get_all_level_actors() if a.get_path_name() not in before]))
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
    vcls = ''
    if kind == 'verse_device':
        # La classe d'un appareil Verse posé est VerseDevice_C ; sa classe Verse est celle de l'objet « script ».
        try:
            sc = a.get_editor_property('script')
            if sc: vcls = sc.get_class().get_name()
        except Exception:
            pass
    tags = [str(x) for x in a.tags]
    try:
        for comp in a.get_components_by_class(unreal.ActorComponent):
            if 'TagMarkup' in comp.get_class().get_name():
                for t in comp.get_editor_property('tags') or []:
                    n = t.get_name() if hasattr(t, 'get_name') else str(t)
                    if n and n not in tags: tags.append(n)
    except Exception:
        pass
    out.append({'id': a.get_path_name(), 'name': a.get_actor_label(), 'type': kind, 'class': cls, 'verse_class': vcls, 'folder': str(a.get_folder_path()),
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

/// Pose un appareil (#81) : depuis un asset du Content Browser, ou sur le modèle d'un acteur du niveau (copie avec
/// ses réglages). Nom, dossier, position et rotation facultatifs ; rotation en [pitch, yaw, roll] comme la liste.
const PY_SPAWN: &str = r#"import unreal, json
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
A = json.loads(ARGS)
if A.get('like'):
    src = [a for a in sub.get_all_level_actors() if a.get_path_name() == A['like']]
    if not src: raise Exception('Appareil modèle introuvable dans le niveau : ' + A['like'])
    act = sub.duplicate_actors(src, None, unreal.Vector(300, 0, 0))[0]
else:
    obj = unreal.EditorAssetLibrary.load_asset(A['asset'])
    if obj is None: raise Exception('Asset introuvable : ' + A['asset'])
    act = sub.spawn_actor_from_object(obj, unreal.Vector(0, 0, 0), unreal.Rotator(0, 0, 0))
    if act is None: raise Exception('UEFN refuse de poser cet asset : ' + A['asset'])
loc = A.get('location')
if loc: act.set_actor_location(unreal.Vector(*loc), False, False)
rot = A.get('rotation')
if rot: act.set_actor_rotation(unreal.Rotator(roll=rot[2], pitch=rot[0], yaw=rot[1]), False)
if A.get('name'): act.set_actor_label(A['name'])
if A.get('folder'): act.set_folder_path(A['folder'])
print('SPAWNED=' + act.get_path_name())
"#;

const PY_DUP: &str = r#"import unreal, json
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
src = [a for a in sub.get_all_level_actors() if a.get_path_name() in IDS]
new = sub.duplicate_actors(src, None, unreal.Vector(300, 0, 0))
print(json.dumps([a.get_path_name() for a in new]))
"#;

const PY_DEL: &str = r#"import unreal, base64
sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
sel = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem).get_selection_set()
targets = [a for a in sub.get_all_level_actors() if a.get_path_name() in IDS]
prev = sub.get_selected_level_actors()
sub.set_selected_level_actors(targets)
txt = unreal.TypedElementCommonActions().copy_selected_elements_to_string(sel) if targets else ''
sub.set_selected_level_actors([a for a in prev if a not in targets])
if targets and not txt:
    raise RuntimeError('sauvegarde impossible')
sub.destroy_actors(targets)
print('T3D=' + base64.b64encode((txt or '').encode('utf-8')).decode('ascii'))
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

    #[test]
    fn build_all() {
        let v: serde_json::Value = serde_json::from_str(r#"{"severity":"Warning","code":2009,"message":"deprecated","filePath":"/bb30-guid/ai/vault_npc_thief.verse","span":{"startLine":135,"startCharacter":53,"endLine":135,"endCharacter":64}}"#).unwrap();
        let d = super::build_diag(&v);
        assert_eq!(d["path"], "Content/ai/vault_npc_thief.verse");
        assert_eq!(d["line"], 136);
        assert_eq!(d["col"], 54);
        assert_eq!(d["severity"], "warning");
        assert_eq!(d["code"], "2009");
    }

    #[test]
    fn return_value() {
        assert_eq!(
            super::unwrap_return(r#"{"returnValue":"a\nb\n"}"#),
            "a\nb\n"
        );
        assert_eq!(super::unwrap_return(r#"{"returnValue":[1,2]}"#), "[1,2]");
        assert_eq!(super::unwrap_return("texte"), "texte");
    }

    #[test]
    fn toolsets() {
        let t = super::parse_toolsets("- A.B: un\nsuite\n- C.D: deux");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].0, "A.B");
        assert_eq!(t[0].1, "un suite");
    }
}
