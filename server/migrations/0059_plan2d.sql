-- Plan 2D de la map, étage par étage, à l'échelle (#162). Coordonnées et tailles en centimètres, comme le panneau
-- Détails d'UEFN (X vers l'avant, Y vers la droite) : le plan et le niveau réel se comparent sans conversion.
CREATE TABLE plan_floors (
    id INTEGER PRIMARY KEY,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    z_min REAL NOT NULL DEFAULT 0,
    z_max REAL NOT NULL DEFAULT 400,
    position INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX plan_floors_map ON plan_floors(map_id, position);
CREATE TABLE plan_objects (
    id INTEGER PRIMARY KEY,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    floor_id INTEGER NOT NULL REFERENCES plan_floors(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    device TEXT NOT NULL DEFAULT '',
    x REAL NOT NULL DEFAULT 0,
    y REAL NOT NULL DEFAULT 0,
    w REAL NOT NULL DEFAULT 0,
    h REAL NOT NULL DEFAULT 0,
    rotation REAL NOT NULL DEFAULT 0,
    points TEXT NOT NULL DEFAULT '[]',
    updated_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX plan_objects_map ON plan_objects(map_id, floor_id);
