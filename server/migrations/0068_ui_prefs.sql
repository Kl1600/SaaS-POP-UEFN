-- Préférences d'affichage de chaque compte (#493) : ordre et replis de la barre latérale, didacticiels déjà vus.
-- Un objet JSON, gardé sur le compte pour suivre l'utilisateur d'un appareil à l'autre.
ALTER TABLE users ADD COLUMN ui_prefs TEXT NOT NULL DEFAULT '{}';
