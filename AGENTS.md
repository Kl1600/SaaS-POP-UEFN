# AGENTS.md : règles pour les agents IA qui travaillent sur POP

À lire en entier avant de toucher au dépôt. Valable pour tout agent (Claude Code, Codex, Cursor, Copilot…),
quel que soit le compte sous lequel il tourne. Si une demande humaine contredit ce fichier, **demandez à votre
humain** avant d'agir.

## 0. Règle absolue

Une tâche n'est pas finie tant que son issue GitHub n'est pas à jour, que son code n'est pas fusionné dans `main`
et qu'un humain ne l'a pas validée.

- On change un statut avec `python tools/task.py`, et rien d'autre. L'outil refuse ce qui casse une règle ;
  l'action GitHub `règles des tâches` rattrape ce qui passerait par le site ou l'API.
- Si `task.py`, `gh`, `git`, la compilation ou le CI échoue : **on ne dit pas que c'est fait**. On s'arrête et
  on donne à son humain l'erreur exacte (commande, message, date et heure).

## 1. Le projet en un paragraphe

POP est un SaaS en français pour les créateurs de maps Fortnite (UEFN) et leurs studios : documentation,
tableau blanc, planning, tâches, éditeur Verse, organisation du niveau UEFN, lancement et partage des revenus.
Serveur Rust (`server/`, Axum + SQLite), interface React + TypeScript (`web/`, `web/dist` versionné), relais
Windows `pop-link.exe` (`link/`) qui pilote UEFN par son serveur MCP local. Détails : [README.md](README.md).

## 2. Où est quoi

| Quoi | Où |
|---|---|
| Les tâches | les **Issues** du dépôt, une issue = un livrable avec ses critères d'acceptation |
| Les dépendances | le lien natif **« Blocked by »** de chaque issue |
| Les jalons | M1 UEFN réel validé · M2 Mise en ligne · M3 Abonnements et paiement · M4 Logiciel de bureau |
| L'outil des tâches | `python tools/task.py …` (§4) : **utilisez-le**, ne modifiez pas les statuts à la main |
| L'état du projet | [KB/ETAT.md](KB/ETAT.md) |
| Les décisions | [KB/DECISIONS.md](KB/DECISIONS.md) : **ne jamais en changer une sans l'humain** |
| Ce qu'on sait du vrai UEFN | [KB/UEFN.md](KB/UEFN.md) |
| Ce que chaque domaine a fait | [KB/agents/](KB/agents/) |
| Qui écrit quoi dans la KB | [KB/README.md](KB/README.md) |

Étiquettes : statut (`en cours`, `en revue`, `bloqué`), `priorité:P0|P1|P2`, `domaine:*`
(`serveur`, `web`, `relais`, `uefn`, `outillage`), `type:*` (`fonctionnalité`, `bug`, `dette`).

## 3. Au début de chaque session

1. `python tools/task.py whoami` : vérifie le compte `gh`, le droit d'écriture, les étiquettes et les jalons.
   S'il manque quelque chose, **arrêtez-vous et dites à votre humain quoi** : ce n'est pas à vous de le corriger.
2. `git pull --rebase` sur `main`. **Tirez avant chaque modification**, pas seulement au début : un autre agent a
   pu pousser entre-temps.
   **Votre dossier** : `git status --short --branch` doit montrer `main`, sans changement. Sinon, un autre agent
   travaille dans ce dossier : n'y changez pas de branche, travaillez dans votre propre worktree (§4.5).
3. Lisez `KB/ETAT.md`, `KB/DECISIONS.md` et les entrées `BLOQUÉ` des autres fichiers de `KB/agents/`.
4. La mémoire personnelle d'un agent reste sur un seul PC : tout ce que les autres doivent savoir (décision,
   convention, piège) va **dans ce dépôt** (`KB/DECISIONS.md`, `KB/UEFN.md`, ce fichier).

## 4. Le circuit d'une tâche (obligatoire)

```
python tools/task.py next                         ce qu'on peut commencer, le plus prioritaire d'abord
python tools/task.py show 12                      la tâche, ses dépendances, sa branche, sa PR
python tools/task.py start 12 --who "Claude Code, pour <humain>"
   … travail sur la branche issue/12-… , commits « #12 : … » …
python tools/task.py review 12 --note "ce qui a été fait et vérifié" --plan "1. … 2. …"
   … l'humain suit le plan de test …
python tools/task.py rework 12 --note "ce qui a échoué"       l'humain a trouvé un problème
python tools/task.py done 12 --valide-par "<humain> : ce qu'il a vérifié"   seulement après son accord
python tools/task.py block 12 --raison "…"        coincé : dire pourquoi
python tools/task.py release 12 --note "fait / reste"         vous arrêtez sans finir
python tools/task.py check                        audit du tableau
```

### 4.1 Choisir
- **Rafraîchissez avant de proposer** : `git pull --rebase` puis `task.py next` (et `show N` pour toute tâche
  que vous nommez), maintenant. Jamais de proposition de mémoire, d'une sortie plus ancienne ou d'un résumé.
