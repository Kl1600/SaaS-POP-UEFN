#!/usr/bin/env python3
"""Messages automatiques vers le Discord de POP, par webhook (#185). Bibliothèque standard seulement.

Utilisé par les actions GitHub `discord : issues` et `déployer` :

  python tools/discord.py issue                       issue ouverte, fermée ou rouverte (lit GITHUB_EVENT_PATH)
  python tools/discord.py notes --de A --a B          notes de version entre deux commits (Markdown, sur stdout)
  python tools/discord.py deploiement --statut ok --commit SHA [--notes fichier] [--release URL] [--journaux URL]

Le webhook vient de la variable d'environnement DISCORD_WEBHOOK (un secret du dépôt, jamais dans un fichier).
S'il manque, on prévient et on sort sans erreur : Discord ne fait jamais échouer un déploiement.
`--essai` affiche le message au lieu de l'envoyer.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

SITE = "https://pop-uefn.com"
NOIR, VERT, ROUGE, GRIS = 0x111111, 0x2E7D32, 0xC62828, 0x8A8A8A
# Limites de Discord : 4096 caractères de description par embed, 256 de titre.
MAX_DESCRIPTION = 3900
MAX_TITRE = 250


def couper(texte, limite, suite="…"):
    """Coupe à la dernière fin de ligne avant la limite, pour ne pas casser une ligne au milieu."""
    if len(texte) <= limite:
        return texte
    coupe = texte[: limite - len(suite)]
    if "\n" in coupe:
        coupe = coupe[: coupe.rfind("\n")]
    return coupe.rstrip() + "\n" + suite


def envoyer(message, essai=False):
    if essai:
        print(json.dumps(message, ensure_ascii=False, indent=2))
        return 0
    webhook = os.environ.get("DISCORD_WEBHOOK", "").strip()
    if not webhook:
        print("::warning::Pas de webhook Discord (secret du dépôt absent) : message non envoyé.")
        return 0
    corps = json.dumps(message).encode("utf-8")
    requete = urllib.request.Request(
        webhook,
        data=corps,
        method="POST",
        # Sans User-Agent explicite, Discord (Cloudflare) refuse parfois les requêtes de urllib.
        headers={"Content-Type": "application/json", "User-Agent": "POP-notifications (GitHub Actions)"},
    )
    try:
        with urllib.request.urlopen(requete, timeout=20) as reponse:
            print(f"Discord : message envoyé ({reponse.status}).")
    except urllib.error.HTTPError as e:
        # On ne recopie jamais l'URL du webhook : elle vaut un mot de passe.
        print(f"::warning::Discord a refusé le message : HTTP {e.code} {e.read()[:300]!r}")
    except OSError as e:
        print(f"::warning::Discord injoignable : {e}")
    return 0


# ---------------------------------------------------------------- issues

def message_issue(evenement):
    action = evenement.get("action")
    issue = evenement["issue"]
    qui = (evenement.get("sender") or {}).get("login", "?")
    etiquettes = [e["name"] for e in issue.get("labels", [])]
    if action == "opened":
        entete, couleur = "Nouvelle issue", NOIR
    elif action == "reopened":
        entete, couleur = "Issue rouverte", NOIR
    elif action == "closed" and issue.get("state_reason") == "not_planned":
        entete, couleur = "Issue abandonnée", GRIS
    elif action == "closed":
        entete, couleur = "Issue terminée", VERT
    else:
        return None
    champs = []
    if etiquettes:
        champs.append({"name": "Étiquettes", "value": couper(", ".join(etiquettes), 1000), "inline": True})
    if issue.get("milestone"):
        champs.append({"name": "Jalon", "value": issue["milestone"]["title"], "inline": True})
    embed = {
        "title": couper(f"#{issue['number']} {issue['title']}", MAX_TITRE),
        "url": issue["html_url"],
        "color": couleur,
        "author": {"name": f"{entete} · par {qui}"},
        "fields": champs,
    }
    return {"username": "POP · GitHub", "embeds": [embed], "allowed_mentions": {"parse": []}}


# ---------------------------------------------------------------- notes de version

LIGNE = re.compile(r"^#(\d+)\s*:\s*(.*?)(?:\s*\(#\d+\))?$")


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True, encoding="utf-8").stdout


def notes(de, a, depot):
    """Une ligne par commit de main entre `de` (exclu) et `a` : « #N : titre (#PR) » devient un lien vers #N."""
    base = f"https://github.com/{depot}" if depot else ""
    if de:
        plage = f"{de}..{a}"
        sujets = git("log", "--first-parent", "--format=%s", plage).splitlines()
        if not sujets and git("rev-parse", de).strip() == git("rev-parse", a).strip():
            return "Même version que le déploiement précédent : rien de nouveau."
        if not sujets and subprocess.run(["git", "merge-base", "--is-ancestor", a, de]).returncode == 0:
            return "Retour à une version précédente : rien de nouveau."
    else:
        # Premier déploiement annoncé : on ne remonte pas toute l'histoire.
        sujets = git("log", "--first-parent", "--format=%s", "-n", "20", a).splitlines()
    if not sujets:
        return "Même version que le déploiement précédent : rien de nouveau."
    lignes = []
    for sujet in sujets:
        m = LIGNE.match(sujet.strip())
        if m and base:
            lignes.append(f"- [#{m.group(1)}]({base}/issues/{m.group(1)}) {m.group(2)}")
        elif m:
            lignes.append(f"- #{m.group(1)} {m.group(2)}")
        else:
            lignes.append(f"- {sujet.strip()}")
    texte = "\n".join(lignes)
    if not de:
        texte = "Premier déploiement annoncé : les 20 derniers changements.\n" + texte
    return texte


