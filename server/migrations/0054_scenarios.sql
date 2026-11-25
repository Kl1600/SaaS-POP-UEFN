-- Scénarios de test rejouables (#348) : étapes (action, résultat attendu), passages par version, verdict par étape.
CREATE TABLE scenarios (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    -- 'draft' : proposé par une IA, à valider par un humain ; 'ready' : jouable.
    status TEXT NOT NULL DEFAULT 'ready',
    agent_id INTEGER,
    created_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX scenarios_map ON scenarios (map_id, id);
CREATE TABLE scenario_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scenario_id INTEGER NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    pos INTEGER NOT NULL,
    action TEXT NOT NULL,
    expected TEXT NOT NULL DEFAULT ''
);
CREATE INDEX scenario_steps_s ON scenario_steps (scenario_id, pos);
CREATE TABLE scenario_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scenario_id INTEGER NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    version TEXT NOT NULL DEFAULT '',
    tester_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT
);
CREATE INDEX scenario_runs_s ON scenario_runs (scenario_id, id);
CREATE TABLE scenario_results (
    run_id INTEGER NOT NULL REFERENCES scenario_runs(id) ON DELETE CASCADE,
    step_id INTEGER NOT NULL REFERENCES scenario_steps(id) ON DELETE CASCADE,
    -- 'pass' réussi, 'fail' échoué, 'skip' non testé
    verdict TEXT NOT NULL,
    comment TEXT NOT NULL DEFAULT '',
    capture_key TEXT,
    task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
    PRIMARY KEY (run_id, step_id)
);
