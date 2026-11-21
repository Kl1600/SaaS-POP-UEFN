# Domaine web

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

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
