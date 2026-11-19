#!/usr/bin/env bash
# Met POP à jour sur le serveur. Lancé par l'action GitHub « déployer » (.github/workflows/deployer.yml), par SSH,
# avec une clé qui ne peut lancer QUE ce script (voir activer-deploiement.sh). Utilisable aussi à la main :
#   bash /opt/pop/deployer.sh                 dernière image de main
#   bash /opt/pop/deployer.sh sha-1a2b3c4     une version précise (revenir en arrière)
# Ce que fait le script :
#   1. refuse toute demande qui n'est pas « latest » ou « sha-<commit> » (la clé GitHub ne peut rien passer d'autre) ;
#   2. écrit la version choisie dans POP_IMAGE du fichier .env (un « docker compose up -d » à la main la garde) ;
#   3. télécharge l'image, redémarre POP, vérifie que le site répond en HTTPS ;
#   4. si le site ne répond pas : affiche les journaux et échoue, SANS revenir en arrière tout seul
#      (la nouvelle version a pu migrer la base, qu'une version plus ancienne ne sait pas lire).
set -euo pipefail

DEPOT_IMAGE=ghcr.io/kl1600/saas-pop-uefn
DOSSIER=/opt/pop

# Par SSH, la version arrive dans SSH_ORIGINAL_COMMAND (la commande demandée par le client, remplacée par ce script).
version="${1:-${SSH_ORIGINAL_COMMAND:-latest}}"
if ! [[ "$version" =~ ^(latest|sha-[0-9a-f]{7,40})$ ]]; then
  echo "Version refusée : « $version ». Attendu : latest ou sha-<7 caractères du commit>."
  exit 2
fi

cd "$DOSSIER"
[ -f .env ] || { echo "Pas de fichier $DOSSIER/.env : voir deploy/README.md, « Premier démarrage »."; exit 1; }

# Un seul déploiement à la fois.
exec 9>"$DOSSIER/.deployer.lock"
flock -n 9 || { echo "Un déploiement est déjà en cours sur le serveur."; exit 1; }

image="$DEPOT_IMAGE:$version"
avant="$(grep -E '^POP_IMAGE=' .env | tail -n1 | cut -d= -f2- || true)"
echo "== Version demandée : $image (avant : ${avant:-$DEPOT_IMAGE:latest})"

echo "== Téléchargement de l'image"
docker pull -q "$image"
revision="$(docker image inspect -f '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$image" 2>/dev/null || true)"
[ -n "$revision" ] && echo "   commit : $revision"

# POP_IMAGE dans .env : on remplace la ligne (commentée ou non), sinon on l'ajoute. Copie de l'ancien .env à côté.
cp -p .env .env.avant-deploiement
if grep -qE '^#?POP_IMAGE=' .env; then
  sed -i -E "s|^#?POP_IMAGE=.*|POP_IMAGE=$image|" .env
else
  printf '\nPOP_IMAGE=%s\n' "$image" >> .env
fi

echo "== Redémarrage"
docker compose up -d --remove-orphans

# Le site doit répondre en HTTPS, par Caddy, comme pour un visiteur (sans dépendre du DNS vu du serveur).
domaine="$(grep -E '^POP_DOMAIN=' .env | tail -n1 | cut -d= -f2-)"
echo "== Vérification de https://$domaine"
ok=0
for _ in $(seq 1 45); do
  if curl -fsS -o /dev/null --max-time 5 --resolve "$domaine:443:127.0.0.1" "https://$domaine/"; then ok=1; break; fi
  sleep 2
done
if [ "$ok" != 1 ]; then
  echo "ÉCHEC : https://$domaine ne répond pas après 90 secondes. Derniers journaux de POP :"
  docker compose logs --tail 80 pop || true
  echo ""
  echo "Rien n'a été remis en arrière automatiquement. Version d'avant : ${avant:-$DEPOT_IMAGE:latest}."
  echo "Y revenir (relancer « déployer » avec son sha-…) seulement si la nouvelle version n'a ajouté aucune migration."
  exit 1
fi

docker image prune -f >/dev/null
echo "== En ligne : $image${revision:+ (commit ${revision:0:7})}"
docker compose ps
