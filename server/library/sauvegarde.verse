# Sauvegarde (module de l'équipe POP) : garde les données de chaque joueur d'une partie à l'autre (données
# persistantes de Verse) : pièces, score, niveau, record, nombre de parties et des compteurs libres par nom.
# Depuis votre Verse : Donnees(Joueur), AjouterPieces(Joueur, 10), DefinirNiveau(Joueur, 3),
# ProposerRecord(Joueur, 1200), Compteur(Joueur, "coffres") et AjouterAuCompteur(Joueur, "coffres", 1).
using { /Fortnite.com/Devices }
using { /Verse.org/Simulation }

TexteSauvegarde<localizes>(S : string) : message = "{S}"

# Données gardées pour chaque joueur. Pour en ajouter : nouveau champ avec une valeur par défaut, à la fin
# (ne jamais retirer ni renommer un champ d'une île publiée : les données déjà gardées seraient perdues).
donnees_pop := class<final><persistable>:
    Version : int = 1
    Pieces : int = 0
    Score : int = 0
    Niveau : int = 1
    Record : int = 0
    Parties : int = 0
    Compteurs : [string]int = map{}

# Copie des données, pour n'en changer qu'un champ.
CopieDonneesPop<constructor>(Avant : donnees_pop)<transacts> := donnees_pop:
    Version := Avant.Version
    Pieces := Avant.Pieces
    Score := Avant.Score
    Niveau := Avant.Niveau
    Record := Avant.Record
    Parties := Avant.Parties
    Compteurs := Avant.Compteurs

# Données persistantes : une entrée par joueur, gardée par Fortnite entre les parties.
var SauvegardePop : weak_map(player, donnees_pop) = map{}

sauvegarde_pop := class(creative_device):
    # Message de bienvenue à un joueur qui revient (laisser vide : pas de message).
    @editable
    Bienvenue : hud_message_device = hud_message_device{}
    @editable
    AfficherBienvenue : logic = true

    OnBegin<override>()<suspends> : void =
        for (Joueur : GetPlayspace().GetPlayers()):
            Arrivee(Joueur)
        GetPlayspace().PlayerAddedEvent().Subscribe(Arrivee)

    # Un joueur arrive : on crée ses données s'il n'en a pas, et on compte une partie de plus.
    Arrivee(Joueur : player) : void =
        if (not SauvegardePop[Joueur]):
            if (set SauvegardePop[Joueur] = donnees_pop{}) {}
        if (Avant := SauvegardePop[Joueur]):
            if:
                set SauvegardePop[Joueur] = donnees_pop:
                    CopieDonneesPop<constructor>(Avant)
                    Parties := Avant.Parties + 1
            if (AfficherBienvenue?, Avant.Parties > 0):
                Bienvenue.SetText(TexteSauvegarde("Bon retour ! Niveau {Avant.Niveau}, record {Avant.Record}"))
                Bienvenue.Show(Joueur)

    # Données du joueur (valeurs par défaut s'il n'en a pas encore).
    Donnees(Joueur : player) : donnees_pop =
        if (D := SauvegardePop[Joueur]) then D else donnees_pop{}

    AjouterPieces(Joueur : player, N : int) : void =
        D := Donnees(Joueur)
        if:
            set SauvegardePop[Joueur] = donnees_pop:
                CopieDonneesPop<constructor>(D)
                Pieces := Max(0, D.Pieces + N)

    AjouterScore(Joueur : player, N : int) : void =
        D := Donnees(Joueur)
        if:
            set SauvegardePop[Joueur] = donnees_pop:
                CopieDonneesPop<constructor>(D)
                Score := D.Score + N

    DefinirNiveau(Joueur : player, Niveau : int) : void =
        D := Donnees(Joueur)
        if:
            set SauvegardePop[Joueur] = donnees_pop:
                CopieDonneesPop<constructor>(D)
                Niveau := Niveau

    # Garde Valeur comme record si elle le bat ; rend true dans ce cas.
    ProposerRecord(Joueur : player, Valeur : int) : logic =
        D := Donnees(Joueur)
        if (Valeur > D.Record):
            if:
                set SauvegardePop[Joueur] = donnees_pop:
                    CopieDonneesPop<constructor>(D)
                    Record := Valeur
            return true
        false

    Compteur(Joueur : player, Nom : string) : int =
        if (V := Donnees(Joueur).Compteurs[Nom]) then V else 0

    AjouterAuCompteur(Joueur : player, Nom : string, N : int) : void =
        D := Donnees(Joueur)
        Nouveau := Compteur(Joueur, Nom) + N
        if:
            set SauvegardePop[Joueur] = donnees_pop:
                CopieDonneesPop<constructor>(D)
                Compteurs := ConcatenateMaps(D.Compteurs, map{Nom => Nouveau})

    # Remet les données d'un joueur à zéro (par exemple un bouton « Recommencer »).
    Effacer(Joueur : player) : void =
        if (set SauvegardePop[Joueur] = donnees_pop{}) {}
