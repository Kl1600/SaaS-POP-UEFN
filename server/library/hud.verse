# HUD (module de l'équipe POP) : un panneau à l'écran de chaque joueur, avec un titre et des lignes nommées
# (par exemple « Score » et « Pièces »), que votre Verse met à jour pour chaque joueur.
# Depuis votre Verse : Definir(Joueur, "Score", "120"), DefinirNombre(Joueur, "Pièces", 35), Montrer(Joueur)
# et Cacher(Joueur).
using { /Fortnite.com/Devices }
using { /Fortnite.com/UI }
using { /Verse.org/Simulation }
using { /Verse.org/Colors }
using { /UnrealEngine.com/Temporary/UI }
using { /UnrealEngine.com/Temporary/SpatialMath }

TexteHud<localizes>(S : string) : message = "{S}"

hud_pop := class(creative_device):
    # Titre en haut du panneau (laisser vide : pas de titre).
    @editable
    Titre : string = "Partie"
    # Lignes du panneau, dans l'ordre ; chacune commence à « Nom : 0 ».
    @editable
    Lignes : []string = array{"Score", "Pièces"}
    # Position du coin haut gauche du panneau, en fraction de l'écran (0,0 : en haut à gauche).
    @editable
    PositionX : float = 0.02
    @editable
    PositionY : float = 0.18
    # Montrer le panneau dès l'arrivée du joueur.
    @editable
    MontrerAuDebut : logic = true

    var Textes : [player][]text_block = map{}
    var Panneaux : [player]canvas = map{}

    OnBegin<override>()<suspends> : void =
        for (Joueur : GetPlayspace().GetPlayers()):
            Creer(Joueur)
        GetPlayspace().PlayerAddedEvent().Subscribe(Creer)

    # Panneau d'un joueur : fond sombre, titre, puis une ligne de texte par nom de Lignes.
    Creer(Joueur : player) : void =
        if (Panneaux[Joueur]):
            return
        Valeurs := for (L : Lignes):
            text_block{DefaultText := TexteHud("{L} : 0"), DefaultTextColor := NamedColors.White}
        TitreBloc := text_block{DefaultText := TexteHud(Titre), DefaultTextColor := NamedColors.White}
        EnTete : []stack_box_slot = if (Titre = "") then array{} else array{stack_box_slot{Widget := TitreBloc, HorizontalAlignment := horizontal_alignment.Left}}
        Pile := stack_box:
            Orientation := orientation.Vertical
            Slots := EnTete + (for (V : Valeurs):
                stack_box_slot{Widget := V, HorizontalAlignment := horizontal_alignment.Left})
        Fond := overlay:
            Slots := array:
                overlay_slot:
                    Widget := color_block{DefaultColor := NamedColors.Black, DefaultOpacity := 0.55}
                    HorizontalAlignment := horizontal_alignment.Fill
                    VerticalAlignment := vertical_alignment.Fill
                overlay_slot:
                    Widget := Pile
                    Padding := margin{Left := 16.0, Top := 10.0, Right := 16.0, Bottom := 10.0}
        Panneau := canvas:
            Slots := array:
                canvas_slot:
                    Anchors := anchors{Minimum := vector2{X := PositionX, Y := PositionY}, Maximum := vector2{X := PositionX, Y := PositionY}}
                    Alignment := vector2{X := 0.0, Y := 0.0}
                    SizeToContent := true
                    Widget := Fond
        if:
            set Textes[Joueur] = Valeurs
            set Panneaux[Joueur] = Panneau
        if (MontrerAuDebut?):
            Montrer(Joueur)

    Montrer(Joueur : player) : void =
        if (UI := GetPlayerUI[Joueur], Panneau := Panneaux[Joueur]):
            UI.RemoveWidget(Panneau)
            UI.AddWidget(Panneau)

    Cacher(Joueur : player) : void =
        if (UI := GetPlayerUI[Joueur], Panneau := Panneaux[Joueur]):
            UI.RemoveWidget(Panneau)

    # Change la valeur d'une ligne (nom tel qu'écrit dans Lignes) pour un joueur.
    Definir(Joueur : player, Ligne : string, Valeur : string) : void =
        if (T := Textes[Joueur]):
            for (I -> L : Lignes, L = Ligne, Bloc := T[I]):
                Bloc.SetText(TexteHud("{L} : {Valeur}"))

    DefinirNombre(Joueur : player, Ligne : string, Valeur : int) : void =
        Definir(Joueur, Ligne, "{Valeur}")