# ---------------------------------------------------------------- déploiements

def message_deploiement(statut, commit, texte_notes, release, journaux):
    court = commit[:7]
    if statut == "ok":
        description = couper((texte_notes or "").strip(), MAX_DESCRIPTION)
        liens = [f"[Ouvrir POP]({SITE})"]
        if release:
            liens.append(f"[Notes sur GitHub]({release})")
        embed = {
            "title": f"POP est à jour · version {court}",
            "url": SITE,
            "color": VERT,
            "description": description + "\n\n" + " · ".join(liens),
        }
    else:
        embed = {
            "title": f"Déploiement raté · version {court}",
            "color": ROUGE,
            "description": "Le site garde sa version précédente."
            + (f"\n[Voir les journaux]({journaux})" if journaux else ""),
        }
    return {"username": "POP · Déploiements", "embeds": [embed], "allowed_mentions": {"parse": []}}


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--essai", action="store_true", help="afficher le message au lieu de l'envoyer")
    sous = p.add_subparsers(dest="commande", required=True)
    s = sous.add_parser("issue")
    s.add_argument("--evenement", default=os.environ.get("GITHUB_EVENT_PATH"))
    s = sous.add_parser("notes")
    s.add_argument("--de", default="")
    s.add_argument("--a", required=True)
    s.add_argument("--depot", default=os.environ.get("GITHUB_REPOSITORY", ""))
    s = sous.add_parser("deploiement")
    s.add_argument("--statut", choices=["ok", "echec"], required=True)
    s.add_argument("--commit", required=True)
    s.add_argument("--notes", help="fichier Markdown des notes de version")
    s.add_argument("--release", default="")
    s.add_argument("--journaux", default="")
    args = p.parse_args()

    if args.commande == "issue":
        if not args.evenement:
            sys.exit("Pas d'événement GitHub (GITHUB_EVENT_PATH ou --evenement).")
        with open(args.evenement, encoding="utf-8") as f:
            message = message_issue(json.load(f))
        if message is None:
            print("Action d'issue ignorée.")
            return 0
        return envoyer(message, args.essai)
    if args.commande == "notes":
        print(notes(args.de, args.a, args.depot))
        return 0
    texte = ""
    if args.notes and os.path.exists(args.notes):
        with open(args.notes, encoding="utf-8-sig") as f:
            texte = f.read()
    return envoyer(message_deploiement(args.statut, args.commit, texte, args.release, args.journaux), args.essai)


if __name__ == "__main__":
    sys.exit(main())
