-- Onglet Discover de « Lancer et suivre » : préparation de la sortie et suivi des 14 jours de test d'Epic.
CREATE TABLE map_discover (
  map_id INTEGER PRIMARY KEY REFERENCES maps(id) ON DELETE CASCADE,
  title TEXT NOT NULL DEFAULT '',
  thumb TEXT NOT NULL DEFAULT '{}',      -- résultat de l'analyse de la vignette (faite dans le navigateur)
  trailer INTEGER NOT NULL DEFAULT 0,
  channels INTEGER NOT NULL DEFAULT 0,
  updates TEXT NOT NULL DEFAULT '[]',    -- dates des mises à jour publiées
  hourly_at TEXT
);
-- Joueurs en même temps, heure par heure, pour les îles reliées à une map POP.
CREATE TABLE fn_hourly (
  code TEXT NOT NULL,
  ts TEXT NOT NULL,
  ccu INTEGER,
  PRIMARY KEY (code, ts)
);
