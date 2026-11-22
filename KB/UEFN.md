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

## Session de test (testé le 18/11/2026, projet TestPopHub, #5)

- `SessionToolset` : `GetSessionStatus` (`Connected` / `Disconnected`), `GetGameState` (`Running`…),
  `PushChanges { bVerseOnly }`, `StartSession`, `StopSession`, `StartGame`, `StopGame`, et surtout
  **`GetVerseOutput { pattern, maxResults, cursor }`** : il lit les `Print` et les erreurs Verse de la session
  (horodatage, fichier, ligne, `runId` de la partie). Pratique pour vérifier un envoi sans regarder l'écran.
- `PushChanges` **relance les appareils Verse dans la même partie** (même `runId`, `OnBegin` rejoué) : un `Print`
  modifié s'affiche sans relancer la session.
- `bVerseOnly: true` suffit quand seul le Verse a changé : **12 s** contre **78 s** pour un envoi complet (recuisson
  des assets). L'éditeur Verse de POP l'utilise ; le bouton « Envoyer en session » de l'Organiseur fait un envoi
  complet, nécessaire pour une modification d'acteur ou de réglage.
- Sans session, `GetSessionStatus` répond `Disconnected` : pop-link refuse avant d'appeler `PushChanges`.
- L'éditeur de code (Monaco) de POP ne reçoit pas la saisie quand le navigateur qui l'affiche est caché : pour un
  test piloté, garder la page visible.

## Archivage Lore (testé le 18/11/2026, projet TestPopHub, #4)

- `CheckIn { description }` enregistre tout puis archive **toutes** les modifications en attente en une révision ;
  le message saisi dans POP (préfixé `#N` si une tâche est liée) apparaît tel quel dans l'historique Lore d'UEFN.
- Sans modification en attente, `CheckIn` répond `CheckIn: there is nothing to check in.` : POP affiche « Rien à
  archiver : aucune modification en attente dans Lore. »
