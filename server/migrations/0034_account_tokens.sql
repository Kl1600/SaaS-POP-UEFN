-- Jetons MCP au niveau du compte (#145) : `map_id` devient la map de travail du jeton, facultative et choisie par
-- l'outil `use_map`. Un ancien jeton de map garde sa map comme map de travail. Supprimer la map ne supprime plus
-- le jeton : il perd seulement sa map de travail.
-- SQLite ne sait pas rendre une colonne facultative : on reconstruit la table. sqlx exécute la migration dans une
-- transaction, où les clés étrangères ne se coupent pas : supprimer l'ancienne table vide donc
-- `uefn_actions.agent_id` (ON DELETE SET NULL). On le garde de côté et on le remet après.
CREATE TEMP TABLE keep_agents AS SELECT id, agent_id FROM uefn_actions WHERE agent_id IS NOT NULL;
CREATE TABLE api_tokens_new (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  map_id INTEGER REFERENCES maps(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  kind TEXT NOT NULL DEFAULT 'autre',
  last_used TEXT,
  calls INTEGER NOT NULL DEFAULT 0,
  scope TEXT NOT NULL DEFAULT 'write',
  task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
  task_since TEXT
);
INSERT INTO api_tokens_new (id, user_id, map_id, name, token_hash, created_at, kind, last_used, calls, scope, task_id, task_since)
  SELECT id, user_id, map_id, name, token_hash, created_at, kind, last_used, calls, scope, task_id, task_since FROM api_tokens;
DROP TABLE api_tokens;
ALTER TABLE api_tokens_new RENAME TO api_tokens;
CREATE UNIQUE INDEX api_tokens_task ON api_tokens (task_id) WHERE task_id IS NOT NULL;
CREATE INDEX idx_api_tokens_user ON api_tokens (user_id);
UPDATE uefn_actions SET agent_id = (SELECT k.agent_id FROM keep_agents k WHERE k.id = uefn_actions.id)
  WHERE id IN (SELECT id FROM keep_agents);
DROP TABLE keep_agents;
