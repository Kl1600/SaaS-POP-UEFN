# Économie (module de l'équipe POP) : un solde de pièces par joueur.
# Un bouton fait gagner des pièces ; un autre achète un objet (le distributeur le donne si le joueur a assez).
# Depuis votre Verse : Ajouter(Joueur, 25), Solde(Joueur), et if (Depenser[Joueur, 50]) pour payer.
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }
using { /UnrealEngine.com/Temporary/Diagnostics }

TextePieces<localizes>(S : string) : message = "{S}"

economie_pop := class(creative_device):
    # Bouton qui fait gagner des pièces (facultatif).
    @editable
    BoutonGain : button_device = button_device{}
    @editable
    Gain : int = 10

    # Bouton d'achat (facultatif) : retire Prix pièces, puis le distributeur donne l'objet.
    @editable
    BoutonAchat : button_device = button_device{}
    @editable
    Prix : int = 50
    @editable
    Distributeur : item_granter_device = item_granter_device{}

    # Message à l'écran du joueur (facultatif) : son solde après chaque changement.
    @editable
    Affichage : hud_message_device = hud_message_device{}

    var Soldes : [player]int = map{}

    OnBegin<override>()<suspends> : void =
        BoutonGain.InteractedWithEvent.Subscribe(OnGain)
        BoutonAchat.InteractedWithEvent.Subscribe(OnAchat)

    OnGain(Agent : agent) : void =
        if (Joueur := player[Agent]):
            Ajouter(Joueur, Gain)

    OnAchat(Agent : agent) : void =
        if (Joueur := player[Agent]):
            if (Depenser[Joueur, Prix]):
                Distributeur.GrantItem(Agent)
                Montrer(Joueur)
            else:
                Affichage.SetText(TextePieces("Pas assez de pièces : {Solde(Joueur)} / {Prix}"))
                Affichage.Show(Agent)

    # Solde actuel du joueur (0 s'il n'a encore rien gagné).
    Solde(Joueur : player)<transacts> : int =
        if (S := Soldes[Joueur]) then S else 0

    Ajouter(Joueur : player, Montant : int) : void =
        Nouveau := Solde(Joueur) + Montant
        if (set Soldes[Joueur] = Nouveau) {}
        Montrer(Joueur)

    # Échoue (sans rien retirer) si le joueur n'a pas assez de pièces.
    Depenser(Joueur : player, Montant : int)<transacts><decides> : void =
        S := Solde(Joueur)
        S >= Montant
        set Soldes[Joueur] = S - Montant

    Montrer(Joueur : player) : void =
        Print("Pièces : {Solde(Joueur)}")
        Affichage.SetText(TextePieces("Pièces : {Solde(Joueur)}"))
        Affichage.Show(Joueur)
