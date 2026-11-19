#!/usr/bin/env python3
"""Le seul moyen de changer le statut d'une tâche de POP (voir AGENTS.md).

    python tools/task.py whoami                      vérifie compte, droits, étiquettes et jalons
    python tools/task.py next                        tâches qu'on peut commencer, la meilleure d'abord
    python tools/task.py show 12                     une tâche, ses dépendances, sa PR
    python tools/task.py start 12 --who "Claude Code, pour Yassine"
                                                     prend la tâche et crée la branche issue/12-…
    python tools/task.py review 12 --note "..." --plan "1. ... 2. ..."
                                                     pousse la branche, ouvre la PR, passe « en revue »
    python tools/task.py rework 12 --note "..."      la validation humaine a échoué : retour « en cours »
    python tools/task.py done 12 --valide-par "Yassine : ce qu'il a vérifié"
                                                     après la validation humaine et un CI vert : fusionne la PR
    python tools/task.py block 12 --raison "..."     dit pourquoi une tâche commencée est coincée
    python tools/task.py release 12 --note "..."     rend la tâche (fait, reste, branche)
    python tools/task.py check                       audit du tableau (code de sortie 1 s'il y a un problème)

Statuts (étiquettes) : à faire = ouverte sans étiquette de statut ; « en cours » ; « en revue » ;
« bloqué » en plus d'« en cours » ; fait = fermée par la fusion de sa PR.
Dépendances : le lien natif « Blocked by » de GitHub. Il faut Python 3.9+ et gh connecté.
Si gh ou git échoue, rien n'est marqué : on ne dit pas que c'est fait (AGENTS.md, règle 0).
"""
import argparse
import json
import pathlib
import re
import secrets
import shutil
import subprocess
import sys
import time
import urllib.parse
from datetime import datetime, timezone

ROOT = pathlib.Path(__file__).resolve().parents[1]
API_VERSION = "2022-11-28"
DOING, REVIEW, BLOCKED = "en cours", "en revue", "bloqué"
STATUS_LABELS = (DOING, REVIEW, BLOCKED)
PRIORITIES = ("P0", "P1", "P2")
PRIORITY_PREFIX = "priorité:"
VALIDATED_PREFIX = "Validé par :"
CLAIM_RE = re.compile(r"<!-- tache-prise id=(\w+) -->")
RELEASE_MARK = "<!-- tache-rendue -->"
STALE_HOURS = 48
# Étiquettes et jalons que l'outil suppose (créés une fois, voir KB/README.md).
REQUIRED_LABELS = list(STATUS_LABELS) + [PRIORITY_PREFIX + p for p in PRIORITIES]
NETWORK_ERRORS = ("dial tcp", "connectex", "i/o timeout", "connection reset", "TLS handshake")

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
except AttributeError:
    pass


class Refused(RuntimeError):
    """Une règle refuse l'action : rien n'a été changé."""


# ---------------------------------------------------------------- gh et git

def gh_path():
    found = shutil.which("gh")
    if found:
        return found
    default = pathlib.Path(r"C:\Program Files\GitHub CLI\gh.exe")  # winget, avant redémarrage du terminal
    if default.exists():
        return str(default)
    raise Refused("gh n'est pas installé : winget install GitHub.cli (ou brew install gh), puis gh auth login")


def gh(*args, payload=None, check=True):
    """Lance gh dans le dépôt ; renvoie le JSON lu, ou le texte brut."""
    command = [gh_path(), *args] + (["--input", "-"] if payload is not None else [])
    for attempt in range(4):
        done = subprocess.run(command, cwd=ROOT, input=json.dumps(payload) if payload is not None else None,
                              capture_output=True, text=True, encoding="utf-8")
        # Une connexion qui n'a pas abouti n'a rien changé : on peut réessayer sans risque.
        if done.returncode == 0 or not any(m in done.stderr for m in NETWORK_ERRORS):
            break
        time.sleep(3 * (attempt + 1))
    if check and done.returncode != 0:
        raise Refused(f"gh {' '.join(args[:3])} a échoué : {done.stderr.strip()[:500]}")
    text = done.stdout.strip()
    try:
        return json.loads(text) if text else None
    except json.JSONDecodeError:
        return text