- `GetRevisionControlStatus` : `bHasChangesToCheckIn` passe à `false` après l'archivage et `revision` change. Les
  champs `branch` / `revision` changent aussi quand des modifications sont en attente (un identifiant `01a1…`
  apparaît, puis la branche principale revient après l'archivage) : ce n'est pas un archivage.
- Deux projets du même compte ont le même `remote_url` et la même `identity` dans `.lore/config.toml`, mais un
  `.lore/id` différent : c'est `id` qui identifie le dépôt.
- **Réouverture d'UEFN** : le serveur MCP répond **avant** que le projet soit chargé (`CheckIn: no project is
  currently open.` pendant environ 2 min 30, même pour un petit projet). POP garde l'action en attente et réessaie
  toutes les 20 s pendant 10 minutes.
- Une session MCP ne survit pas au redémarrage d'UEFN (`Missing required Mcp-Session-Id header`) ; `ping` passe
  pourtant sans session. pop-link refait donc une connexion complète (initialize) au lieu de se fier au `ping`.

## Parcours complet de l'éditeur Verse (testé le 18/11/2026, projet TestPopHub, #7)

- `BuildAll` donne l'étendue exacte d'une erreur (`span.startLine/startCharacter/endLine/endCharacter`, à partir de
  0) : `Printt` ligne 20 → `startLine 19, startCharacter 8, endCharacter 14`. POP souligne maintenant cette étendue
  (avant : toujours 40 caractères).
- « Envoyer à UEFN » avec session et Lore cochés : écriture, compilation, session (Verse seul) et archivage en
  **38 s** ; le `Print` modifié arrive en jeu et la révision porte le message `#N …`.
- « Revenir à cette version » réécrit le fichier sur le disque puis recompile dans UEFN.
- Deux comptes dans le même fichier (onglets `127.0.0.1` et `localhost`, cookies séparés) : chacun voit la saisie de
  l'autre et sa présence dans l'onglet du fichier.

## Poser un appareil (testé le 21/11/2026, projet TestPopHub, #81)

- `place_device` par `asset` : `EditorAssetLibrary.load_asset` veut le chemin **de la classe** de l'appareil, avec
  `_C` (ex. `/CRD_PlayerSpawn/BP_Creative_Player_Spawner_Prop.BP_Creative_Player_Spawner_Prop_C`) ; le chemin du
  paquet seul (`/CRD_PlayerSpawn/BP_Creative_Player_Spawner_Prop`) répond « Asset introuvable ». Le chemin de classe
  d'un acteur se lit par `a.get_class().get_path_name()`.
- Par `like` (copie d'un appareil du niveau) : nom, dossier et position appliqués ; un Spawn Pad copié crée aussi
  son `FortPlayerStartCreative`, que la suppression du Spawn Pad retire avec lui.
- Annulation depuis le journal (humain) : l'appareil disparaît du niveau. Annulation par une IA (`undo_action`) :
  rien n'est supprimé avant la validation d'un humain (`/api/approvals/{id}/approve`).

## Interface de jeu (HUD, menus) créée par une IA (recherche puis test du 21/11/2026, projet TestPopHub, #115)

- Un Widget Blueprint (UMG) se crée par Python (`AssetTools.create_asset` + `WidgetBlueprintFactory`, à vérifier dans
  UEFN) et s'affiche depuis le Verse par `Assets.digest.verse` (`PlayerUI.AddWidget(Dossier.WBP_X{})`), mais **ne se
  pilote pas** : un champ ajouté par `VerseFieldsToolset.AddVerseField` (MCP ou Python) est listé mais n'arrive pas
  dans la classe Verse générée (bug signalé sur le forum Epic, nov. 2026). Construire l'arbre de widgets par script
  reste mal documenté.
- Choix de POP : `create_widget` génère un **appareil Verse** (`Content/POP_UI/pop_ui_<nom>.verse`) qui construit l'interface
  avec l'API UI de Verse (`canvas`, `text_block`, `button_loud`) : texte, compteur par joueur, bouton qui active un
  appareil. Après compilation il est dans le Content Browser et se pose comme un appareil. À revoir quand les champs
  Verse des widgets UMG marcheront.
- À vérifier au premier test : compilation des trois modèles, le compteur qui monte, le bouton qui rend la souris
  (`ui_input_mode.All`) puis la retire à la fermeture.
- Testé : le Verse **hors de `Content/`** (ex. `POP_UI/` à la racine du projet) n'est pas compilé, sans aucune
  erreur ; dans `Content/POP_UI/`, l'appareil apparaît comme `POP_UI-pop_ui_score` (`ListDeviceAssets`).
- Un appareil Verse se pose par `DeviceToolset.PlaceDevice { assetPath: { refPath: "/<projet>/_Verse.POP_UI-pop_ui_score" } }`
  (Python refuse). Posé, sa classe est `VerseDevice_C` ; **sa classe Verse est celle de
  `actor.get_editor_property('script')`** : pop-link la remonte dans `verse_class`.
- Un appareil posé pendant une session n'apparaît en jeu qu'après un envoi **complet** (`PushChanges { bVerseOnly: false }`) :
  le compteur « Score 0 » s'affiche alors en haut de l'écran.

## Importer un fichier (testé le 21/11/2026, projet TestPopHub, #90)

- `AssetImportTask` (automated, `replace_existing=False`) : PNG → `Texture2D`, WAV → `SoundWave`, FBX → `StaticMesh`
  **plus un matériau par matériau du fichier** (6 pour le cube de test), tous dans le dossier choisi.
- Le contenu du projet est monté sous le **nom du monde** (`get_editor_world().get_path_name().split('/')[1]`), ici
  un identifiant (`/44f07e39-…/`), pas sous le nom du dossier du projet : `/TestPopHub/POP` n'importe rien.
- `find_package_referencers_for_asset` : un acteur posé dans le niveau apparaît comme
  `/<projet>/__ExternalActors__/<map>/…` ; les matériaux d'un FBX sont « utilisés par » son mesh. Pour annuler un
  import, on ignore les références internes au lot et on retire le mesh avant ses matériaux.
- `EditorAssetLibrary.delete_directory` retire un dossier vide du Content Browser ; le dossier reste sur le disque
  jusqu'à ce qu'on le supprime.

## Décor : meshes, réglages de l'île, mesh en Verse (testé le 23/11/2026, projet TestPopHub, UEFN 43.12, #148)

**Toolsets repérés** (62 au total) :
- `EditorToolset.LevelToolset` : `AddToSceneFromAsset` / `AddToSceneFromClass` (poser), `ReplaceActorsWithAsset`,
  `SetActorFolder`, `RemoveFromScene`, `TraceWorld`, dossiers, data layers.
- `EditorToolset.StaticMeshToolset` : `ImportFile` (FBX, OBJ, glTF → StaticMesh), `GetBounds`, matériaux, LOD,
  collisions, Nanite. `EditorToolset.ActorToolset` : transformations, labels, composants.
- `ValkyrieToolset.EntityToolset` / `PrefabToolset` (Scene Graph : entités, composants, prefabs Verse),
  `PCGToolset`, `geometry_toolset.geometry_ops.GeometryTools` (hors périmètre de #148).
- **Aucun toolset Landscape ni « île »** : les réglages de l'île passent par `DeviceToolset` (voir plus bas).

**Poser un mesh** (`place_mesh`, action relais `mesh_spawn`) : `EditorActorSubsystem.spawn_actor_from_object` avec
un `StaticMesh` crée un `FortStaticMeshActor`, puis position, rotation et `set_actor_scale3d`.
- Formes de base disponibles dans UEFN : `/Engine/BasicShapes/Plane|Cube|Sphere|Cylinder|Cone`, toutes de **1 m**
  (le plan : 1 m × 1 m, épaisseur nulle). Plan de 20 m × 20 m = échelle `[20, 20, 1]` (vérifié : bornes 2 000 cm).
- Un mesh importé (`StaticMeshToolset.ImportFile` ou `import_asset`) se pose de la même façon à sa taille d'origine
  × l'échelle (OBJ de 2 m, échelle 2 → 4 m vérifié).
- L'annulation supprime l'acteur créé (`actor_delete`), comme `place_device`.

**Réglages de l'île** : ce sont les options d'un appareil classique, `IslandSettings0`
(`Device_ExperienceSettings_V2_UEFN_C`, chemin `…:PersistentLevel.IslandSettings_0`). `ListClassicDeviceOptions`
en rend **287** avec valeur actuelle et valeurs permises (réponse de 177 Ko) ; `MaxPlayers` accepte 1 à 16, puis
20, 24, 30, 32, 40, 48, 50, 60…100. `set_island_settings` vérifie nom et valeur, puis passe par `actor_update`
(`SetClassicDeviceOption`) : 16 → 12 puis annulation → 16 vérifiés dans UEFN.

**Un mesh importé utilisable par le Verse, sans manipulation** : avec le Scene Graph activé (par défaut sur
TestPopHub : `EnableSceneGraph` dans le `.uplugin`), chaque StaticMesh du projet apparaît dans
`Assets.digest.verse` comme un composant `NomDuMesh := class(mesh_component)` (et `NomDuMesh_asset:mesh`) dans le
module de son dossier. Testé en session : ce Verse fait apparaître le mesh importé `POP_TEST/POP_Bloc`, et le
`Print` est arrivé sans erreur :
```verse
if (Racine := GetSimulationEntity[]):
    E := entity{}
    Racine.AddEntities(array{E})
    E.AddComponents(array{POP_TEST.POP_Bloc{Entity := E}})
    E.SetGlobalTransform(transform{Translation := vector3{Forward := 0.0, Left := 0.0, Up := 400.0}})
```
(`using { /Verse.org/SceneGraph }`, `{ /Verse.org/SpatialMath }`, `{ /Verse.org/Simulation }`). Le Verse
compilé, le digest se met à jour à la compilation suivante (`BuildAll`).

**Ce qui ne marche pas** : un blueprint créé en Python avec pour parent `BuildingProp` (mesh réglé sur le CDO,
compilé, enregistré) **n'apparaît pas** comme `creative_prop_asset` dans le digest, donc `SpawnProp` ne peut pas
l'utiliser. Pas d'outil `make_prop` : la voie Scene Graph ci-dessus suffit. Pour `SpawnProp` (ancienne API), il
faut un prop de la galerie Fortnite (`creative_prop_asset` du digest Fortnite), pas un mesh importé.

## Ouvrir UEFN sur un projet sans le launcher Epic (constat de benybens du 26/11/2026, UEFN 43.12, #402)

- **Exécutable** : manifestes `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item`, entrée
  `AppName = Fortnite_Studio` (`InstallLocation` + `LaunchExecutable`, par exemple
  `E:\Fortnite\FortniteGame\Binaries\Win64\UnrealEditorFortnite-Win64-Shipping.exe`). Lancé sans argument, il se
  connecte seul au compte Epic quand le launcher tourne en fond.
- **Projet présélectionné** : UEFN ignore le `.uefnproject` passé en argument et `LastProjectFileName`. Il
  présélectionne le projet dont `LastOpenedTime` est le plus récent dans
  `%LOCALAPPDATA%\UnrealEditorFortnite\Saved\Config\WindowsEditor\VK_Projects\<projectId>.ini`, section
  `[/Script/ValkyrieEditor.ValkyriePerProjectUserConfig]`, heure UTC au format `2026.11.26-15.21.51`.
  `<projectId>` = `bindings.projectId` du `.uefnproject`.
- **Reste un clic** : UEFN reste sur l'écran de choix avec le bon projet présélectionné ; il faut cliquer « Lancer ».
  `bStartupWithLastProject=True` n'y change rien ; le serveur MCP tourne déjà sur cet écran mais aucun toolset
  n'ouvre un projet.
- **Ce que fait Forge** (« Ouvrir dans UEFN », #402) : met `LastOpenedTime` à l'heure actuelle (seule ligne
  modifiée, section créée si besoin), lance l'exécutable, puis dit de cliquer « Lancer ». Rien si UEFN est déjà
  ouvert.
- **Pas encore testé** : le lancement quand le launcher Epic est fermé (critère 6 de #402).

## Couleur et matériaux d'un prop de galerie (testé le 22/11/2026, projet GrassCutter, tâche POP #10)

- **Un asset du projet ne peut pas référencer un asset de Fortnite** (`/Game/Creative/…`, `/CRG_Props/…`) : un
  matériau créé dans le projet avec une texture Fortnite, ou une instance de matériau dont le parent est un
  matériau Fortnite, donne « Disallowed reference » / « illegally references » dans le Message Log, et
  `PushChanges` répond `Failed: Validation failed`. Pendant cet échec, le serveur MCP d'UEFN a cessé de répondre
  plusieurs minutes. Les assets créés en Python (`AssetTools.create_asset`) ne sont validés qu'à l'envoi en
  session : vérifier le Message Log (« Asset Check ») avant.
- Conséquence : `creative_prop.SetMaterial` ne peut utiliser qu'un matériau entièrement fait dans le projet, et le
  matériau d'origine d'un prop de galerie ne peut pas être remis par le Verse (pas de `GetMaterial`, et l'original
  n'est pas dans `Assets.digest.verse`).
- `editor_only_set_instance_material_vector_parameter("BlueprintColor", couleur)` sur un prop d'herbe
  (`CP_Grass_Tall_01`) ne teinte que son premier composant (1 touffe sur 9) ; masquer les autres composants ne
  tient pas (le Blueprint se reconstruit).
- `CP_P_Atmospheric_Sparkles` (étincelles) n'est pas un `creative_prop` : `SetDeviceProperty` le refuse
  (« not valid Devices_creative_prop »). `CP_ArtDeco_Gold_Bar_Pile` (pile de lingots) en est un : c'est le repère
  retenu pour l'herbe dorée de GrassCutter, déplacé par `TeleportTo[Position, Rotation]` (garde l'échelle).
- `place_device` pose aussi un prop de galerie par le chemin de sa classe
  (`/Game/Creative/Sets/ArtDeco_Bank/Props/CP_ArtDeco_Gold_Bar_Pile.CP_ArtDeco_Gold_Bar_Pile_C`).
- `duplicate_actors` de POP ne crée qu'une copie par identifiant distinct : passer N fois le même identifiant n'en
  crée qu'une.
