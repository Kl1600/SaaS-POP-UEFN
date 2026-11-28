-- 3D Studio (#327) : modèles 3D faits par un script Blender de l'IA de l'utilisateur, sur son PC (#351).
-- Ils attendent « Accepter » (import dans UEFN, journalisé et annulable) ou « Refuser » (rien n'est importé).
CREATE TABLE studio3d_modeles (
    id INTEGER PRIMARY KEY,
    map_id INTEGER NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
    agent_id INTEGER REFERENCES api_tokens(id) ON DELETE SET NULL,
    nom TEXT NOT NULL,
    script TEXT NOT NULL,
    apercu BLOB,
    triangles INTEGER NOT NULL DEFAULT 0,
    objets INTEGER NOT NULL DEFAULT 0,
    -- Fichier gardé par Forge sur le PC du créateur, importé tel quel après « Accepter ».
    chemin TEXT NOT NULL,
    fichier TEXT NOT NULL,
    statut TEXT NOT NULL DEFAULT 'propose' CHECK (statut IN ('propose', 'accepte', 'refuse')),
    decide_par INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX studio3d_modeles_map ON studio3d_modeles(map_id, id);
