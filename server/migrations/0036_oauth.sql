-- Connexion des IA par OAuth (#285), selon la spécification d'autorisation de MCP.
-- Un jeton d'accès OAuth est une ligne de `api_tokens` (même liste, même lecture seule, même révocation) qui expire
-- (`expires_at`) et qui vient d'un client enregistré (`oauth_client`). Les jetons classiques gardent `expires_at` NULL.
ALTER TABLE api_tokens ADD COLUMN expires_at TEXT;
ALTER TABLE api_tokens ADD COLUMN oauth_client TEXT;

-- Clients enregistrés dynamiquement (RFC 7591) : clients publics, sans secret, PKCE obligatoire.
CREATE TABLE oauth_clients (
  client_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  redirect_uris TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Codes d'autorisation : à usage unique, 10 minutes.
CREATE TABLE oauth_codes (
  code_hash TEXT PRIMARY KEY,
  client_id TEXT NOT NULL REFERENCES oauth_clients(client_id) ON DELETE CASCADE,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  redirect_uri TEXT NOT NULL,
  code_challenge TEXT NOT NULL,
  scope TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used INTEGER NOT NULL DEFAULT 0
);

-- Jetons de rafraîchissement : un par jeton d'accès, remplacé à chaque usage (rotation). Réutiliser un ancien jeton
-- de rafraîchissement révoque la connexion.
CREATE TABLE oauth_refresh (
  token_hash TEXT PRIMARY KEY,
  api_token_id INTEGER NOT NULL REFERENCES api_tokens(id) ON DELETE CASCADE,
  client_id TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT
);
CREATE INDEX idx_oauth_refresh_token ON oauth_refresh (api_token_id);
