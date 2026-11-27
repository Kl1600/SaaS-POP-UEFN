-- Agents ouverts dans Forge (#487) : leur clé est marquée pour être rangée sous l'IA de l'utilisateur, pas comptée
-- comme une IA connectée de plus. Les clés déjà créées par les terminaux en direct de Forge (#466 : nom « Claude »,
-- type claude_code, sans OAuth ; une connexion manuelle de Claude Code s'appelle « Claude Code ») aussi.
ALTER TABLE api_tokens ADD COLUMN forge INTEGER NOT NULL DEFAULT 0;
UPDATE api_tokens SET forge = 1 WHERE name = 'Claude' AND kind = 'claude_code' AND oauth_client IS NULL;
