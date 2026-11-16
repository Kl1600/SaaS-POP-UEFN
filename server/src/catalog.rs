//! Catalogue des appareils classiques : libellés, valeurs autorisées, aide en français, valeur par défaut
//! et règles d'options inactives. Il complète ce qu'UEFN renvoie (qui ne fournit ni l'aide ni les règles).
use serde_json::{json, Value};

fn opt(key: &str, label: &str, kind: &str, values: &[&str], default: Value, help: &str) -> Value {
    json!({ "key": key, "label": label, "type": kind, "values": values, "default": default, "help": help })
}

/// (classe, nom affiché, icône, options, règles « inactive si ») ; règle = (option, option qui commande, valeurs qui activent, raison).
pub fn device(class: &str) -> Option<(&'static str, &'static str, Vec<Value>, Vec<(&'static str, &'static str, Vec<&'static str>, &'static str)>)> {
    let c = class.to_lowercase();
    Some(if c.contains("item_spawner") || c.contains("itemspawner") {
        ("Distributeur d’objets", "◆", vec![
            opt("item", "Objet", "text", &[], json!(""), "Objet donné ou posé par le distributeur."),
            opt("respawn", "Réapparition", "enum", &["Jamais", "Après ramassage", "Minuteur"], json!("Après ramassage"), "Quand l’objet revient après avoir été pris."),
            opt("respawn_time", "Temps de réapparition (s)", "number", &[], json!(10), "Délai avant le retour de l’objet."),
            opt("enabled_at_start", "Activé au début", "bool", &[], json!(true), "Le distributeur fonctionne dès le début de la partie."),
            opt("team", "Équipe autorisée", "enum", &["Toutes", "Équipe 1", "Équipe 2"], json!("Toutes"), "Seule cette équipe peut ramasser l’objet."),
            opt("visible", "Visible en jeu", "bool", &[], json!(false), "Affiche le socle du distributeur aux joueurs."),
        ], vec![("respawn_time", "respawn", vec!["Minuteur"], "Inactif : « Réapparition » n’est pas sur « Minuteur ».")])
    } else if c.contains("trigger") {
        ("Déclencheur", "⚡", vec![
            opt("triggered_by", "Déclenchable par", "enum", &["Joueur", "Véhicule", "Tous"], json!("Joueur"), "Ce qui active le déclencheur en marchant dessus."),
            opt("times", "Nombre de déclenchements", "number", &[], json!(0), "0 = illimité."),
            opt("delay", "Délai (s)", "number", &[], json!(0), "Attente avant d’envoyer le signal."),
            opt("visible", "Visible en jeu", "bool", &[], json!(false), "Montre la plaque aux joueurs."),
            opt("sound", "Son", "bool", &[], json!(true), "Joue un son quand il est activé."),
        ], vec![])
    } else if c.contains("button") {
        ("Bouton", "●", vec![
            opt("text", "Texte d’interaction", "text", &[], json!("Interagir"), "Texte affiché au joueur."),
            opt("duration", "Durée d’interaction (s)", "number", &[], json!(0), "Temps de maintien pour activer."),
            opt("team", "Équipe autorisée", "enum", &["Toutes", "Équipe 1", "Équipe 2"], json!("Toutes"), "Équipe qui peut appuyer."),
            opt("uses", "Nombre d’utilisations", "number", &[], json!(0), "0 = illimité."),
        ], vec![])
    } else if c.contains("player_spawn") || c.contains("playerspawn") {
        ("Point d’apparition", "A", vec![
            opt("team", "Équipe", "enum", &["Toutes", "Équipe 1", "Équipe 2"], json!("Toutes"), "Équipe qui apparaît ici."),
            opt("enabled", "Activé", "bool", &[], json!(true), "Le point est utilisable."),
            opt("visible", "Visible en jeu", "bool", &[], json!(false), "Montre le socle aux joueurs."),
        ], vec![])
    } else if c.contains("barrier") {
        ("Barrière", "▮", vec![
            opt("enabled_at_start", "Activée au début", "bool", &[], json!(true), "La barrière bloque dès le début."),
            opt("material", "Matériau", "enum", &["Verre", "Énergie", "Invisible"], json!("Énergie"), "Apparence de la barrière."),
            opt("block_shots", "Bloque les tirs", "bool", &[], json!(true), "Les projectiles ne traversent pas."),
            opt("team", "Équipe qui peut passer", "enum", &["Aucune", "Équipe 1", "Équipe 2"], json!("Aucune"), "Cette équipe traverse la barrière."),
        ], vec![("block_shots", "material", vec!["Verre", "Énergie"], "Inactif : une barrière invisible ne bloque jamais les tirs.")])
    } else if c.contains("timer") {
        ("Minuteur", "◷", vec![
            opt("duration", "Durée (s)", "number", &[], json!(480), "Durée du compte."),
            opt("direction", "Sens", "enum", &["Décompte", "Chronomètre"], json!("Décompte"), "Compte à rebours ou chronomètre."),
            opt("start_at_begin", "Démarrer au début", "bool", &[], json!(true), "Démarre avec la manche."),
            opt("show", "Afficher le temps", "bool", &[], json!(true), "Affiche le temps à l’écran."),
        ], vec![("duration", "direction", vec!["Décompte"], "Inactif : un chronomètre n’a pas de durée.")])
    } else if c.contains("vending") {
        ("Distributeur automatique", "€", vec![
            opt("item", "Objet vendu", "text", &[], json!(""), "Objet proposé à l’achat."),
            opt("cost", "Prix", "number", &[], json!(100), "Coût de l’objet."),
            opt("currency", "Ressource", "enum", &["Or", "Bois", "Pierre", "Métal"], json!("Or"), "Ce que le joueur dépense."),
        ], vec![])
    } else {
        return None;
    })
}

