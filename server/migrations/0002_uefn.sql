-- UEFN Link : relais local, Éditeur Verse, Organiseur

CREATE TABLE relay_devices (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL DEFAULT 'UEFN Link',
  token_hash TEXT NOT NULL UNIQUE,
  pair_code_hash TEXT,
  pair_expires TEXT,
  last_seen TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Dernier état connu (lecture seule quand UEFN est fermé).
CREATE TABLE uefn_state (
  map_id INTEGER PRIMARY KEY REFERENCES maps(id) ON DELETE CASCADE,
  project TEXT NOT NULL DEFAULT '',
  actors TEXT NOT NULL DEFAULT '[]',
  symbols TEXT NOT NULL DEFAULT '[]',
  tags TEXT NOT NULL DEFAULT '[]',
  locks TEXT NOT NULL DEFAULT '[]',
  topview TEXT,
  topview_bounds TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE verse_files (
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  content TEXT NOT NULL,
  sha TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (map_id, path)
);

CREATE TABLE verse_versions (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  content TEXT NOT NULL,
  user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  task_number INTEGER,
  note TEXT NOT NULL DEFAULT '',
  at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_verse_versions ON verse_versions(map_id, path, id);

-- Journal de toutes les actions envoyées à UEFN (annulables).
CREATE TABLE uefn_actions (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  source TEXT NOT NULL DEFAULT 'web',      -- web | mcp | undo
  action TEXT NOT NULL,
  args TEXT NOT NULL,
  summary TEXT NOT NULL DEFAULT '',
  before TEXT,
  after TEXT,
  undo TEXT,
  task_number INTEGER,
  status TEXT NOT NULL DEFAULT 'queued',   -- queued | sent | done | failed | undone
  error TEXT,
  at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_uefn_actions ON uefn_actions(map_id, id);

-- Actions demandées par une IA qui attendent la validation d'un humain (suppressions).
CREATE TABLE approvals (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  action TEXT NOT NULL,
  args TEXT NOT NULL,
  summary TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',  -- pending | approved | refused
  decided_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
  at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE saved_filters (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  query TEXT NOT NULL
);

-- Jetons personnels pour l'IA connectée du créateur (serveur MCP de POP).
CREATE TABLE api_tokens (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
