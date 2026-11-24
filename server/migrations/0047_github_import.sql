-- Import complet d'un dépôt GitHub dans une map (#128) : jalon de chaque tâche, et identifiants GitHub des jalons
-- et des commentaires pour qu'un nouvel import ne crée pas de doublon.
ALTER TABLE tasks ADD COLUMN milestone_id INTEGER REFERENCES milestones(id) ON DELETE SET NULL;
ALTER TABLE milestones ADD COLUMN github_milestone INTEGER;
ALTER TABLE comments ADD COLUMN github_comment INTEGER;
CREATE INDEX comments_github ON comments(map_id, github_comment);
