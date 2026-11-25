-- Catalogue des appareils créatifs (#119) : lu dans l'Asset Registry d'UEFN par le relais, gardé par version
-- d'UEFN (le même pour tous les studios), cherché par nom pour poser un appareil.
CREATE TABLE device_catalog (
    version TEXT NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    folder TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (version, path)
);
CREATE TABLE device_catalog_versions (
    version TEXT PRIMARY KEY,
    fetched_at TEXT NOT NULL DEFAULT (datetime('now'))
);
