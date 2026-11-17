# POP

L'espace de travail des créateurs de maps Fortnite (UEFN) : pages, tableau blanc, planning, tâches, publication et rémunération, en temps réel.

- `server/` : serveur Rust (Axum + SQLite). API, sessions, temps réel (WebSocket).
- `web/` : interface React + TypeScript (Vite).

## Lancer sur le Mac

1. Installer Rust (une seule fois) : `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`, puis ouvrir un nouveau terminal.
2. Dans le dossier `pop` :
   ```sh
   ./start.sh --reseau
   ```
   `--reseau` rend POP joignable depuis le PC Windows du même Wi-Fi et affiche l'adresse à utiliser. Sans l'option, POP n'est visible que sur le Mac.
3. Ouvrir http://127.0.0.1:8080, **créer votre compte**, puis votre studio et votre première map. Il n'y a aucune donnée d'exemple : tout ce que vous voyez est à vous. Invitez l'équipe depuis Équipe et intégrations.

Les données sont dans `server/pop-data.db` (une sauvegarde = copier ce fichier, POP arrêté).

## Tester avec le vrai UEFN (PC Windows)

1. Sur le PC, ouvrir le projet dans UEFN, puis :
   - Projet → Paramètres du projet → **Enable Project Python Scripts** ;
   - Édition → Préférences de l'éditeur → **Agentic Workflows → Model Context Protocol → Start Server** (port 8000).
