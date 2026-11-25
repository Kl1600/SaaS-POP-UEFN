-- Bibliothèque d'assets (#91) : les fichiers sont dans un stockage objet compatible S3 (Cloudflare R2), jamais sur le
-- disque du serveur ; ici seulement la fiche. studio_id NULL = asset de l'équipe POP, visible par tous.
CREATE TABLE assets (
  id INTEGER PRIMARY KEY,
  studio_id INTEGER REFERENCES studios(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,                       -- extension : fbx, png, wav…
  size INTEGER NOT NULL,
  object_key TEXT NOT NULL UNIQUE,
  origin TEXT NOT NULL,                     -- pop | studio
  rights TEXT NOT NULL,                     -- déclaration de droits faite à l'envoi
  status TEXT NOT NULL DEFAULT 'pending',   -- pending (envoi en cours) | ready
  author_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX assets_studio ON assets (studio_id, status);
