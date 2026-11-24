//! Moteur d'analyse Verse de POP : 100 % local, sans service externe.
//! Il découpe le fichier (classes, réglages @editable, variables, fonctions, effets, abonnements),
//! explique, relit, corrige les erreurs fréquentes par règles et écrit des modules à partir de modèles.
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}
re!(RE_USING, r"^\s*using\s*\{\s*([^}]+?)\s*\}");
re!(
    RE_CLASS,
    r"^(\s*)([A-Za-z_]\w*)\s*(?:<[^>]*>)*\s*:=\s*(class|struct|enum|interface|module)\s*(?:<[^>]*>)*\s*(?:\(([^)]*)\))?\s*:?"
);
re!(
    RE_FIELD,
    r"^(\s*)(var\s+)?([A-Za-z_]\w*)\s*(?:<[^>]*>)*\s*:\s*([^=]+?)\s*(?:=\s*(.+))?$"
);
re!(
    RE_FUNC,
    r"^(\s*)([A-Za-z_]\w*)\s*((?:<[^>]*>)*)\s*\(([^)]*)\)\s*((?:<[^>]*>)*)\s*:\s*([^=]+?)\s*=\s*(.*)$"
);
re!(
    RE_SUB,
    r"([A-Za-z_][\w.]*?)\.([A-Za-z_]\w*Event)\.Subscribe\(\s*([A-Za-z_]\w*)\s*\)"
);
// Seulement pour l'assistant Verse local (explain_line), gardé pour Forge (#86).
#[allow(dead_code)]
static RE_CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z_][\w]*(?:\.[A-Za-z_]\w*)*)\s*(\(|\[)").unwrap());
re!(RE_EFFECT, r"<([a-z_]+)>");
re!(RE_INT_DECIMAL, r":\s*int\s*=\s*(\d+\.\d+)");
re!(RE_IDENT, r"[A-Za-z_]\w*");
re!(RE_INT_ARG, r"(Sleep|Seconds|Delay)\s*\(\s*(\d+)\s*\)");
re!(
    RE_COMPOUND,
    r"^(\s*)([A-Za-z_]\w*(?:\[[^\]]*\])?)\s*(\+=|-=|\*=|/=|=)\s*(.+)$"
);
re!(RE_QUOTED, r#"[`'"‘“]([A-Za-z_][\w]*)[`'"’”]"#);

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub default: Option<String>,
    pub editable: bool,
    pub var: bool,
    pub line: usize,
}
#[allow(dead_code)]
// Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
#[derive(Debug, Clone)]
pub struct Func {
    pub name: String,
    pub params: String,
    pub effects: Vec<String>,
    pub ret: String,
    pub line: usize,
    pub end: usize,
    pub indent: usize,
}
#[allow(dead_code)]
// Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub kind: String,
    pub parent: String,
    pub line: usize,
    pub fields: Vec<Field>,
    pub funcs: Vec<Func>,
    pub doc: String,
}
#[allow(dead_code)]
// Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
#[derive(Debug, Default)]
pub struct Analysis {
    pub usings: Vec<String>,
    pub classes: Vec<Class>,
    pub subs: Vec<(String, String, String, usize)>,
    pub top_comment: String,
    pub lines: usize,
}
#[derive(Debug, Clone)]
pub struct Issue {
    pub line: usize,
    pub severity: &'static str,
    pub text: String,
}

/// Contexte du projet : symboles des digests (nom → module), tags, appareils du niveau.
#[allow(dead_code)] // Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
pub struct Project {
    pub symbols: Vec<Value>,
    pub tags: Vec<String>,
    pub devices: Vec<(String, String)>,
}

impl Project {
    fn module_of(&self, ident: &str) -> Option<String> {
        if let Some(s) = self.symbols.iter().find(|s| {
            s["name"] == ident && s["module"].as_str().map(|m| !m.is_empty()).unwrap_or(false)
        }) {
            return s["module"]
                .as_str()
                .map(|m| m.trim_end_matches('/').to_string());
        }
        builtin_module(ident).map(String::from)
    }
}

fn builtin_module(id: &str) -> Option<&'static str> {
    Some(match id {
        "agent" | "player" | "Sleep" | "GetPlayspace" | "session" | "GetSession" | "spawn" => {
            "/Verse.org/Simulation"
        }
        "tag" | "GetCreativeObjectsWithTag" => "/Verse.org/Simulation/Tags",
        "GetRandomInt" | "GetRandomFloat" | "Shuffle" => "/Verse.org/Random",
        "fort_character" | "GetFortCharacter" => "/Fortnite.com/Characters",
        "team" | "GetTeamCollection" | "fort_team_collection" => "/Fortnite.com/Teams",
        "GetPlayerUI" | "player_ui" | "widget" => "/UnrealEngine.com/Temporary/UI",
        "text_block" | "canvas" | "button_loud" | "button_quiet" | "stack_box" => {
            "/Fortnite.com/UI"
        }
        "vector3" | "rotation" | "IdentityRotation" | "MakeRotationFromYawPitchRollDegrees" => {
            "/UnrealEngine.com/Temporary/SpatialMath"
        }
        "log" | "log_channel" => "/UnrealEngine.com/Temporary/Diagnostics",
        "message" => "/Verse.org/Verse",
        s if s.ends_with("_device") => "/Fortnite.com/Devices",
        _ => return None,
    })
}

fn indent_of(l: &str) -> usize {
    l.len() - l.trim_start().len()
}

/// Retire commentaires et chaînes pour l'analyse des parenthèses / appels.
fn code_part(l: &str) -> String {
    let mut out = String::new();
    let mut in_str = false;
    let mut depth_interp = 0;
    for c in l.chars() {
        if in_str {
            if c == '{' {
                depth_interp += 1
            } else if c == '}' && depth_interp > 0 {
                depth_interp -= 1
            } else if c == '"' && depth_interp == 0 {
                in_str = false;
                out.push('"');
            }
            if depth_interp == 0 && c != '"' {
                out.push(' ')
            }
            continue;
        }
        if c == '#' {
            break;
        }
        if c == '"' {
            in_str = true;
            out.push('"');
            continue;
        }
        out.push(c);
    }
    out
}

