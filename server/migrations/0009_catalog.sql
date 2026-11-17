-- Parcours complet du catalogue d'îles (et pas seulement les dernières sorties) + sources de maps populaires.
ALTER TABLE fn_status ADD COLUMN cursor TEXT;
ALTER TABLE fn_status ADD COLUMN sweeps INTEGER NOT NULL DEFAULT 0;
ALTER TABLE fn_status ADD COLUMN seed_ok TEXT;
ALTER TABLE fn_status ADD COLUMN seed_error TEXT;
CREATE TABLE fn_seed (
  code TEXT PRIMARY KEY,
  source TEXT NOT NULL,
  rank INTEGER,
  seen_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_fn_islands_metrics ON fn_islands(metrics_at);
