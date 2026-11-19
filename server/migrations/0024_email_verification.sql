-- Vérification des adresses e-mail (#11) : le rôle d'administrateur exige une adresse vérifiée.
-- Les comptes créés avant cette migration sont considérés comme vérifiés (sinon ils perdraient leurs droits).
ALTER TABLE users ADD COLUMN email_verified_at TEXT;
UPDATE users SET email_verified_at = created_at;

CREATE TABLE email_verifications (
  token_hash TEXT PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  expires_at TEXT NOT NULL,
  used_at TEXT
);
CREATE INDEX idx_email_verifications_user ON email_verifications(user_id);
