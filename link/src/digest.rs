//! Lecture des digests Verse (API Fortnite, Verse, UnrealEngine, assets du projet) pour l'autocomplétion.
use serde_json::{json, Value};

const EFFECTS: [&str; 10] = ["decides", "transacts", "suspends", "varies", "computes", "reads", "writes", "allocates", "converges", "no_rollback"];

fn ident(s: &str) -> Option<&str> {
    let end = s.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(s.len());
    (end > 0 && !s.chars().next().unwrap().is_ascii_digit()).then(|| &s[..end])
}

/// Extrait les déclarations publiques : modules, classes, fonctions, champs, événements.
pub fn parse(source: &str, file: &str, out: &mut Vec<Value>) {
    let mut doc: Vec<String> = vec![];
    let mut stack: Vec<(usize, String)> = vec![]; // (indentation, conteneur)
    let mut module_path = String::new();
    for raw in source.lines() {
        let indent = raw.len() - raw.trim_start().len();
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(c) = line.strip_prefix('#') {
            let c = c.trim();
            if let Some(p) = c.strip_prefix("Module import path:") {
                module_path = p.trim().to_string();
            } else {
                doc.push(c.to_string());
            }
            continue;
        }
        while stack.last().map(|(i, _)| *i >= indent).unwrap_or(false) {
            stack.pop();
        }
        if line.starts_with("using") || line.starts_with('@') {
            doc.clear();
            continue;
        }
        let Some(name) = ident(line) else { doc.clear(); continue };
        let rest = &line[name.len()..];
        if !rest.starts_with('<') && !rest.starts_with('(') && !rest.starts_with(':') && !rest.trim_start().starts_with(":=") {
            doc.clear();
            continue;
        }
        if rest.starts_with('<') && !rest.starts_with("<public>") && !rest.starts_with("<native><public>") && !rest.contains("<public>") {
            doc.clear();
            continue;
        }
        let sig = line.trim_end_matches(" = external {}").trim_end_matches(':').to_string();
        let kind = if rest.contains(":= module") || rest.contains(":=module") {
            "module"
        } else if rest.contains(":= class") || rest.contains(":= interface") || rest.contains(":= struct") || rest.contains(":= enum") {
            if rest.contains(":= enum") { "enum" } else if rest.contains(":= struct") { "struct" } else if rest.contains(":= interface") { "interface" } else { "class" }
        } else if rest.contains("listenable(") || name.ends_with("Event") {
            "event"
        } else if rest.contains('(') && rest.find('(') < rest.find(':') {
            "function"
        } else {
            "field"
        };
        let effects: Vec<&str> = EFFECTS.iter().copied().filter(|e| line.contains(&format!("<{e}>"))).collect();
        let container = stack.last().map(|(_, c)| c.clone()).unwrap_or_default();
        out.push(json!({
            "name": name, "kind": kind, "signature": sig, "effects": effects, "doc": doc.join(" "),
            "container": container, "module": module_path, "source": file,
        }));
        if matches!(kind, "module" | "class" | "struct" | "interface" | "enum") {
            stack.push((indent, name.to_string()));
        }
        doc.clear();
        if out.len() > 40_000 {
            return;
        }
    }
}

/// Tags Verse déclarés dans le projet : `nom := class(tag){}`.
pub fn tags(files: &[(String, String)]) -> Vec<String> {
    let mut t = vec![];
    for (_, c) in files {
        for l in c.lines() {
            let l = l.trim();
            if let Some(i) = l.find(":= class(tag)") {
                let n = l[..i].trim();
                let n = n.split('<').next().unwrap_or(n).trim();
                if ident(n) == Some(n) && !t.contains(&n.to_string()) {
                    t.push(n.to_string());
                }
            }
        }
    }
    t.sort();
    t
}

/// Petit digest d'exemple pour le mode simulation (signatures courantes de l'API Fortnite).
#[cfg(test)]
pub const SAMPLE: &str = r#"# Module import path: /Fortnite.com/Devices
Devices<public> := module:
    # Base de tous les appareils créatifs écrits en Verse.
    creative_device<public> := class<concrete>(creative_object):
        # Appelé au début de la partie.
        OnBegin<public>()<suspends>:void = external {}
        # Appelé à la fin de la partie.
        OnEnd<public>():void = external {}
        # Position de l'appareil.
        GetTransform<public>()<transacts>:transform = external {}

    # Bouton sur lequel un joueur peut interagir.
    button_device<public> := class<concrete><final>(creative_device_base):
        # Signalé quand un agent interagit avec le bouton.
        InteractedWithEvent<public>:listenable(agent) = external {}
        Enable<public>():void = external {}
        Disable<public>():void = external {}
        SetInteractionText<public>(Text:message):void = external {}

    # Plaque qui envoie un signal quand on marche dessus.
    trigger_device<public> := class<concrete><final>(creative_device_base):
        TriggeredEvent<public>:listenable(?agent) = external {}
        Trigger<public>(Agent:agent):void = external {}
        Reset<public>():void = external {}

    # Distribue un objet.
    item_spawner_device<public> := class<concrete><final>(creative_device_base):
        ItemPickedUpEvent<public>:listenable(agent) = external {}
        SpawnItem<public>():void = external {}
        Enable<public>():void = external {}
        Disable<public>():void = external {}

    # Compte à rebours ou chronomètre.
    timer_device<public> := class<concrete><final>(creative_device_base):
        SuccessEvent<public>:listenable(?agent) = external {}
        Start<public>():void = external {}
        Pause<public>():void = external {}
        SetActiveDuration<public>(Time:float):void = external {}

    # Point d'apparition d'un joueur.
    player_spawner_device<public> := class<concrete><final>(creative_device_base):
        SpawnedEvent<public>:listenable(agent) = external {}

# Module import path: /Fortnite.com/Characters
Characters<public> := module:
    # Renvoie le personnage d'un agent s'il existe.
    GetFortCharacter<public>(Agent:agent)<transacts><decides>:fort_character = external {}

# Module import path: /Verse.org/Simulation
Simulation<public> := module:
    # Attend un nombre de secondes.
    Sleep<public>(Seconds:float)<suspends>:void = external {}
    # Renvoie la session en cours.
    GetSession<public>()<transacts>:session = external {}

# Module import path: /Verse.org/Verse
Verse<public> := module:
    # Écrit un message dans le journal.
    Print<public>(Message:string, ?Duration:float = 2.0)<transacts>:void = external {}
    # Convertit un nombre en chaîne.
    ToString<public>(Value:int)<computes>:string = external {}
"#;

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn sample() {
        let mut v = vec![];
        parse(SAMPLE, "Fortnite.digest.verse", &mut v);
        let b = v.iter().find(|s| s["name"] == "InteractedWithEvent").unwrap();
        assert_eq!(b["container"], "button_device");
        assert_eq!(b["kind"], "event");
        let g = v.iter().find(|s| s["name"] == "GetFortCharacter").unwrap();
        assert!(g["effects"].as_array().unwrap().iter().any(|e| e == "decides"));
        assert_eq!(g["module"], "/Fortnite.com/Characters");
        assert_eq!(tags(&[("a".into(), "vault_plate := class(tag){}".into())]), vec!["vault_plate"]);
    }
}
