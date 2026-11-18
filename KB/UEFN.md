# Ce qu'on sait du vrai UEFN

Faits vérifiés sur le vrai UEFN (pas sur un faux serveur), avec la date du test. Ajoutez en bas de la section
concernée ; corrigez une ligne fausse en le disant.

## Serveur MCP d'UEFN (testé le 16-17/11/2026, projet VaultRush)

- Adresse : `http://127.0.0.1:8000/mcp`, protocole MCP `2025-06-18`, réponses JSON (pas de flux SSE).
  À démarrer dans UEFN : Édition → Préférences de l'éditeur → Agentic Workflows → Model Context Protocol → Start Server.
- **Origine** : accepté sans en-tête `Origin` ou avec `http://localhost[:port]` / `http://127.0.0.1[:port]` ;
  **403** pour un site en ligne, `null` et `chrome-extension://…`. Une page POP hébergée ne peut donc pas piloter
  UEFN : il faut le relais local.
- `tools/list` ne renvoie que 3 outils méta : `list_toolsets`, `describe_toolset`, `call_tool`. Les vrais outils
  sont dans 62 toolsets (854 outils) et s'appellent par
  `call_tool { toolset_name, tool_name (nom court), arguments }`. Le nom complet en `tool_name` est refusé.
- Les résultats arrivent enveloppés : `{"returnValue": …}`. Une erreur revient avec `isError: true` et un texte.
- Positions : les outils prennent et renvoient les coordonnées natives du moteur (X avant, Y droite, Z haut),
  pas les coordonnées LUF affichées dans l'éditeur.

## Outils utilisés par pop-link (`link/src/mcpdriver.rs`)

| Rôle | Toolset · outil | Arguments |
|---|---|---|
| Python | `editor_toolset.toolsets.python_scripting.PythonScriptingTools` · `execute_python` | `script: { content, language: "Python" }` ; la sortie `print` revient dans `returnValue` |
| Compiler | `VerseToolset.VerseToolset` · `BuildAll` | aucun ; renvoie une liste `{severity, code, message, filePath, span}` |
| Session | `ValkyrieFortniteToolset.SessionToolset` · `GetSessionStatus`, `PushChanges` | `PushChanges { bVerseOnly }` ; statut `Disconnected` sans session |
| Options classiques | `ValkyrieFortniteToolset.DeviceToolset` · `ListClassicDeviceOptions`, `SetClassicDeviceOption` | `devicePath: { refPath }`, `optionName`, `value` (une des `allowedValues`) |
| Réglages Verse | même toolset · `ListDeviceProperties`, `GetDeviceProperties`, `SetDeviceProperty` | `device: { refPath }` ; `value` encodée en JSON |
| Archivage | `ValkyrieEditor.ValkyrieRevisionControl` · `CheckIn` | `description` ; enregistre tout puis archive **toutes** les modifications en attente |

## Pièges

- `BuildAll` : lignes et colonnes **comptées à partir de 0** ; chemins Verse `/<id du projet>/x.verse`, qui
  correspondent à `Content/x.verse` sur le disque.
- Une suppression d'acteurs par Python (`destroy_actors`, même dans `ScopedEditorTransaction`, ou
  `delete_selected_actors`) **n'entre pas** dans l'historique d'annulation d'UEFN : Ctrl+Z ne la rattrape pas.
  Restauration fiable : `TypedElementCommonActions().copy_selected_elements_to_string(selection)` avant de
  supprimer, puis `paste_elements_from_string(...)` (même nom, position, dossier, options ; nouvel identifiant).
- La liste d'acteurs lue par Python ne contient pas les options des appareils : elles se lisent une par une.
- `unreal.Paths.project_dir()` renvoie `../../../FortniteGame/`, pas le dossier du projet ; le chemin du monde
  (`get_editor_world().get_path_name()`) contient le nom de la map.
- Projet de test : VaultRush, Lore actif (`GetRevisionControlStatus`), 1 175 acteurs, un seul appareil Verse, sans
  réglage `@editable`.

## Réglages @editable d'un appareil Verse (testé le 18/11/2026, projet TestPopHub, #6)

- Appareil de test `pop_test_device` : `@editable` `Nombre : int`, `Actif : logic`, `Texte : string`,
  `Bouton : button_device`.
- `ListDeviceProperties { device: { refPath } }` renvoie un schéma JSON dont les **noms sont en minuscules**
  (`nombre`, `actif`, `texte`, `bouton`) : `int` → `"type": "integer"`, `logic` → `"boolean"`, `string` → `{}` (aucun
  type), référence → `"type": "object"` avec `refPath`.
- `GetDeviceProperties` exige `propertyNames` (liste) et renvoie un objet `{ "nombre": 3, "actif": true,
  "texte": "bonjour", "bouton": null }` ; une référence vide vaut `null`.
- `SetDeviceProperty { device, propertyName, value }` : `value` est du **JSON dans une chaîne** (`"7"`, `"false"`,
  `"\"salut\""`, `{"refPath": "…"}`).
- **UEFN arrondit sans prévenir** : `2.5` envoyé à un `int` devient `2`. POP refuse donc lui-même une valeur qui
  change de type (`check_prop_type` dans `server/src/uefn.rs`).
- Une référence invalide est refusée par UEFN (texte anglais) ; POP l'affiche en français et garde le texte brut
  dans le journal.
- Un appareil Verse ne se pose pas par Python (`spawn_actor_from_class` refuse une `VerseClass`) : il se glisse
  depuis le Content Browser (`<projet> Content → CreativeDevices`).
- Python renvoie `None` comme dossier d'un acteur sans dossier : l'Organiseur affiche un dossier « None »
  (à corriger dans `link/src/mcpdriver.rs`).
