-- Fenêtre « Connecte ton IA » (#134) : s'ouvre tant qu'aucune IA du compte n'a appelé le MCP. ai_intro_skipped = 1 :
-- la personne a choisi « Passer » ; la pastille d'état de l'IA (barre du haut) reste pour connecter plus tard.
ALTER TABLE users ADD COLUMN ai_intro_skipped INTEGER NOT NULL DEFAULT 0;