pub fn analyze(src: &str) -> Analysis {
    let lines: Vec<&str> = src.lines().collect();
    let mut a = Analysis {
        lines: lines.len(),
        ..Default::default()
    };
    // Commentaire d'en-tête (avant la première déclaration).
    let mut head = vec![];
    for l in &lines {
        let t = l.trim();
        if t.starts_with("using") || t.is_empty() {
            continue;
        }
        if let Some(c) = t.strip_prefix('#') {
            head.push(c.trim().to_string())
        } else {
            break;
        }
    }
    a.top_comment = head.join(" ");
    let mut cur: Option<usize> = None;
    let mut editable_next = false;
    let mut pending_doc: Vec<String> = vec![];
    for (i, raw) in lines.iter().enumerate() {
        let n = i + 1;
        let t = raw.trim();
        if let Some(c) = RE_USING.captures(raw) {
            a.usings.push(c[1].trim().to_string());
            continue;
        }
        if t.is_empty() {
            continue;
        }
        if let Some(c) = t.strip_prefix('#') {
            pending_doc.push(c.trim().to_string());
            continue;
        }
        let ind = indent_of(raw);
        if let Some(k) = cur {
            if ind == 0 && !t.starts_with('@') {
                let _ = k;
                cur = None;
            }
        }
        if t.starts_with("@editable") {
            editable_next = true;
            if t == "@editable" {
                continue;
            }
        }
        let body = t.trim_start_matches("@editable").trim();
        if ind == 0 {
            if let Some(c) = RE_CLASS.captures(body) {
                a.classes.push(Class {
                    name: c[2].into(),
                    kind: c[3].into(),
                    parent: c
                        .get(4)
                        .map(|m| m.as_str().trim().to_string())
                        .unwrap_or_default(),
                    line: n,
                    fields: vec![],
                    funcs: vec![],
                    doc: pending_doc.join(" "),
                });
                cur = Some(a.classes.len() - 1);
                pending_doc.clear();
                continue;
            }
        }
        pending_doc.clear();
        for c in RE_SUB.captures_iter(&code_part(raw)) {
            a.subs
                .push((c[1].to_string(), c[2].to_string(), c[3].to_string(), n))
        }
        let Some(ci) = cur else {
            editable_next = false;
            continue;
        };
        let class_ind = indent_of(lines[a.classes[ci].line - 1]);
        let member_ind = lines
            .iter()
            .skip(a.classes[ci].line)
            .find(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
            .map(|l| indent_of(l))
            .unwrap_or(4);
        if ind != member_ind || ind <= class_ind {
            continue;
        }
        if let Some(c) = RE_FUNC.captures(body) {
            let effects: Vec<String> = RE_EFFECT
                .captures_iter(&format!("{}{}", &c[3], &c[5]))
                .map(|e| e[1].to_string())
                .collect();
            let end = (i + 1..lines.len())
                .find(|&j| {
                    let l = lines[j];
                    !l.trim().is_empty() && indent_of(l) <= ind
                })
                .unwrap_or(lines.len());
            a.classes[ci].funcs.push(Func {
                name: c[2].into(),
                params: c[4].trim().into(),
                effects,
                ret: c[6].trim().into(),
                line: n,
                end,
                indent: ind,
            });
        } else if let Some(c) = RE_FIELD.captures(body) {
            if !body.contains('(') || !body.contains(":=") {
                a.classes[ci].fields.push(Field {
                    name: c[3].into(),
                    ty: c[4].trim().into(),
                    default: c.get(5).map(|m| m.as_str().trim().to_string()),
                    editable: editable_next,
                    var: c.get(2).is_some(),
                    line: n,
                });
            }
        }
        editable_next = false;
    }
    a
}

fn func_at(a: &Analysis, line: usize) -> Option<&Func> {
    a.classes
        .iter()
        .flat_map(|c| c.funcs.iter())
        .find(|f| line >= f.line && line <= f.end)
}

#[allow(dead_code)] // Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
fn ty_fr(t: &str) -> String {
    match t {
        "int" => "nombre entier".into(),
        "float" => "nombre décimal".into(),
        "logic" => "vrai/faux".into(),
        "string" => "texte".into(),
        "agent" => "joueur ou IA".into(),
        "player" => "joueur".into(),
        "message" => "texte traduisible".into(),
        t if t.ends_with("_device") => format!(
            "appareil {}",
            t.trim_end_matches("_device").replace('_', " ")
        ),
        t if t.starts_with('[') && t.contains(']') => format!("table {t}"),
        t if t.starts_with("[]") => format!("liste de {}", ty_fr(&t[2..])),
        t => t.into(),
    }
}

#[allow(dead_code)] // Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
fn effect_fr(e: &str) -> &'static str {
    match e {
        "suspends" => "peut attendre (asynchrone)",
        "decides" => "peut échouer (à appeler dans un if ou entre [ ])",
        "transacts" => "annulable en cas d'échec",
        "override" => "remplace la version du parent",
        "localizes" => "texte traduisible",
        "computes" => "calcul pur",
        "public" => "public",
        "private" => "privé",
        _ => "",
    }
}

#[allow(dead_code)]
// Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
/// Phrase pour une ligne de code.
fn explain_line(l: &str, a: &Analysis) -> Option<String> {
    let t = l.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(c) = t.strip_prefix('#') {
        return Some(format!("Commentaire : {}", c.trim()));
    }
    if let Some(c) = RE_USING.captures(t) {
        return Some(format!(
            "Importe le module {} pour utiliser ses types et fonctions.",
            &c[1]
        ));
    }
    if t.starts_with("@editable") {
        return Some(
            "Le champ suivant devient un réglage modifiable dans UEFN, sans toucher au code."
                .into(),
        );
    }
    if let Some(c) = RE_CLASS.captures(t) {
        return Some(format!(
            "Déclare {} « {} »{}.",
            match &c[3] {
                "class" => "la classe",
                "struct" => "la structure",
                "enum" => "l'énumération",
                "interface" => "l'interface",
                _ => "le module",
            },
            &c[2],
            c.get(4)
                .map(|p| format!(", qui hérite de {}", p.as_str()))
                .unwrap_or_default()
        ));
    }
    if let Some(c) = RE_FUNC.captures(t) {
        let effs: Vec<String> = RE_EFFECT
            .captures_iter(&format!("{}{}", &c[3], &c[5]))
            .map(|e| e[1].to_string())
            .filter(|e| !effect_fr(e).is_empty())
            .map(|e| format!("{e} : {}", effect_fr(&e)))
            .collect();
        return Some(format!(
            "Définit la fonction {}({}) qui renvoie {}{}.",
            &c[2],
            &c[4],
            ty_fr(c[6].trim()),
            if effs.is_empty() {
                String::new()
            } else {
                format!(" ; effets → {}", effs.join(", "))
            }
        ));
    }
    if let Some(c) = RE_SUB.captures(t) {
        return Some(format!(
            "Quand « {} » déclenche {}, appelle {}.",
            &c[1], &c[2], &c[3]
        ));
    }
    if let Some(v) = t.strip_prefix("set ") {
        return Some(format!("Modifie la variable : {v}."));
    }
    if t.starts_with("if ") || t.starts_with("if(") {
        return Some("Condition : le bloc s'exécute seulement si toutes les expressions entre parenthèses réussissent.".into());
    }
    if t.starts_with("else") {
        return Some("Sinon : exécuté si la condition précédente a échoué.".into());
    }
    if t.starts_with("for ") || t.starts_with("for(") {
        return Some(
            "Boucle : répète le bloc pour chaque élément (les éléments qui échouent sont ignorés)."
                .into(),
        );
    }
    if t.starts_with("loop") {
        return Some("Boucle infinie : à quitter avec break ou return (attention à mettre un Sleep pour ne pas bloquer).".into());
    }
    if t.starts_with("spawn") {
        return Some("Lance le bloc en parallèle, sans attendre sa fin.".into());
    }
    if t.starts_with("race") {
        return Some("Lance plusieurs blocs en parallèle et garde le premier qui termine.".into());
    }
    if t.starts_with("sync") {
        return Some("Lance plusieurs blocs en parallèle et attend qu'ils finissent tous.".into());
    }
    if t.starts_with("return") {
        return Some("Renvoie la valeur et quitte la fonction.".into());
    }
    if t.starts_with("Sleep(") {
        return Some(format!(
            "Attend {} seconde(s) (fonction <suspends>).",
            t.trim_start_matches("Sleep(").trim_end_matches(')')
        ));
    }
    if t.starts_with("Print(") {
        return Some(
            "Affiche un message dans le journal (et à l'écran en session de test).".into(),
        );
    }
    if t.contains(":=") {
        let (n, v) = t.split_once(":=").unwrap();
        return Some(format!("Crée la constante {} = {}.", n.trim(), v.trim()));
    }
    if let Some(c) = RE_FIELD.captures(t) {
        return Some(format!(
            "Déclare {} {} de type {}{}.",
            if c.get(2).is_some() {
                "la variable"
            } else {
                "le champ"
            },
            &c[3],
            ty_fr(c[4].trim()),
            c.get(5)
                .map(|d| format!(", valeur de départ {}", d.as_str()))
                .unwrap_or_default()
        ));
    }
    if let Some(c) = RE_CALL.captures(&code_part(t)) {
        let name = &c[1];
        let dev = a
            .classes
            .iter()
            .flat_map(|k| k.fields.iter())
            .find(|f| name.starts_with(&format!("{}.", f.name)) && f.ty.ends_with("_device"));
        if let Some(f) = dev {
            return Some(format!(
                "Commande l'appareil {} ({}) : {}.",
                f.name,
                ty_fr(&f.ty),
                name.rsplit('.').next().unwrap_or("")
            ));
        }
        return Some(format!(
            "Appelle {}{}.",
            name,
            if &c[2] == "[" {
                " (appel qui peut échouer)"
            } else {
                ""
            }
        ));
    }
    None
}

