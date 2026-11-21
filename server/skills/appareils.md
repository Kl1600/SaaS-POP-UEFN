# Placer et régler des appareils

1. **Réservez votre tâche** avec `claim_task` avant de modifier le niveau.
2. **Chercher** : `list_actors` (filtre texte : nom, type, dossier, tags, options), `get_actor` pour le détail.
3. **Poser** : `place_device` avec `asset` (chemin de l'appareil dans le Content Browser) ou `like` (copie d'un appareil du niveau avec ses réglages). Donnez `name` et `folder`. Positions en coordonnées du moteur : X avant, Y droite, Z haut (pas les coordonnées affichées dans l'éditeur).
4. **Régler** : `update_actors` avec `options` (appareils classiques : une des valeurs autorisées) ou `props` (réglages `@editable` d'un appareil Verse ; une référence s'écrit `{ "refPath": "…" }`).
5. **Ranger** : un dossier par zone ou par type, des noms clairs (`Bouton_Depart`, `Spawn_Rouge_1`).
6. **Vérifier** : `check_level` après vos changements ; corrigez d'abord les problèmes « bloquant ».
7. **Supprimer** : `delete_actors` crée une demande ; un humain valide dans l'Organiseur. Ne comptez pas sur une suppression immédiate.
8. Un acteur ou un dossier verrouillé dans Lore par un autre membre est refusé : choisissez une autre tâche ou prévenez l'humain.
