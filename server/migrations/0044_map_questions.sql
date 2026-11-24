-- Parcours de questions-boutons (#135) : questions de cadrage posées par POP (clé fixe, une fois par map) ou
-- questions posées par une IA avec l'outil MCP ask_user. Une réponse = un des choix, ou un texte libre (« Autre »).
CREATE TABLE map_questions (
    id INTEGER PRIMARY KEY,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    cle TEXT,
    auteur TEXT NOT NULL,
    agent_id INTEGER REFERENCES api_tokens(id) ON DELETE SET NULL,
    question TEXT NOT NULL,
    choix TEXT NOT NULL,
    autre INTEGER NOT NULL DEFAULT 1,
    ordre INTEGER NOT NULL DEFAULT 0,
    reponse TEXT,
    repondu_par INTEGER REFERENCES users(id) ON DELETE SET NULL,
    repondu_le TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX map_questions_cle ON map_questions(map_id, cle);
CREATE INDEX map_questions_map ON map_questions(map_id, id);
