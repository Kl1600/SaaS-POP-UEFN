CREATE TABLE password_resets (
  token_hash TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  expires_at TEXT NOT NULL,
  used_at TEXT
);

CREATE TABLE notifications (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  map_id INTEGER REFERENCES maps(id) ON DELETE CASCADE,
  text TEXT NOT NULL,
  link TEXT NOT NULL DEFAULT '',
  read_at TEXT,
  at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_notif_user ON notifications(user_id, read_at);
