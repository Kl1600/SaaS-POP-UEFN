-- Réglage par studio (#302, décision du 24/11) : « Les IA peuvent supprimer seules ». Désactivé par défaut : une
-- suppression demandée par une IA attend alors la validation d'un humain.
ALTER TABLE studios ADD COLUMN ai_can_delete INTEGER NOT NULL DEFAULT 0;
