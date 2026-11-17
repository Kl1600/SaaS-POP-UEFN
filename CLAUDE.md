@AGENTS.md

## Notes pour Claude Code
- Les règles ci-dessus sont obligatoires. Commencez chaque session par `python tools/task.py whoami`, puis `next`.
- Votre mémoire automatique est locale à un PC et n'est **pas** partagée : ce que les autres agents doivent savoir
  va dans ce dépôt (`KB/DECISIONS.md`, `KB/UEFN.md`, `AGENTS.md`).
- Le serveur MCP d'UEFN (`http://127.0.0.1:8000/mcp`) ne répond que si UEFN est ouvert avec son serveur MCP démarré,
  et seulement depuis le PC où il tourne.
