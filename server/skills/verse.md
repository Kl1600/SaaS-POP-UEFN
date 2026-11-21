# Écrire du Verse pour UEFN avec POP

1. **Bibliothèque d'abord** : `search_library` puis `get_library_module`. Réutilisez un module du studio quand il convient ; n'écrivez du neuf que s'il n'y a rien.
2. **Fichiers** : `list_verse_files`, `read_verse_file`, puis `write_verse_file` (chemin relatif au projet, ex. `Content/jeu.verse`). Indiquez `task` (numéro de la tâche POP) pour que l'écriture soit rattachée à la tâche.
3. **Structure d'un appareil Verse** :
   ```verse
   using { /Fortnite.com/Devices }
   using { /Verse.org/Simulation }

   jeu := class(creative_device):
       @editable Bouton : button_device = button_device{}

       OnBegin<override>()<suspends>:void =
           Bouton.InteractedWithEvent.Subscribe(QuandAppuye)

       QuandAppuye(Agent : agent):void =
           Print("Bouton appuyé")
   ```
4. **Compiler** : `compile_verse`. Les lignes et colonnes renvoyées par UEFN commencent à 0. Lisez `aide` quand elle est présente (corrections proposées par POP).
5. **Tester** : `start_session` (une fois), puis après chaque modification `push_to_session` (Verse seul : rapide), et `session_output` pour lire les `Print` et les erreurs. Corrigez et recommencez.
6. Un appareil Verse ne se pose pas par Python : glissez-le depuis le Content Browser (`<projet> Content → CreativeDevices`), ou demandez-le à l'humain.
7. Les réglages `@editable` se lisent et se règlent avec `get_actor` et `update_actors` (`props`). Une valeur qui change de type est refusée (UEFN arrondirait sans prévenir).
