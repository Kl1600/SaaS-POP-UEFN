-- Espaces Creator (#143) : à qui appartient un projet UEFN (`bindings.projectVersePath` du .uefnproject).
ALTER TABLE maps ADD COLUMN creator_space TEXT;
ALTER TABLE maps ADD COLUMN lore_id TEXT;
-- Espace Creator relié à un studio (facultatif) : les projets de cet espace y vont directement.
ALTER TABLE studios ADD COLUMN creator_space TEXT;
-- Espace Creator de la personne (ses propres projets restent en solo).
ALTER TABLE users ADD COLUMN creator_space TEXT;
-- « Laisser en solo » : espaces dont la personne ne veut plus qu'on propose de ranger les maps.
CREATE TABLE creator_space_solo (
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  space TEXT NOT NULL,
  PRIMARY KEY (user_id, space)
);
