-- Analyse : notes quotidiennes des niches (pour vérifier après coup si elles avaient raison),
-- modèle appris (poids des axes, chance de percer) et relevés horaires du marché.
ALTER TABLE fn_niches ADD COLUMN score REAL;
ALTER TABLE fn_niches ADD COLUMN axes TEXT;
CREATE TABLE idea_model (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  weights TEXT,          -- poids des 5 axes appris par vérification après coup
  calib TEXT,            -- détails de la vérification (échantillons, corrélations)
  launch TEXT,           -- modèle « chance de percer »
  calibrated_at TEXT,
  trained_at TEXT
);
INSERT INTO idea_model (id) VALUES (1);
ALTER TABLE fn_status ADD COLUMN hourly_ok TEXT;
