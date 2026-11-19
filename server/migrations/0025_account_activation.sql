-- Comptes créés par l'administrateur, activés par la personne avec un lien reçu par e-mail (#42) :
-- elle choisit son mot de passe, l'admin ne le connaît jamais.
ALTER TABLE users ADD COLUMN activation_pending INTEGER NOT NULL DEFAULT 0;

CREATE TABLE account_activations (
  token_hash TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  waitlist_id INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  expires_at TEXT NOT NULL,
  used_at TEXT
);
CREATE INDEX idx_account_activations_user ON account_activations(user_id);