/// Points d'attention, sans compilateur : règles fréquentes de Verse.
pub fn lint(src: &str, a: &Analysis, p: &Project) -> Vec<Issue> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = vec![];
    let all_fields: Vec<&Field> = a.classes.iter().flat_map(|c| c.fields.iter()).collect();
    let funcs: BTreeSet<String> = a
        .classes
        .iter()
        .flat_map(|c| c.funcs.iter().map(|f| f.name.clone()))
        .collect();
    // Parenthèses par ligne logique.
    for (i, l) in lines.iter().enumerate() {
        let c = code_part(l);
        let bal = |o: char, cl: char| c.matches(o).count() as i64 - c.matches(cl).count() as i64;
        if bal('(', ')') != 0
            || bal('[', ']') != 0
            || (bal('{', '}') != 0
                && !c.trim_end().ends_with('{')
                && !c.trim_start().starts_with('}'))
        {
            out.push(Issue {
                line: i + 1,
                severity: "error",
                text: "Parenthèse, crochet ou accolade non refermé sur cette ligne.".into(),
            });
        }
        if l.starts_with('\t') && lines.iter().any(|x| x.starts_with("    ")) {
            out.push(Issue {
                line: i + 1,
                severity: "warning",
                text: "Tabulation mélangée avec des espaces : Verse est sensible à l'indentation."
                    .into(),
            });
        }
        // Print ou appel d'appareil dans une condition.
        let ct = c.trim();
        if ct.starts_with("if") && ct.contains('(') {
            let cond = &ct[ct.find('(').unwrap() + 1..ct.rfind(')').unwrap_or(ct.len())];
            if cond.contains("Print(")
                || all_fields.iter().any(|f| {
                    f.ty.ends_with("_device")
                        && cond.contains(&format!("{}.", f.name))
                        && !cond.contains(&format!("{}[", f.name))
                })
            {
                out.push(Issue { line: i + 1, severity: "error", text: "Appel non annulable (no_rollback, ex. Print ou commande d'appareil) dans une condition : à déplacer dans le bloc du if.".into() });
            }
        }
        // Modification d'une variable sans set.
        if let Some(m) = RE_COMPOUND.captures(&c) {
            let base = m[2].split('[').next().unwrap_or("");
            if &m[3] != "="
                && all_fields.iter().any(|f| f.var && f.name == base)
                && !ct.starts_with("set ")
                && !ct.starts_with("var ")
            {
                out.push(Issue {
                    line: i + 1,
                    severity: "error",
                    text: format!("{base} est une variable : il faut écrire « set {} ».", ct),
                });
            }
        }
        // Sleep(5) au lieu de Sleep(5.0).
        if RE_INT_ARG.is_match(&c) {
            out.push(Issue {
                line: i + 1,
                severity: "error",
                text: "Sleep attend un nombre décimal : écrivez 5.0 au lieu de 5.".into(),
            })
        }
    }
    // Effets manquants.
    for f in a.classes.iter().flat_map(|c| c.funcs.iter()) {
        let body: String = lines[f.line.min(lines.len())..f.end.min(lines.len())]
            .iter()
            .map(|l| code_part(l))
            .collect::<Vec<_>>()
            .join("\n");
        let suspends = [
            "Sleep(", "Await(", ".Await()", "race:", "sync:", "rush:", "branch:",
        ]
        .iter()
        .any(|k| body.contains(k));
        if suspends && !f.effects.iter().any(|e| e == "suspends") {
            out.push(Issue {
                line: f.line,
                severity: "error",
                text: format!(
                    "{} attend (Sleep, Await, race…) : sa signature doit contenir <suspends>.",
                    f.name
                ),
            });
        }
    }
    // Gestionnaires d'événements inexistants.
    for (src_name, ev, h, n) in &a.subs {
        if !funcs.contains(h) {
            out.push(Issue {
                line: *n,
                severity: "error",
                text: format!(
                    "{src_name}.{ev} est relié à {h}, qui n'est défini nulle part dans ce fichier."
                ),
            })
        }
    }
    // Réglages @editable jamais utilisés, appareils sans valeur.
    for f in &all_fields {
        let used = lines
            .iter()
            .enumerate()
            .any(|(i, l)| i + 1 != f.line && code_part(l).contains(f.name.as_str()));
        if f.editable && !used {
            out.push(Issue {
                line: f.line,
                severity: "warning",
                text: format!("Le réglage {} n'est utilisé nulle part.", f.name),
            })
        }
        if f.default.is_none() && !f.var {
            out.push(Issue {
                line: f.line,
                severity: "error",
                text: format!(
                    "{} n'a pas de valeur par défaut (ex. {} = {}{{}}).",
                    f.name, f.name, f.ty
                ),
            })
        }
    }
    // using manquants.
    let have: Vec<String> = a
        .usings
        .iter()
        .map(|u| u.trim_end_matches('/').to_string())
        .collect();
    let mut missing: HashMap<String, (usize, String)> = HashMap::new();
    for (i, l) in lines.iter().enumerate() {
        if RE_USING.is_match(l) {
            continue;
        }
        for m in RE_IDENT.find_iter(&code_part(l)) {
            if let Some(module) = p.module_of(m.as_str()) {
                if !have.iter().any(|h| h == &module) && !missing.contains_key(&module) {
                    missing.insert(module, (i + 1, m.as_str().to_string()));
                }
            }
        }
    }
    for (module, (n, id)) in missing {
        out.push(Issue {
            line: n,
            severity: "error",
            text: format!(
                "{id} vient de {module} : il manque « using {{ {module} }} » en haut du fichier."
            ),
        })
    }
    out.sort_by_key(|x| x.line);
    out
}

