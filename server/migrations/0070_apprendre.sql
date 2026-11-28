-- Bibliothèque qui apprend (#120) : les fichiers Verse écrits pendant les tâches finies sont proposés à la
-- bibliothèque du studio ; les leçons proposées par les IA attendent la validation d'un membre.
ALTER TABLE verse_modules ADD COLUMN source_map_id INTEGER REFERENCES maps(id) ON DELETE SET NULL;
ALTER TABLE verse_modules ADD COLUMN source_path TEXT;
-- Fichiers que l'équipe ne veut pas garder : plus proposés.
CREATE TABLE bibliotheque_ignores (
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    PRIMARY KEY (map_id, path)
);
CREATE TABLE lecons (
    id INTEGER PRIMARY KEY,
    studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
    map_id INTEGER REFERENCES maps(id) ON DELETE SET NULL,
    agent_id INTEGER REFERENCES api_tokens(id) ON DELETE SET NULL,
    titre TEXT NOT NULL,
    contenu TEXT NOT NULL,
    statut TEXT NOT NULL DEFAULT 'propose' CHECK (statut IN ('propose', 'valide', 'refuse')),
    decide_par INTEGER REFERENCES users(id) ON DELETE SET NULL,
    competence TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX lecons_studio ON lecons(studio_id, statut);
