# Domaine web

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

### 2026-11-28 · #543 · Popy devient Léo
- Fait : components/LeoVisage.tsx (carré sombre, yeux orange qui clignent, regardent la souris ou au hasard ; coupé si prefers-reduced-motion) sur le bouton flottant et l'en-tête du panneau ; tous les textes « Popy » → « Léo » (FR, EN, serveur guide.rs / mcp.rs). Les identifiants (popy-fab, guide) restent.
- Nom choisi par l'humain : « Léo » (pas « Léon », nom de l'assistant de Vinteer).

### 2026-11-28 · #539 · menus de la barre latérale ouverts à droite
- Fait : MapSwitcher.tsx (position fixe à droite de .side, alignée sur le haut du bouton) et menu du compte dans Layout.tsx (fixe, aligné sur le bas du bouton) ; retour au placement d'avant si la place manque (mobile).
- Vérifié : Playwright 1440 px (menu des maps x=263 / haut 118, menu du compte bas 847 = bas du bouton, fermeture au clic à côté).

### 2026-11-28 · #533 · didacticiel : flou, défilement, centrage
- Fait : Didacticiel.tsx : voile flouté (.tuto-voile, backdrop-filter) percé par clip-path path(evenodd) autour de la zone ; contour à 8 px de chaque côté, gardé dans l'écran ; défilement automatique (scrollIntoView center/start) si la zone est coupée ; molette, tactile et touches de défilement bloquées pendant le didacticiel ; Précédent toujours visible, ← → Échap ; points de progression.
- Vérifié : Playwright 1440×760 sur le Dashboard (écarts 8 px, molette sans effet, défilement auto à l'étape 3, page libre à la fin).

### 2026-11-28 · #519 · vitrine : prix par jour et explications du comparatif
- Fait : pricing.perDay et pricing.hints (alignés sur pricing.rows, '' pour les titres de groupe) dans les 16 dictionnaires ; Landing.tsx les affiche (.lperday, small.lhint).
- Vérifié : captures FR, EN, AR (droite à gauche).

### 2026-11-28 · #516 · abonnement : prix par jour, annuel, fin d'essai
- Fait : Billing.tsx : « moins de 0,50 € / d'1 € par jour », choix Mensuel / Annuel (2 mois offerts, 129,90 € et 249,90 €), réassurance ; écran de fin d'essai plus doux. stripe.rs : STRIPE_PRICE_STARTER_ANNUAL / STRIPE_PRICE_PRO_ANNUAL facultatifs (choix affiché si les deux existent), reconnus par le webhook ; checkout { kind, annuel }.
- Reste à faire par l'humain ou benybens : créer les deux prix annuels dans Stripe et les mettre dans /opt/pop/.env.
- Vérifié : faux Stripe local (POP_STRIPE_BASE) : Annuel Pro envoie price_a_p ; captures.

### 2026-11-28 · #508 · icônes Tabler (style Vinteer)
- Fait : @tabler/icons-react ; web/src/icons.tsx ré-exporte chaque icône sous son ancien nom lucide (props size / strokeWidth / className, trait 1,75 par défaut, classe pi pi-<nom>). Tous les imports passent par ce module ; seul lucideAll.ts (icônes choisies pour les pages) garde lucide.
- Nouvelle icône : l'ajouter dans icons.tsx (vérifier que IconXxx existe dans @tabler/icons-react).
- Vérifié : 6 onglets de map + vitrine, aucune icône vide ; JS principal +2 %.

### 2026-11-28 · #506 · barre latérale façon Vinteer
- Fait : .side 284 px (≈256 affichés avec zoom 0,9), titres de catégorie en capitales ; tri au pointeur dans Layout.tsx (triDebut/triMove/triFin : setPointerCapture sur la poignée, translateY de l'onglet et des voisins, bornes de la catégorie), plus de glisser-déposer HTML5.
- Piège : clientY est en pixels écran, translateY en pixels CSS : diviser par le zoom.
- Vérifié : Playwright 1440 px (largeur 255,6 px, capture en cours de glisser, bornes, rechargement).

### 2026-11-28 · #503 · poignée dans un cadre
- Fait : .nav-grip en carré bordé arrondi 26 px (comme la capture de l'humain).
- Vérifié : Playwright 1440 px, capture + glisser dans Organiser.

### 2026-11-28 · #499 · poignée de déplacement des onglets
- Fait : Layout.tsx : poignée ⋮⋮ (GripVertical) à droite de l'onglet, visible au survol ; seule elle est draggable (setDragImage sur le lien entier) ; dépôt accepté seulement dans la catégorie d'origine (dragCat).
- Vérifié : Playwright 1440 px (poignée, texte, autre catégorie, rechargement).

### 2026-11-28 · #496 · didacticiel au premier passage
- Fait : components/Didacticiel.tsx : 2 à 4 bulles par onglet (Dashboard, Planifier, Tâches, Pages, Tableau blanc, Lancer et suivre, Rémunérer, Maps à faire), zone visée entourée, Précédent / Suivant / Passer ; onglets vus dans ui_prefs.tours ; Paramètres → Affichage → « Revoir les didacticiels ». Piège : html { zoom: .9 } sur grand écran, il faut diviser getBoundingClientRect et innerWidth par le zoom pour placer un élément fixe.
- Vérifié : navigateur 1440 px, les 8 onglets.

### 2026-11-28 · #493 · barre latérale personnalisable
- Fait : Layout.tsx : titres de catégorie cliquables (replier / déplier, chevron), onglets de la map déplaçables par glisser-déposer entre Organiser, Créer et Publier (arrange() : ordre du compte puis onglets nouveaux à leur place d'origine) ; Studio et Analyse repliables. Préférences dans ui_prefs.nav ({order, collapsed}). Barre en icônes : pas de repli ni de glisser. Paramètres → Mon compte → Affichage : « Rétablir l'ordre d'origine ».
- Vérifié : navigateur 1440 px (repli, glisser Rémunérer avant Planifier, gardé après rechargement, rétablir) et 900 px.

### 2026-11-27 · #448 · Planifier sans l'avis de faisabilité au-dessus du calendrier
- Fait : Plan.tsx : AvisFaisabilite n'est plus affiché sur le calendrier, seulement dans la vue « Équipe et faisabilité ». Le Dashboard le garde.
- Vérifié : navigateur 1440 px.

### 2026-11-26 · #396 · Planifier : détail de la tâche sur place
- Fait : PlanCalendar.tsx : « Ouvrir la tâche » ouvre TaskDrawer (exporté de Tasks.tsx) au lieu d'aller dans Tâches ; la liste des tâches n'est chargée qu'à l'ouverture (dépendances) ; à la fermeture, le calendrier se recharge.
- Vérifié : navigateur 1440 px.

### 2026-11-25 · #384 · crédits IA retirés de l'interface
- Fait : carte « Crédits IA » et packs retirés de Paramètres → Abonnement (Billing.tsx) ; bouton du plan de production par l'IA retiré de Tâches (AiPlan.tsx supprimé) ; onglet admin « Paiements et IA » retiré ; ligne « Crédits » retirée des CGV.
- Vérifié : navigateur 1440 px (Abonnement, Tâches, Administration, CGV).

### 2026-11-25 · #379 · Connecter une IA en 3 étapes
- Fait : ConnectTabs (AiTokens.tsx) : choix de l'IA (Claude, ChatGPT, Gemini, Le Chat en grand, les autres sous « Autres IA »), puis 3 étapes numérotées avec une phrase et un bouton chacune (copier ou installer, ouvrir l'IA ou coller, « Autoriser » qui passe au vert). Notes et clé de secours repliées dans « Besoin d'aide ? ».
- Vérifié : navigateur 1440 px.

### 2026-11-25 · #375 · « Connecter une IA » par onglet
- Fait : AiTokens.tsx : TABS / ConnectTabs, un onglet par IA (Claude, ChatGPT, Gemini, Le Chat, Cursor, VS Code, Windsurf, Codex, Claude Code, Gemini CLI, Autre client MCP), étapes vérifiées dans la doc officielle de chacune le 25/11/2026, adresse ou configuration OAuth à copier, liens profonds Cursor et VS Code, et clé API en secours par onglet. ConnectAI.tsx : les onglets remplacent l'ancien « en un clic » ; le prompt pour agents de code est replié. IA connectées : type, OAuth ou clé, dernier usage, bouton Révoquer. Logos ChatGPT, Le Chat, Gemini (AiLogo).
- Vérifié : navigateur 1440 px, révocation testée avec Gemini CLI (Disconnected, 401).

### 2026-11-25 · #371 · l'Organiseur retiré du site vitrine
- Fait : Landing.tsx : bloc de fonctionnalité et capture de l'Organiseur retirés (FEATURES garde l'index du texte, k), entrée du méga-menu masquée (index 4), ligne « Organiseur d'acteurs » du comparatif masquée (ORGANISEUR_ROW) ; mention retirée de l'accroche dans les 16 langues. Les textes restent dans les dictionnaires.
- Vérifié : navigateur 1440 px, fr et en.

### 2026-11-25 · #372 · Dashboard sans les trois blocs du haut
- Fait : Overview.tsx n'affiche plus la scène du studio (#176), les premiers pas avec l'IA (#146) ni le parcours de questions de cadrage (#135), à la demande de Kl1600. Les composants Parcours et PremiersPas sont retirés de la page ; StudioScene.tsx et les routes serveur (suggestions, questions, outil MCP ask_user) restent.
- Vérifié : navigateur 1440 px sur une map avec données.

### 2026-11-25 · #368 · retour à la version précédente du site vitrine
- Fait : Landing.tsx et landing.css remis comme avant #362 (demande de Kl1600) ; les menus à jour de la capture (fr, en) sont gardés.
- Vérifié : navigateur 1440 px.

### 2026-11-25 · #362 · site vitrine simple et hiérarchisé
- Fait : Landing.tsx : nav sans méga-menu ; hero avec un seul bouton principal (« Voir les fonctionnalités » en lien discret), sans ligne « micro » ; retirés : bandeau de logos, bloc avant/après, chiffres, grandes captures alternées et listes à puces ; fonctions en liste courte (icône, titre, première phrase seulement, fonction first()) avec l'IA en dernière ligne ; tarifs en 2 cartes, comparatif replié (<details>) ; FAQ toute repliée. Capture de l'app : menus actuels (fr, en). Les textes inutilisés restent dans les dictionnaires.
- Vérifié : navigateur 1440 et 390 px.

### 2026-11-25 · #360 · barre latérale à gauche sur téléphone et tablette
- Fait : ≤ 1024 px la barre est repliée en icônes par défaut (dépliable) ; ≤ 760 px elle reste à gauche en rail de 60 px (plus de barre en haut), le bouton « Replier » l'ouvre par-dessus la page avec un fond assombri, fermée en touchant à côté ou en changeant de page (useMq dans Layout.tsx). L'avatar du compte reste visible ≤ 900 px ; barre du haut sans retour à la ligne sur téléphone.
- Vérifié : navigateur 390, 820 et 1440 px.

### 2026-11-24 · #316 · Analyse : salon « Maps à faire »
- Fait : Analyse n'a plus qu'une entrée dans la barre (« Maps à faire », /analyse/a-faire) ; les pages détaillées restent joignables par un lien en bas du salon. Premier passage : 4 sources facultatives (YouTube, TikTok, X, Reddit) puis « Passer » (mémorisé par compte, localStorage) ; dans le salon, une ligne « Sources » toujours visible, chaque source connectable seule à tout moment (vert connecté, rouge erreur avec le message en info-bulle). Liste : rang, type de map, une phrase, concurrence, « Voir » (exemple qui marche). Reprise du début de travail d'une autre session.
- Vérifié : navigateur 1440 et 390 px, connexions X et Reddit contre des serveurs simulés.

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

### 2026-11-24 · #68 · statistiques par jour dans l'administration
- Fait : migration 0043 (`admin_stats_day`, photo quotidienne des comptes actifs et des plans, mise à jour chaque minute par `digest_loop` et à l'ouverture de la vue générale) ; `GET /api/admin/overview?days=30|90` rend `stats` (inscriptions, comptes actifs, demandes par jour ; Starter, Pro, essais et leur évolution). « Actif » = connecté ou a agi (tâche, page, UEFN) ce jour-là. Web : courbes en barres (`Barres`), choix 30 / 90 jours, carte Abonnements.
- Limite : l'évolution des plans commence le jour de la mise en service (la base ne gardait pas l'historique des plans).
- Vérifié : `cargo test` (`admin::stats_tests`) ; Chrome sans tête en clair et en sombre, 90 barres.

### 2026-11-24 · #134 · « Mes IA » sur l'accueil (au lieu de « Connecte ton IA » par map)
- Fait (périmètre changé par l'humain, voir les commentaires de #134) : section « Mes IA » sous les maps (`components/ConnectAI.tsx`, `AiSection`) : une carte par IA du compte, « en attente de connexion » tant que `last_used` est vide, « connectée » au premier appel MCP, × pour retirer une tentative ; « Connecter une IA » : prompt à coller avec clé incluse (l'IA s'ajoute POP), en un clic et clé API repliés. Pastille d'état dans la barre du haut (`AiPill`, mène à `/#ia`). Paramètres → IA connectées réutilise le même bouton (`ConnectAiButton`). Aucune migration.
- Corrigé en passant : `AiLogo` suffixe les id des dégradés (un logo caché rendait les autres copies invisibles).
- Vérifié : `cargo test` (`agent_en_attente_puis_connecte`) ; navigateur en local, appels MCP réels par HTTP ; testé par l'humain avec Claude Code et le prompt.
- Leçon : un mot = une chose ; « IA » = l'outil connecté au compte, « agent » = une IA au travail sur une tâche (rôle, `claim_task`).
### 2026-11-24 · #78 F4 · interface de Forge d'après les maquettes validées
- Fait : `web/src/pages/Forge.tsx` refait (barre latérale Accueil, Agents, Construire, Bibliothèque, Outils ; sélecteur studio / map ; « Ouvrir POP » ; profil avec menu : Réglages, IA connectées, thème, mon compte, déconnexion ; cloche des notifications ; « En ce moment » = agents de la map et leur tâche avec « Voir dans POP ») ; liaison : seulement une map existante + « Créer la map dans POP » (navigateur) ; écrans vides avec leurs issues. Branche rebasée sur `main` (92 commits de retard) et réduite à un seul commit.
- Vérifié : `tsc` + build ; Forge compilé sous Linux (`cargo build`) ; page `/forge` vue dans Chrome avec un relais simulé pour la capture (clair, sombre, menu du profil). Pas vérifié : Windows, vrai UEFN, garde de navigation dans WebView2 (PC de benybens).

### 2026-11-24 · #78 · retours de Kl1600 sur Forge (test Windows)
- Fait : la fenêtre de Forge n'a plus le cadre ni le menu de Windows (`decorations(false)`) ; barre de titre dessinée par la page (`TitleBar` dans Forge.tsx, `.barre` dans forge/ui/index.html) : glisser = déplacer, double-clic = agrandir, réduire / agrandir / fermer par la commande `fenetre` ; l'ancien menu (état du relais, changer l'adresse, recharger, F5) est dans le menu du profil par `action_forge`. Sélecteur de map sur le modèle de `MapSwitcher` du site (classes `mapsw-*`), sans gestion des maps (renvoi vers POP). Un compte sans map n'a plus l'écran de bienvenue du site (« Télécharger UEFN Link ») dans Forge.
- Vérifié : Chrome sans tête avec un pont Tauri simulé (appels `fenetre` et `action_forge`, choix de map), `cargo test` de forge sous Linux ; reste à voir dans la vraie fenêtre Windows.

```
FAIT | #445 Forge · Accueil : trois gros boutons d'action selon le contexte | 28/11
Owner : benybens | Sur la tâche : Claude Code, pour benybens
Fait : contexteForge (relais, fermé, concept, dev, prête), 3 cartes en haut de l'Accueil, « Ouvrir UEFN » dans le bandeau UEFN fermé ; testé dans une Forge isolée avec le vrai UEFN (TestPopHub)
Reste : rien ; #528 (Auto Start Server non détecté) ouverte
Bloque : rien
```

```
FAIT | #531 CI rouge sur main : web/dist construit depuis des sources en CRLF (régression de #445) | 28/11
Owner : benybens | Sur la tâche : Claude Code, pour benybens
Fait : web/dist reconstruit depuis une copie en LF ; mêmes noms d'assets que le CI
Reste : rien
Piège : sous Windows (core.autocrlf=true), un fichier réécrit en CRLF change les empreintes de Vite : construire web/dist
        depuis une copie en LF (git -c core.autocrlf=false worktree add …), sinon le CI de main passe au rouge.
```
