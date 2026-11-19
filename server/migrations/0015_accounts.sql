-- Bêta : comptes créés par l'administrateur (identifiant + mot de passe), double authentification retirée.
ALTER TABLE users ADD COLUMN username TEXT COLLATE NOCASE;
ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0;
CREATE UNIQUE INDEX IF NOT EXISTS users_username ON users(username) WHERE username IS NOT NULL;
UPDATE users SET totp_enabled = 0, totp_secret = NULL, recovery = '[]';
UPDATE sessions SET sudo_until = NULL;
