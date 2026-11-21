//! Interface UEFN générée par POP (#115) : HUD et menus simples (texte, compteur, bouton) écrits en Verse.
//!
//! Choix (voir KB/UEFN.md) : un widget UMG créé par script s'affiche depuis le Verse mais ne s'y pilote pas (les champs
//! Verse ajoutés par VerseFieldsToolset n'arrivent pas dans la classe générée). On génère donc un appareil Verse
//! (`creative_device`) qui construit l'interface avec l'API UI de Verse : il apparaît dans le Content Browser après
//! compilation, se pose dans le niveau et se pilote (compteur mis à jour, bouton qui déclenche un appareil).
//! Le fichier passe par l'action `write_file` : journalisé, annulable, refusé sur un fichier verrouillé dans Lore.
use serde_json::Value;

/// Dossier des fichiers générés.
/// Sous `Content/` : UEFN ne compile que le Verse du dossier Content du projet (testé le 21/11/2026).
pub const DOSSIER: &str = "Content/POP_UI";

#[derive(Debug, PartialEq)]
pub struct Widget {
    /// texte, compteur ou bouton
    pub kind: String,
    /// identifiant : minuscules, chiffres, « _ » (devient la classe pop_ui_<nom>)
    pub nom: String,
    /// texte affiché (libellé du compteur, du bouton)
    pub texte: String,
    /// haut, centre ou bas
    pub position: String,
}

impl Widget {
    pub fn classe(&self) -> String {
        format!("pop_ui_{}", self.nom)
    }
    pub fn chemin(&self) -> String {
        format!("{DOSSIER}/{}.verse", self.classe())
    }
}

/// Lit et contrôle la demande. Les messages d'erreur sont en français, pour l'IA comme pour l'humain.
pub fn lire(a: &Value) -> Result<Widget, String> {
    let kind = a["kind"].as_str().unwrap_or("").trim().to_lowercase();
    if !["texte", "compteur", "bouton"].contains(&kind.as_str()) {
        return Err("kind doit être texte, compteur ou bouton.".into());
    }
    let nom = a["name"].as_str().unwrap_or("").trim().to_lowercase();
    if nom.is_empty()
        || nom.len() > 40
        || !nom.starts_with(|c: char| c.is_ascii_lowercase())
        || !nom
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err("name : 1 à 40 caractères, minuscules sans accent, chiffres et « _ », en commençant par une lettre.".into());
    }
    let texte = a["text"].as_str().unwrap_or("").trim().to_string();
    if texte.is_empty() || texte.chars().count() > 80 {
        return Err("text : 1 à 80 caractères.".into());
    }
    // Liste blanche : rien qui ait un sens dans une chaîne Verse ({ } " \ < > & # ~).
    if !texte
        .chars()
        .all(|c| c.is_alphanumeric() || " .,;:!?-'’()%+/=*".contains(c))
    {
        return Err(
            "text : lettres, chiffres, espaces et . , ; : ! ? - ' ( ) % + / = * seulement.".into(),
        );
    }
    let position = a["position"]
        .as_str()
        .unwrap_or("haut")
        .trim()
        .to_lowercase();
    if !["haut", "centre", "bas"].contains(&position.as_str()) {
        return Err("position doit être haut, centre ou bas.".into());
    }
    Ok(Widget {
        kind,
        nom,
        texte,
        position,
    })
}

/// Emplacement dans l'écran : ancre, décalage, alignement.
fn slot(position: &str) -> String {
    let (y, decalage) = match position {
        "centre" => ("0.5", "margin{}"),
        "bas" => ("1.0", "margin{Top := -120.0}"),
        _ => ("0.0", "margin{Top := 60.0}"),
    };
    format!(
        "Anchors := anchors{{Minimum := vector2{{X := 0.5, Y := {y}}}, Maximum := vector2{{X := 0.5, Y := {y}}}}}, Offsets := {decalage}, Alignment := vector2{{X := 0.5, Y := {y}}}, SizeToContent := true"
    )
}

const ENTETE: &str = "using { /Fortnite.com/Devices }
using { /Fortnite.com/UI }
using { /UnrealEngine.com/Temporary/UI }
using { /UnrealEngine.com/Temporary/SpatialMath }
using { /Verse.org/Simulation }
using { /Verse.org/Colors }
";

