-- Fil d'activité des agents (#298) : chaque appel d'outil MCP d'un agent sur sa map de travail (sans les arguments,
-- qui peuvent contenir du code ou du texte privé). Sert au fil en direct de Forge et à l'étape de la tâche.
CREATE TABLE agent_calls (
    id INTEGER PRIMARY KEY,
    agent_id INTEGER NOT NULL REFERENCES api_tokens(id) ON DELETE CASCADE,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
    tool TEXT NOT NULL,
    ok INTEGER NOT NULL,
    at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX agent_calls_map ON agent_calls(map_id, id);
CREATE INDEX agent_calls_agent ON agent_calls(agent_id, id);