def api(path, method="GET", payload=None):
    return gh("api", "-X", method, "-H", f"X-GitHub-Api-Version: {API_VERSION}", path, payload=payload)


def git(*args, check=True):
    done = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, encoding="utf-8")
    if check and done.returncode != 0:
        raise Refused(f"git {' '.join(args[:3])} a échoué : {(done.stderr or done.stdout).strip()[:500]}")
    return done.stdout.strip()


_cache = {}


def repo():
    if "repo" not in _cache:
        _cache["repo"] = gh("repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner")
    return _cache["repo"]


def me():
    if "me" not in _cache:
        _cache["me"] = gh("api", "user", "-q", ".login")
    return _cache["me"]


def issue(number):
    item = api(f"repos/{repo()}/issues/{number}")
    if "pull_request" in item:
        raise Refused(f"#{number} est une pull request, pas une tâche")
    return item


def all_issues(state="open"):
    issues, page = [], 1
    while True:
        batch = api(f"repos/{repo()}/issues?state={state}&per_page=100&page={page}")
        issues += [i for i in batch if "pull_request" not in i]
        if len(batch) < 100:
            return issues
        page += 1


def blockers(number):
    return api(f"repos/{repo()}/issues/{number}/dependencies/blocked_by") or []


def blocking(number):
    return api(f"repos/{repo()}/issues/{number}/dependencies/blocking") or []


def open_blockers(number):
    return [b for b in blockers(number) if b["state"] == "open"]


def labels_of(item):
    return {label["name"] for label in item["labels"]}


def priority(item):
    for p in PRIORITIES:
        if PRIORITY_PREFIX + p in labels_of(item):
            return p
    return None


def comments(number):
    out, page = [], 1
    while True:
        batch = api(f"repos/{repo()}/issues/{number}/comments?per_page=100&page={page}")
        out += batch
        if len(batch) < 100:
            return out
        page += 1


def comment(number, body):
    return api(f"repos/{repo()}/issues/{number}/comments", "POST", {"body": body})


def add_label(number, name):
    api(f"repos/{repo()}/issues/{number}/labels", "POST", {"labels": [name]})


def remove_label(number, name):
    gh("api", "-X", "DELETE", f"repos/{repo()}/issues/{number}/labels/{urllib.parse.quote(name)}", check=False)


def status(item):
    labels = labels_of(item)
    if item["state"] == "closed":
        return "fait" if item.get("state_reason") != "not_planned" else "abandonné"
    if REVIEW in labels:
        return REVIEW
    if DOING in labels:
        return DOING + (" (bloqué)" if BLOCKED in labels else "")
    return "à faire"


def fmt(item):
    milestone = (item.get("milestone") or {}).get("title") or "sans jalon"
    areas = ", ".join(sorted(l.split(":", 1)[1] for l in labels_of(item) if l.startswith("domaine:")))
    return f"#{item['number']:<4} [{priority(item) or '--'}] {item['title']}  · {milestone}" + (f" · {areas}" if areas else "")


def slug(number, title):
    words = re.sub(r"[^a-z0-9]+", "-", title.lower().encode("ascii", "ignore").decode()).strip("-")
    return f"issue/{number}-{words[:40].rstrip('-') or 'tache'}"


def issue_branch(number):
    """Branche existante de la tâche (locale ou distante), sinon None."""
    out = git("branch", "-a", "--format=%(refname:short)", check=False)
    for name in out.splitlines():
        name = name.strip().removeprefix("origin/")
        if name.startswith(f"issue/{number}-"):
            return name
    return None


def pr_for(number):
    """PR ouverte (ou fusionnée) dont la branche est issue/N-…"""
    prs = gh("pr", "list", "--repo", repo(), "--state", "all", "--limit", "300",
             "--json", "number,url,state,headRefName,isDraft")
    prs = [p for p in prs or [] if p["headRefName"].startswith(f"issue/{number}-")]
    prs.sort(key=lambda p: (p["state"] != "OPEN", -p["number"]))
    return prs[0] if prs else None


