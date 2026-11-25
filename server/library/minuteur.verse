# Minuteur (module de l'équipe POP) : compte à rebours affiché à tous les joueurs (mm:ss), lancé par un bouton
# ou dès le début de la partie ; à zéro, termine la partie (facultatif) et envoie un événement à votre Verse.
# Depuis votre Verse : Lancer(), Arreter(), et FinEvent.Await() pour attendre la fin.
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }
using { /UnrealEngine.com/Temporary/Diagnostics }

TexteMinuteur<localizes>(S : string) : message = "{S}"

minuteur_pop := class(creative_device):
    # Durée en secondes.
    @editable
    Duree : int = 60
    # Lancer tout seul au début de la partie, sans bouton.
    @editable
    DemarrerSeul : logic = false
    @editable
    BoutonDepart : button_device = button_device{}
    # Message à l'écran de tous les joueurs (laisser son texte vide : le minuteur l'écrit).
    @editable
    Affichage : hud_message_device = hud_message_device{}
    # Fin de partie à zéro (facultatif : laisser vide pour seulement envoyer FinEvent).
    @editable
    FinDePartie : end_game_device = end_game_device{}
    @editable
    TerminerLaPartie : logic = false

    FinEvent : event() = event(){}
    var EnCours : logic = false
    var Stop : logic = false

    OnBegin<override>()<suspends> : void =
        BoutonDepart.InteractedWithEvent.Subscribe(OnDepart)
        if (DemarrerSeul?):
            Lancer()

    OnDepart(Agent : agent) : void =
        if (not EnCours?):
            spawn{ Lancer() }

    Arreter() : void =
        set Stop = true

    Lancer()<suspends> : void =
        set EnCours = true
        set Stop = false
        var Reste : int = Duree
        loop:
            Afficher(Reste)
            if (Reste <= 0 or Stop?):
                break
            Sleep(1.0)
            set Reste -= 1
        set EnCours = false
        if (not Stop?):
            FinEvent.Signal()
            if (TerminerLaPartie?, Joueur := GetPlayspace().GetPlayers()[0]):
                FinDePartie.Activate(Joueur)

    Afficher(Reste : int) : void =
        if (M := Quotient[Reste, 60], S := Mod[Reste, 60]):
            Zero := if (S < 10) then "0" else ""
            Affichage.SetText(TexteMinuteur("{M}:{Zero}{S}"))
            Affichage.Show()
