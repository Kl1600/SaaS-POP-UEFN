-- Tâches UEFN (#127) : une tâche marquée « UEFN » ne passe à « Faite » qu'avec le message de son check-in Lore,
-- qui commence par #N. Le message est gardé dans l'historique de la tâche (task_events).
ALTER TABLE tasks ADD COLUMN uefn INTEGER NOT NULL DEFAULT 0;
