# Runner local pour GitHub Actions

Quand les minutes gratuites de GitHub Actions du compte Kl1600 sont épuisées (2 000 min par mois, budget à 0 $),
GitHub ne lance plus aucun job : le CI ne passe plus et `task.py done` ne fusionne plus rien. Un runner
« self-hosted » sur un PC de l'équipe fait tourner les jobs Linux à la place, sans consommer ces minutes.

## Comment ça marche

Chaque workflow choisit son runner avec `runs-on: ${{ vars.RUNNER || 'ubuntu-latest' }}` :

- variable de dépôt `RUNNER` absente : les jobs tournent chez GitHub, comme avant ;
- `RUNNER` = `self-hosted` : les jobs tournent sur le runner local.

Le job `forge (Windows, build + installateur)` est **sauté** tant que `RUNNER` est définie (le runner local est
Linux). Un contrôle sauté compte comme bon pour `task.py done` : pendant ce temps, une PR qui touche `forge/` ou
`link/` doit compiler Forge à la main sous Windows avant `review`.

Revenir chez GitHub (par exemple le 1er du mois, quand le quota repart) : supprimer la variable `RUNNER`. Aucun
changement de code.

## Installer le runner (une fois, sur le PC qui le fait tourner)

Seul un administrateur du dépôt (Kl1600) peut créer un runner et définir la variable.

1. **Kl1600** : dépôt → *Settings → Actions → Runners → New self-hosted runner* → **Linux**, **x64**. Garder la
   page ouverte : elle donne la version du runner et un jeton valable une heure. Ne jamais copier ce jeton dans un
   fichier, une issue ou un commit.
2. **Sur le PC**, dans PowerShell : `wsl -d Ubuntu-22.04`, puis dans Ubuntu, un utilisateur dédié (le runner
   exécute le code de toutes les branches poussées : il ne doit pas voir vos fichiers) :
   ```
   sudo adduser --disabled-password --gecos "" runner
   sudo usermod -aG docker runner   # seulement si Docker est installé dans WSL (workflows image Docker, déployer)
   sudo apt-get update && sudo apt-get install -y build-essential pkg-config curl git jq unzip xz-utils
   sudo -iu runner
   curl --proto '=https' -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
   ```
3. **Toujours en tant que `runner`**, suivre la partie *Download* puis *Configure* de la page de l'étape 1
   (dossier `~/actions-runner`, `./config.sh --url … --token …`). Nom : `pop-<pc>` ; étiquettes : laisser
   `self-hosted,Linux,X64`.
4. Lancer : `cd ~/actions-runner && ./run.sh`. Laisser la fenêtre ouverte : le runner doit rester lancé pour que
   le CI tourne. Il apparaît « Idle » dans *Settings → Actions → Runners*.
5. **Kl1600** : *Settings → Secrets and variables → Actions → Variables → New repository variable* :
   `RUNNER` = `self-hosted`.
6. Relancer le dernier CI de `main` (*Re-run all jobs*) : les jobs doivent passer par `pop-<pc>`. Le CI ne tourne
   plus que sur `main` (décision du 22/11).

## À savoir

- Un seul runner exécute un job à la fois : le CI est plus lent qu'avec GitHub. Un deuxième runner (même
  procédure, autre nom) les fait tourner deux par deux.
- Le PC doit rester allumé avec WSL et `run.sh` lancés ; sinon les jobs attendent en file.
- `docker.yml`, `publier-image.yml` et `deployer.yml` demandent Docker dans WSL (`sudo apt-get install -y docker.io`,
  `sudo usermod -aG docker runner`, puis redémarrer le service du runner).
- Retirer le runner : `./config.sh remove --token <jeton de la page Runners>`, puis supprimer l'utilisateur.
- WSL s'éteint quand plus aucun programme ne l'utilise, et le runner avec lui. Le garder allumé :
  `wsl -d Ubuntu-22.04 -u runner --exec sleep infinity` dans une fenêtre réduite (à relancer après un redémarrage
  du PC). Installé en service (`sudo ./svc.sh install runner`, systemd activé dans `/etc/wsl.conf`), le runner
  repart tout seul dès qu'Ubuntu démarre.
