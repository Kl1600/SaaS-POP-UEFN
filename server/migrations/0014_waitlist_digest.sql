-- Les admins reçoivent un résumé par heure des demandes d'accès, au lieu d'un e-mail par demande.
-- Numéro 0014 : 0012 et 0013 sont pris par des PR ouvertes (#11, #15).
ALTER TABLE waitlist ADD COLUMN admin_notified_at TEXT;
-- Les demandes déjà présentes ont été signalées une par une à leur arrivée.
UPDATE waitlist SET admin_notified_at = created_at;
