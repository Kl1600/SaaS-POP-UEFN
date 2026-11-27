-- Fils de discussion séparés (#473) : NULL = terminal de l'agent ; « miniature » = outil Miniature de Forge.
ALTER TABLE agent_messages ADD COLUMN canal TEXT;
UPDATE agent_messages SET canal = 'miniature' WHERE body LIKE '%(Outil Miniature : tâche #%';