def claimable_reason(item):
    """Pourquoi la tâche ne peut pas être prise, ou None."""
    n = item["number"]
    if item["state"] != "open":
        return f"#{n} est fermée ({status(item)})"
    labels = labels_of(item)
    if REVIEW in labels:
        return f"#{n} est en revue : elle attend la validation d'un humain"
    if DOING in labels:
        who = ", ".join(a["login"] for a in item["assignees"]) or "personne"
        return f"#{n} est déjà en cours ({who})"
    others = [a["login"] for a in item["assignees"] if a["login"] != me()]
    if others:
        return f"#{n} est assignée à {', '.join(others)}"
    if not item.get("milestone"):
        return f"#{n} n'a pas de jalon : chaque tâche doit en avoir un (demandez à votre humain lequel)"
    if not priority(item):
        return f"#{n} n'a pas d'étiquette de priorité ({', '.join(PRIORITY_PREFIX + p for p in PRIORITIES)})"
    waiting = open_blockers(n)
    if waiting:
        return f"#{n} est bloquée par " + ", ".join(f"#{b['number']} {b['title']}" for b in waiting)
    return None


def require_clean_tree():
    if git("status", "--porcelain"):
        raise Refused("des fichiers sont modifiés et non commités : commitez ou rangez-les d'abord (git status)")


def unpushed(branch):
    git("fetch", "-q", "origin")
    remote = git("rev-parse", "--verify", "-q", f"origin/{branch}", check=False)
    if not remote:
        return int(git("rev-list", "--count", f"origin/main..{branch}") or 0)
    return int(git("rev-list", "--count", f"origin/{branch}..{branch}") or 0)


# ---------------------------------------------------------------- commandes

def cmd_whoami(a):
    problems = []
    print(f"compte gh : {me()}  ·  dépôt : {repo()}")
    perms = api(f"repos/{repo()}").get("permissions", {})
    if not perms.get("push"):
        problems.append("ce compte n'a pas le droit d'écriture sur le dépôt")
    have = {l["name"] for l in api(f"repos/{repo()}/labels?per_page=100")}
    missing = [l for l in REQUIRED_LABELS if l not in have]
    if missing:
        problems.append("étiquettes manquantes : " + ", ".join(missing))
    milestones = [m["title"] for m in api(f"repos/{repo()}/milestones?state=open&per_page=100")]
    print("jalons ouverts : " + (", ".join(milestones) or "aucun"))
    if not milestones:
        problems.append("aucun jalon ouvert")
    if problems:
        for p in problems:
            print("MANQUE : " + p)
        print("Ce n'est pas à l'agent de le corriger : prévenez votre humain.")
        return 1
    print("OK : accès et configuration du tableau en place")
    return 0


def cmd_next(a):
    issues = [i for i in all_issues() if not (labels_of(i) & set(STATUS_LABELS))]
    ready, blocked, broken = [], 0, []
    for i in issues:
        reason = claimable_reason(i)
        if reason is None:
            ready.append(i)
        elif "bloquée par" in reason:
            blocked += 1
        elif "jalon" in reason or "priorité" in reason:
            broken.append(i)
    rank = {p: k for k, p in enumerate(PRIORITIES)}
    for i in sorted(ready, key=lambda i: (rank.get(priority(i), 9), (i.get("milestone") or {}).get("number", 999), i["number"])):
        print(fmt(i))
    print(f"{len(ready)} prête(s), {blocked} bloquée(s)" + (f", {len(broken)} sans jalon ou priorité (voir check)" if broken else ""))
    return 0


