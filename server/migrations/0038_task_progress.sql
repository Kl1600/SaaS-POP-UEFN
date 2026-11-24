-- Notes de reprise d'une tâche (#114) : ce qu'un agent a fait, ce qui reste, les fichiers touchés. La plus récente est
-- donnée à l'agent qui reprend la tâche. auto = 1 : note posée par POP quand une réservation silencieuse a été libérée.
CREATE TABLE task_progress (
    id INTEGER PRIMARY KEY,
    task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id INTEGER REFERENCES api_tokens(id) ON DELETE SET NULL,
    agent_name TEXT NOT NULL DEFAULT '',
    done TEXT NOT NULL DEFAULT '',
    todo TEXT NOT NULL DEFAULT '',
    files TEXT NOT NULL DEFAULT '[]',
    auto INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX task_progress_task ON task_progress(task_id, id);
