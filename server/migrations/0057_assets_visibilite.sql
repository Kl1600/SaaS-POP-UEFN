-- Bibliothèque personnelle d'assets (#382) : chaque asset appartient à son auteur ; privé (par défaut pour les
-- nouveaux), studio ou public. Les assets déjà envoyés restent visibles par leur studio.
ALTER TABLE assets ADD COLUMN visibility TEXT NOT NULL DEFAULT 'studio';
CREATE INDEX assets_auteur ON assets (author_id, status);
-- Lien d'import à usage limité (15 min) pour que le relais télécharge un asset sans clé (#382).
CREATE TABLE asset_import_links (
    token TEXT PRIMARY KEY,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL
);
