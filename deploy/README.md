# Mettre POP en ligne sur un VPS (Hetzner CPX02)

Un seul serveur : **Caddy** (HTTPS) → **POP** (image Docker) → base SQLite dans le volume `pop-data`,
copiée en continu hors du serveur par **Litestream**. Le serveur ne compile jamais : l'image est construite par
GitHub Actions (`.github/workflows/publier-image.yml`) à chaque fusion dans `main` et publiée sur
`ghcr.io/kl1600/saas-pop-uefn`.

Mesure du 18/11/2026 (release de `main`, base neuve) : POP utilise 26 Mo de RAM au repos et 65 Mo au plus fort de
10 studios, 100 WebSocket et 486 requêtes/s. Avec Ubuntu, Docker, Caddy et Litestream, on reste sous 500 Mo :
le CPX02 (1 Go + 2 Go de swap) suffit pour la bêta. Passer au CPX12 (2 Go) depuis la console Hetzner si besoin
(« CPU et RAM seulement », pour pouvoir redescendre).

| Fichier | Rôle |
|---|---|
| `docker-compose.yml` | les trois services (`pop`, `caddy`, `litestream`) |
| `Caddyfile` | HTTPS automatique (Let's Encrypt) et proxy vers POP |
| `litestream.yml` | sauvegarde continue de `/data/pop.db` vers un stockage S3 |
| `env.exemple` | modèle du fichier `.env` du serveur (domaine, e-mails, SMTP, stockage) |
| `installer-serveur.sh` | prépare un Ubuntu 24.04 neuf (mises à jour, swap, Docker, pare-feu, SSH par clé) |
| `deployer.sh` | met POP à jour sur le serveur (lancé par l'action GitHub « déployer » ou à la main) |
| `activer-deploiement.sh` | autorise la clé de GitHub à lancer `deployer.sh`, et rien d'autre |

## 1. Avant de commencer

1. **Le serveur** : Hetzner Cloud, CPX02, Ubuntu 24.04, **avec votre clé SSH**, option Backups conseillée.
2. **Le domaine** : chez votre registrar, un enregistrement `A` (et `AAAA` pour l'IPv6) vers l'adresse du serveur.
   Il doit répondre **avant** le premier démarrage, sinon Caddy n'obtient pas de certificat.
3. **L'image** : la première fois, lancer l'action « publier l'image » (onglet Actions du dépôt → *Run workflow*),
   ou fusionner une PR dans `main`.
4. **Un jeton GitHub pour lire l'image** (le paquet est privé comme le dépôt) : GitHub → Settings → Developer
   settings → Personal access tokens (classic) → droit **`read:packages` seulement**. Il se tape sur le serveur,
   jamais dans un fichier du dépôt.

## 2. Préparer le serveur (une fois)

Depuis la racine du dépôt, sur votre PC :

```sh
scp -r deploy root@<IP>:/root/pop-deploy
ssh root@<IP> 'bash /root/pop-deploy/installer-serveur.sh'
```

Ensuite on se connecte avec **`ssh pop-admin@<IP>`** : le script coupe la connexion root et les mots de passe.

## 3. Premier démarrage

```sh
cd /opt/pop
nano .env                                   # POP_DOMAIN, POP_ADMIN_EMAILS, SMTP…
docker login ghcr.io -u <compte-github>     # coller le jeton read:packages
docker compose pull
docker compose up -d
docker compose logs -f caddy pop            # attendre « certificate obtained », puis Ctrl+C
```

Ouvrir `https://<domaine>`, créer votre compte avec l'adresse mise dans `POP_ADMIN_EMAILS`
(ou, après coup : `docker compose exec pop /app/pop-server admin ajouter vous@exemple.com`), puis activer la double
authentification (obligatoire pour l'administration).

Le relais UEFN se connecte alors à `https://<domaine>`.

## 4. Mettre à jour, revenir en arrière

**Depuis GitHub (un clic)** : onglet **Actions** → **déployer** → **Run workflow**, version `latest` (dernier
`main`) ou `sha-<7 premiers caractères du commit>` pour une version précise. Il faut le droit d'écriture sur le dépôt.
L'action vérifie que l'image de `main` est construite, se connecte au serveur, lance `deployer.sh` (télécharge
l'image, redémarre POP, vérifie que `https://<domaine>` répond) et échoue en rouge sinon, avec les journaux de POP.
Ce n'est **jamais automatique** à chaque fusion : une version peut migrer la base, on choisit le moment.
Mise en place : §4.1.

**À la main, sur le serveur** : `bash /opt/pop/deployer.sh` (ou `bash /opt/pop/deployer.sh sha-1a2b3c4`).

La version déployée est écrite dans `POP_IMAGE` du `.env` (l'ancien `.env` est gardé dans `.env.avant-deploiement`).
Revenir en arrière = déployer le `sha-…` d'avant. Attention : une version plus ancienne ne connaît pas les migrations
plus récentes ; rien n'est remis en arrière automatiquement.

### 4.1 Déployer depuis GitHub : mise en place (une fois)

Sur votre PC, une clé réservée à GitHub (sans phrase de passe, elle ne servira qu'à lancer `deployer.sh`) :

```sh
ssh-keygen -t ed25519 -N "" -C deployer-github -f pop-deployer
```

Sur un serveur installé avant l'ajout de ces scripts, copier d'abord les deux scripts :

```sh
scp deploy/deployer.sh deploy/activer-deploiement.sh pop-admin@<IP>:/opt/pop/
```

Puis autoriser la clé (elle ne pourra lancer que `/opt/pop/deployer.sh`, sans terminal ni redirection) :

```sh
ssh pop-admin@<IP> "bash /opt/pop/activer-deploiement.sh '$(cat pop-deployer.pub)'"
```

Le script affiche la ligne à mettre dans `DEPLOY_KNOWN_HOSTS`. Dans GitHub, **le propriétaire du dépôt**
(seul à voir cette page) : **Settings → Secrets and variables → Actions → New repository secret**, trois fois :

| Secret | Valeur |
|---|---|
| `DEPLOY_HOST` | l'adresse IP (ou le nom) du serveur |
| `DEPLOY_SSH_KEY` | tout le contenu du fichier **privé** `pop-deployer` |
| `DEPLOY_KNOWN_HOSTS` | la ligne affichée par `activer-deploiement.sh`, avec la même adresse que `DEPLOY_HOST` |

Ensuite, **effacez `pop-deployer` de votre PC** (la clé privée n'existe plus que dans GitHub).
Ce sont des secrets du dépôt, pas d'un environnement GitHub : sur un dépôt privé, les environnements demandent un
abonnement payant.

Le serveur doit déjà être connecté à `ghcr.io` (`docker login`, §3) : `deployer.sh` tourne en `pop-admin`.
Retirer l'accès : supprimer la ligne `deployer-github` de `/home/pop-admin/.ssh/authorized_keys`.

## 5. Sauvegardes

Trois niveaux :

| Quoi | Où | Durée |
|---|---|---|
| Copie quotidienne faite par POP (`VACUUM INTO`) | `/data/backups/` dans le volume (même disque) | 14 jours |
| Litestream, en continu | stockage S3 **hors du serveur** | 7 jours, à la seconde près |
| Option Backups d'Hetzner | image complète du serveur | 7 jours |

**Activer Litestream** : créer un bucket (Cloudflare R2 ou Backblaze B2 : 10 Go gratuits) et une clé limitée à
ce bucket, remplir les variables `LITESTREAM_*` de `.env`, décommenter `COMPOSE_PROFILES=sauvegarde`, puis
`docker compose up -d`. Vérifier : `docker compose logs litestream` doit montrer « replicating » sans erreur.

### Restaurer la base

```sh
cd /opt/pop
docker compose stop pop litestream
docker compose run --rm --entrypoint sh litestream -c \
  'mv /data/pop.db /data/pop.db.avant-restauration; rm -f /data/pop.db-wal /data/pop.db-shm; \
   litestream restore -config /etc/litestream.yml -o /data/pop.db /data/pop.db'
docker compose up -d
```

(Ajouter `-timestamp 2026-11-18T14:00:00Z` à `litestream restore` pour revenir à un instant précis.)
Sans Litestream : arrêter `pop`, copier un fichier de `/data/backups/` à la place de `/data/pop.db`, redémarrer.

## 6. Surveiller

- `docker compose ps` : les services tournent (`Up`) ;
- `docker compose logs --tail 100 pop` : erreurs de POP ;
- `free -h` et `df -h` : mémoire et disque (20 Go sur le CPX02) ;
- `docker stats --no-stream` : mémoire par conteneur.

## Pièges connus

- **E-mails** : Hetzner bloque les ports 25 et 465 sur les nouveaux comptes → SMTP sur le port **587**
  (Brevo, Postmark, Resend…).
- **Ne jamais compiler sur le serveur** (`docker build`) : la compilation Rust dépasse 1 Go de RAM.
- **Cloudflare devant POP** (proxy orange) : passer `POP_TRUST_PROXY` à `cloudflare` dans `docker-compose.yml`,
  sinon toutes les limites par adresse IP visent Cloudflare.
- **Ancien volume à root** : voir le README principal (`chown -R 10001:10001 /data`).
