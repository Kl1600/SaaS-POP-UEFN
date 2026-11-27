# Domaine outillage

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

```
FAIT | #253 CI et déploiement : vérification complète, base saine | 23/11 20:20
Owner : benybens | Sur la tâche : Claude Code, pour benybens
Fait : base = d0ebd9c ; CI 45255357594 vert (7 jobs) ; déployer 45256109076 vert (4 jobs, Discord 204)
Reste : rien ; RUNNER absente (dépôt public, minutes gratuites) ; cache sous 10 Go
Bloque : rien
```

```
FAIT | #185 Discord : messages automatiques pour les issues et les déploiements | 22/11 19:30
Owner : benybens | Sur la tâche : Claude Code, pour benybens
Fait : tools/discord.py (webhooks, notes de version depuis git), action « discord : issues »,
       job « notes de version et Discord » dans « déployer » (release deploy-AAAA-MM-JJ-<sha>), deploy/README.md §4.2.
       Vérifié en réel : release deploy-2026-11-22-7809574, messages de réussite et d'échec dans #deploiements,
       ouverture et fermeture d'issues dans #issues (issue de test #219).
Reste : rien
Bloque : rien
Piège : les actions d'issues ne tournent que depuis main (impossible à tester avant la fusion) ; « déployer » lancé
        depuis une branche utilise les workflows de cette branche (runs-on compris).
Piège : le runner local (KB/RUNNER.md) doit avoir `gh`, installé depuis cli.github.com (celui d'Ubuntu 22.04 est
        trop vieux) : sans lui, « déployer » échoue à « version à déployer » (gh: command not found).
Piège : sans User-Agent explicite, Discord refuse parfois urllib ; ne jamais afficher l'URL d'un webhook dans un journal
```
