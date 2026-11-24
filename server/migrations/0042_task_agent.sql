-- Orchestrateur (#112) : agent IA à qui l'Orchestrateur a attribué une tâche. Seul cet agent peut la réserver
-- (claim_task) tant qu'elle lui est attribuée. NULL = libre pour tous les agents.
ALTER TABLE tasks ADD COLUMN agent_id INTEGER REFERENCES api_tokens(id) ON DELETE SET NULL;
CREATE INDEX idx_tasks_agent ON tasks (agent_id) WHERE agent_id IS NOT NULL;
