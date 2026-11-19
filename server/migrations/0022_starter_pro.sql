-- Deux abonnements : Starter (12,99 €/mois) et Pro (24,99 €/mois, avec Analyse).
-- Les anciens plans sont convertis : Créateur → Starter, Studio et Studio+ → Pro.
-- Nouveau compte : 7 jours d'essai de Pro (trial_until).
UPDATE users SET plan = 'pro' WHERE plan IN ('studio', 'studio_plus');
UPDATE users SET plan = 'starter' WHERE plan NOT IN ('pro', 'starter');
ALTER TABLE users ADD COLUMN trial_until TEXT;
