#!/usr/bin/env bash
# Prépare un serveur Ubuntu 24.04 neuf pour POP (Hetzner CPX02 ou équivalent). À lancer UNE fois, en root :
#   scp -r deploy root@<IP>:/root/pop-deploy
#   ssh root@<IP> 'bash /root/pop-deploy/installer-serveur.sh'
# Ce que fait le script :
#   1. mises à jour du système et mises à jour de sécurité automatiques ;
#   2. 2 Go de swap (le CPX02 n'a que 1 Go de RAM) ;
#   3. Docker depuis le dépôt officiel de Docker ;
#   4. pare-feu : seuls SSH (22), HTTP (80) et HTTPS (443) sont ouverts ;
#   5. utilisateur « pop-admin » (sudo + docker) avec la même clé SSH que root ;
#   6. SSH : connexion par clé seulement, plus de connexion root ;
#   7. copie des fichiers de déploiement dans /opt/pop.
set -euo pipefail

[ "$(id -u)" = 0 ] || { echo "À lancer en root."; exit 1; }
. /etc/os-release
[ "${ID:-}" = ubuntu ] || echo "Attention : prévu pour Ubuntu 24.04, système trouvé : ${PRETTY_NAME:-inconnu}."
ICI="$(cd "$(dirname "$0")" && pwd)"
export DEBIAN_FRONTEND=noninteractive

# La clé SSH doit être en place AVANT de couper la connexion par mot de passe, sinon on perd l'accès.
if [ ! -s /root/.ssh/authorized_keys ]; then
  echo "Aucune clé SSH dans /root/.ssh/authorized_keys : ajoutez votre clé (console Hetzner) puis relancez."
  exit 1
fi

echo "== 1. Mises à jour"
apt-get update -q
apt-get upgrade -yq
apt-get install -yq ca-certificates curl gnupg ufw unattended-upgrades sqlite3
dpkg-reconfigure -f noninteractive unattended-upgrades

echo "== 2. Swap de 2 Go"
if ! swapon --show | grep -q /swapfile; then
  fallocate -l 2G /swapfile
  chmod 600 /swapfile
  mkswap /swapfile
  swapon /swapfile
  grep -q '^/swapfile' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
fi
# N'utiliser le swap qu'en dernier recours.
echo 'vm.swappiness=10' > /etc/sysctl.d/99-pop.conf
sysctl -q --system

echo "== 3. Docker"
if ! command -v docker >/dev/null; then
  install -m 0755 -d /etc/apt/keyrings
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
  chmod a+r /etc/apt/keyrings/docker.asc
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu ${VERSION_CODENAME} stable" \
    > /etc/apt/sources.list.d/docker.list
  apt-get update -q
  apt-get install -yq docker-ce docker-ce-cli containerd.io docker-compose-plugin
fi
systemctl enable --now docker

echo "== 4. Pare-feu"
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 443/udp
ufw --force enable

echo "== 5. Utilisateur pop-admin"
if ! id pop-admin >/dev/null 2>&1; then
  adduser --disabled-password --gecos "" pop-admin
fi
usermod -aG sudo,docker pop-admin
install -d -m 700 -o pop-admin -g pop-admin /home/pop-admin/.ssh
install -m 600 -o pop-admin -g pop-admin /root/.ssh/authorized_keys /home/pop-admin/.ssh/authorized_keys
# sudo sans mot de passe : pop-admin n'en a pas (connexion par clé seulement).
echo 'pop-admin ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/pop-admin
chmod 440 /etc/sudoers.d/pop-admin

echo "== 6. SSH par clé seulement"
# « 00- » : sshd garde la PREMIÈRE valeur lue, et 50-cloud-init.conf (images cloud) remettrait les mots de passe.
cat > /etc/ssh/sshd_config.d/00-pop.conf <<'EOF'
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin no
EOF
sshd -t
systemctl reload ssh

echo "== 7. Fichiers de déploiement dans /opt/pop"
install -d -m 750 -o pop-admin -g pop-admin /opt/pop
for f in docker-compose.yml Caddyfile litestream.yml env.exemple README.md; do
  install -m 640 -o pop-admin -g pop-admin "$ICI/$f" /opt/pop/"$f"
done
if [ ! -f /opt/pop/.env ]; then
  install -m 600 -o pop-admin -g pop-admin /opt/pop/env.exemple /opt/pop/.env
fi

echo ""
echo "Terminé. Désormais : ssh pop-admin@<IP> (root ne peut plus se connecter)."
echo "Suite : deploy/README.md, « Premier démarrage »."
