-- Résumés en français des actions de l'IA (#321) : un détail court et sans contenu privé par appel (nom du fichier
-- Verse écrit, nombre d'erreurs de compilation, statut donné à une tâche). Jamais le code ni le texte envoyé.
ALTER TABLE agent_calls ADD COLUMN detail TEXT;
