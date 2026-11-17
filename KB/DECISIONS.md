# Décisions partagées

Une ligne par décision, ajoutée en bas : `date | décision | signée par`. On ne modifie pas la ligne d'un autre :
une décision qui change en remplace une autre par une nouvelle ligne qui le dit. Agents : ces décisions sont
fixes ; si une demande ou une issue semble en contredire une, arrêtez-vous et demandez à votre humain.

| Date | Décision | Par |
|---|---|---|
| avant 16/11 | Toute l'interface et tous les messages sont en français | équipe |
| avant 16/11 | Style inspiré du template Addveo : barre latérale noire, grand panneau blanc arrondi, cartes blanches bordées ; sobre, noir et blanc, thèmes clair et sombre ; pas de violet | équipe |
| avant 16/11 | Pas de données de démo ni de simulation : tout est réel et testable | équipe |
| avant 16/11 | L'IA de POP reste locale (algorithmes, `server/src/engine.rs`), aucun appel à une API d'IA externe | équipe |
| avant 16/11 | Sécurité : SQL paramétré, rôle vérifié sur chaque route, en-tête `x-pop: 1` sur les requêtes autres que GET, sessions par cookie HttpOnly, Argon2id | équipe |
| avant 16/11 | Chaque action UEFN est journalisée et annulable ; une suppression demandée par une IA attend un humain | équipe |
| avant 16/11 | Une table ou une colonne = une nouvelle migration numérotée ; on ne modifie jamais une migration existante | équipe |
| avant 16/11 | La publication des îles reste dans le portail créateur d'Epic : POP prépare la checklist et les notes de version | équipe |
| avant 16/11 | Le compte de test local « kl » n'existe que via `start.sh` (`POP_DEV_ACCOUNT=1`), jamais en production | équipe |
| 16/11 | Le relais local `pop-link.exe` reste nécessaire : le serveur MCP d'UEFN refuse toute requête venant d'un site web (voir `UEFN.md`) | benybens |
| 17/11 | Tâches sur les Issues GitHub avec des étiquettes de statut (pas de GitHub Project) ; branche et PR par issue ; validation humaine avant fusion | benybens |
| 17/11 | Jalons : M1 UEFN réel validé, M2 Mise en ligne, M3 Abonnements et paiement, M4 Logiciel de bureau ; chaque issue en a un | benybens |
| 17/11 | Le CI vérifie compilation et tests (serveur, relais), build du web et `web/dist` à jour, migrations jamais modifiées ; rustfmt et clippy pas encore bloquants (dette à traiter dans une tâche) | benybens |
