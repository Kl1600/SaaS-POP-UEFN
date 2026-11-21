-- Compétences du studio (#113) : consignes écrites par l'équipe, lues par les IA connectées (MCP get_skill), en plus
-- des compétences fournies par POP (fichiers server/skills/).
CREATE TABLE skills (
  id INTEGER PRIMARY KEY,
  studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  author_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (studio_id, name)
);
