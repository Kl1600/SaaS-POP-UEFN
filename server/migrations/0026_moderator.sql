-- Rôle modérateur (#65) : liste d'attente, dépannage des comptes (déblocage, lien de mot de passe) et consultation
-- des comptes. Donné ou retiré par un administrateur ; comme l'administrateur, il exige une adresse vérifiée.
ALTER TABLE users ADD COLUMN is_moderator INTEGER NOT NULL DEFAULT 0;
