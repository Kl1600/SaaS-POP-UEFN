-- Configurations d'appareils réutilisables (#89) : la classe d'un appareil et ses réglages (options classiques,
-- réglages @editable d'un appareil Verse), gardés dans la bibliothèque du studio pour les appliquer ou les reposer.
CREATE TABLE device_configs (
    id INTEGER PRIMARY KEY,
    studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    class TEXT NOT NULL DEFAULT '',
    verse_class TEXT NOT NULL DEFAULT '',
    asset TEXT NOT NULL DEFAULT '',
    options TEXT NOT NULL DEFAULT '{}',
    props TEXT NOT NULL DEFAULT '{}',
    author_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (studio_id, name)
);