def cmd_show(a):
    i = issue(a.number)
    print(fmt(i))
    print(f"statut : {status(i)}  ·  assignée : {', '.join(x['login'] for x in i['assignees']) or 'personne'}")
    for b in blockers(a.number):
        print(f"  bloquée par #{b['number']} {b['title']} ({'ouverte' if b['state'] == 'open' else 'fermée'})")
    for b in blocking(a.number):
        print(f"  bloque #{b['number']} {b['title']} ({'ouverte' if b['state'] == 'open' else 'fermée'})")
    branch = issue_branch(a.number)
    if branch:
        print(f"branche : {branch}")
    pr = pr_for(a.number)
    if pr:
        print(f"PR : {pr['url']} ({pr['state'].lower()})")
    reason = claimable_reason(i)
    print("peut être prise : oui" if reason is None else f"peut être prise : non ({reason})")
    print()
    print(i["body"] or "")
    return 0


def cmd_start(a):
    i = issue(a.number)
    reason = claimable_reason(i)
    if reason:
        raise Refused(reason)
    if not a.sans_branche:
        require_clean_tree()
        git("fetch", "-q", "origin")
    n = a.number
    token = secrets.token_hex(4)
    api(f"repos/{repo()}/issues/{n}/assignees", "POST", {"assignees": [me()]})
    add_label(n, DOING)
    branch = issue_branch(n) or slug(n, i["title"])
    mine = comment(n, f"EN COURS. Sur la tâche : {a.who} · compte {me()} · branche `{branch}`\n<!-- tache-prise id={token} -->")
    # Course entre deux agents : la première prise (après le dernier rendu) gagne.
    claims = []
    for c in comments(n):
        if RELEASE_MARK in c["body"]:
            claims = []
        m = CLAIM_RE.search(c["body"])
        if m:
            claims.append(m.group(1))
    if claims and claims[0] != token:
        api(f"repos/{repo()}/issues/comments/{mine['id']}", "DELETE")
        api(f"repos/{repo()}/issues/{n}/assignees", "DELETE", {"assignees": [me()]})
        raise Refused(f"#{n} vient d'être prise par un autre agent : prise annulée, choisissez une autre tâche")
    if not a.sans_branche:
        if git("rev-parse", "--verify", "-q", branch, check=False):
            git("switch", branch)
        elif git("rev-parse", "--verify", "-q", f"origin/{branch}", check=False):
            git("switch", "-c", branch, "--track", f"origin/{branch}")
        else:
            git("switch", "-c", branch, "origin/main")
        print(f"branche : {branch}")
    print(f"#{n} en cours")
    return 0


def cmd_review(a):
    n = a.number
    i = issue(n)
    if DOING not in labels_of(i):
        raise Refused(f"#{n} n'est pas en cours ({status(i)}) : rien à envoyer en revue")
    if me() not in [x["login"] for x in i["assignees"]]:
        raise Refused(f"#{n} n'est pas assignée à {me()}")
    branch = git("branch", "--show-current")
    if not branch.startswith(f"issue/{n}-"):
        raise Refused(f"vous êtes sur « {branch} », pas sur la branche de #{n} (issue/{n}-…)")
    require_clean_tree()
    if not int(git("rev-list", "--count", f"origin/main..{branch}") or 0):
        raise Refused(f"la branche {branch} n'a aucun commit par rapport à main")
    git("push", "-u", "origin", branch)
    body = f"Closes #{n}\n\n## Ce qui a été fait\n{a.note}\n\n## Plan de test manuel\n{a.plan}\n"
    pr = pr_for(n)
    if pr and pr["state"] == "OPEN":
        gh("pr", "edit", str(pr["number"]), "--repo", repo(), "--body", body)
    else:
        gh("pr", "create", "--repo", repo(), "--base", "main", "--head", branch, "--title", f"#{n} : {i['title']}", "--body", body)
        pr = pr_for(n)
    remove_label(n, DOING)
    remove_label(n, BLOCKED)
    add_label(n, REVIEW)
    comment(n, f"EN REVUE : {pr['url']}\nFait : {a.note}\n\nPlan de test manuel :\n{a.plan}")
    print(f"#{n} en revue · {pr['url']}")
    print("Donnez le plan de test à votre humain et attendez sa validation avant « done ».")
    return 0