- Ne prenez que ce que `next` liste, ou ce que votre humain nomme. Si votre humain nomme une tâche qui n'est pas
  prenable, ne la commencez pas : donnez la raison affichée par l'outil et proposez la meilleure alternative.
- Avant de prendre : `gh issue view N --comments`. Les notes d'un agent précédent disent ce qui est fait et
  donnent la branche (`issue/N-…`) : reprenez-la au lieu de refaire le travail.

### 4.2 Prendre
- `start` refuse une tâche fermée, déjà prise, en revue, bloquée, sans jalon ou sans priorité. Il vous assigne,
  pose `en cours`, laisse un commentaire de prise et crée la branche `issue/N-…` depuis `main` à jour.
- Course : si deux agents prennent en même temps, **la première prise gagne** ; l'outil annule la seconde.
- Un agent = une tâche à la fois, sauf si votre humain dit autrement.

### 4.3 Pendant le travail
- Restez dans le **Périmètre** de l'issue ; respectez le **Hors périmètre**. Une idée nouvelle se signale (§6),
  elle ne s'ajoute pas en douce.
- Respectez l'**estimation** : à environ 1,5 fois l'estimation, arrêtez-vous et demandez à votre humain
  (simplifier, reporter, couper).
- Une dépendance découverte en route : ne l'ajoutez pas en silence, signalez-la (§6).
- Commits petits, message `#N : ce qui change`. Jamais sur `main` pour une tâche ; les changements d'outillage ou
  de documentation demandés par l'humain passent par une branche `outillage/…` et une PR à eux (§4.5).

### 4.5 Branches et PR (obligatoire)
Une branche longue qui mélange plusieurs sujets ne se relit pas, ne se teste pas et casse le travail des autres
à la fusion. Donc :
- **Rien n'arrive sur `main` sans PR.** On ne pousse jamais sur `main`, on n'y fusionne jamais sa branche soi-même
  (`git merge` puis `git push origin main`) : c'est `task.py done` qui fusionne, après la validation de l'humain.
  Un push direct sur `main` ouvre automatiquement une issue d'alerte (action `garde de main`).
