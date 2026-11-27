"""Nouveautés de POP Forge (#437) : lit forge/NOUVEAUTES.md.

  python3 tools/nouveautes.py notes 0.3.2     texte de la version (lignes « - … »), code 1 si absente
"""
import re
import sys
from pathlib import Path

FICHIER = Path(__file__).resolve().parent.parent / "forge" / "NOUVEAUTES.md"


def versions(texte: str) -> dict:
    """{version: [lignes]} dans l'ordre du fichier."""
    res, cur = {}, None
    for l in texte.splitlines():
        m = re.match(r"^##\s+([0-9][^\s·]*)", l)
        if m:
            cur = m.group(1)
            res[cur] = []
        elif cur and l.strip().startswith("- "):
            res[cur].append(l.strip()[2:].strip())
    return res


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] != "notes":
        sys.exit(__doc__)
    lignes = versions(FICHIER.read_text(encoding="utf-8")).get(sys.argv[2])
    if not lignes:
        print(f"forge/NOUVEAUTES.md n'a pas d'entrée « ## {sys.argv[2]} · date » avec au moins une ligne « - … ».", file=sys.stderr)
        sys.exit(1)
    print("\n".join("· " + l for l in lignes))
