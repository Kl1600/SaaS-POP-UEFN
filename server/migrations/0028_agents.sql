-- Registre des agents IA (#93) : chaque jeton MCP est un agent. Il réserve une tâche POP à la fois (une tâche n'a
-- qu'un agent), et ses actions UEFN sont rattachées à lui dans le journal.
ALTER TABLE api_tokens ADD COLUMN task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL;
ALTER TABLE api_tokens ADD COLUMN task_since TEXT;
CREATE UNIQUE INDEX api_tokens_task ON api_tokens (task_id) WHERE task_id IS NOT NULL;
ALTER TABLE uefn_actions ADD COLUMN agent_id INTEGER REFERENCES api_tokens(id) ON DELETE SET NULL;
CREATE INDEX idx_uefn_actions_agent ON uefn_actions (agent_id, id);