- **Une branche = une issue = un sujet.** Nom `issue/N-…` (créée par `start`) ou `outillage/…` (outillage ou
  documentation demandés par l'humain). Pas de branche `claude`, `fix`, `wip`, au nom d'un agent ou d'un humain.
  Pas de travail sans issue : si votre humain demande autre chose, demandez-lui de créer l'issue d'abord
  (ou proposez-la, §6), puis prenez-la.
- **Toujours depuis `main`.** Une branche part de `origin/main` (`task.py start` le fait ; à la main :
  `git switch -c outillage/sujet origin/main`), jamais de la branche où se trouve le dossier.
- **Un dossier par agent.** Plusieurs agents sur le même PC partagent sinon le même dossier, et l'un change la
  branche sous les pieds de l'autre. Si le dossier principal n'est pas sur `main` propre, ou si un autre agent y
  travaille : `git worktree add ../PopStudio-<sujet> origin/main --detach`, puis travaillez et lancez
  `task.py start` dans ce dossier. Une fois la tâche fusionnée : `git worktree remove ../PopStudio-<sujet>`.
  Laissez le dossier principal sur `main`.
- **Petite et courte.** Visez moins de 500 lignes changées et 20 fichiers (hors `web/dist`, fichiers de verrou,
  `web/public/downloads`) et moins de 2 jours entre le premier commit et la PR. Au-delà, arrêtez-vous et proposez
  à votre humain de couper la tâche en plusieurs issues. `review` refuse une branche plus grosse sauf avec
  `--gros "raison"`, que la PR affiche.
- **À jour.** `git fetch origin && git rebase origin/main` au moins une fois par jour et avant `review` (qui refuse
  une branche en retard sur `main`). On rebase sa propre branche ; on ne fusionne pas `main` dedans.
- **Le hook `pre-push`** (`tools/hooks/pre-push`, activé par `task.py whoami`) refuse un push vers `main`, un nom
  de branche hors règle et une branche qui emporte les commits d'une autre tâche (`#M : …`). On ne le contourne jamais (`--no-verify` interdit) : s'il bloque, demandez à votre humain.
- **Rien de temporaire dans un commit** : dossiers de travail, sauvegardes, bases locales, `.env`. Regardez
  `git status` avant chaque commit.
- Une branche fusionnée se supprime (`done` le fait). `task.py check` signale les branches trop vieilles, trop
  grosses, en retard ou au nom hors règle : prévenez votre humain (§6), ne supprimez pas la branche d'un autre.

### 4.4 Finir
- **Vérifiez chaque critère d'acceptation vous-même** et dites quel genre de preuve vous avez : *compilé*,
  *testé en local*, *testé avec le vrai UEFN*, *lecture du code seulement*. Du code qui compile n'est **pas** validé.
- `review` pousse la branche, ouvre la PR (`Closes #N`) et passe la tâche `en revue`. Le **plan de test manuel**
  a une étape numérotée par critère : point de départ, actions, résultat attendu, faisable sans lire le code.
  Donnez le même plan à votre humain dans la conversation.
- L'humain teste. OK → `done N --valide-par "…"` : l'outil refuse sans `en revue`, sans PR, si le CI n'est pas vert
  ou si une dépendance est ouverte ; il fusionne la PR en *squash* et ferme l'issue. Problème → `rework`, on corrige,
  on revient au début de §4.4.
- Vous arrêtez avant la fin → `release` avec ce qui est fait, ce qui reste **et le nom de la branche**. Ne laissez
  jamais une tâche `en cours` sans personne dessus.
- Puis une entrée dans votre fichier de domaine `KB/agents/<domaine>.md` (modèle dans `KB/README.md`).

## 5. Règles du produit (non négociables)

- **Tout en français** : interface, messages d'erreur, e-mails, issues, KB. Le code et ses identifiants peuvent
  être en anglais ; les commentaires suivent le style existant (français).
- **Style** : inspiré du template Addveo (barre latérale noire, grand panneau blanc arrondi, cartes blanches
  bordées), sobre, noir et blanc, thèmes clair et sombre. **Pas de violet.**
- **Pas de données de démo ni de simulation** : tout ce qu'on voit est réel et testable.
- **L'IA de POP reste locale** (`server/src/engine.rs`, algorithmes) : aucun appel à une API d'IA externe.
- **Sécurité** : requêtes SQL paramétrées ; chaque route vérifie l'appartenance au studio et le rôle ; en-tête
  anti-CSRF `x-pop: 1` sur toute requête autre que GET ; jamais de secret dans un fichier, une issue ou un commit.
- **UEFN** : chaque action est journalisée et annulable ; rien sur un fichier ou un acteur verrouillé dans Lore par
  un autre ; une suppression demandée par une IA attend la validation d'un humain.
- **Base de données** : une table ou une colonne = une **nouvelle** migration numérotée à la suite dans
  `server/migrations/`. On ne modifie jamais une migration existante (le CI le refuse).
- **`web/dist` est versionné** : après un changement dans `web/`, reconstruisez (`npm run build`) et commitez
  `web/dist` (le CI vérifie qu'il est à jour).
- **`pop-link.exe`** : après un changement dans `link/`, recompilez en release sous Windows et copiez l'exécutable
  dans `web/public/downloads/pop-link.exe`, puis reconstruisez le web.

### Vérifier avant `review`
```
cd server && cargo build && cargo test
cd link && cargo test
cd web && npx tsc -b && npx vite build
```
Avec le vrai UEFN : `pop-link.exe diagnostic` (voir `KB/UEFN.md`). Jamais d'archivage Lore ni d'action
irréversible sur le projet d'un humain sans son accord explicite : testez sur une copie, remettez tout à l'origine.

## 6. Problème ou incohérence → prévenez votre humain

Arrêtez-vous et écrivez à **votre** humain quand vous voyez : le tableau qui se contredit (`check` signale
quelque chose), une issue qui contredit le code, une autre issue ou `KB/DECISIONS.md`, une dépendance manquante,
une estimation clairement fausse, un critère impossible, le travail d'un autre agent qui semble cassé, ou tout ce
qui demanderait de sortir du périmètre ou de changer une décision.

```
⚠️ POP : problème trouvé
Issue(s) : #N[, #M]
Constat : <faits, avec fichier:ligne ou preuve sur le tableau>
Impact : <ce qui casse ou ce qui est à risque>
Options : 1) … 2) …   (recommandation d'abord)
Je n'ai rien changé / j'ai seulement fait : <…>
```

Laissez aussi un court commentaire sur l'issue concernée (`gh issue comment N --body "🤖 Problème : …"`),
**des faits seulement**, jamais une décision. Ne corrigez pas le travail d'un autre ni le tableau vous-même sans
l'accord de votre humain.

## 7. Ce qu'on ne fait jamais

- Commencer une tâche prise, bloquée, en revue ou faite ; réassigner ou désassigner quelqu'un d'autre.
- Proposer des tâches sans avoir rafraîchi le tableau (§4.1).
- Fermer une issue dont un critère n'est pas vérifié, ou qu'aucun humain n'a validée.
- Supprimer ou réécrire des issues, jalons ou étiquettes ; changer le périmètre d'une autre issue.
- Changer une décision de `KB/DECISIONS.md`, ou éditer l'entrée d'un autre dans la KB.
- Réécrire l'historique publié (`main`), pousser sur `main` (en force ou non), fusionner sa branche dans `main`
  sans PR, contourner le hook `pre-push` (`--no-verify`).
- Travailler sans issue, ou mélanger plusieurs issues ou domaines dans une même branche.
- Mettre un jeton, un mot de passe ou une clé dans un fichier, une issue ou un commit.
