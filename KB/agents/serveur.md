# Domaine serveur

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

```
EN COURS | #42 Administration : créer un compte qui s'active par e-mail (la personne choisit son mot de passe) | 19/11 14:55
Owner : benybens | Sur la tâche : Claude Code, pour Yassine
Fait : migration 0025, activation.rs (lien 7 j, renvoi), connexion refusée avant activation, page /activer, Administration
Reste : validation humaine
Bloque : rien
```

```
FAIT | #8 Formater le code Rust avec rustfmt et le vérifier dans le CI | 19/11 14:10
Owner : benybens | Sur la tâche : Claude Code, pour Yassine
Fait : cargo fmt (server/, link/), cargo fmt --check dans ci.yml (serveur, relais)
Reste : rien (CI testé avec un commit mal formaté)
Bloque : rien
```

```
EN COURS | #38 Administration : dépanner un compte (nouveau mot de passe, débloquer, couper la double authentification, supprimer) | 19/11 11:20
Owner : benybens | Sur la tâche : Claude Code, pour benybens
Fait : routes /api/admin/users/{id}/…, reset lève le blocage, manage::deletion_plan partagé avec delete_me
Reste : validation humaine (plan de test de la PR)
Bloque : rien
```

```
FAIT | #11 Sécurité : vérifier l'adresse e-mail avant d'activer un compte et de donner le rôle admin | 19/11 13:30
Owner : benybens | Sur la tâche : Claude Code, pour Yassine
Fait : migration 0024 (ex-0012, renumérotée au rebase du 19/11), verify.rs, is_admin exige une adresse vérifiée, page /verifier-email et bandeau
Reste : rien (validé le 19/11 avec un vrai SMTP Gmail)
Bloque : rien
```

```
EN COURS | #12 Sécurité : lire l'adresse IP du client de façon sûre et ne plus donner de droits selon l'IP | 17/11 15:10
Owner : benybens | Sur la tâche : Claude Code, pour Yassine
Fait : client_ip (security.rs) selon POP_TRUST_PROXY, plus d'admin selon l'IP, commande pop-server admin
Reste : validation humaine (plan de test de la PR), puis test derrière un vrai proxy à la mise en ligne
Bloque : rien
```

