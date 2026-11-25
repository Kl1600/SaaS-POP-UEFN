# Équipes (module de l'équipe POP) : chaque joueur qui arrive rejoint l'équipe la moins nombreuse.
# Réglez le nombre d'équipes dans les Réglages de l'île ; ce module ne fait que les équilibrer.
# Depuis votre Verse : NombreDans(Equipe) et Equilibrer() pour tout rééquilibrer d'un coup.
using { /Fortnite.com/Devices }
using { /Fortnite.com/Teams }
using { /Verse.org/Simulation }
using { /UnrealEngine.com/Temporary/Diagnostics }

equipes_pop := class(creative_device):
    # Désactiver pour garder les équipes choisies par les Réglages de l'île.
    @editable
    Actif : logic = true

    OnBegin<override>()<suspends> : void =
        GetPlayspace().PlayerAddedEvent().Subscribe(OnArrivee)
        Equilibrer()

    OnArrivee(Joueur : player) : void =
        Placer(Joueur)

    # Replace chaque joueur présent, un par un.
    Equilibrer() : void =
        for (Joueur : GetPlayspace().GetPlayers()):
            Placer(Joueur)

    # Met le joueur dans l'équipe la moins nombreuse (sans compter le joueur lui-même).
    Placer(Joueur : player) : void =
        if (Actif?):
            Collection := GetPlayspace().GetTeamCollection()
            var Meilleure : ?team = false
            var Plus_petite : int = 1000000
            for (Equipe : Collection.GetTeams()):
                var N : int = NombreDans(Equipe)
                if (Collection.IsOnTeam[Joueur, Equipe]):
                    set N -= 1
                if (N < Plus_petite):
                    set Plus_petite = N
                    set Meilleure = option{Equipe}
            if (Choix := Meilleure?, not Collection.IsOnTeam[Joueur, Choix]):
                if (Collection.AddToTeam[Joueur, Choix]):
                    Print("Joueur placé dans l'équipe la moins nombreuse")

    NombreDans(Equipe : team) : int =
        if (Membres := GetPlayspace().GetTeamCollection().GetAgents[Equipe]) then Membres.Length else 0
