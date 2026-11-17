-- Rejeu des codes de double authentification (#15) : dernier pas de temps TOTP accepté par compte.
-- Un code dont le pas est inférieur ou égal est refusé (« déjà utilisé »).
ALTER TABLE users ADD COLUMN totp_last_step INTEGER NOT NULL DEFAULT 0;
