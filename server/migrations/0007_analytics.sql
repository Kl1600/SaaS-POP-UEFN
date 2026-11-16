-- Données publiques de l'API Fortnite (Data API d'Epic), collectées par POP.
CREATE TABLE fn_islands (
  code TEXT PRIMARY KEY,
  title TEXT NOT NULL DEFAULT '',
  creator TEXT NOT NULL DEFAULT '',
  category TEXT NOT NULL DEFAULT '',
  created_in TEXT NOT NULL DEFAULT '',
  tags TEXT NOT NULL DEFAULT '[]',
  first_seen TEXT NOT NULL DEFAULT (datetime('now')),
  seen_new INTEGER NOT NULL DEFAULT 0,      -- 1 = vue dans les dernières sorties
  metrics_at TEXT
);
CREATE TABLE fn_metrics (
  code TEXT NOT NULL REFERENCES fn_islands(code) ON DELETE CASCADE,
  day TEXT NOT NULL,
  unique_players INTEGER,
  plays INTEGER,
  peak_ccu INTEGER,
  minutes INTEGER,
  avg_minutes REAL,
  favorites INTEGER,
  recommends INTEGER,
  d1 REAL,
  d7 REAL,
  PRIMARY KEY (code, day)
);
CREATE INDEX idx_fn_metrics_day ON fn_metrics(day);
CREATE TABLE fn_watch (
  code TEXT PRIMARY KEY,
  added_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
  added_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE fn_status (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  last_run TEXT,
  last_ok TEXT,
  last_error TEXT,
  islands INTEGER NOT NULL DEFAULT 0
);
INSERT INTO fn_status (id) VALUES (1);
-- Vidéos TikTok collectées (via Apify, avec le jeton du studio).
CREATE TABLE tt_videos (
  id TEXT NOT NULL,
  studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
  query TEXT NOT NULL,
  text TEXT NOT NULL DEFAULT '',
  author TEXT NOT NULL DEFAULT '',
  url TEXT NOT NULL DEFAULT '',
  music TEXT NOT NULL DEFAULT '',
  hashtags TEXT NOT NULL DEFAULT '[]',
  created TEXT,
  plays INTEGER NOT NULL DEFAULT 0,
  likes INTEGER NOT NULL DEFAULT 0,
  shares INTEGER NOT NULL DEFAULT 0,
  comments INTEGER NOT NULL DEFAULT 0,
  fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (id, studio_id)
);
CREATE INDEX idx_tt_studio ON tt_videos(studio_id, created);
ALTER TABLE maps ADD COLUMN island_code TEXT;
