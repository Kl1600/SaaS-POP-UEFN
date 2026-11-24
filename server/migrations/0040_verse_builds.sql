-- Versions du Verse qui compilent (#301) : à chaque compilation sans erreur, POP garde tous les fichiers Verse de la
-- map (les 50 dernières par map), pour revenir à la dernière version qui compilait.
CREATE TABLE verse_builds (
    id INTEGER PRIMARY KEY,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    agent_id INTEGER REFERENCES api_tokens(id) ON DELETE SET NULL,
    files TEXT NOT NULL,
    at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX verse_builds_map ON verse_builds(map_id, id);
