-- L'IA « chance de percer » garde sa mémoire (#70) :
-- chaque version entraînée, avec sa précision mesurée sur des maps plus récentes que celles de l'entraînement ;
CREATE TABLE ia_models (
  id INTEGER PRIMARY KEY,
  trained_at TEXT NOT NULL DEFAULT (datetime('now')),
  n INTEGER NOT NULL,          -- maps apprises
  positives INTEGER NOT NULL,  -- dont celles qui ont percé
  test_n INTEGER,              -- maps les plus récentes, gardées de côté pour la vérification
  auc REAL,                    -- probabilité qu'une map qui perce soit mieux notée qu'une map qui ne perce pas
  brier REAL,                  -- erreur moyenne des probabilités (0 = parfait)
  hit_rate REAL,               -- part des maps annoncées « fortes » (50 % ou plus) qui ont vraiment percé
  params TEXT NOT NULL         -- moyennes, écarts, poids
);
-- et chaque prédiction faite pour une nouvelle map, notée une fois les 14 jours de test d'Epic passés.
CREATE TABLE ia_predictions (
  code TEXT NOT NULL REFERENCES fn_islands(code) ON DELETE CASCADE,
  day TEXT NOT NULL,           -- jour de la prédiction
  age INTEGER NOT NULL,        -- âge de la map ce jour-là (0 = jour de sortie)
  model_id INTEGER REFERENCES ia_models(id) ON DELETE SET NULL,
  prob REAL NOT NULL,
  features TEXT NOT NULL,
  best INTEGER,                -- plus haut nombre de joueurs pendant le test, une fois noté
  outcome INTEGER,             -- 1 = a percé, 0 = non ; NULL tant que pas noté (ou jamais mesuré)
  graded_at TEXT,
  PRIMARY KEY (code, day)
);
CREATE INDEX idx_ia_predictions_grade ON ia_predictions(graded_at, day);
