-- Informations de l'éditeur (mentions légales, confidentialité, CGU, CGV), remplies par l'administrateur.
CREATE TABLE legal_info (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  data TEXT NOT NULL DEFAULT '{}',
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO legal_info (id) VALUES (1);
ALTER TABLE users ADD COLUMN terms_at TEXT;
