-- Map courante du compte (#296) : la même dans Forge et sur le site, changée de l'un ou l'autre côté.
ALTER TABLE users ADD COLUMN current_map_id INTEGER REFERENCES maps(id) ON DELETE SET NULL;
