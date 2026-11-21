# Domaine web

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

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
