# Domaine web

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

### 2026-11-24 · #290 · connexion en un clic par IA
- Fait : IA connectées → « Connecter une IA en un clic » : onglets Claude (copie l'adresse puis ouvre claude.ai/settings/connectors), ChatGPT, Cursor (deeplink cursor://anysphere.cursor-deeplink/mcp/install, config base64 {url}), VS Code (vscode:mcp/install?{name,type:http,url}), Claude Code (commande), Autre ; 3 étapes chacun. La copie se fait avant l'ouverture d'onglet (sinon le presse-papiers refuse). Fiche Popy ai-connect mise à jour.
- Vérifié : navigateur (rendu, boutons) ; deeplinks : format de la doc Cursor / VS Code, non ouverts ici (pas d'éditeur).

### 2026-11-24 · #288 · Popy épuré
- Fait : récap = une phrase + 2 tâches max (Prendre/Continuer) ; objectif affiché seulement s'il est en retard ; plus de liste de points ni de bulle « Une question ? » ; 3 suggestions max ; barre de saisie mise en avant (.gask-main : bordure orange, ombre, 44 px).
- Vérifié : navigateur.

### 2026-11-24 · #284 · Popy : un seul chat
- Fait : plus d'onglets « Mon jour » / « Poser une question » ; le chat s'ouvre sur un message récap (phrase, 3 points max, objectif, 3 tâches max avec Prendre / Continuer), puis l'invitation à poser une question. « Voir ma journée » remet le récap. GuidePanel et AskBar supprimés (plus utilisés).
- Vérifié : navigateur (récap, Prendre, question, 390 px).

### 2026-11-24 · #282 · bêta : seul le message de confirmation après l'envoi
- Fait : CSS .lwbox:has(.lwdone) masque l'en-tête (Bêta privée, titre, introduction, erreur) une fois la demande envoyée, dans la fenêtre du site vitrine et sur /inscription.
- Vérifié : navigateur (/inscription : en-tête visible avant, masqué après).

### 2026-11-24 · #280 · IA connectées : seulement les IA vraiment connectées
- Fait : « Vos IA » et le compteur des cartes ne gardent que les jetons déjà utilisés (last_used) ; les jetons jamais utilisés sont « en attente » (ligne discrète avec Annuler) ; recliquer « Connecter » remplace le jeton en attente de la même IA ; la fenêtre de connexion recharge toutes les 3 s et passe de « En attente de connexion… » à « Connectée ».
- Vérifié : navigateur (2 clics + fermeture → 1 seul jeton en base, rien dans Vos IA ; appel simulé → Connectée, la ligne apparaît).

### 2026-11-24 · #277 · logo de la barre latérale plus grand
- Fait : logo 34 → 56 px de haut en barre normale (halo agrandi), barre repliée inchangée.
- Vérifié : navigateur 1440 px, barre normale et repliée.

### 2026-11-24 · #276 · bêta : message de confirmation
- Fait : okText/approvedText de la liste d'attente, dans les 16 langues : annoncent l'e-mail d'activation (lien pour choisir son mot de passe) et rappellent les spams, au lieu de « identifiant et mot de passe provisoire ».
- Vérifié : tsc -b, vite build.

### 2026-11-23 · #271 · menu du compte
- Fait : adresse e-mail coupée par « … » avec l'adresse complète au survol (.acct-mail) ; menu borné à l'écran ; barre latérale repliée : le menu sort à droite de la barre (position fixe, 260 px) au lieu de faire 39 px de large.
- Vérifié : navigateur (1440 px normal et replié, 1100 px, 390 px).

### 2026-11-23 · #269 · supprimer une map ou un studio
- Fait : sélecteur de map, bouton « ⋯ » au survol d'une map (clic droit aussi) : Renommer (PATCH /api/maps/N), Déplacer vers… (previewMove + ConfirmMove), Supprimer la map ; « ⋯ » sur un studio d'équipe : Ouvrir, Paramètres, Supprimer le studio (propriétaire). Paramètres > onglet « Maps et studios » (components/MapsStudios.tsx) : studios et maps, Ouvrir, Supprimer, Quitter. Fenêtre commune components/DeleteDialog.tsx (ce qui sera perdu, Exporter le studio d'abord, nom à taper) qui remplace les prompt() de la page du studio. Dernière map perso supprimable depuis l'interface ; dernière map d'un studio d'équipe : explication au lieu du bouton.
- Vérifié : navigateur (renommer, supprimer la map ouverte → accueil, dernière map, onglet, suppression du studio).

### 2026-11-23 · #265 · arborescence profonde
- Fait : retrait 16 px par niveau (chevron .tchev 16 px, guides 9+7 px), au-delà de 4 niveaux le retrait est plafonné (marque « ⋯ », .tdeep), titre complet au survol (title), colonne élargissable 200–480 px en tirant son bord (.tree-resize, var --treew, pop.tree.width ; double-clic = 250), lignes en color-mix(--text 22 %) pour le sombre.
- Vérifié : navigateur, 11 niveaux.

### 2026-11-23 · #261 · calendrier : heure, détail, tâches
- Fait : migration 0035 (milestones.time HH:MM facultative, milestones.detail) ; add_milestone accepte time/detail, nouvelle route PATCH /api/milestones/{id} (update_milestone) ; /plan/timeline renvoie time et detail. Web : clic sur un jour ou « Ajouter » → fenêtre « Nouvel événement » : Objectif (titre, date, heure, détail) ou Tâche (titre, jour prévu, personne, via POST /tasks). Heure affichée en gras sur la pastille, détail dans la fiche, bouton Modifier.
- Tests : tests_objectifs (format de l'heure, création, refus de 25:00, modification).
- Vérifié : navigateur.

### 2026-11-23 · #260 · logos des IA
- Fait : components/AiLogo.tsx, logos de @lobehub/icons-static-svg 1.106 (MIT), importés en ?raw (SVG du paquet, pas de saisie) : Claude Code, Claude, Codex, Copilot (VS Code), Gemini CLI en couleur ; Cursor, Windsurf et MCP (« Autre IA ») en une couleur qui suit le thème. Le logo Codex officiel est bleu-violet : c'est la couleur de la marque, pas de la DA de POP.
- Vérifié : navigateur, clair et sombre.

### 2026-11-23 · #259 · barre du haut
- Fait : loupe, cloche, bouton clair/sombre (Soleil/Lune, app.toggleDark) et + alignés à droite (.top-spacer) ; la recherche ouverte (Ctrl K) s'élargit vers la gauche.
- Vérifié : navigateur (1440 et 390 px, bascule gardée après rechargement).

### 2026-11-23 · #257 · Planifier : calendrier façon Apple
- Fait : onglet Planning (Gantt) retiré, Planifier ouvre directement PlanCalendar. Colonne de gauche : mini-mois (point sur les jours chargés), « Calendriers » = filtres par type avec case de couleur (pop.cal.filters) + « Tâches finies » (pop.cal.done), « À planifier » (tâches sans jour, à glisser). En-tête : mois en gras + année, sélecteur Semaine / Mois / Année, flèches, Aujourd'hui, Recalculer, + Objectif. Glisser une tâche sur un jour → POST /plan/move (le serveur recalcule les dépendances : une tâche glissée sur un jour passé revient à aujourd'hui). Week-ends grisés, vue Année en 12 mini-mois.
- Vérifié : navigateur (glisser, filtres mémorisés, 3 vues, sombre, 390 px).

### 2026-11-22 · #151 · flèches entre objets
- Fait : tracé en glissant d'un objet à un autre (les deux clics restent possibles), ligne d'aperçu orange qui suit la souris, cadre orange sur l'objet de départ (plein) et l'objet visé (pointillé) ; Échap, clic dans le vide ou changement d'outil annulent ; flèche sélectionnée : 2 poignées (.endhandle, data-endhandle=id:from|to) pour raccrocher un bout à un autre objet (annulable). Objet sous le pointeur trouvé par elementFromPoint (marche pendant la capture du pointeur). Les bouts partent du bord (edge) et suivent les objets.
- Vérifié : navigateur (geste, aperçu, Échap, clic vide, deux clics, raccrochage, annuler/rétablir, déplacement, 2e onglet).

### 2026-11-22 · #193 · calendrier de Planifier
- Fait : onglet « Calendrier » (pages/PlanCalendar.tsx) à la place de « Saisons et mises à jour » : grille mois (6 semaines, lundi d'abord, aujourd'hui en rond rouge) et semaine, pastilles colorées par tag (Sortie, Objectif de la map, Date à éviter, Date Fortnite à viser, Impact revenus, Tâche planifiée), filtres par tag mémorisés (localStorage pop.cal.filters), clic sur un événement → détail (ouvrir la tâche, supprimer l'objectif), clic sur un jour → nouvel objectif. Carte « Objectifs » et bloc des saisons retirés (ajout et suppression d'objectif passent par le calendrier). Données : /plan/timeline + /seasons, sans changement serveur. /planifier?vue=calendrier ouvre l'onglet.
- Piège : la classe `.cal` existe déjà (mini-calendrier) : la carte s'appelle `.pcal`.
- Vérifié : navigateur (clair, sombre, 390 px).

### 2026-11-22 · #207 · artefact Claude dans le tableau blanc
- Fait : nouvel objet `artifact` (menu Plus d'outils > Artefact Claude) : lien + titre, affiché en carte avec « Ouvrir dans Claude ». Déplaçable, redimensionnable, annulable, synchronisé (même circuit que les autres objets), exporté en SVG.
- Constat (22/11) : claude.ai renvoie `Content-Security-Policy: frame-ancestors 'self'` : impossible de l'afficher dans une iframe de POP, d'où la carte. Les artefacts sont aussi privés par défaut (connexion claude.ai).
- Sécurité : seuls les liens https://claude.ai/(artifact|public/artifacts|share)/<id> passent, vérifiés dans le navigateur ET par le serveur (boards::artifact_url_ok, test tests_artefact) ; un autre objet est ignoré.
- Vérifié : navigateur (lien refusé, ajout, synchro dans un 2e onglet, déplacement, redimensionnement, 3 annulations) + cargo test.

### 2026-11-22 · #202 · idées de maps
- Fait : chiffres des cartes en grille icône + valeur + libellé (joueurs par jour, temps de jeu moyen, nouvelles maps qui percent, évolution sur 7 jours) ; bannières sobres (fond neutre, liseré de couleur du genre à gauche, plus de dégradé) ; fiche élargie (980 px) : accroche, badges et actions en haut, puis deux colonnes (pourquoi / chiffres à battre + graphique), puis maps à étudier / twists et pièges ; mini-courbe des cartes retirée ; graphique « Joueurs par jour » avec axes (joueurs, jours) dans la fiche.
- Vérifié : navigateur avec des données de marché factices dans la base de test locale (/tmp), clair et sombre.

### 2026-11-22 · #204 · onglet Test
- Fait : « Échange de playtests » devient « Test » ; onglet « Pourquoi ils partent » retiré. Section « Demandes de test » : à tester (demandes des autres) + nos demandes avec leur état (en cours / terminée, retours reçus : champ serveur `received`) + formulaire. Section « Retours » : raisons d'arrêt (ancien /churn) puis chaque retour avec ses réponses. Crédits inchangés. Aide, guide et README mis à jour.
- Vérifié : navigateur (publication d'une demande, retour inséré dans la base de test locale) + cargo test.

### 2026-11-22 · #188 · sélecteur de map
- Fait : groupes titrés « Mes maps (seul) » puis un par studio, avec rôle et nombre de maps ; map ouverte marquée (barre orange + « Ouverte ») ; créations regroupées en bas sous « Créer » (map perso, map dans un studio, studio) au lieu d'une ligne sous chaque groupe ; « Toutes mes maps » devient « Vue d’ensemble des maps » ; astuce de glisser-déposer. Le glisser-déposer (#167) est inchangé.
- Vérifié : navigateur (3 groupes, chaque bouton Créer ouvre la bonne fenêtre, glisser vers un autre studio : zone surlignée puis contrôle du serveur).

### 2026-11-22 · #201 · pages du studio
- Fait : « Maps du studio » = bouton « Créer une map » en haut à gauche + tableau (map, état, genre, sortie visée modifiable par un admin, avancement, dernière activité, gains, suppression) ; grand bloc « Créer » retiré (« Nouveau studio » reste dans l'espace perso et le +). Nouvel onglet « Paramètres » (/studio/N/parametres) avec nom, export, transfert, zone sensible (StudioSettings, sorti de Maps). Serveur : maps_summary renvoie `last_activity` (MAX(activity.at)), sans migration.
- Vérifié : navigateur (création d'une map dans le studio, renommage) + cargo test.

### 2026-11-22 · #203 · Marché allégé
- Fait : bloc « Votre map » (code d'île + maps suivies) et « Joueurs par jour » retirés du Marché (studio et Analyse). Le code d'île se relie toujours dans Lancer et suivre, une map se suit toujours depuis sa fiche (bouton Suivre). En-tête et « En bref » raccourcis (3 lignes), « En bref » et « Les genres » repliables (details.fold). Maps qui marchent : recherche à droite de la ligne des filtres.
- Vérifié : navigateur avec une base de test locale (60 îles factices dans /tmp, jamais dans le dépôt).

### 2026-11-22 · #197 · icône de page
- Fait : icône à gauche du titre (bouton .picon) ; panneau IconPicker chargé à l'ouverture : 1 539 icônes Lucide (étiquettes tirées de lucide-static 0.460, ISC) en catégories Jeu / Économie / Carte et décor / Projet / Équipe, recherche avec mots-clés français (épée, argent, trophée…) et classement, emojis (unicode-emoji-json, MIT, jusqu'à la v14) et récentes ; « Retirer ». Stockage : emoji ou `lucide:<nom>` (limite serveur portée à 64 caractères). Rendu dans l'arborescence par PageIcon, qui charge lucideAll à part.
- Piège : `.card .card` rend une carte imbriquée transparente, d'où la règle `.card .ipick.card`.
- Données : components/iconData.json, régénérable depuis lucide-static/tags.json et unicode-emoji-json/data-by-group.json.
- Vérifié : navigateur (recherche, choix, emoji, retrait, sombre, chunks à part).

### 2026-11-22 · #198 · arborescence des pages
- Fait : lignes de rattachement en angle droit entre une page et ses sous-pages (.tguide, .telbow, en CSS), « Tout replier », repli mémorisé par map (localStorage pop.tree.closed.<map>).
- Vérifié : navigateur (3 niveaux, repli gardé après rechargement).

### 2026-11-22 · #192 · infobulles
- Fait : composant `Tooltips` (components/Tooltip.tsx), monté dans Layout : une bulle noire unique (claire en thème sombre) au survol (300 ms) et au focus clavier, pour `[data-tip]` et les boutons à icône (.iconbtn, .tool, .plusbtn, .popy-fab) ; au-dessus ou en dessous selon la place, recentrée pour ne pas sortir de l'écran ; un `title` est repris (pas de double bulle). Anciennes infobulles CSS ::after retirées.
- Piège : positions en pixels CSS avec `cssRect`/`uiScale` (zoom 90 %).
- Vérifié : navigateur (tableau blanc, barre du haut, Popy, admin).

### 2026-11-22 · #190 · Popy
- Fait : « Boussole » renommée « Popy » (interface, aide, messages du serveur, outil MCP next_task) ; icône retirée de la barre du haut ; bouton rond flottant `.popy-fab` en bas à droite sur les pages d'une map, il ouvre l'assistant (évènement `boussole` gardé dans le code).
- Vérifié : navigateur (bouton au même endroit sur 3 pages, ouverture), aucune erreur JS.

### 2026-11-22 · #214 · « jalon » devient « objectif »
- Fait : remplacement dans web/src et server/src (accords : l’objectif, de l’objectif, cet objectif, Nouvel objectif), traductions anglaises milestone → goal ; mots-clés de l'aide gardent « jalon » pour la recherche ; valeur stockée `kind = 'jalon'` (plan.rs) et mots-clés de guide.rs inchangés.
- Vérifié : cargo test, tsc + vite build, navigateur (Dashboard, Planifier : aucun « jalon »).

### 2026-11-22 · #210 · Organiseur retiré
- Fait : page Organiseur retirée du menu ; /organiseur et la pastille d'état mènent à Paramètres → UEFN Link (`UefnLinkSettings` dans Organizer.tsx, chargé à la demande) : relais (Pairing), suppressions à valider, journal des actions UEFN avec annulation. Mentions de l'Organiseur retirées de l'appli, de l'aide et du README ; décision notée dans KB/DECISIONS.md. Page d'accueil publique (Landing) pas encore mise à jour.
- Vérifié : navigateur, aucune erreur JS.

### 2026-11-22 · #205 · IA connectées dans Paramètres
- Fait : page Settings.tsx en onglets (Mon compte, IA connectées ?onglet=ia) ; AI.tsx s'affiche intégrée (embedded) pour la map ouverte ; /map/N/ia redirige ; entrée « IA connectées » retirée de la barre ; logos (Brand) devant chaque IA (Codex et « Autre » gardent l'icône robot : pas de logo OpenAI dans simple-icons).
- Corrige aussi #200 : la classe .brand des logos entrait en conflit avec celle du logo POP (renommée .blogo).
- Vérifié : navigateur, aucune erreur JS.

### 2026-11-22 · #200 · logos des intégrations
- Fait : composant `Brand` (components/Brand.tsx) : logos simple-icons (CC0, nouvelle dépendance), dessinés en currentColor ; Discord, GitHub, Lore (logo Epic Games) devant leur titre dans Intégrations. +12 Ko au bundle.
- Vérifié : tsc + vite build.

### 2026-11-22 · #194 · Planifier allégé
- Fait : carte « Charge par personne » retirée ; onglet Générateur retiré de Planifier : il s'ouvre par /planifier?vue=generateur, depuis la dernière ligne du sélecteur de map (quand une map est ouverte).
- Vérifié : navigateur, aucune erreur JS.

### 2026-11-22 · #212 · rouvrir une tâche finie
- Fait : `reblock_after_reopen` (tasks.rs) quand une tâche passe de « fait » à autre chose : les suivantes non commencées l'attendent de nouveau (évènement + journal « de nouveau bloquée » ; elles restent « à faire », le serveur refuse déjà de les passer en cours), celles en cours restent et leur responsable reçoit une notification.
- Vérifié : cargo test `rouvrir_rebloque_les_taches_suivantes`.

### 2026-11-22 · #195 · fiche de tâche large
- Fait : nom complet du responsable sur les cartes ; détail : « Prise le » (dernier passage à « En cours » lu dans task_events, champ started_at de /api/tasks/{id} : pas de migration) et « Finie le » (done_at) ; Drawer reçoit wide, fiche en deux colonnes (.tgrid : contenu à gauche, propriétés et dates à droite, empilé sur mobile).
- Vérifié : cargo build, navigateur 1440 et 390 px, aucune erreur JS.

### 2026-11-22 · #196 · tâches bloquées sur le Kanban
- Fait : onglet « Par personne » (Charge de la semaine, Qui bloque qui) retiré ; carte qui attend une autre tâche : bordure rouge, icône danger (infobulle « Bloquée par #N ») et « Attend #N · ne peut pas être prise » ; détail : encadré « Bloquée par » avec liens cliquables (TaskDrawer reçoit onOpen). Le refus de passer en cours existait déjà côté serveur.
- Vérifié : navigateur (onglets, icône, glisser refusé avec message, lien vers #1), aucune erreur JS.

### 2026-11-22 · #209 · Tâches sans « Fichiers verrouillés »
- Fait : onglet et composant Locks retirés de Tasks.tsx ; route /locks et webhook Lore gardés côté serveur.
- Vérifié : tsc + vite build.

### 2026-11-22 · #199 · Rémunérer allégé
- Fait : cartes « Simulateur passage à 50 % » et « Offres en jeu » retirées de Money.tsx (API /offers et données gardées).
- Vérifié : tsc + vite build.

### 2026-11-22 · #206 · tableau blanc sans Atelier
- Fait : menu « Atelier » (minuteur, vote, masquer, regrouper, suivre) retiré de la barre du tableau ; données et code de méta conservés.
- Vérifié : tsc + vite build.

### 2026-11-22 · #191 · barre du haut
- Fait : avatars de présence retirés ; recherche = loupe qui s'ouvre en champ (clic ou Ctrl K via l'évènement `pop:search`, Échap ou sortie à vide la referme) ; bouton « + » (`CreateMenu` : tâche si map modifiable, map, studio) ; « Relais hors ligne » → « UEFN déconnecté » ; compte en bas de la barre latérale (`.side-foot`), menu ouvert vers le haut avec Aide et support.
- Vérifié : navigateur 1440 px, chaque élément cliqué, aucune erreur JS.
- Piège : `.side` a `overflow-x: hidden` ; un menu qui en dépasse est coupé, d'où `.acct-menu` à la largeur de la barre.

### 2026-11-22 · #189 · Dashboard
- Fait : « Vue d'ensemble » renommée « Dashboard » (menu, titre, aide) ; Boussole retirée de la page (bouton flottant : #190) ; tuiles `.tiles.six` (6, 3 × 2 ou 2 × 3, jamais une seule) ; journal limité à 5 + « Voir plus » (fiche avec les 60 dernières actions, `LIMIT 60` dans `plan::overview`) ; 3 prochains jalons + « Voir plus » vers Planifier ; calendrier du mois retiré.
- Vérifié : navigateur 1800, 1440, 1000 et 390 px, aucune erreur JS.

### 2026-11-22 · #174 · thème et langue dans Paramètres, interface à 90 %
- Fait (travail de l'agent 2, repris car sans accès GitHub) : thème et langue déplacés vers Paramètres → Mon compte ; `zoom` 90 % sur `<html>` au-delà de 1100 px (`--ui-zoom`), conversions souris dans `web/src/zoom.ts` ; fiches centrées (#172) recalculées avec `--ui-zoom`.
- Vérifié : navigateur 1440 et 1000 px : zoom 0,9 / 1, barre sans thème ni langue, Clair/Sombre gardé après rechargement, post-it posé et glissé au bon endroit, sélecteur sous son bouton, Boussole centrée, langue sur la page d'accueil publique. Organiseur avec UEFN : non testé (pas d'UEFN ici).
- Piège : avec `zoom` sur `<html>`, `clientX` et `getBoundingClientRect` sont en pixels d'écran ; passer par `cssRect`/`uiScale` de `zoom.ts` pour poser des éléments, et diviser les `100vh`/`100vw` par `--ui-zoom`.

### 2026-11-22 · #172 · fiches en fenêtre centrée arrondie
- Fait : `.drawer` devient une fenêtre centrée aux coins arrondis (hauteur selon le contenu, défilement interne, plein écran ou presque sur mobile) ; `Layer` dans `ui.tsx` rend les fiches dans `.app` par un portail ; Boussole : `.guide-sugs` en `minmax(0,1fr)` (les boutons ne débordent plus).
- Vérifié : navigateur, 1440 px et 390 px, clair et sombre : Boussole et détail de tâche centrés, sans défilement horizontal ; Échap et clic à côté ferment. Fiches île, genre, idée : compilé et code lu seulement (pas de données de marché en local).
- Piège : un `position: fixed` dans une page se cale sur le parent animé ou transformé, pas sur l'écran ; passer par `Layer` (portail dans `.app`, pas dans `body`, sinon on perd les variables du thème).

### 2026-11-21 · #169 · menu du compte sans création
- Fait : « Nouvelle map » et « Nouveau studio » retirés du menu du compte ; on crée depuis le sélecteur de map.
- Vérifié : navigateur (menu, création map et studio depuis le sélecteur), aucune erreur JS.

### 2026-11-21 · #167 · sélecteur : plus d'« Accueil », glisser-déposer des maps
- Fait : lien « Accueil » retiré de la barre latérale (la page `/` reste via « Toutes mes maps ») ; dans le sélecteur, une map se glisse vers un autre groupe → même confirmation que l'Accueil ; la confirmation et l'aperçu sont dans `components/MoveMap.tsx`, partagés par `Home.tsx` et `MapSwitcher.tsx`.
- Vérifié : navigateur (perso → studio, studio → perso, annuler, clic simple, Accueil), aucune erreur JS.
- Piège : le serveur refuse de sortir la dernière map d'un studio (message en toast) ; à revoir depuis que #140 permet les studios vides.

### 2026-11-21 · #140 · studio sans première map
- Fait : `POST /api/studios` accepte `map_name` absent ou vide (aucune map créée, `map_id: null`) ; fenêtre « Nouveau studio » : « Première map (facultatif) », Genre et Sortie visée affichés seulement si un nom de map est saisi ; studio vide → page Équipe du studio.
- Vérifié : `cargo test` (`studio_avec_ou_sans_premiere_map`) ; navigateur : création vide, pages du studio, Accueil, sélecteur de maps, puis « Une map » en équipe, et studio avec première map comme avant ; aucune erreur JS.
- Piège : `pop-server` lancé pour les tests navigateur doit être recompilé (`cargo build`) : `cargo test` ne met pas à jour le binaire.

### 2026-11-21 · #67 · annonce pour tous les comptes
- Fait : migration 0031 (annonces + fermetures), `server/src/announce.rs` (publier, retirer, fermer ; une seule active ; date de fin), annonce dans `/api/me` ; bandeau dans `Layout.tsx` et onglet Administration → Annonce.
- Vérifié : `cargo test` (publier, fermer, nouvelle annonce, retirer, échue, non-admin refusé) ; bandeau vu en clair, sombre et mobile (captures).
- Piège : les styles sont dans `@layer app` ; un `!important` hors couche perd contre celui d'une couche.
```
EN COURS | #84 Forge F10 · Organiseur : rangement et renommage automatiques | 21/11 02:20
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : bouton « Ranger le niveau » (aperçu avant → après), action serveur actor_organize annulable, tests
Reste : test avec le vrai UEFN
Bloque : rien
```
### 2026-11-22 · #150 · Pages : plus rien de coupé à droite
- Fait : le titre de page et les cellules des tableaux sont des zones de texte qui grandissent (`AutoText` dans `Pages.tsx`) au lieu d'`<input>` qui ne passent jamais à la ligne ; Entrée n'y ajoute pas de ligne. Le bloc de code défile seul à l'horizontale, une longue URL se coupe (`overflow-wrap:anywhere`, `min-width:0` sur `.btext`).
- Vérifié : navigateur en local à 900 px, thèmes clair et sombre (largeur de page = largeur de fenêtre), saisie dans le titre et une cellule enregistrée.
- Leçon : un `<input>` dans une cellule ou un titre coupe toujours le texte long ; dans un conteneur flex, un enfant qui doit rétrécir a besoin de `min-width:0`.
### 2026-11-22 · #171 · noms des jalons lisibles dans la chronologie
- Fait : `MilestoneRow` dans `Plan.tsx` mesure chaque nom après le rendu, le garde dans la piste (bords gauche et droit) et le pose sur la première rangée libre, dans l'ordre des dates ; la ligne « Jalons » grandit avec les rangées. Nom complet et date au survol du losange et du nom ; nom raccourci au-delà de 220 px.
- Vérifié : navigateur en local, 8 jalons dont 6 sur 5 jours consécutifs et 2 le même jour : aucun chevauchement, aucun nom hors de la piste, en clair (écran large) et en sombre (900 px).
- Vu en passant (hors périmètre) : les dates du haut de la chronologie se chevauchent aussi au début de la période (« 15 nov. »).
### 2026-11-22 · #163 · vue « Dépendances » des tâches (lecture)
- Fait : onglet « Dépendances » dans Tâches (`?vue=dependances`), composant `components/DepGraph.tsx` : un nœud par tâche, colonne = plus longue chaîne de dépendances avant elle (ce qui débloque à gauche), une bande par lot (« Sans fonctionnalité » à la fin), flèches SVG. États sans couleur : prenable (trait plein), en cours (trait épais), bloquée (pointillés), faite (remplie) ; chemin critique repris de `/plan/timeline` (barre à gauche). Survol : dépendances et tâches débloquées en évidence ; clic : panneau de la tâche ; filtres membre/étiquette/texte de la page grisent le reste. Aucune nouvelle route.
- Vérifié : navigateur en local sur une copie de GrassCutter (22 tâches, 26 dépendances, lue par le MCP le 22/11) : ordre gauche-droite du critère 1, cadres des lots, états, survol, clic, filtre, thème sombre.
- Suite : édition des flèches et des lots, `lot` dans le MCP → #182.
### 2026-11-22 · #182 · modifier les dépendances et les lots depuis la vue Dépendances
- Fait : dans `DepGraph`, tirer le rond à droite d'une tâche vers une autre ajoute la dépendance ; cliquer une flèche puis « Retirer la dépendance » (ou Suppr) la retire ; refus de boucle affiché tel que le serveur le dit. Champ « Fonctionnalité » (liste des lots de la map) dans le panneau d'une tâche. Serveur : `lot` dans `PATCH /api/tasks/:id` (journalisé), `set_deps` vérifie tout avant d'écrire puis écrit dans une transaction (un refus n'efface plus les dépendances existantes), MCP `create_task` avec `lot` et nouvel outil `set_task_lot`.
- Vérifié : `cargo test` (`tests_lots_deps`, test MCP), navigateur en local sur la copie de GrassCutter (boucle #21 → #18 refusée en français, flèche #12 → #21 ajoutée et visible dans le panneau, retirée, #20 rangée dans « Terrains »).
- Leçon : dans une page zoomée (`html{zoom}`), convertir la souris en coordonnées du graphe avec le rapport `getBoundingClientRect().width / offsetWidth`.

### 2026-11-24 · #129 · importer des fichiers Markdown en pages
- Fait : `POST /api/maps/{mid}/pages/import` (`pages::import_md`, 300 fichiers, 1 Mo chacun, 10 Mo en tout ; `.md` seulement, refus clair sinon) ; un dossier = une page parente ; un `# titre` en tête devient le titre ; liens `(autre.md)` réécrits vers `/map/{mid}/pages/{id}` ; pages `team`, version 1 gardée. `from_markdown` : titres 1 à 6, élément de liste replié sur plusieurs lignes gardé entier, langue du code laissée vide si absente. Pages : bouton « Importer » (fichiers ou dossier entier) ; gras, italique, code et liens Markdown affichés dans le texte des blocs (composant `Inline`), le texte redevient brut pendant la saisie (clés `riche` / `saisie` pour que React ne garde pas l'ancien texte).
- Limite : la mise en forme dans les cellules de tableau reste en Markdown brut.
- Vérifié : `cargo test` (`pages::tests_import`) ; Chrome sans tête : import du dossier `KB/` de POP (13 fichiers) + une carte d'essai, liens entre pages, thèmes clair et sombre, aller-retour de `DECISIONS.md` par l'export Markdown (33 lignes sur 33).
