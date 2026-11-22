# La KB de POP : qui écrit quoi

La mémoire commune de l'équipe et de ses agents. On lit tout ; on n'édite jamais l'entrée d'un autre.

| Fichier | Qui l'écrit | Quoi |
|---|---|---|
| `ETAT.md` | le coordinateur seulement | l'état du projet, 10 lignes au plus, **réécrit**, jamais allongé |
| `DECISIONS.md` | tout le monde | une ligne par décision, **ajoutée en bas**, signée ; une décision qui change en remplace une autre par une nouvelle ligne qui le dit |
| `UEFN.md` | tout le monde | faits vérifiés sur le vrai UEFN (outils MCP, formats, pièges), avec la date du test |
| `RUNNER.md` | tout le monde | runner local de GitHub Actions : quand s'en servir, installation, retour chez GitHub |
| `agents/<domaine>.md` | l'agent de ce domaine seulement | une entrée par tâche, sur le modèle ci-dessous |
| `ARCHIVE.md` | chacun pour ses propres entrées | entrées FAIT anciennes déplacées, non lue par défaut |

Domaines : `serveur`, `web`, `relais`, `produit` (conception, planning, documentation).

## Modèle d'entrée (300 caractères au plus)

```
FAIT | #12 titre exact de l'issue | 17/11 21:40
Owner : <compte GitHub> | Sur la tâche : Claude Code, pour <humain>
Fait : une ligne
Reste : une ligne
Bloque : une ligne, ou rien
```

Le type vaut FAIT, EN COURS, BLOQUÉ ou INFO. Une seule entrée EN COURS par tâche : on la modifie, puis on la passe
en FAIT. Au-delà de 15 entrées, déplacez vos FAIT les plus anciens dans `ARCHIVE.md`.

## Écrire dans la KB

- `git pull --rebase` avant de modifier ; les changements de KB passent par la branche de la tâche ou par une PR
  de documentation.
- Pas de prose ni de code recopié : le chemin du fichier (`server/src/uefn.rs:282`), le nom de l'outil, pas son
  contenu.
- Jamais de secret, de clé ou de mot de passe.

## Mettre en place le tableau (une fois)

Les étiquettes et les jalons que `tools/task.py` suppose ont été créés le 17/11/2026. `python tools/task.py whoami`
dit ce qui manque, s'il manque quelque chose.
