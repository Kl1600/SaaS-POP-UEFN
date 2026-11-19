-- POP : schéma initial (SQLite)
PRAGMA foreign_keys = ON;

CREATE TABLE users (
  id INTEGER PRIMARY KEY,
  email TEXT NOT NULL UNIQUE COLLATE NOCASE,
  name TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  credits INTEGER NOT NULL DEFAULT 50,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE sessions (
  token_hash TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  expires_at TEXT NOT NULL
);

CREATE TABLE studios (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE memberships (
  studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('owner','admin','member','freelance','viewer')),
  specialty TEXT NOT NULL DEFAULT '',
  portfolio TEXT NOT NULL DEFAULT '',
  hours_per_week INTEGER NOT NULL DEFAULT 10,
  PRIMARY KEY (studio_id, user_id)
);

CREATE TABLE invitations (
  id INTEGER PRIMARY KEY,
  studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
  email TEXT NOT NULL,
  role TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  accepted_at TEXT
);

CREATE TABLE maps (
  id INTEGER PRIMARY KEY,
  studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  genre TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'production',
  release_date TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE tasks (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  number INTEGER NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  tag TEXT NOT NULL DEFAULT 'code' CHECK (tag IN ('code','carte','conception','test')),
  status TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo','doing','blocked','done')),
  assignee_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  minutes INTEGER NOT NULL DEFAULT 60,
  planned_day TEXT,
  lot TEXT NOT NULL DEFAULT '',
  ai INTEGER NOT NULL DEFAULT 0,
  origin TEXT NOT NULL DEFAULT '',
  done_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (map_id, number)
);

CREATE TABLE task_deps (
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  depends_on INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, depends_on)
);

CREATE TABLE subtasks (
  id INTEGER PRIMARY KEY,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  done INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE task_events (
  id INTEGER PRIMARY KEY,
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  what TEXT NOT NULL,
  via TEXT NOT NULL DEFAULT 'Bouton',
  at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE comments (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  target TEXT NOT NULL,           -- 'task:12', 'page:3:bloc-id'
  user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  body TEXT NOT NULL,
  resolved INTEGER NOT NULL DEFAULT 0,
  at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE pages (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  parent_id INTEGER REFERENCES pages(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  icon TEXT NOT NULL DEFAULT '',
  blocks TEXT NOT NULL DEFAULT '[]',
  share TEXT NOT NULL DEFAULT 'team' CHECK (share IN ('private','team','link')),
  favorite INTEGER NOT NULL DEFAULT 0,
  created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
  share_token TEXT UNIQUE,
  position INTEGER NOT NULL DEFAULT 0,
  version INTEGER NOT NULL DEFAULT 1,
  updated_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE page_versions (
  id INTEGER PRIMARY KEY,
  page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
  version INTEGER NOT NULL,
  title TEXT NOT NULL,
  blocks TEXT NOT NULL,
  user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  sha256 TEXT NOT NULL,
  at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE boards (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  objects TEXT NOT NULL DEFAULT '{}',
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE milestones (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  date TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'jalon'
);

CREATE TABLE activity (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  detail TEXT NOT NULL DEFAULT '',
  at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE revenues (
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  month TEXT NOT NULL,            -- 'YYYY-MM'
  engagement_cents INTEGER NOT NULL DEFAULT 0,
  sales_cents INTEGER NOT NULL DEFAULT 0,
  source TEXT NOT NULL DEFAULT 'manuel',
  PRIMARY KEY (map_id, month)
);

CREATE TABLE share_rules (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  user_id INTEGER REFERENCES users(id) ON DELETE CASCADE, -- NULL = réserve du studio
  kind TEXT NOT NULL CHECK (kind IN ('percent','fixed','per_task')),
  value INTEGER NOT NULL           -- percent: points de base (5000 = 50 %), fixed/per_task: centimes
);

CREATE TABLE offers (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  price_vbucks INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'live'
);

CREATE TABLE offer_sales (
  offer_id INTEGER NOT NULL REFERENCES offers(id) ON DELETE CASCADE,
  month TEXT NOT NULL,
  sales INTEGER NOT NULL DEFAULT 0,
  revenue_cents INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (offer_id, month)
);

CREATE TABLE publish_checks (
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('ok','warn','block','todo')),
  note TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (map_id, key)
);

CREATE TABLE launch_steps (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  time TEXT NOT NULL,
  title TEXT NOT NULL,
  owner TEXT NOT NULL DEFAULT '',
  done INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE playtest_listings (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  owner_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  minutes INTEGER NOT NULL DEFAULT 15,
  reward INTEGER NOT NULL DEFAULT 20,
  slots INTEGER NOT NULL DEFAULT 5,
  island_code TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE playtest_feedback (
  id INTEGER PRIMARY KEY,
  listing_id INTEGER NOT NULL REFERENCES playtest_listings(id) ON DELETE CASCADE,
  tester_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  understood TEXT NOT NULL DEFAULT '',
  left_minute INTEGER NOT NULL,
  reason TEXT NOT NULL DEFAULT '',
  comeback TEXT NOT NULL DEFAULT '',
  score INTEGER NOT NULL DEFAULT 3,
  at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (listing_id, tester_id)
);

CREATE TABLE integrations (
  studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 0,
  config TEXT NOT NULL DEFAULT '{}',
  PRIMARY KEY (studio_id, kind)
);

CREATE INDEX idx_tasks_map ON tasks(map_id, status);
CREATE INDEX idx_activity_map ON activity(map_id, at);
CREATE INDEX idx_comments_target ON comments(map_id, target);

CREATE TABLE file_locks (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  task_number INTEGER,
  since TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (map_id, path)
);

-- essai #46
