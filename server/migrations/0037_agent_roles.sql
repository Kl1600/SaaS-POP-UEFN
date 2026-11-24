-- Rôle d'un agent IA (#111) : consignes et outils permis (server/src/roles.rs). NULL = sans rôle, tous les outils.
ALTER TABLE api_tokens ADD COLUMN role TEXT;
