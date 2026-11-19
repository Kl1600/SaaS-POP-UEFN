-- Paiements Stripe, crédits IA, chiffres privés du Creator Portal.

-- Origine du plan : 'manual' (fixé par l'administrateur, comptes bêta : jamais bloqués),
-- 'trial' (nouveau compte en essai), 'stripe' (abonnement payé).
ALTER TABLE users ADD COLUMN plan_source TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE users ADD COLUMN stripe_customer TEXT;
ALTER TABLE users ADD COLUMN sub_id TEXT;
ALTER TABLE users ADD COLUMN sub_status TEXT;
ALTER TABLE users ADD COLUMN sub_end TEXT;
-- Crédits IA achetés (distincts des crédits de playtest gagnés en testant les maps des autres).
ALTER TABLE users ADD COLUMN ai_credits INTEGER NOT NULL DEFAULT 0;

-- Événements Stripe déjà traités (un même événement peut être renvoyé).
CREATE TABLE stripe_events (
  id TEXT PRIMARY KEY,
  type TEXT NOT NULL,
  at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Mouvements de crédits IA : achats (+) et utilisations (−), avec le coût réel chez Claude.
CREATE TABLE ai_ledger (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  delta INTEGER NOT NULL,
  reason TEXT NOT NULL,
  ref TEXT,
  tokens_in INTEGER,
  tokens_out INTEGER,
  cost_usd REAL,
  amount_cents INTEGER,   -- pour un achat : montant encaissé (centimes d'euro)
  at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_ai_ledger_user ON ai_ledger(user_id, at);

-- Chiffres privés importés du Creator Portal (CSV d'analytics), un chiffre par jour et par mesure.
CREATE TABLE map_private_stats (
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  day TEXT NOT NULL,
  metric TEXT NOT NULL,
  value REAL NOT NULL,
  PRIMARY KEY (map_id, day, metric)
);
CREATE TABLE map_private_imports (
  id INTEGER PRIMARY KEY,
  map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
  user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  filename TEXT,
  rows INTEGER NOT NULL,
  metrics TEXT NOT NULL,
  at TEXT NOT NULL DEFAULT (datetime('now'))
);
