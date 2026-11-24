-- CI GitHub Actions dans POP (#217) : runs et jobs reçus par le webhook (workflow_run, workflow_job) ou repris par
-- l'API, rattachés au studio, à la map de l'intégration et, pour une branche issue/N-…, à la tâche N.
CREATE TABLE ci_runs (
    id INTEGER PRIMARY KEY,
    studio_id INTEGER NOT NULL REFERENCES studios(id) ON DELETE CASCADE,
    map_id INTEGER REFERENCES maps(id) ON DELETE CASCADE,
    task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
    github_id INTEGER NOT NULL,
    workflow TEXT NOT NULL DEFAULT '',
    branch TEXT NOT NULL DEFAULT '',
    sha TEXT NOT NULL DEFAULT '',
    pr INTEGER,
    event TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT '',
    conclusion TEXT,
    started_at TEXT,
    updated_at TEXT NOT NULL DEFAULT '',
    url TEXT NOT NULL DEFAULT '',
    UNIQUE (studio_id, github_id)
);
CREATE INDEX ci_runs_map ON ci_runs(map_id, github_id);
CREATE INDEX ci_runs_task ON ci_runs(task_id);
CREATE TABLE ci_jobs (
    id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES ci_runs(id) ON DELETE CASCADE,
    github_id INTEGER NOT NULL UNIQUE,
    name TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT '',
    conclusion TEXT,
    started_at TEXT,
    completed_at TEXT,
    steps TEXT NOT NULL DEFAULT '[]',
    url TEXT NOT NULL DEFAULT ''
);
CREATE INDEX ci_jobs_run ON ci_jobs(run_id);
