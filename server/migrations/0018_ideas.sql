-- Idées de maps : profil du créateur (pour des idées faisables), fil d'idées, historique des niches.
CREATE TABLE idea_profiles (
  user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  team_size INTEGER NOT NULL DEFAULT 1,
  hours_week INTEGER NOT NULL DEFAULT 10,
  weeks INTEGER NOT NULL DEFAULT 6,
  level TEXT NOT NULL DEFAULT 'confirme',
  skills TEXT NOT NULL DEFAULT '[]',
  genres TEXT NOT NULL DEFAULT '[]',
  avoid TEXT NOT NULL DEFAULT '[]',
  notify INTEGER NOT NULL DEFAULT 1,
  filled INTEGER NOT NULL DEFAULT 0,
  generated_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE ideas (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  niche TEXT NOT NULL,
  score REAL NOT NULL DEFAULT 0,
  fit REAL NOT NULL DEFAULT 0,
  rank REAL NOT NULL DEFAULT 0,
  data TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL DEFAULT 'new',   -- new | seen | saved | dismissed | started
  active INTEGER NOT NULL DEFAULT 1,    -- 0 = n'est plus dans les meilleures opportunités
  map_id INTEGER REFERENCES maps(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (user_id, niche)
);
CREATE INDEX idx_ideas_user ON ideas(user_id, active, rank);

-- Joueurs par niche et par jour, gardés au-delà des 7 jours de l'API d'Epic.
CREATE TABLE fn_niches (
  day TEXT NOT NULL,
  niche TEXT NOT NULL,
  players INTEGER NOT NULL,
  islands INTEGER NOT NULL,
  PRIMARY KEY (day, niche)
);