#[allow(dead_code)] // Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
pub fn explain(path: &str, src: &str, selection: Option<&str>, p: &Project) -> String {
    let a = analyze(src);
    if let Some(sel) = selection {
        let lines: Vec<String> = sel
            .lines()
            .filter_map(|l| explain_line(l, &a).map(|e| format!("- `{}` → {e}", l.trim())))
            .collect();
        return format!(
            "**Sélection ({} lignes)**\n\n{}",
            sel.lines().count(),
            if lines.is_empty() {
                "Rien à expliquer dans la sélection.".into()
            } else {
                lines.join("\n")
            }
        );
    }
    let mut s = format!(
        "**{}** · {} lignes · {} déclaration(s)\n",
        path.rsplit('/').next().unwrap_or(path),
        a.lines,
        a.classes.len()
    );
    if !a.top_comment.is_empty() {
        s += &format!("\nRôle annoncé : {}\n", a.top_comment)
    }
    if !a.usings.is_empty() {
        s += &format!("\nModules importés : {}\n", a.usings.join(", "))
    }
    for c in &a.classes {
        let what = if c.parent.contains("creative_device") {
            "appareil Verse à poser dans le niveau".to_string()
        } else if c.parent.is_empty() {
            c.kind.clone()
        } else {
            format!("{} dérivé de {}", c.kind, c.parent)
        };
        s += &format!("\n### {} ({what})\n", c.name);
        if !c.doc.is_empty() {
            s += &format!("{}\n", c.doc)
        }
        let ed: Vec<String> = c
            .fields
            .iter()
            .filter(|f| f.editable)
            .map(|f| {
                format!(
                    "{} ({}{})",
                    f.name,
                    ty_fr(&f.ty),
                    f.default
                        .as_ref()
                        .map(|d| format!(", {d}"))
                        .unwrap_or_default()
                )
            })
            .collect();
        if !ed.is_empty() {
            s += &format!("- Réglages dans UEFN : {}\n", ed.join(" · "))
        }
        let vars: Vec<String> = c
            .fields
            .iter()
            .filter(|f| f.var)
            .map(|f| {
                let writers: Vec<String> = c
                    .funcs
                    .iter()
                    .filter(|fu| {
                        src.lines()
                            .skip(fu.line)
                            .take(fu.end - fu.line)
                            .any(|l| l.contains(&format!("set {}", f.name)))
                    })
                    .map(|fu| fu.name.clone())
                    .collect();
                format!(
                    "{} ({}){}",
                    f.name,
                    ty_fr(&f.ty),
                    if writers.is_empty() {
                        String::new()
                    } else {
                        format!(", modifiée par {}", writers.join(", "))
                    }
                )
            })
            .collect();
        if !vars.is_empty() {
            s += &format!("- État gardé en mémoire : {}\n", vars.join(" · "))
        }
        for f in &c.funcs {
            let effs: Vec<&str> = f
                .effects
                .iter()
                .map(|e| effect_fr(e))
                .filter(|e| !e.is_empty())
                .collect();
            let trig: Vec<String> = a
                .subs
                .iter()
                .filter(|x| x.2 == f.name)
                .map(|x| format!("{}.{}", x.0, x.1))
                .collect();
            let head = if f.name == "OnBegin" {
                "Au lancement de la partie (OnBegin)".to_string()
            } else if f.name == "OnEnd" {
                "À la fin de la partie (OnEnd)".to_string()
            } else {
                format!("{}({})", f.name, f.params)
            };
            s += &format!(
                "- **{head}**{}{} :\n",
                if trig.is_empty() {
                    String::new()
                } else {
                    format!(", déclenchée par {}", trig.join(", "))
                },
                if effs.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", effs.join(", "))
                }
            );
            for l in src
                .lines()
                .skip(f.line)
                .take(f.end - f.line)
                .filter(|l| !l.trim().is_empty())
                .take(12)
            {
                if let Some(e) = explain_line(l, &a) {
                    s += &format!("  - {e}\n")
                }
            }
        }
    }
    let issues = lint(src, &a, p);
    if !issues.is_empty() {
        s += "\n### Points d'attention\n";
        for i in issues.iter().take(20) {
            s += &format!("- Ligne {} : {}\n", i.line, i.text)
        }
    }
    s
}

fn split_top_commas(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut depth = 0;
    let mut cur = String::new();
    let mut in_str = false;
    for c in s.chars() {
        if c == '"' {
            in_str = !in_str
        }
        if !in_str {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(cur.trim().to_string());
                    cur.clear();
                    continue;
                }
                _ => {}
            }
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string())
    }
    out
}

