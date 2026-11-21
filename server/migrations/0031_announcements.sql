-- Annonce affichée en bandeau à tous les comptes connectés (#67). Une seule active : la plus récente non retirée
-- et non échue. Chaque compte peut fermer une annonce ; une nouvelle annonce réapparaît pour tous.
CREATE TABLE announcements (
    id INTEGER PRIMARY KEY,
    text TEXT NOT NULL,
    level TEXT NOT NULL DEFAULT 'info',
    ends_at TEXT,
    created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    removed_at TEXT
);
CREATE TABLE announcement_dismissals (
    announcement_id INTEGER NOT NULL REFERENCES announcements(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (announcement_id, user_id)
);
