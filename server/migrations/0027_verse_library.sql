-- Bibliothèque Verse du studio (#85) : modules partagés par l'équipe, que les humains et les IA (MCP) consultent
-- avant d'écrire du neuf. Une modification augmente la version ; les appareils requis sont une liste JSON.
CREATE TABLE verse_modules (
  id INTEGER PRIMARY KEY,
  studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'autre',
  description TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL,
  devices TEXT NOT NULL DEFAULT '[]',
  version INTEGER NOT NULL DEFAULT 1,
  author_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (studio_id, name)
);
CREATE INDEX verse_modules_studio ON verse_modules (studio_id, category);
