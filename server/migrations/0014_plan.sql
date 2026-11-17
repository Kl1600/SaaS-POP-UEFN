-- Abonnement du compte : 'createur', 'studio' ou 'studio_plus'. Analyse est comprise à partir de 'studio'.
-- Bêta : tout le monde démarre en 'studio' ; l'administrateur peut changer le plan de chacun.
ALTER TABLE users ADD COLUMN plan TEXT NOT NULL DEFAULT 'studio';
