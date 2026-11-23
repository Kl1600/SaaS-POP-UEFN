-- Objectifs du calendrier (#261) : heure facultative (HH:MM) et détail.
ALTER TABLE milestones ADD COLUMN time TEXT;
ALTER TABLE milestones ADD COLUMN detail TEXT NOT NULL DEFAULT '';
