-- Photos de l'île (#306) : captures 1920 × 1080 prises dans UEFN par le relais, rangées dans le stockage objet.
CREATE TABLE island_captures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    s3_key TEXT NOT NULL,
    location TEXT NOT NULL,
    rotation TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    bytes INTEGER NOT NULL DEFAULT 0,
    created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX island_captures_map ON island_captures (map_id, id);