def cmd_rework(a):
    n = a.number
    i = issue(n)
    if REVIEW not in labels_of(i):
        raise Refused(f"#{n} n'est pas en revue ({status(i)})")
    remove_label(n, REVIEW)
    add_label(n, DOING)
    comment(n, f"À REPRENDRE : {a.note}")
    print(f"#{n} de nouveau en cours")
    return 0


def pr_checks(pr_number):
    """(bons, en attente, en échec) des contrôles CI de la PR."""
    rows = gh("pr", "checks", str(pr_number), "--repo", repo(), "--json", "name,bucket", check=False)
    if not isinstance(rows, list):
        return [], [], []
    good = [r["name"] for r in rows if r["bucket"] in ("pass", "skipping")]
    waiting = [r["name"] for r in rows if r["bucket"] == "pending"]
    bad = [r["name"] for r in rows if r["bucket"] in ("fail", "cancel")]
    return good, waiting, bad


def close_issue(n):
    """Ferme l'issue après la fusion. GitHub la ferme souvent lui-même quelques secondes après (« Closes #N ») :
    on lui laisse le temps, et si notre fermeture est refusée parce qu'il l'a fermée entre-temps, ce n'est pas
    une erreur."""
    for _ in range(5):
        if issue(n)["state"] == "closed":
            return
        time.sleep(2)
    try:
        api(f"repos/{repo()}/issues/{n}", "PATCH", {"state": "closed", "state_reason": "completed"})
    except Refused:
        if issue(n)["state"] != "closed":
            raise


def cmd_done(a):
    n = a.number
    i = issue(n)
    if REVIEW not in labels_of(i):
        raise Refused(f"#{n} n'est pas en revue ({status(i)}) : « done » vient après « review » et la validation d'un humain")
    if not a.valide_par.strip():
        raise Refused("--valide-par est obligatoire : qui a validé, et ce qu'il a vérifié")
    waiting = open_blockers(n)
    if waiting:
        raise Refused(f"#{n} est bloquée par " + ", ".join(f"#{b['number']}" for b in waiting))
    pr = pr_for(n)
    if not pr or pr["state"] != "OPEN":
        raise Refused(f"#{n} n'a pas de PR ouverte (issue/{n}-…) : relancez « review »")
    branch = pr["headRefName"]
    if git("rev-parse", "--verify", "-q", branch, check=False) and unpushed(branch):
        raise Refused(f"la branche {branch} a des commits non poussés : git push d'abord")
    good, pending, bad = pr_checks(pr["number"])
    if bad:
        raise Refused("le CI est en échec (" + ", ".join(bad) + ") : corrigez avant de fusionner")
    if pending:
        raise Refused("le CI tourne encore (" + ", ".join(pending) + ") : attendez qu'il soit vert")
    if not good:
        raise Refused("aucun contrôle CI sur la PR : attendez que le CI démarre")
    comment(n, f"{VALIDATED_PREFIX} {a.valide_par}" + (f"\nNote : {a.note}" if a.note else ""))
    gh("pr", "merge", str(pr["number"]), "--repo", repo(), "--squash", "--delete-branch")
    for label in STATUS_LABELS:
        remove_label(n, label)
    close_issue(n)
    print(f"#{n} fait · PR #{pr['number']} fusionnée")
    if git("branch", "--show-current") == branch and not git("status", "--porcelain"):
        git("switch", "main")
        git("pull", "--ff-only", "-q")
        print("retour sur main à jour")
    return 0


def cmd_block(a):
    n = a.number
    i = issue(n)
    if DOING not in labels_of(i):
        raise Refused(f"#{n} n'est pas en cours ({status(i)}) : on ne bloque qu'une tâche commencée")
    add_label(n, BLOCKED)
    comment(n, f"BLOQUÉ : {a.raison}")
    print(f"#{n} marquée bloquée")
    return 0


