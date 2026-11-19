-- Espace perso : chaque compte a ses maps « seul », hors de tout studio d'équipe.
ALTER TABLE studios ADD COLUMN personal INTEGER NOT NULL DEFAULT 0;
