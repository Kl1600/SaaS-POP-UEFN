# Domaine relais

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

```
FAIT | #148 MCP · Construire le décor d'une map | 23/11 22:30
Owner : benybens | Sur la tâche : Claude Code, pour benybens
Fait : place_mesh (mesh_spawn), get/set_island_settings, mesh importé en Verse par Scene Graph (KB/UEFN.md)
Reste : rien ; pas de make_prop (prop BuildingProp absent du digest)
Bloque : rien
```

```
EN COURS | #81 Forge F7 · Relais et serveur : poser un appareil classique | 21/11 01:30
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : action actor_spawn (asset ou modèle du niveau), MCP place_device, annulation par IA soumise à validation
Reste : test avec le vrai UEFN (chemin d'asset d'un appareil, spawn_actor_from_object)
EN COURS | #82 Forge F8 · Session de test pilotable par les IA (démarrer, arrêter, lire la sortie) | 21/11 02:00
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : actions relais start/stop_session et session_output (GetVerseOutput), outils MCP, aide du moteur sur compile_verse
Reste : test avec le vrai UEFN (noms d'arguments de StartSession à confirmer)
EN COURS | #90 Forge F16 · Relais : importer un fichier dans le projet UEFN | 21/11 01:50
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : actions asset_import (fichier du PC ou URL du serveur POP) et asset_delete, MCP import_asset, tests
Reste : test avec le vrai UEFN (AssetImportTask, dossier par défaut /<projet>/POP)
Bloque : rien
```

```
EN COURS | #77 Forge F3 · UEFN Link intégré à Forge | 20/11 23:30
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : relais dans Forge (config de pop-link), menu UEFN Link, état dans le titre, conflit pop-link.exe
Reste : test avec le vrai UEFN sur Windows
Bloque : rien
```

```
FAIT | #76 Forge F2 · Squelette de l'application Windows (Tauri 2) qui ouvre POP | 20/11 22:58
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : forge/ Tauri 2, page d'adresse, job CI Windows ; validé par Kl1600 sur Windows
Reste : rien
Bloque : rien
```

```
FAIT | #75 Forge F1 · Relais : faire de pop-link une bibliothèque réutilisable | 20/11 22:00
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : lib.rs (start → Handle), validé sur Mac par Kl1600
Reste : test avec le vrai UEFN, repris dans #77
Bloque : rien
```

```
INFO | PR #1 pop-link : piloter le vrai UEFN via ses toolsets MCP | 17/11 03:00
Owner : benybens | Sur la tâche : Claude Code, pour Yassine
Fait : toolsets + call_tool, options, compilation, suppression annulable (T3D), testés sur VaultRush
Reste : tester CheckIn, PushChanges en session, réglages @editable
Bloque : rien
```

```
FAIT | #158 Relais : pop-link.exe téléchargeable périmé | 22/11 17:00
Owner : benybens | Sur la tâche : Claude Code, pour benybens
Fait : exe recompilé (url 2.5.9), PROTO=2 = RELAY_PROTO serveur, « relais périmé » dans POP, CI refuse link/ sans exe ; testé avec le vrai UEFN
Reste : rien
Bloque : rien
```
