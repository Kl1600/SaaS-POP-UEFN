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
| 17/11 | Aucun compte ne devient administrateur selon son adresse IP : administrateurs par `POP_ADMIN_EMAILS` ou `pop-server admin ajouter <e-mail>` sur la machine du serveur ; `POP_TRUST_PROXY` vaut vide, `1` (dernier élément de X-Forwarded-For) ou `cloudflare` (#12) | benybens |
| 18/11 | Hébergement de la bêta (M2) : Hetzner Cloud CPX02 (1 vCPU, 1 Go, facturé à l'heure, sans engagement) ; image construite par GitHub Actions et publiée sur ghcr.io, jamais compilée sur le serveur ; Caddy pour le HTTPS, Litestream vers un stockage S3 hors du serveur (`deploy/`). Étape suivante si la charge monte : CPX12, puis Netcup VPS 500 | benybens |
| 17/11 | Une adresse e-mail est vérifiée par un lien reçu à cette adresse (vérification 24 h ou réinitialisation du mot de passe), jamais par une invitation ou un lien d’accès ; le rôle d’administrateur exige une adresse vérifiée ; sans SMTP les adresses sont vérifiées d’office ; les comptes antérieurs à la migration 0024 sont vérifiés (#11) | benybens |
| 19/11 | Code Rust formaté par rustfmt avec ses réglages par défaut (pas de `rustfmt.toml`) ; `cargo fmt --check` bloque les contrôles `serveur` et `relais` du CI. Remplace « rustfmt pas encore bloquant » de la décision du 17/11 (#8) | benybens |
