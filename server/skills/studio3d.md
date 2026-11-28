# Faire un modèle 3D avec Blender

L'humain vous demande un objet 3D (dans son terminal Forge ou son chat). Vous écrivez un **script Python pour Blender**
(bpy) qui le construit, et vous l'envoyez avec `blender_build` (name : nom court, script : le code). Forge l'exécute
sur le PC de l'humain (Blender doit être installé : Forge → Outils → Blender) et garde le FBX (`local_path`).
**N'importez dans UEFN que si l'humain le veut** : `import_asset` avec `local_path` et `folder` (journalisé, annulable).

1. **Scène vide** au départ ; construisez le modèle avec des primitives, des modificateurs (Bevel, Array, Mirror,
   Solidify, Boolean…) et `bmesh`. Pas d'add-on, pas de fichier à charger.
2. **Isolation** : le script n'a **ni Internet ni accès aux fichiers** de l'humain ; il peut seulement lire et écrire
   dans le dossier `TRAVAIL` (variable déjà définie). Il est arrêté après **120 s** : restez simple et rapide.
3. **Échelle et position** : 1 unité Blender = 1 mètre (un joueur mesure environ 1,9 m). Posez l'objet sur le sol
   (base à z = 0), centré sur l'origine.
4. **Léger** : visez quelques milliers de triangles (Epic conseille de 400 à 9 000 polygones selon la taille ;
   son validateur avertit au-delà de 30 000 sommets). `blender_build` rend le nombre de triangles : si `too_heavy`,
   simplifiez et renvoyez.
5. **Couleurs** : un matériau Principled BSDF par couleur (Base Color) ; pas de texture image.
6. Pas d'export à faire : POP exporte le FBX lui-même. En cas d'erreur, `blender_build` rend le message de Blender :
   corrigez et renvoyez.
7. Dites à l'humain ce que vous avez fait (taille, triangles) et demandez-lui s'il veut l'importer.
