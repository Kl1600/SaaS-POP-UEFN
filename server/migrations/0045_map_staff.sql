-- Équipe d'une map et avis de faisabilité (#149) : qui travaille sur la map, combien d'heures par semaine, ses
-- indisponibilités, la date de début, la marge de sécurité et la part des tâches confiée à l'IA.
CREATE TABLE map_staff (
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    hours_per_week INTEGER NOT NULL,
    PRIMARY KEY (map_id, user_id)
);
CREATE TABLE map_absences (
    id INTEGER PRIMARY KEY,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL
);
CREATE INDEX map_absences_map ON map_absences(map_id);
ALTER TABLE maps ADD COLUMN start_date TEXT;
ALTER TABLE maps ADD COLUMN margin_pct INTEGER NOT NULL DEFAULT 20;
ALTER TABLE maps ADD COLUMN ai_share_pct INTEGER NOT NULL DEFAULT 0;
