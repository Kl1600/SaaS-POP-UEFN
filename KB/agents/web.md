# Domaine web

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

### 2026-11-21 · #67 · annonce pour tous les comptes
- Fait : migration 0031 (annonces + fermetures), `server/src/announce.rs` (publier, retirer, fermer ; une seule active ; date de fin), annonce dans `/api/me` ; bandeau dans `Layout.tsx` et onglet Administration → Annonce.
- Vérifié : `cargo test` (publier, fermer, nouvelle annonce, retirer, échue, non-admin refusé) ; bandeau vu en clair, sombre et mobile (captures).
- Piège : les styles sont dans `@layer app` ; un `!important` hors couche perd contre celui d'une couche.
