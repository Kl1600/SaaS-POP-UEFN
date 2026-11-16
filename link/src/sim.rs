//! Mode simulation : un faux UEFN (niveau VaultRush en mémoire, compilation approximative) pour tester
//! toute la chaîne sans Windows ni UEFN.
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::Path;

pub struct Sim {
    pub actors: Vec<Value>,
    pub closed: bool,
    next: u32,
}

fn actor(id: &str, name: &str, kind: &str, class: &str, folder: &str, loc: [f64; 3], tags: &[&str], options: Value) -> Value {
    json!({
        "id": id, "name": name, "type": kind, "class": class, "folder": folder, "hidden": false,
        "transform": { "location": loc, "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0] },
        "tags": tags, "options": options, "props": {}, "events": []
    })
}

impl Sim {
    pub fn new() -> Self {
        let mut a = vec![];
        for i in 1..=14 {
            let x = -2600.0 + ((i - 1) % 7) as f64 * 700.0;
            let y = -1800.0 + ((i - 1) / 7) as f64 * 900.0;
            a.push(actor(&format!("trigger_{i:02}"), &format!("vault_plate_{i:02}"), "device", "trigger_device", "Banque/Coffres", [x, y, 0.0], &["vault_plate"],
                json!({ "triggered_by": "Joueur", "times": 1, "delay": 0, "visible": false, "sound": true })));
        }
        for p in 1..=3 {
            for j in 1..=2 {
                let base = [[-9000.0, 5200.0], [-6500.0, -6000.0], [8200.0, 6400.0]][p - 1];
                a.push(actor(&format!("spawner_{p}_{j}"), &format!("Distributeur lingot P{p}-{j}"), "device", "item_spawner_device", &format!("Planques/Planque {p}"),
                    [base[0] + j as f64 * 400.0, base[1], 0.0], &["loot_spawner"],
                    json!({ "item": "Lingot d’or", "respawn": if j == 1 { "Minuteur" } else { "Après ramassage" }, "respawn_time": 30, "enabled_at_start": true, "team": "Équipe 1", "visible": false })));
            }
        }
        for i in 1..=4 {
            a.push(actor(&format!("spawn_r{i}"), &format!("Spawn voleurs {i}"), "device", "player_spawner_device", "Fuite", [-11000.0, -2000.0 + i as f64 * 500.0, 0.0], &["spawn_robbers"], json!({ "team": "Équipe 1", "enabled": true, "visible": false })));
            a.push(actor(&format!("spawn_c{i}"), &format!("Spawn gendarmes {i}"), "device", "player_spawner_device", "Commissariat", [9000.0, -5200.0 + i as f64 * 500.0, 0.0], &["spawn_cops"], json!({ "team": "Équipe 2", "enabled": true, "visible": false })));
        }
        a.push(actor("barrier_vault", "Barrière coffre", "device", "barrier_device", "Banque", [-500.0, -2600.0, 0.0], &[], json!({ "enabled_at_start": true, "material": "Énergie", "block_shots": true, "team": "Aucune" })));
        a.push(actor("barrier_jail", "Barrière prison", "device", "barrier_device", "Commissariat", [8600.0, -3800.0, 0.0], &["jail"], json!({ "enabled_at_start": true, "material": "Verre", "block_shots": true, "team": "Équipe 2" })));
        a.push(actor("timer_round", "Minuteur manche", "device", "timer_device", "Banque", [0.0, 0.0, 400.0], &[], json!({ "duration": 480, "direction": "Décompte", "start_at_begin": true, "show": true })));
        a.push(actor("button_alarm", "Bouton alarme", "device", "button_device", "Banque", [-300.0, 2400.0, 0.0], &["alarm"], json!({ "text": "Déclencher l’alarme", "duration": 1, "team": "Équipe 2", "uses": 0 })));
        a.push(actor("button_jail", "Bouton porte prison", "device", "button_device", "Commissariat", [8900.0, -3300.0, 0.0], &["jail"], json!({ "text": "Ouvrir la cellule", "duration": 3, "team": "Équipe 1", "uses": 0 })));
        a.push(actor("shop_robbers", "Boutique voleurs", "device", "vending_machine_device", "Fuite", [-10500.0, 1200.0, 0.0], &["shop"], json!({ "item": "Perceuse", "cost": 300, "currency": "Or" })));
        a.push(actor("shop_cops", "Boutique gendarmes", "device", "vending_machine_device", "Commissariat", [9800.0, -2000.0, 0.0], &["shop"], json!({ "item": "Menottes", "cost": 200, "currency": "Or" })));
        let mut vm = actor("vault_manager", "vault_manager", "verse_device", "vault_manager_device", "Banque", [0.0, -400.0, 0.0], &[], json!({}));
        vm["props"] = json!({ "VaultReward": 250, "ArrestBounty": 120, "AlarmThreshold": 0.5, "VaultPlates": "14 plaques (tag vault_plate)" });
        vm["schema"] = json!([
            { "key": "VaultReward", "label": "VaultReward", "type": "number", "values": [], "default": 250, "help": "Butin gagné par coffre (propriété @editable du code Verse)." },
            { "key": "ArrestBounty", "label": "ArrestBounty", "type": "number", "values": [], "default": 120, "help": "Prime par arrestation." },
            { "key": "AlarmThreshold", "label": "AlarmThreshold", "type": "number", "values": [], "default": 0.5, "help": "Part du butin volé qui déclenche l’alarme." }
        ]);
        a.push(vm);
        for (id, n, x, y) in [("sm_bank", "SM_Banque", -500.0, -1000.0), ("sm_police", "SM_Commissariat", 9000.0, -3600.0), ("sm_road_1", "SM_Route_01", 0.0, 3500.0), ("sm_road_2", "SM_Route_02", 4500.0, 0.0)] {
            a.push(actor(id, n, "mesh", "StaticMeshActor", "Décor", [x, y, 0.0], &[], json!({})));
        }
        // Liens d'événements : qui déclenche quoi.
        let link = |a: &mut Vec<Value>, from: &str, ev: &str, to: &str, f: &str| {
            if let Some(x) = a.iter_mut().find(|x| x["id"] == from) {
                x["events"].as_array_mut().unwrap().push(json!({ "event": ev, "target": to, "function": f }));
            }
        };
        link(&mut a, "button_alarm", "InteractedWithEvent", "timer_round", "Start");
        link(&mut a, "trigger_01", "TriggeredEvent", "barrier_vault", "Disable");
        link(&mut a, "button_jail", "InteractedWithEvent", "barrier_jail", "Disable");
        Self { actors: a, closed: false, next: 1 }
    }

    pub fn locks() -> Value {
        json!([{ "target": "barrier_jail", "by": "chloe@demo.pop" }, { "target": "Content/escape_heli.verse", "by": "chloe@demo.pop" }])
    }

    fn find(&mut self, id: &str) -> Result<&mut Value> {
        self.actors.iter_mut().find(|a| a["id"] == id).ok_or_else(|| anyhow!("acteur introuvable : {id}"))
    }

    pub fn patch(&mut self, patches: &[Value]) -> Result<()> {
        for p in patches {
            let id = p["id"].as_str().unwrap_or("").to_string();
            let a = self.find(&id)?;
            if let Some(ch) = p["changes"].as_object() {
                for (k, v) in ch {
                    match k.as_str() {
                        "options" | "props" => {
                            for (ok, ov) in v.as_object().cloned().unwrap_or_default() {
                                a[k][ok] = ov;
                            }
                        }
                        _ => a[k] = v.clone(),
                    }
                }
            }
        }
        Ok(())
    }

    pub fn duplicate(&mut self, ids: &[String]) -> Result<Vec<String>> {
        let mut created = vec![];
        for id in ids {
            let mut c = self.find(id)?.clone();
            let nid = format!("{id}_copie{}", self.next);
            self.next += 1;
            c["id"] = json!(nid);
            c["name"] = json!(format!("{} (copie)", c["name"].as_str().unwrap_or("")));
            if let Some(l) = c["transform"]["location"].as_array_mut() {
                l[0] = json!(l[0].as_f64().unwrap_or(0.0) + 300.0);
            }
            c["events"] = json!([]);
            self.actors.push(c);
            created.push(nid);
        }
        Ok(created)
    }

    pub fn delete(&mut self, ids: &[String]) {
        self.actors.retain(|a| !ids.iter().any(|i| a["id"] == *i));
        for a in self.actors.iter_mut() {
            if let Some(ev) = a["events"].as_array_mut() {
                ev.retain(|e| !ids.iter().any(|i| e["target"] == *i));
            }
        }
    }

    pub fn restore(&mut self, actors: &[Value]) {
        for a in actors {
            if !self.actors.iter().any(|x| x["id"] == a["id"]) {
                self.actors.push(a.clone());
            }
        }
    }

    /// Compilation simulée : parenthèses non fermées, effet no_rollback dans une condition, appareil sans `using`.
    pub fn compile(files: &[(String, String)]) -> Vec<Value> {
        let mut d = vec![];
        for (path, src) in files {
            let mut stack: Vec<(char, usize, usize)> = vec![];
            let mut in_str = false;
            for (ln, line) in src.lines().enumerate() {
                let code = line.split('#').next().unwrap_or("");
                for (col, ch) in code.chars().enumerate() {
                    if ch == '"' {
                        in_str = !in_str;
                    }
                    if in_str {
                        continue;
                    }
                    match ch {
                        '(' | '[' | '{' => stack.push((ch, ln + 1, col + 1)),
                        ')' | ']' | '}' => {
                            let want = match ch { ')' => '(', ']' => '[', _ => '{' };
                            if stack.last().map(|s| s.0) == Some(want) {
                                stack.pop();
                            } else {
                                d.push(json!({ "path": path, "line": ln + 1, "col": col + 1, "severity": "error", "code": "3100", "message": format!("« {ch} » inattendu.") }));
                            }
                        }
                        _ => {}
                    }
                }
                in_str = false;
                let t = code.trim_start();
                if (t.starts_with("if (") || t.starts_with("if(")) && t.contains("Print(") {
                    d.push(json!({ "path": path, "line": ln + 1, "col": line.find("Print(").unwrap_or(0) + 1, "severity": "error", "code": "3512",
                        "message": "Cet appel a l’effet « no_rollback » et ne peut pas être utilisé dans une condition (contexte <decides>)." }));
                }
                if t.contains(":= class(creative_device)") && !src.contains("using { /Fortnite.com/Devices }") {
                    d.push(json!({ "path": path, "line": ln + 1, "col": 1, "severity": "error", "code": "3506", "message": "Identifiant inconnu « creative_device » : ajoutez using { /Fortnite.com/Devices }." }));
                }
                if t.contains("<decides>") && !t.contains("<transacts>") && t.contains("():") {
                    d.push(json!({ "path": path, "line": ln + 1, "col": 1, "severity": "warning", "code": "W1", "message": "Fonction <decides> sans <transacts> : vérifiez les effets." }));
                }
            }
            for (c, l, col) in stack {
                d.push(json!({ "path": path, "line": l, "col": col, "severity": "error", "code": "3100", "message": format!("« {c} » jamais fermé.") }));
            }
        }
        d
    }

    /// Vue du dessus générée (SVG) avec les bornes du monde en centimètres.
    pub fn topview() -> (String, Value) {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800" viewBox="0 0 1200 800">
<rect width="1200" height="800" fill="#2f3a2c"/>
<rect x="0" y="560" width="1200" height="240" fill="#2b4a63"/>
<rect x="0" y="330" width="1200" height="40" fill="#5b5b5b"/><rect x="820" y="0" width="40" height="560" fill="#5b5b5b"/><rect x="280" y="0" width="30" height="330" fill="#5b5b5b"/>
<rect x="400" y="200" width="300" height="160" fill="#c8c2b4"/><text x="550" y="290" font-size="22" text-anchor="middle" font-family="sans-serif">Banque</text>
<rect x="980" y="130" width="160" height="150" fill="#9fb3c8"/><text x="1060" y="210" font-size="18" text-anchor="middle" font-family="sans-serif">Commissariat</text>
<rect x="110" y="560" width="120" height="40" fill="#6b4f36"/><text x="170" y="620" font-size="16" fill="#fff" text-anchor="middle" font-family="sans-serif">Port</text>
</svg>"##;
        use base64::Engine;
        let uri = format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(svg));
        (uri, json!({ "x0": -12000.0, "y0": -8000.0, "x1": 12000.0, "y1": 8000.0 }))
    }

    /// Fichiers d'exemple créés dans un projet vide.
    pub fn seed_project(root: &Path) -> Result<()> {
        let c = root.join("Content");
        std::fs::create_dir_all(&c)?;
        let files = [
            ("tags.verse", "using { /Verse.org/Simulation/Tags }\n\nvault_plate := class(tag){}\nloot_spawner := class(tag){}\nspawn_robbers := class(tag){}\nspawn_cops := class(tag){}\njail := class(tag){}\nalarm := class(tag){}\nshop := class(tag){}\n"),
            ("vault_manager.verse", "using { /Fortnite.com/Devices }\nusing { /Verse.org/Simulation }\n\n# Gère le braquage : coffres, butin et alarme.\nvault_manager_device := class(creative_device):\n\n    @editable\n    VaultReward : int = 250\n\n    @editable\n    ArrestBounty : int = 120\n\n    @editable\n    AlarmThreshold : float = 0.5\n\n    @editable\n    RoundTimer : timer_device = timer_device{}\n\n    var Stolen : int = 0\n\n    OnBegin<override>()<suspends> : void =\n        Print(\"Braquage prêt\")\n        RoundTimer.Start()\n\n    OnVaultOpened(Agent : agent) : void =\n        set Stolen += VaultReward\n        Print(\"Coffre ouvert : {Stolen}\")\n"),
            ("hud_loot.verse", "using { /Fortnite.com/Devices }\nusing { /Verse.org/Simulation }\n\n# #14 Compteur de butin dans le HUD.\nhud_loot_device := class(creative_device):\n\n    OnBegin<override>()<suspends> : void =\n        Print(\"HUD butin prêt\")\n"),
            ("escape_heli.verse", "using { /Fortnite.com/Devices }\n\n# #21 Hélicoptère d'évasion (verrouillé par Chloé dans Lore).\nescape_heli_device := class(creative_device):\n\n    @editable\n    LandingPad : trigger_device = trigger_device{}\n"),
        ];
        for (n, body) in files {
            let p = c.join(n);
            if !p.exists() {
                std::fs::write(p, body)?;
            }
        }
        std::fs::write(root.join("Fortnite.digest.verse"), crate::digest::SAMPLE)?;
        Ok(())
    }
}
