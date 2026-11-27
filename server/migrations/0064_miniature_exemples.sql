-- Outil Miniature de Forge (#468) : images d'exemple jointes à la demande (clés du stockage objet, JSON).
ALTER TABLE thumbnail_requests ADD COLUMN examples TEXT NOT NULL DEFAULT '[]';