/// Corrige par règles. Renvoie le fichier corrigé et la liste des corrections.
pub fn fix(src: &str, diags: &[Value], p: &Project) -> (String, Vec<String>) {
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let mut notes = vec![];
    let a = analyze(src);
    let mut issues: Vec<(usize, String, String)> = diags
        .iter()
        .map(|d| {
            (
                d["line"].as_u64().unwrap_or(1) as usize,
                d["code"].as_str().unwrap_or("").to_string(),
                d["message"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    // Les points d'attention locaux servent aussi quand la compilation n'a rien remonté.
    for i in lint(src, &a, p)
        .into_iter()
        .filter(|i| i.severity == "error")
    {
        issues.push((i.line, String::new(), i.text))
    }
    let mut add_usings: BTreeSet<String> = BTreeSet::new();
    let mut inserts: Vec<(usize, String)> = vec![];
    let mut done: BTreeSet<(usize, &'static str)> = BTreeSet::new();
    for (n, code, msg) in &issues {
        let idx = n.saturating_sub(1).min(lines.len().saturating_sub(1));
        if lines.is_empty() {
            break;
        }
        let low = msg.to_lowercase();
        let l = lines[idx].clone();
        let c = code_part(&l);
        if (code == "3100"
            || low.contains("non refermé")
            || low.contains("expected ')'")
            || low.contains("unbalanced"))
            && done.insert((idx, "brackets"))
        {
            let mut stack = vec![];
            let mut in_str = false;
            for ch in c.chars() {
                if ch == '"' {
                    in_str = !in_str
                }
                if in_str {
                    continue;
                }
                match ch {
                    '(' | '[' | '{' => stack.push(ch),
                    ')' | ']' | '}' => {
                        stack.pop();
                    }
                    _ => {}
                }
            }
            if !stack.is_empty() {
                let close: String = stack
                    .iter()
                    .rev()
                    .map(|o| match o {
                        '(' => ')',
                        '[' => ']',
                        _ => '}',
                    })
                    .collect();
                let t = l.trim_end();
                lines[idx] = if t.ends_with(':') && !t.ends_with("::") {
                    format!("{}{}:", &t[..t.len() - 1], close)
                } else {
                    format!("{t}{close}")
                };
                notes.push(format!(
                    "Ligne {n} : ajout de « {close} » pour refermer l'expression."
                ));
            }
        } else if (code == "3512" || low.contains("no_rollback")) && done.insert((idx, "rollback"))
        {
            let t = l.trim_start();
            if let (Some(o), Some(cl)) = (t.find('('), t.rfind(')')) {
                let parts = split_top_commas(&t[o + 1..cl]);
                let fields: Vec<&Field> = a.classes.iter().flat_map(|k| k.fields.iter()).collect();
                let is_side = |x: &String| {
                    x.starts_with("Print(")
                        || fields.iter().any(|f| {
                            f.ty.ends_with("_device")
                                && x.starts_with(&format!("{}.", f.name))
                                && !x.contains('[')
                        })
                };
                let (side, keep): (Vec<String>, Vec<String>) = parts.into_iter().partition(is_side);
                if !side.is_empty() {
                    let ind = indent_of(&l);
                    let cond = if keep.is_empty() {
                        "true".to_string()
                    } else {
                        keep.join(", ")
                    };
                    lines[idx] =
                        format!("{}{}({}){}", " ".repeat(ind), &t[..o], cond, &t[cl + 1..]);
                    let body_ind = lines
                        .get(idx + 1)
                        .filter(|x| indent_of(x) > ind)
                        .map(|x| indent_of(x))
                        .unwrap_or(ind + 4);
                    for s in side.iter().rev() {
                        inserts.push((idx + 1, format!("{}{}", " ".repeat(body_ind), s)))
                    }
                    notes.push(format!("Ligne {n} : {} sorti de la condition et placé au début du bloc (un appel no_rollback ne peut pas être annulé).", side.join(", ")));
                }
            }
        } else if code == "3506"
            || low.contains("unknown identifier")
            || low.contains("identifiant inconnu")
            || low.contains("il manque « using")
        {
            let id = RE_QUOTED
                .captures(msg)
                .map(|c| c[1].to_string())
                .or_else(|| {
                    msg.split_whitespace()
                        .next()
                        .filter(|w| RE_IDENT.is_match(w))
                        .map(String::from)
                });
            if let Some(id) = id {
                if let Some(m) = p.module_of(&id) {
                    if add_usings.insert(m.clone()) {
                        notes.push(format!(
                            "Ajout de « using {{ {m} }} » : {id} vient de ce module."
                        ))
                    }
                } else if let Some(best) = closest(&id, p, &a) {
                    lines[idx] = replace_word(&lines[idx], &id, &best);
                    notes.push(format!(
                        "Ligne {n} : {id} n'existe pas, remplacé par {best} (nom le plus proche)."
                    ));
                }
            }
        } else if low.contains("ambig") && done.insert((idx, "ambig")) {
            if let Some(id) = RE_QUOTED.captures(msg).map(|c| c[1].to_string()) {
                if let Some(m) = p.module_of(&id) {
                    lines[idx] = replace_word(&lines[idx], &id, &format!("({m}:){id}"));
                    notes.push(format!("Ligne {n} : {id} précisé avec son module ({m})."));
                }
            }
        } else if (low.contains("suspends") || low.contains("<suspends>"))
            && done.insert((idx, "suspends"))
        {
            if let Some(f) = func_at(&a, *n).or_else(|| {
                a.classes
                    .iter()
                    .flat_map(|c| c.funcs.iter())
                    .find(|f| f.line == *n)
            }) {
                let fi = f.line - 1;
                if !lines[fi].contains("<suspends>") {
                    if let Some(pos) = lines[fi].find(')') {
                        lines[fi].insert_str(pos + 1, "<suspends>");
                        notes.push(format!(
                            "Ligne {} : <suspends> ajouté à {} (elle attend).",
                            f.line, f.name
                        ));
                    }
                }
            }
        } else if (low.contains("decides")
            || low.contains("failure context")
            || low.contains("peut échouer"))
            && done.insert((idx, "decides"))
        {
            let ind = indent_of(&l);
            let t = l.trim();
            if !t.starts_with("if") && t.contains('[') {
                let body_ind = ind + 4;
                lines[idx] = format!("{}if ({}):", " ".repeat(ind), t.trim_end_matches(':'));
                inserts.push((
                    idx + 1,
                    format!("{}# TODO : suite si l'appel réussit", " ".repeat(body_ind)),
                ));
                notes.push(format!(
                    "Ligne {n} : l'appel qui peut échouer est placé dans un if."
                ));
            }
        } else if (low.contains("décimal") || (low.contains("float") && low.contains("int")))
            && done.insert((idx, "float"))
        {
            let nl = RE_INT_ARG.replace_all(&lines[idx], "$1($2.0)").to_string();
            if nl != lines[idx] {
                lines[idx] = nl;
                notes.push(format!(
                    "Ligne {n} : nombre entier converti en décimal (ex. 5 → 5.0)."
                ))
            } else if let Some(cap) = RE_INT_DECIMAL.captures(&lines[idx].clone()) {
                lines[idx] = lines[idx].replacen(": int", ": float", 1);
                let _ = cap;
                notes.push(format!(
                    "Ligne {n} : type changé en float pour accepter une valeur décimale."
                ));
            }
        } else if low.contains("set ") && low.contains("variable") && done.insert((idx, "set")) {
            let ind = indent_of(&l);
            lines[idx] = format!("{}set {}", " ".repeat(ind), l.trim());
            notes.push(format!(
                "Ligne {n} : ajout de « set » pour modifier la variable."
            ));
        } else if low.contains("valeur par défaut") && done.insert((idx, "default")) {
            if let Some(f) = a
                .classes
                .iter()
                .flat_map(|c| c.fields.iter())
                .find(|f| f.line == *n)
            {
                let d = default_for(&f.ty);
                lines[idx] = format!("{} = {d}", lines[idx].trim_end());
                notes.push(format!(
                    "Ligne {n} : valeur par défaut {d} donnée à {}.",
                    f.name
                ));
            }
        } else if low.contains("défini nulle part") && done.insert((idx, "handler")) {
            if let Some((_, ev, h, _)) = a.subs.iter().find(|s| s.3 == *n) {
                if let Some(c) = a.classes.iter().find(|c| c.line < *n) {
                    let ind = c.funcs.first().map(|f| f.indent).unwrap_or(4);
                    let param = if ev.contains("Interacted")
                        || ev.contains("Triggered")
                        || ev.contains("Agent")
                        || ev.contains("Eliminat")
                    {
                        "Agent : agent"
                    } else {
                        ""
                    };
                    let at = lines.len();
                    inserts.push((at, String::new()));
                    inserts.push((
                        at,
                        format!(
                            "{}{h}({param}) : void =\n{}Print(\"{h}\")",
                            " ".repeat(ind),
                            " ".repeat(ind + 4)
                        ),
                    ));
                    notes.push(format!("Fonction {h} créée (appelée par {ev})."));
                }
            }
        }
    }
    inserts.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (at, s) in inserts {
        let at = at.min(lines.len());
        lines.insert(at, s)
    }
    if !add_usings.is_empty() {
        let last_using = lines
            .iter()
            .rposition(|l| RE_USING.is_match(l))
            .map(|i| i + 1)
            .unwrap_or(0);
        for (k, m) in add_usings.iter().enumerate() {
            lines.insert(last_using + k, format!("using {{ {m} }}"))
        }
        if last_using == 0 {
            lines.insert(add_usings.len(), String::new())
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    (out, notes)
}

fn default_for(t: &str) -> String {
    match t {
        "int" => "0".into(),
        "float" => "0.0".into(),
        "logic" => "false".into(),
        "string" => "\"\"".into(),
        t if t.starts_with("[]") => "array{}".into(),
        t if t.starts_with('[') => "map{}".into(),
        t => format!("{t}{{}}"),
    }
}

fn replace_word(line: &str, from: &str, to: &str) -> String {
    Regex::new(&format!(r"\b{}\b", regex::escape(from)))
        .unwrap()
        .replace_all(line, regex::NoExpand(to))
        .to_string()
}

fn lev(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push(
                (prev[j] + (ca != *cb) as usize)
                    .min(prev[j + 1] + 1)
                    .min(cur[j] + 1),
            )
        }
        prev = cur;
    }
    prev[b.len()]
}

fn closest(id: &str, p: &Project, a: &Analysis) -> Option<String> {
    let mut names: Vec<String> = p
        .symbols
        .iter()
        .filter_map(|s| s["name"].as_str().map(String::from))
        .collect();
    names.extend(a.classes.iter().flat_map(|c| {
        c.fields
            .iter()
            .map(|f| f.name.clone())
            .chain(c.funcs.iter().map(|f| f.name.clone()))
            .chain(std::iter::once(c.name.clone()))
    }));
    names
        .into_iter()
        .filter(|n| n != id)
        .map(|n| (lev(&id.to_lowercase(), &n.to_lowercase()), n))
        .filter(|(d, n)| *d <= 2.max(n.len() / 5))
        .min_by_key(|x| x.0)
        .map(|x| x.1)
}

#[allow(dead_code)] // Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
const TEMPLATES: &[(&str, &[&str], &str)] = &[
    ("Économie", &["économie", "economie", "argent", "monnaie", "gold", "pièces", "achat", "boutique", "shop", "acheter", "vendre", "prix"], "# Monnaie par joueur : gagner, dépenser, consulter.\n{NAME} := class(creative_device):\n\n    var Balances : [agent]int = map{}\n\n    Earn(Agent : agent, Amount : int) : void =\n        Current := Balances[Agent] or 0\n        if (set Balances[Agent] = Current + Amount) {}\n\n    Spend(Agent : agent, Amount : int)<decides><transacts> : void =\n        Current := Balances[Agent]\n        Current >= Amount\n        set Balances[Agent] = Current - Amount\n\n    GetBalance(Agent : agent) : int =\n        Balances[Agent] or 0\n"),
    ("Inventaire", &["inventaire", "objet", "item", "ramasser", "butin", "loot", "sac"], "# Objets possédés par joueur.\n{NAME} := class(creative_device):\n\n    var Items : [agent][]string = map{}\n\n    Add(Agent : agent, Item : string) : void =\n        Current := Items[Agent] or array{}\n        if (set Items[Agent] = Current + array{Item}) {}\n\n    Has(Agent : agent, Item : string)<decides><transacts> : void =\n        List := Items[Agent]\n        List.Find[Item]\n"),
    ("HUD", &["hud", "affich", "écran", "ecran", "interface", "ui", "widget", "texte à l"], "# Affiche un texte à l'écran du joueur.\n{NAME} := class(creative_device):\n\n    MakeLabel<localizes>(Value : string) : message = \"{Value}\"\n\n    Show(Player : player, Value : string) : void =\n        if (PlayerUI := GetPlayerUI[Player]):\n            Widget := text_block{DefaultText := MakeLabel(Value)}\n            PlayerUI.AddWidget(Widget)\n"),
    ("Sauvegarde", &["sauvegard", "persist", "profil", "progression", "save"], "# Données persistantes par joueur.\nplayer_profile := class<final><persistable>:\n    Gold : int = 0\n    Level : int = 1\n\nvar Profiles : weak_map(player, player_profile) = map{}\n\n{NAME} := class(creative_device):\n\n    Load(Player : player) : player_profile =\n        Profiles[Player] or player_profile{}\n\n    Save(Player : player, Profile : player_profile) : void =\n        if (set Profiles[Player] = Profile) {}\n"),
    ("Équipes", &["équipe", "equipe", "team", "camp", "gendarme", "voleur"], "# Répartition et lecture des équipes.\n{NAME} := class(creative_device):\n\n    GetTeamIndex(Agent : agent)<decides><transacts> : int =\n        Teams := GetPlayspace().GetTeamCollection().GetTeams()\n        Team := GetPlayspace().GetTeamCollection().GetTeam[Agent]\n        var Found : int = -1\n        for (Index -> T : Teams, T = Team):\n            set Found = Index\n        Found >= 0\n        Found\n"),
    ("Minuteur", &["minuteur", "timer", "chrono", "compte à rebours", "manche", "round", "durée"], "# Compte à rebours de manche.\n{NAME} := class(creative_device):\n\n    @editable\n    Seconds : float = 480.0\n\n    OnBegin<override>()<suspends> : void =\n        Sleep(Seconds)\n        Print(\"Fin de la manche\")\n"),
];

#[allow(dead_code)] // Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
fn snake(s: &str) -> String {
    let t: String = s
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c if c.is_ascii_alphanumeric() => c,
            _ => '_',
        })
        .collect();
    let stop = [
        "le", "la", "les", "de", "des", "du", "un", "une", "et", "a", "au", "aux", "en", "qui",
        "que", "pour", "avec", "sur", "dans", "the", "of",
    ];
    let words: Vec<&str> = t
        .split('_')
        .filter(|w| !w.is_empty() && !stop.contains(w))
        .take(3)
        .collect();
    let s = words.join("_");
    if s.is_empty() || s.chars().next().unwrap().is_ascii_digit() {
        format!("module_{s}")
    } else {
        s
    }
}

#[allow(dead_code)]
// Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
/// Écrit un module pour une tâche : modèle choisi par mots-clés, appareils et tags du niveau cités dans la tâche.
pub fn write(
    path: &str,
    src: &str,
    task: Option<(i64, String, String)>,
    p: &Project,
    conventions: &str,
) -> (String, String) {
    let (num, title, desc) = task.clone().unwrap_or((
        0,
        path.rsplit('/')
            .next()
            .unwrap_or(path)
            .trim_end_matches(".verse")
            .replace('_', " "),
        String::new(),
    ));
    let text = format!("{title} {desc}").to_lowercase();
    let a = analyze(src);
    let suffix = if conventions.to_lowercase().contains("_manager") {
        "_manager"
    } else if a.classes.iter().any(|c| c.name.ends_with("_device"))
        || conventions.contains("_device")
    {
        "_device"
    } else {
        "_manager"
    };
    let name = format!("{}{}", snake(&title), suffix);
    let name = if a.classes.iter().any(|c| c.name == name) {
        format!("{name}_2")
    } else {
        name
    };
    let toks: Vec<&str> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let hit = |k: &str| {
        if k.contains(' ') {
            text.contains(k)
        } else {
            toks.iter().any(|w| w.starts_with(k.trim()))
        }
    };
    let tpl = TEMPLATES
        .iter()
        .map(|(n, kws, body)| (kws.iter().filter(|k| hit(k)).count(), n, body))
        .filter(|x| x.0 > 0)
        .max_by_key(|x| x.0);
    // Appareils et tags du niveau mentionnés.
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() > 3)
        .map(String::from)
        .collect();
    let devs: Vec<&(String, String)> = p
        .devices
        .iter()
        .filter(|(n, cls)| {
            let n = n.to_lowercase();
            words
                .iter()
                .any(|w| n.contains(w.as_str()) || cls.to_lowercase().contains(w.as_str()))
        })
        .take(6)
        .collect();
    let tags: Vec<&String> = p
        .tags
        .iter()
        .filter(|t| {
            text.contains(&t.to_lowercase())
                || words.iter().any(|w| t.to_lowercase().contains(w.as_str()))
        })
        .take(3)
        .collect();
    let mut body = String::new();
    let mut usings: BTreeSet<&str> = ["/Fortnite.com/Devices", "/Verse.org/Simulation"]
        .into_iter()
        .collect();
    let what;
    if let Some((_, tname, t)) = tpl {
        what = format!("modèle {tname}");
        body = t.replace("{NAME}", &name);
        if body.contains("GetPlayerUI") {
            usings.insert("/UnrealEngine.com/Temporary/UI");
            usings.insert("/Fortnite.com/UI");
        }
        if body.contains("GetTeamCollection") {
            usings.insert("/Fortnite.com/Teams");
        }
    } else {
        what = "appareil générique relié aux appareils de la tâche".into();
        body += &format!(
            "# {}\n{name} := class(creative_device):\n",
            if num > 0 {
                format!("Tâche #{num} : {title}")
            } else {
                title.clone()
            }
        );
    }
    // Ajoute les appareils cités en @editable et les abonnements.
    let mut fields = String::new();
    let mut subs = String::new();
    let mut handlers = String::new();
    for (n, cls) in &devs {
        let fname: String = n
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            })
            .collect();
        let fname = if fname.is_empty() || fname.chars().next().unwrap().is_ascii_digit() {
            format!("Device{fname}")
        } else {
            fname
        };
        let ty = if cls.ends_with("_device") {
            cls.clone()
        } else {
            "creative_device".into()
        };
        fields += &format!("\n    @editable\n    {fname} : {ty} = {ty}{{}}\n");
        let ev = match ty.as_str() {
            "trigger_device" => Some("TriggeredEvent"),
            "button_device" => Some("InteractedWithEvent"),
            "item_spawner_device" => Some("ItemPickedUpEvent"),
            "timer_device" => Some("SuccessEvent"),
            "player_spawner_device" => Some("SpawnedEvent"),
            _ => None,
        };
        if let Some(ev) = ev {
            let h = format!("On{fname}");
            subs += &format!("        {fname}.{ev}.Subscribe({h})\n");
            let param = if ty == "trigger_device" {
                "MaybeAgent : ?agent"
            } else {
                "Agent : agent"
            };
            handlers += &format!(
                "\n    {h}({param}) : void =\n        Print(\"{} : {}\")\n",
                n, ev
            );
        }
    }
    if !tags.is_empty() {
        usings.insert("/Verse.org/Simulation/Tags");
        for t in &tags {
            subs += &format!("        for (Obj : GetCreativeObjectsWithTag({t}{{}})):\n            Print(\"Objet tagué {t} trouvé\")\n")
        }
    }
    if !fields.is_empty() || !subs.is_empty() {
        let class_line = body
            .find(&format!("{name} := class"))
            .map(|i| i + body[i..].find('\n').unwrap_or(0) + 1)
            .unwrap_or(body.len());
        let mut extra = fields.clone();
        if !subs.is_empty() && !body.contains("OnBegin<override>") {
            extra += &format!("\n    OnBegin<override>()<suspends> : void =\n{subs}");
        }
        body.insert_str(class_line, &extra);
        body += &handlers;
    } else if tpl.is_none() {
        body += "\n    OnBegin<override>()<suspends> : void =\n        Print(\"Démarrage\")\n";
    }
    // Fusion avec le fichier existant.
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let have: Vec<String> = a.usings.clone();
    let mut head = String::new();
    for u in &usings {
        if !have.iter().any(|h| h.trim_end_matches('/') == *u) {
            head += &format!("using {{ {u} }}\n")
        }
    }
    let out = if src.trim().is_empty() {
        format!("{head}\n{body}")
    } else {
        let last_using = lines
            .iter()
            .rposition(|l| RE_USING.is_match(l))
            .map(|i| i + 1)
            .unwrap_or(0);
        if !head.is_empty() {
            lines.insert(last_using, head.trim_end().to_string())
        }
        format!("{}\n\n{body}", lines.join("\n").trim_end())
    };
    let mut expl = format!(
        "Module **{name}** écrit avec le {what}{}.",
        if num > 0 {
            format!(" pour la tâche #{num} « {title} »")
        } else {
            String::new()
        }
    );
    if !devs.is_empty() {
        expl += &format!(
            "\n- Appareils du niveau reliés : {}.",
            devs.iter()
                .map(|d| d.0.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
    if !tags.is_empty() {
        expl += &format!(
            "\n- Tags utilisés : {}.",
            tags.iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
    expl +=
        "\n- Posez l'appareil dans le niveau puis réglez ses champs @editable dans l'Organiseur.";
    (out, expl)
}

#[allow(dead_code)]
// Assistant Verse local (expliquer, écrire, relire) : plus appelé depuis le retrait de l'éditeur Verse du site, gardé pour Forge (#86).
/// Relecture : points d'attention + documentation, et fichier avec commentaires ajoutés.
pub fn review(path: &str, src: &str, p: &Project) -> (String, String) {
    let a = analyze(src);
    let issues = lint(src, &a, p);
    let mut doc = format!("# {}\n\n", path.rsplit('/').next().unwrap_or(path));
    if !a.top_comment.is_empty() {
        doc += &format!("{}\n\n", a.top_comment)
    }
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let mut inserts: Vec<(usize, String)> = vec![];
    for c in &a.classes {
        doc += &format!("## {}\n", c.name);
        let ed: Vec<&Field> = c.fields.iter().filter(|f| f.editable).collect();
        if !ed.is_empty() {
            doc += &format!(
                "Réglages : {}\n",
                ed.iter()
                    .map(|f| format!("{} ({})", f.name, ty_fr(&f.ty)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        let evs: Vec<String> = a
            .subs
            .iter()
            .map(|s| format!("{}.{} → {}", s.0, s.1, s.2))
            .collect();
        if !evs.is_empty() {
            doc += &format!("Événements : {}\n", evs.join(", "))
        }
        let used_tags: Vec<&String> = p.tags.iter().filter(|t| src.contains(t.as_str())).collect();
        if !used_tags.is_empty() {
            doc += &format!(
                "Tags : {}\n",
                used_tags
                    .iter()
                    .map(|t| t.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        doc += &format!(
            "Fonctions : {}\n\n",
            c.funcs
                .iter()
                .map(|f| f.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );
        if c.doc.is_empty() && c.line > 0 {
            inserts.push((
                c.line - 1,
                format!(
                    "# {} : {}",
                    c.name,
                    if c.parent.contains("creative_device") {
                        format!(
                            "appareil Verse ({} réglage(s), {} fonction(s)).",
                            ed.len(),
                            c.funcs.len()
                        )
                    } else {
                        format!("{}.", c.kind)
                    }
                ),
            ));
        }
        for f in &c.funcs {
            let prev = lines
                .get(f.line.saturating_sub(2))
                .map(|l| l.trim().starts_with('#'))
                .unwrap_or(false);
            if !prev {
                let trig: Vec<String> = a
                    .subs
                    .iter()
                    .filter(|x| x.2 == f.name)
                    .map(|x| format!("{}.{}", x.0, x.1))
                    .collect();
                let what = if f.name == "OnBegin" {
                    "Lancé au début de la partie.".to_string()
                } else if !trig.is_empty() {
                    format!("Appelé par {}.", trig.join(", "))
                } else {
                    format!("Renvoie {}.", ty_fr(&f.ret))
                };
                inserts.push((f.line - 1, format!("{}# {what}", " ".repeat(f.indent))));
            }
        }
    }
    inserts.sort_by_key(|x| std::cmp::Reverse(x.0));
    for (at, s) in inserts {
        lines.insert(at.min(lines.len()), s)
    }
    let mut expl = if issues.is_empty() {
        "Relecture : aucun problème détecté par les règles de POP.".to_string()
    } else {
        format!(
            "Relecture : {} point(s) d'attention.\n{}",
            issues.len(),
            issues
                .iter()
                .map(|i| format!(
                    "- Ligne {} ({}) : {}",
                    i.line,
                    if i.severity == "error" {
                        "erreur"
                    } else {
                        "conseil"
                    },
                    i.text
                ))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    expl += "\n\n";
    expl += &doc;
    (lines.join("\n") + "\n", expl)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn proj() -> Project {
        Project {
            symbols: vec![],
            tags: vec!["vault_plate".into()],
            devices: vec![("Bouton coffre".into(), "button_device".into())],
        }
    }
    const SRC: &str = "using { /Fortnite.com/Devices }\n\n# Coffres\nvault := class(creative_device):\n\n    @editable\n    Reward : int = 10\n\n    var Stolen : int = 0\n\n    OnBegin<override>()<suspends> : void =\n        if (Print(\"x\"), Stolen > 0):\n            Print(\"ok\")\n        Sleep(5)\n        Stolen += 1\n        Wait(\n";

    #[test]
    fn analyse_et_regles() {
        let a = analyze(SRC);
        assert_eq!(a.classes.len(), 1);
        assert_eq!(a.classes[0].fields.len(), 2);
        assert_eq!(a.classes[0].funcs[0].name, "OnBegin");
        let issues = lint(SRC, &a, &proj());
        let txt: Vec<String> = issues
            .iter()
            .map(|i| format!("{}:{}", i.line, i.text))
            .collect();
        assert!(
            txt.iter()
                .any(|t| t.starts_with("12:") && t.contains("no_rollback")),
            "{txt:?}"
        );
        assert!(txt
            .iter()
            .any(|t| t.starts_with("14:") && t.contains("5.0")));
        assert!(txt
            .iter()
            .any(|t| t.starts_with("15:") && t.contains("set")));
        assert!(txt
            .iter()
            .any(|t| t.starts_with("16:") && t.contains("refermé")));
        assert!(txt.iter().any(|t| t.contains("Simulation")));
        let (fixed, notes) = fix(SRC, &[], &proj());
        assert!(
            fixed.contains("if (Stolen > 0):\n            Print(\"x\")"),
            "{fixed}"
        );
        assert!(fixed.contains("Sleep(5.0)"));
        assert!(fixed.contains("set Stolen += 1"));
        assert!(fixed.contains("Wait()"));
        assert!(fixed.contains("using { /Verse.org/Simulation }"));
        assert!(notes.len() >= 5, "{notes:?}");
        let e = explain("vault.verse", SRC, None, &proj());
        assert!(e.contains("Réglages dans UEFN : Reward"));
        let (code, _) = write(
            "x.verse",
            "",
            Some((
                4,
                "Bouton coffre qui donne de l'argent".into(),
                String::new(),
            )),
            &proj(),
            "",
        );
        assert!(
            code.contains("bouton_coffre_donne_manager := class(creative_device)")
                && code.contains("BoutonCoffre.InteractedWithEvent.Subscribe"),
            "{code}"
        );
        let (rev, _) = review("vault.verse", SRC, &proj());
        assert!(rev.contains("# Lancé au début de la partie."));
    }
}