2. Dans POP (depuis le PC, à l'adresse affichée par `./start.sh --reseau`) : Organiseur → **UEFN Link** → télécharger `pop-link.exe`, puis « Obtenir un code ».
3. Double-cliquer sur `pop-link.exe` : il demande l'adresse de POP, le code et propose les projets trouvés dans `Documents\Fortnite Projects`. Laisser la fenêtre ouverte. Page d'état : http://127.0.0.1:47800.
4. Dans POP, la pastille passe à « UEFN connecté » : l'Organiseur affiche les acteurs du niveau ouvert, l'Éditeur Verse les fichiers `.verse` du projet.

Si quelque chose ne marche pas : dans un terminal sur le PC, `pop-link.exe diagnostic` affiche l'adresse MCP trouvée, la liste des outils qu'UEFN expose et teste Python. Epic ne publie pas la liste exacte des outils ; le relais les choisit par leur nom. Si un rôle est « introuvable », on peut imposer le nom dans `%APPDATA%\pop-link\config.json` :
```json
"tools": { "python": "nom_outil_python", "compile": "nom_outil_compile", "push": "nom_outil_session" }
```
Archivage Lore : ajouter `"lore_cmd": "<commande Lore> {message}"` dans le même fichier.

Recompiler le relais : `cd link && cargo build --release` (sur Windows) ; l'exécutable fourni est dans `web/public/downloads/pop-link.exe`.

## Mettre POP en ligne

Le dépôt contient un `Dockerfile` (interface + serveur, base SQLite dans `/data`). Chez n'importe quel hébergeur qui accepte Docker avec un disque persistant (Railway, Render, Fly.io, un VPS…) :

- construire depuis le dossier `pop`, port **8080** ;
- monter un volume persistant sur **/data** ;
- servir en **HTTPS** (fourni par l'hébergeur). `POP_SECURE_COOKIES=1` est déjà activé dans l'image.

Sur un VPS : `docker build -t pop . && docker run -d -p 8080:8080 -v pop-data:/data pop`, derrière un proxy HTTPS (Caddy, Nginx). Le relais UEFN se connecte alors à `https://votre-domaine`.

## Ce qui fonctionne

- **Comptes et studios** : inscription, connexion, plusieurs studios et plusieurs maps, rôles (propriétaire, admin, membre, freelance, invité en lecture), invitations par lien.
- **Vue d'ensemble** : tuiles, « À faire aujourd'hui », journal d'activité, mini-calendrier des jalons.
- **Pages** : éditeur par blocs (menu « / », raccourcis Markdown, glisser-déposer, dupliquer, transformer), mentions @, pages imbriquées, favoris, commentaires par bloc avec « Résoudre », historique des versions horodatées (SHA-256) avec restauration et export daté, partage privé / équipe / lien en lecture, export Markdown et PDF (impression), bases de données dans une page avec vues Tableau, Kanban, Calendrier, Chronologie, Galerie, Liste. 8 modèles UEFN.
- **Tableau blanc** : canevas infini, zoom, mini-carte, grille et magnétisme, lasso, post-it en 4 nuances de gris, texte, formes, connecteurs droits / coudés / courbes, stylo, surligneur, gomme, cadres (présentation), images et fond « plan de map », commentaires épinglés, formes UEFN, minuteur commun, vote par points avec classement, idées cachées, regroupement, carte mentale au clavier, suivre un membre, curseurs nommés, post-it → tâches, cadre → page, export PNG / SVG / PDF, 8 modèles.
- **Planifier** : générateur en 4 étapes (genre, équipe et heures, IA, date) avec lots, tâches d'1 h, dépendances, jours et assignés suggérés, jalons, option « partir des post-it votés » ; chronologie avec barres déplaçables, recalcul des dépendances, chemin critique, ligne « aujourd'hui », alertes de jalon ; charge par personne avec surcharge ; calendrier des saisons.
- **Tâches** : Kanban avec règle « En cours seulement si les dépendances sont faites », déblocage automatique, historique de chaque changement (bouton, Lore…), fiche tâche (sous-tâches, commentaires, fichiers, origine), vue par personne, « qui bloque qui », fichiers verrouillés.
- **Lancer et suivre** : checklist de modération avec statut et explication, échange de playtests à crédits (questionnaire guidé), « pourquoi ils partent » par minute, checklist heure par heure du jour J.
- **Rémunérer** : règles (%, fixe, à la tâche finie), import manuel ou CSV, relevés par personne (chacun ne voit que le sien, sauf admins), historique, simulateur du passage à 50 % au 01/02/2027, offres en jeu. Aucun virement.
- **Assistant Verse (moteur local)** : analyse du fichier sans aucun service externe : explication en français (rôle, réglages, état, fonctions, événements), relecture (parenthèses, `set` manquant, effets `<suspends>` manquants, appels non annulables dans un `if`, `using` manquants d'après les digests, réglages inutilisés, gestionnaires d'événements absents), corrections automatiques proposées en diff, écriture d'un module depuis une tâche (modèle choisi d'après la tâche, appareils et tags du niveau reliés), documentation dans une page.
- **Analyse** : statistiques publiques des îles Fortnite (API officielle d'Epic, sans clé, collectées toutes les 30 minutes) : maps qui marchent (joueurs du jour, évolution, pic CCU, rétention), meilleurs démarrages (statistiques du premier jour des sorties récentes), marché par genre (part des joueurs, concurrence, opportunités), îles suivies et code de votre propre île. Tendances TikTok autour de Fortnite (hashtags et sons qui montent, vidéos phares, codes de maps cités) via Apify avec le jeton du studio (5 $ de crédit gratuit par mois chez Apify). `POP_ANALYTICS=0` coupe la collecte.
- **Compte et notifications** : nom, mot de passe, mot de passe oublié (e-mail) ou lien de réinitialisation donné par un admin ; notifications (tâche assignée, @mention, tâche débloquée, validation IA demandée) ; recherche globale Ctrl K (tâches, pages, Verse, acteurs, membres) ; page Aide.
- **Intégrations** : webhook Lore / UEFN fonctionnel (check-ins reliés aux tâches, verrous de fichiers, « fini #15 »), verrous Lore pris en compte par l'Éditeur Verse et l'Organiseur. Discord : rapport du soir envoyé automatiquement à l'heure choisie (webhook) et commandes /tache-suivante, /fini, /rapport (application Discord, POP en ligne). GitHub : synchronisation issues ↔ tâches dans les deux sens et webhook (issue fermée → tâche faite).

- **Éditeur Verse** : Monaco (coloration Verse, numéros de ligne, onglets, repli, mini-carte), recherche / remplacement dans tout le projet, aller à la définition, renommer partout, explorateur de fichiers, édition à plusieurs en temps réel (Yjs) avec curseurs nommés. Autocomplétion et survol depuis les digests (API, appareils du niveau, tags), extraits (creative_device, class, for, if, loop, spawn, abonnement, classe de gestion), 6 modules prêts à l'emploi, fiche d'aide par erreur de compilation fréquente. Assistant (expliquer, corriger les erreurs, écrire depuis une tâche, relire et documenter dans une page) avec diff accepté ou refusé ligne à ligne. « Envoyer à UEFN » : écriture, compilation, erreurs soulignées à la ligne, session de test, archive Lore. Historique par fichier, diff, lien à la tâche, retour en arrière renvoyé à UEFN.
- **Organiseur** : options réelles des appareils lues et modifiées dans UEFN à la sélection (outil « Device » d'UEFN MCP), arbre des acteurs (dossiers, type, visibilité, verrou Lore), recherche instantanée (`tag:`, `type:`, `dossier:`, `option:`), filtres enregistrés, multi-sélection (Ctrl/Cmd, Maj), renommer, déplacer, dupliquer, masquer, supprimer avec confirmation. Détails : position / rotation / échelle, tags Verse, options d'appareil en listes avec aide en français, valeur par défaut et options grisées avec la raison, propriétés @editable des appareils Verse, liens d'événements. Modification groupée sur les options communes. Vue du dessus rendue par UEFN, zoom, déplacement des acteurs à la souris, fond possible du tableau blanc. Changements en direct pour toute l'équipe, Ctrl+Z, « Enregistrer le niveau », « Archiver dans Lore », journal avant → après.

## Règles UEFN Link

Chaque action est journalisée et annulable ; droits par rôle (invité en lecture, freelance modifie, membre et plus suppriment) ; rien n'est modifié sur un fichier ou un acteur verrouillé dans Lore par un autre membre. Quand UEFN est fermé, POP passe en lecture seule sur le dernier état connu et met les changements en attente. La publication de l'île reste dans le portail créateur d'Epic : POP prépare la checklist et les notes de version.

## IA connectée (MCP de POP)

Équipe et intégrations → « Créer un jeton » donne un `.mcp.json` à coller dans Claude Code, Cursor, etc. L'IA dispose des mêmes actions (lister / modifier / dupliquer les acteurs, lire / écrire le Verse, compiler, session de test, Lore, annuler), journalisées à son nom ; une suppression attend la validation d'un humain dans l'Organiseur.

## Sécurité

- Mots de passe hachés avec Argon2id, sessions par cookie HttpOnly / SameSite, jeton stocké haché (SHA-256).
- Toutes les requêtes SQL sont paramétrées ; chaque route vérifie l'appartenance au studio et le rôle.
- Protection CSRF (en-tête `X-POP` obligatoire), WebSocket refusé depuis un autre site, limite de tentatives de connexion, taille des requêtes limitée, en-têtes `nosniff` / `DENY`.
- En production derrière HTTPS : `POP_SECURE_COOKIES=1`.

## Variables d'environnement

| Variable | Défaut | Rôle |
| --- | --- | --- |
| `DATABASE_URL` | `sqlite://pop-data.db` | Base SQLite |
| `POP_PUBLIC_URL` | `http://127.0.0.1:8080` | Adresse publique utilisée dans les e-mails |
| `POP_ADMIN_EMAILS` | — | E-mails des administrateurs de la plateforme (séparés par des virgules). Le premier compte créé est aussi administrateur. |
| `POP_OPEN_SIGNUP` | `0` | `1` = inscriptions ouvertes à tous (sinon : liste d’attente + lien d’accès validé dans Administration) |
| `POP_SMTP_HOST`, `POP_SMTP_PORT`, `POP_SMTP_USER`, `POP_SMTP_PASS`, `POP_SMTP_FROM` | — | Envoi d'e-mails (invitations, mot de passe oublié). Sans eux, les liens s'affichent à copier |
| `POP_BIND` | `127.0.0.1` | `0.0.0.0` pour le réseau local ou l'hébergement |
| `PORT` | `8080` | Port (écoute sur 127.0.0.1) |
| `POP_WEB_DIR` | `../web/dist` | Interface construite |
| `POP_SECURE_COOKIES` | `0` | `1` derrière HTTPS |
