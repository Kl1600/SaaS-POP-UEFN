-- Statistiques d'administration par jour (#68) : photo quotidienne des comptes actifs et des abonnements, pour suivre
-- leur évolution (la base ne garde que l'état actuel des plans). Mise à jour chaque minute par la boucle du serveur.
CREATE TABLE admin_stats_day (
  day TEXT PRIMARY KEY,
  active INTEGER NOT NULL DEFAULT 0,
  starter INTEGER NOT NULL DEFAULT 0,
  pro INTEGER NOT NULL DEFAULT 0,
  trials INTEGER NOT NULL DEFAULT 0
);
