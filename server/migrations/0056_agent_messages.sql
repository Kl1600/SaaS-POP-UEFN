-- Parler à ses IA depuis Forge (#381) : un fil par agent. Un message « à toute l'équipe » est copié dans le fil de
-- chaque IA du compte sur la map (equipe = 1). read_at : lu par l'IA (messages d'un humain).
CREATE TABLE agent_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    agent_id INTEGER NOT NULL REFERENCES api_tokens(id) ON DELETE CASCADE,
    -- Auteur : un humain (user_id) ou l'IA elle-même (de_ia = 1).
    user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    de_ia INTEGER NOT NULL DEFAULT 0,
    equipe INTEGER NOT NULL DEFAULT 0,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    read_at TEXT
);
CREATE INDEX agent_messages_fil ON agent_messages (agent_id, id);