/// Le fichier Verse complet.
pub fn verse(w: &Widget) -> String {
    let c = w.classe();
    let t = &w.texte;
    let s = slot(&w.position);
    let corps = match w.kind.as_str() {
        "compteur" => format!(
            r#"# Compteur affiché à chaque joueur. Chaque activation du déclencheur ajoute 1 au compteur de celui qui l'active.
# Depuis un autre Verse : Ajouter(Joueur, N) ou Definir(Joueur, N).
{c} := class(creative_device):
    @editable Declencheur : trigger_device = trigger_device{{}}

    var Valeurs : [player]int = map{{}}
    var Textes : [player]text_block = map{{}}

    Libelle<localizes>(Valeur:int) : message = "{t} {{Valeur}}"

    OnBegin<override>()<suspends>:void =
        for (Joueur : GetPlayspace().GetPlayers()):
            Afficher(Joueur)
        GetPlayspace().PlayerAddedEvent().Subscribe(Afficher)
        Declencheur.TriggeredEvent.Subscribe(SurDeclenche)

    SurDeclenche(Agent:?agent):void =
        if (A := Agent?, Joueur := player[A]):
            Ajouter(Joueur, 1)

    Afficher(Joueur:player):void =
        if (PlayerUI := GetPlayerUI[Joueur]):
            Texte := text_block{{DefaultTextColor := NamedColors.White}}
            Texte.SetText(Libelle(0))
            PlayerUI.AddWidget(canvas{{Slots := array{{canvas_slot{{{s}, Widget := Texte}}}}}})
            if (set Textes[Joueur] = Texte, set Valeurs[Joueur] = 0) {{}}

    Ajouter(Joueur:player, N:int):void =
        Valeur := (if (V := Valeurs[Joueur]) then V else 0) + N
        Definir(Joueur, Valeur)

    Definir(Joueur:player, Valeur:int):void =
        if (set Valeurs[Joueur] = Valeur) {{}}
        if (Texte := Textes[Joueur]):
            Texte.SetText(Libelle(Valeur))
"#
        ),
        "bouton" => format!(
            r#"# Bouton à l'écran. Il s'ouvre quand le déclencheur Ouverture est activé ; un clic active l'appareil Cible
# pour ce joueur, puis ferme le bouton.
{c} := class(creative_device):
    @editable Ouverture : trigger_device = trigger_device{{}}
    @editable Cible : trigger_device = trigger_device{{}}

    var Ouverts : [player]canvas = map{{}}

    Libelle<localizes>() : message = "{t}"

    OnBegin<override>()<suspends>:void =
        Ouverture.TriggeredEvent.Subscribe(SurOuverture)

    SurOuverture(Agent:?agent):void =
        if (A := Agent?, Joueur := player[A]):
            Ouvrir(Joueur)

    Ouvrir(Joueur:player):void =
        if (not Ouverts[Joueur], PlayerUI := GetPlayerUI[Joueur]):
            Bouton := button_loud{{DefaultText := Libelle()}}
            Bouton.OnClick().Subscribe(SurClic)
            Canvas := canvas{{Slots := array{{canvas_slot{{{s}, Widget := Bouton}}}}}}
            PlayerUI.AddWidget(Canvas, player_ui_slot{{InputMode := ui_input_mode.All}})
            if (set Ouverts[Joueur] = Canvas) {{}}

    SurClic(Message:widget_message):void =
        Joueur := Message.Player
        Cible.Trigger(Joueur)
        Fermer(Joueur)

    Fermer(Joueur:player):void =
        if (Canvas := Ouverts[Joueur], PlayerUI := GetPlayerUI[Joueur]):
            PlayerUI.RemoveWidget(Canvas)
        set Ouverts = RetirerJoueur(Ouverts, Joueur)

    RetirerJoueur(Table:[player]canvas, Joueur:player)<transacts>:[player]canvas =
        var Reste : [player]canvas = map{{}}
        for (Cle -> Valeur : Table, Cle <> Joueur):
            if (set Reste[Cle] = Valeur) {{}}
        Reste
"#
        ),
        _ => format!(
            r#"# Texte affiché à chaque joueur pendant toute la partie.
{c} := class(creative_device):
    Libelle<localizes>() : message = "{t}"

    OnBegin<override>()<suspends>:void =
        for (Joueur : GetPlayspace().GetPlayers()):
            Afficher(Joueur)
        GetPlayspace().PlayerAddedEvent().Subscribe(Afficher)

    Afficher(Joueur:player):void =
        if (PlayerUI := GetPlayerUI[Joueur]):
            Texte := text_block{{DefaultTextColor := NamedColors.White}}
            Texte.SetText(Libelle())
            PlayerUI.AddWidget(canvas{{Slots := array{{canvas_slot{{{s}, Widget := Texte}}}}}})
"#
        ),
    };
    format!("# Généré par POP (outil create_widget). Modifiable à la main.\n{ENTETE}\n{corps}")
}

