#!/usr/bin/env bash
# Lance POP.
#   ./start.sh            → http://127.0.0.1:8080 (ce Mac seulement)
#   ./start.sh --reseau   → accessible depuis les autres appareils du même réseau (le PC Windows avec UEFN)
set -e
cd "$(dirname "$0")"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v cargo >/dev/null || { echo "Rust n'est pas installé : voir README.md."; exit 1; }
# L'interface est livrée déjà construite (web/dist). On la reconstruit seulement si Node est présent.
if command -v npm >/dev/null; then (cd web && npm install --no-audit --no-fund && npm run build); fi
[ -f web/dist/index.html ] || { echo "Interface introuvable (web/dist)."; exit 1; }
if [ "$1" = "--reseau" ]; then
  export POP_BIND=0.0.0.0
  IP=$(ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || hostname -I 2>/dev/null | awk '{print $1}')
  echo ""
  echo "  Sur ce Mac       : http://127.0.0.1:8080"
  echo "  Sur le PC (UEFN) : http://$IP:8080   ← adresse à donner à pop-link"
  echo "  (macOS peut demander d'autoriser les connexions entrantes : acceptez.)"
  echo ""
fi
export POP_DEV_ACCOUNT=1
cd server && cargo run --release
