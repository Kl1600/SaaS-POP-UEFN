-- Administration de la plateforme et liste d'attente de la bêta.
ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN disabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN last_seen TEXT;
CREATE TABLE waitlist (
  id INTEGER PRIMARY KEY,
  email TEXT NOT NULL UNIQUE COLLATE NOCASE,
  name TEXT NOT NULL,
  studio TEXT NOT NULL DEFAULT '',
  team TEXT NOT NULL DEFAULT '',
  experience TEXT NOT NULL DEFAULT '',
  links TEXT NOT NULL DEFAULT '',
  message TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'refused', 'registered')),
  token_hash TEXT,
  note TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  decided_at TEXT,
  decided_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
  registered_user INTEGER REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX idx_waitlist_status ON waitlist(status, created_at);
-- Le premier compte existant devient administrateur.
UPDATE users SET is_admin = 1 WHERE id = (SELECT MIN(id) FROM users);
