#!/usr/bin/env bash
# Autorise l'action GitHub « déployer » à mettre POP à jour, et RIEN d'autre. À lancer une fois, sur le serveur,
# en pop-admin, avec la clé PUBLIQUE créée pour GitHub (voir deploy/README.md, « Déployer depuis GitHub ») :
#   bash /opt/pop/activer-deploiement.sh "ssh-ed25519 AAAA… deployer-github"
# La clé est ajoutée à ~/.ssh/authorized_keys avec :
#   command="/opt/pop/deployer.sh"  quoi que GitHub demande, seul ce script tourne (il valide la version reçue) ;
#   restrict                         ni terminal, ni redirection de ports, ni agent, ni X11.
# Relancer le script avec la même clé ne l'ajoute pas deux fois. Pour retirer l'accès : supprimer sa ligne
# (elle se termine par le commentaire de la clé) dans ~/.ssh/authorized_keys.
set -euo pipefail

cle="${1:-}"
# Une seule ligne, rien d'autre : un retour à la ligne glisserait une seconde clé SANS la restriction.
if ! [[ "$cle" =~ ^ssh-ed25519\ [A-Za-z0-9+/=]+(\ [A-Za-z0-9@._-]+)?$ ]] || [[ "$cle" == *$'\n'* ]]; then
  echo "Donnez la clé publique ed25519 entre guillemets : \"ssh-ed25519 AAAA… commentaire\"."
  exit 2
fi
[ "$(id -un)" = pop-admin ] || { echo "À lancer en pop-admin (ssh pop-admin@<IP>)."; exit 1; }
[ -x /opt/pop/deployer.sh ] || chmod 750 /opt/pop/deployer.sh
command -v flock >/dev/null && command -v curl >/dev/null || { echo "flock et curl sont nécessaires."; exit 1; }

fichier="$HOME/.ssh/authorized_keys"
corps="$(echo "$cle" | cut -d' ' -f2)"
if grep -qF "$corps" "$fichier"; then
  echo "Cette clé est déjà dans $fichier : rien à faire."
  exit 0
fi
printf 'command="/opt/pop/deployer.sh",restrict %s\n' "$cle" >> "$fichier"
chmod 600 "$fichier"
echo "Clé ajoutée : elle ne peut lancer que /opt/pop/deployer.sh."
echo ""
echo "Secret DEPLOY_KNOWN_HOSTS de GitHub : la ligne ci-dessous, en remplaçant <DEPLOY_HOST> par la valeur exacte"
echo "du secret DEPLOY_HOST (l'adresse IP ou le nom du serveur) :"
echo "<DEPLOY_HOST> $(cut -d' ' -f1-2 /etc/ssh/ssh_host_ed25519_key.pub)"
