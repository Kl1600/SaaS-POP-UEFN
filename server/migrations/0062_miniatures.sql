-- Miniatures (#307) : bibliothèque interne de POP (inspirations à regarder, ingrédients utilisables avec leur licence)
-- et miniatures composées pour une map. Fichiers dans le stockage objet.
CREATE TABLE thumbnail_kit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    drawer TEXT NOT NULL CHECK (drawer IN ('inspiration', 'ingredient')),
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    keywords TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT '',
    license TEXT NOT NULL DEFAULT '',
    s3_key TEXT NOT NULL,
    bytes INTEGER NOT NULL DEFAULT 0,
    phash INTEGER,
    created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (drawer = 'inspiration' OR length(trim(license)) >= 3)
);
CREATE TABLE map_thumbnails (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    s3_key TEXT NOT NULL,
    logo_key TEXT NOT NULL,
    bytes INTEGER NOT NULL DEFAULT 0,
    created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX map_thumbnails_map ON map_thumbnails (map_id, id);
