-- Réglage par studio (décision du 24/11, #165) : les IA peuvent supprimer seules dans UEFN (journalisé, restaurable).
ALTER TABLE studios ADD COLUMN ia_supprime INTEGER NOT NULL DEFAULT 0;
