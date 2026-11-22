# Domaine web

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

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