def cmd_release(a):
    n = a.number
    i = issue(n)
    if not labels_of(i) & {DOING, REVIEW}:
        raise Refused(f"#{n} n'est pas prise ({status(i)})")
    branch = issue_branch(n)
    for label in STATUS_LABELS:
        remove_label(n, label)
    api(f"repos/{repo()}/issues/{n}/assignees", "DELETE", {"assignees": [x["login"] for x in i["assignees"]]})
    comment(n, f"RENDUE : {a.note}" + (f"\nBranche : `{branch}`" if branch else "") + f"\n{RELEASE_MARK}")
    print(f"#{n} rendue")
    return 0


def cmd_check(a):
    problems = []
    now = datetime.now(timezone.utc)
    for i in all_issues():
        n, labels = i["number"], labels_of(i)
        if not i.get("milestone"):
            problems.append(f"#{n} sans jalon")
        if not priority(i):
            problems.append(f"#{n} sans étiquette de priorité")
        if labels & {DOING, REVIEW} and not i["assignees"]:
            problems.append(f"#{n} {status(i)} sans personne assignée")
        if DOING in labels and REVIEW in labels:
            problems.append(f"#{n} à la fois en cours et en revue")
        if BLOCKED in labels and DOING not in labels:
            problems.append(f"#{n} bloqué sans être en cours")
        if labels & {DOING, REVIEW} and open_blockers(n):
            problems.append(f"#{n} {status(i)} alors que ses dépendances sont ouvertes")
        if REVIEW in labels:
            pr = pr_for(n)
            if not pr or pr["state"] != "OPEN":
                problems.append(f"#{n} en revue sans PR ouverte")
        updated = datetime.fromisoformat(i["updated_at"].replace("Z", "+00:00"))
        if DOING in labels and (now - updated).total_seconds() > STALE_HOURS * 3600:
            problems.append(f"#{n} en cours sans activité depuis plus de {STALE_HOURS} h")
    for i in all_issues("closed")[:100]:
        if labels_of(i) & set(STATUS_LABELS):
            problems.append(f"#{i['number']} fermée mais garde une étiquette de statut")
    for p in problems:
        print("PROBLÈME : " + p)
    print("tableau cohérent" if not problems else f"{len(problems)} problème(s) : prévenez votre humain (AGENTS.md §6)")
    return 1 if problems else 0


def main():
    p = argparse.ArgumentParser(description="Tâches de POP (voir AGENTS.md)")
    s = p.add_subparsers(dest="cmd", required=True)
    s.add_parser("whoami").set_defaults(fn=cmd_whoami)
    s.add_parser("next").set_defaults(fn=cmd_next)
    s.add_parser("check").set_defaults(fn=cmd_check)
    x = s.add_parser("show"); x.add_argument("number", type=int); x.set_defaults(fn=cmd_show)
    x = s.add_parser("start"); x.add_argument("number", type=int); x.add_argument("--who", required=True)
    x.add_argument("--sans-branche", action="store_true", help="ne crée pas de branche (tâche sans code)")
    x.set_defaults(fn=cmd_start)
    x = s.add_parser("review"); x.add_argument("number", type=int); x.add_argument("--note", required=True)
    x.add_argument("--plan", required=True, help="plan de test manuel, une étape numérotée par critère")
    x.set_defaults(fn=cmd_review)
    x = s.add_parser("rework"); x.add_argument("number", type=int); x.add_argument("--note", required=True); x.set_defaults(fn=cmd_rework)
    x = s.add_parser("done"); x.add_argument("number", type=int); x.add_argument("--valide-par", required=True)
    x.add_argument("--note"); x.set_defaults(fn=cmd_done)
    x = s.add_parser("block"); x.add_argument("number", type=int); x.add_argument("--raison", required=True); x.set_defaults(fn=cmd_block)
    x = s.add_parser("release"); x.add_argument("number", type=int); x.add_argument("--note", required=True); x.set_defaults(fn=cmd_release)
    a = p.parse_args()
    try:
        return a.fn(a)
    except Refused as e:
        print(f"REFUSÉ : {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
