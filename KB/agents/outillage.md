# Domaine outillage

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

```
EN REVUE | #185 Discord : messages automatiques pour les issues et les déploiements | 22/11
Owner : benybens | Sur la tâche : Claude Code, pour benybens
Fait : tools/discord.py (webhooks, notes de version depuis git), action « discord : issues »,
       job « notes de version et Discord » dans « déployer » (release deploy-AAAA-MM-JJ-<sha>), deploy/README.md §4.2
Reste : secrets DISCORD_WEBHOOK_ISSUES et DISCORD_WEBHOOK_DEPLOIEMENTS (propriétaire du dépôt), essai réel
Bloque : rien
Piège : sans User-Agent explicite, Discord refuse parfois urllib ; ne jamais afficher l'URL d'un webhook dans un journal
```
