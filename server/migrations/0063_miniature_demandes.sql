-- « Générer une miniature » dans Forge (#308) : la demande devient une tâche POP pour l'IA du client ; la miniature
-- rendue (submit_thumbnail) est proposée dans Forge, puis choisie (« Utiliser »).
ALTER TABLE map_thumbnails ADD COLUMN task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL;
ALTER TABLE map_thumbnails ADD COLUMN submitted_at TEXT;
ALTER TABLE map_thumbnails ADD COLUMN used_at TEXT;
CREATE TABLE thumbnail_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
    idea TEXT NOT NULL DEFAULT '',
    base_id INTEGER REFERENCES map_thumbnails(id) ON DELETE SET NULL,
    created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX thumbnail_requests_map ON thumbnail_requests (map_id, id);
