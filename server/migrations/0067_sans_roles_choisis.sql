-- Plus de rôle choisi par IA (#490) : l'Orchestrateur est Claude quand un Claude est connecté, sinon l'autre IA.
-- Les rôles choisis à la main sont retirés ; le Relecteur, posé par POP lui-même, reste.
UPDATE api_tokens SET role = NULL WHERE role IS NOT NULL AND role <> 'relecteur';
