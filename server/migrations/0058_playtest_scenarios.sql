-- Scénarios joués par les playtesteurs d'une demande de test (#349).
CREATE TABLE playtest_scenarios (
    listing_id INTEGER NOT NULL REFERENCES playtest_listings(id) ON DELETE CASCADE,
    scenario_id INTEGER NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    PRIMARY KEY (listing_id, scenario_id)
);
-- Passage joué par un playtesteur : la demande d'où il vient (NULL : passage de l'équipe).
ALTER TABLE scenario_runs ADD COLUMN listing_id INTEGER REFERENCES playtest_listings(id) ON DELETE SET NULL;