/// Ajoute à un acteur son schéma d'options (aide, valeurs autorisées, options inactives et pourquoi).
fn norm(k: &str) -> String {
    k.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// Libellé, icône et aide en français pour les appareils connus. Les options affichées sont uniquement
/// celles réellement lues dans UEFN ; le catalogue n'apporte que l'aide, les valeurs permises et les règles.
pub fn enrich(actor: &mut Value) {
    let class = actor["class"].as_str().unwrap_or("").to_string();
    let opts = actor["options"].as_object().cloned().unwrap_or_default();
    let props = actor["props"].as_object().cloned().unwrap_or_default();
    let known = device(&class);
    if let Some((label, icon, _, _)) = &known {
        actor["kind_label"] = json!(label);
        actor["icon"] = json!(icon);
    }
    let source = if actor["type"] == "verse_device" && !props.is_empty() { &props } else { &opts };
    if !actor["schema"].is_null() && actor["schema"].as_array().map(|a| !a.is_empty()).unwrap_or(false) && source.is_empty() {
        return;
    }
    let mut schema = vec![];
    for (k, v) in source {
        let mut o = json!({ "key": k, "label": k, "type": if v.is_boolean() { "bool" } else if v.is_number() { "number" } else { "text" }, "values": [], "default": null, "help": "" });
        if let Some((_, _, cat, rules)) = &known {
            if let Some(c) = cat.iter().find(|c| norm(c["key"].as_str().unwrap_or("")) == norm(k) || norm(c["label"].as_str().unwrap_or("")) == norm(k)) {
                for f in ["label", "help", "default"] { o[f] = c[f].clone() }
                if c["type"] == "enum" && c["values"].as_array().map(|vals| vals.iter().any(|x| x == v)).unwrap_or(false) {
                    o["type"] = json!("enum");
                    o["values"] = c["values"].clone();
                }
                let ckey = c["key"].as_str().unwrap_or("");
                if let Some((_, ctrl, ok, why)) = rules.iter().find(|r| r.0 == ckey) {
                    let cur = source.iter().find(|(kk, _)| norm(kk) == norm(ctrl)).map(|(_, vv)| vv.as_str().map(String::from).unwrap_or_else(|| vv.to_string()));
                    if let Some(cur) = cur { if !ok.iter().any(|x| *x == cur) { o["disabled"] = json!(why) } }
                }
            }
        }
        schema.push(o);
    }
    actor["schema"] = json!(schema);
}