/// Un fichier d'interface POP encore utilisé : un acteur du niveau a sa classe. Son annulation casserait le niveau.
pub fn utilise(chemin: &str, acteurs: &[Value]) -> Option<String> {
    let classe = chemin
        .strip_prefix(&format!("{DOSSIER}/"))?
        .strip_suffix(".verse")?;
    acteurs
        .iter()
        .find(|a| {
            // Un appareil Verse posé a la classe générique VerseDevice_C ; sa vraie classe Verse (ex.
            // « POP_UI-pop_ui_score ») vient de pop-link dans `verse_class`.
            ["verse_class", "class", "type"].iter().any(|k| {
                a[*k].as_str().is_some_and(|v| {
                    v.split(['.', '/', ':']).any(|p| {
                        let p = p.strip_suffix("_C").unwrap_or(p);
                        p == classe || p.ends_with(&format!("-{classe}"))
                    })
                })
            })
        })
        .map(|a| a["name"].as_str().unwrap_or("?").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn controles() {
        let ok = json!({ "kind": "compteur", "name": "score", "text": "Score :" });
        let w = lire(&ok).unwrap();
        assert_eq!(w.chemin(), "Content/POP_UI/pop_ui_score.verse");
        assert_eq!(w.position, "haut");
        for a in [
            json!({ "kind": "image", "name": "a", "text": "x" }),
            json!({ "kind": "texte", "name": "mon score", "text": "x" }),
            json!({ "kind": "texte", "name": "9a", "text": "x" }),
            json!({ "kind": "texte", "name": "a", "text": "" }),
            json!({ "kind": "texte", "name": "a", "text": "x\" {Injection}" }),
            json!({ "kind": "texte", "name": "a", "text": "x", "position": "gauche" }),
        ] {
            assert!(lire(&a).is_err(), "{a}");
        }
        assert!(crate::uefn::valid_path(&w.chemin()));
    }

    #[test]
    fn generation() {
        for kind in ["texte", "compteur", "bouton"] {
            let w = lire(&json!({ "kind": kind, "name": "menu_1", "text": "Prêt ? (100 %)", "position": "bas" })).unwrap();
            let v = verse(&w);
            assert!(
                v.contains("pop_ui_menu_1 := class(creative_device):"),
                "{v}"
            );
            assert!(v.contains("\"Prêt ? (100 %)"));
            assert!(v.contains("Y := 1.0"));
            // Accolades équilibrées et indentation par espaces seulement.
            assert_eq!(v.matches('{').count(), v.matches('}').count(), "{kind}");
            assert!(!v.contains('\t'));
        }
        let c = verse(
            &lire(&json!({ "kind": "compteur", "name": "score", "text": "Score :" })).unwrap(),
        );
        assert!(
            c.contains("\"Score : {Valeur}\"") && c.contains("Definir(Joueur:player, Valeur:int)")
        );
    }

    #[test]
    fn utilisation() {
        let acteurs = vec![
            json!({ "name": "Mur", "class": "/Script/Engine.StaticMeshActor" }),
            json!({ "name": "Score HUD", "class": "/MonProjet/_Verse/VerseDevices.pop_ui_score_C" }),
            // Comme dans UEFN : classe générique, vraie classe Verse remontée par pop-link.
            json!({ "name": "Menu", "class": "VerseDevice_C", "type": "verse_device", "verse_class": "POP_UI-pop_ui_menu" }),
            json!({ "name": "Autre appareil", "class": "VerseDevice_C", "type": "verse_device", "verse_class": "pop_test_device" }),
        ];
        assert_eq!(
            utilise("Content/POP_UI/pop_ui_score.verse", &acteurs).as_deref(),
            Some("Score HUD")
        );
        assert_eq!(
            utilise("Content/POP_UI/pop_ui_menu.verse", &acteurs).as_deref(),
            Some("Menu")
        );
        assert_eq!(utilise("Content/POP_UI/pop_ui_bonus.verse", &acteurs), None);
        assert_eq!(utilise("autre/pop_ui_score.verse", &acteurs), None);
    }
}
