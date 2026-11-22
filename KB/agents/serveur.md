# Domaine serveur

Entrées sur le modèle de `KB/README.md`, la plus récente en haut. Seul l'agent de ce domaine écrit ici.

```
EN COURS | #96 Forge F22 · Serveur : vérifications automatiques du niveau | 21/11 01:00
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : checks.rs (lien cassé, réglage vide, option sans effet, tags Verse), GET /api/maps/{mid}/checks, MCP check_level, tests
Reste : test avec le vrai UEFN ; liens d'événements non lisibles par UEFN MCP
EN COURS | #85 Forge F11 · Serveur : bibliothèque de modules Verse du studio | 21/11 00:40
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : migration 0027 verse_modules, library.rs (API, rôles), MCP search_library / get_library_module, tests
Reste : validation par Kl1600
EN COURS | #93 Forge F19 · Serveur : registre des agents IA et attribution des tâches | 21/11 01:20
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : migration 0028, agents.rs (liste, claim_task, release_task, fichiers Verse par agent), journal par agent, tests
Reste : validation par Kl1600 ; fusionner #85 (migration 0027) avant
EN COURS | #113 Forge F27 · Compétences : packs de consignes UEFN et Verse lus par les IA | 21/11 01:40
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : 5 compétences POP (server/skills/), compétences du studio (migration 0029), MCP list_skills / get_skill, tests
Reste : validation par Kl1600
FAIT | #70 IA d'analyse : garder chaque prédiction, la vérifier après coup et suivre la précision jour par jour | 20/11 21:10
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : migration 0027 (ia_models, ia_predictions), model.rs (temporal_split, auc, brier, record_predictions, grade, admin_learning), onglet Admin
Reste : rien ; prochaines étapes #71 (signaux) et #72 (modèle)
Bloque : rien
```

```
FAIT | #65 Administration : rôle modérateur (liste d'attente, dépannage, consultation des comptes) | 20/11 18:20
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : migration 0026, admin::staff et security::require_staff, routes modérateur dans admin.rs, Admin.tsx et Layout.tsx
Reste : rien
Bloque : rien
```

```
FAIT | #52 Déploiement : construire l'image Docker seulement au déploiement, plus à chaque fusion sur main | 19/11 19:40
Owner : benybens | Sur la tâche : Claude Code, pour Yassine
Fait : deployer.yml construit sha-… si absente (publier-image en workflow_call), latest = image déployée ; 5 essais en prod
Reste : rien (fusion 2b78313 sans « publier l'image ») ; piège : task.py done échoue sur « git switch main » en worktree
Bloque : rien
```

```
FAIT | #46 CI : consommer moins de minutes GitHub Actions (jobs selon les dossiers touchés, jobs regroupés) | 19/11 18:10
Owner : benybens | Sur la tâche : Claude Code, pour Yassine
Fait : ci.yml par dossier, migrations + audit en 1 job, audit le lundi ; PR doc 1 min (6 avant), serveur 4, relais+web 4
Reste : voir le 1er audit du lundi 24/11 (schedule) ; règles des tâches vérifiées après fusion
Bloque : rien
```

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


### 2026-11-21 · #17 · ne plus révéler qui a un compte
- Fait : demande d'accès avec une adresse qui a un compte → même réponse `{ok:true}` qu'une adresse inconnue, rien n'est enregistré, et un e-mail « vous avez déjà un compte » part (si SMTP configuré). Inscription : « compte existe déjà » seulement après le contrôle d'accès (invitation ou lien d'accès).
- Vérifié : `cargo test` (test `demande_acces_ne_revele_pas_les_comptes`). Reste visible avec `POP_OPEN_SIGNUP=1` (inscription libre) : il faut bien dire à la personne que son adresse est prise.
### 2026-11-21 · #121 F17a · accès au stockage R2
- Fait : `server/src/s3.rs` (config `POP_S3_*`, liens signés SigV4 par chemin, sans dépendance nouvelle) ; variables dans `deploy/env.exemple` et `docker-compose.yml` (CORS du bucket, tailles max).
- Vérifié : `cargo test` (signature = exemple officiel AWS ; lien R2 sans le secret).
- Suite : #91 (API de la bibliothèque) se reprend sur `main` après fusion, sans `s3.rs` en double.

### 2026-11-21 · #130 · route en double après #96
- Fait : la route des vérifications du niveau (#96) passe à `GET /api/maps/{mid}/level-checks` (`/checks` reste la checklist de la page Lancer) ; test `routeur::routes_sans_doublon` qui construit le routeur complet.
- Vérifié : `cargo test` ; serveur démarré en local, `/checks` et `/level-checks` répondent 200.
- Leçon : `cargo test` ne construisait pas le routeur ; une route en double ne se voyait qu'au démarrage.
### 2026-11-21 · #115 F29 · interface de jeu générée (create_widget)
- Fait : `server/src/widgets.rs` (modèles texte, compteur, bouton → appareil Verse `POP_UI/pop_ui_<nom>.verse`, texte en liste blanche) ; outil MCP `create_widget` (via `write_file` : journalisé, annulable, verrou Lore) ; l'annulation est refusée si un acteur du niveau utilise encore la classe.
- Vérifié : `cargo test` (contrôles, génération, utilisation). Le Verse généré n'est vérifié qu'à la lecture : à compiler dans le vrai UEFN.
- Écart avec l'issue : pas de Widget Blueprint UMG (non pilotable depuis le Verse aujourd'hui, voir KB/UEFN.md).
### 2026-11-21 · #127 · check-in Lore obligatoire pour les tâches UEFN
- Fait : migration 0033 (`tasks.uefn`) ; `tasks::set_status_checked` exige un message « #N … » pour fermer une tâche UEFN, repris du `lore_checkin` de l'agent qui l'a réservée ; case et fenêtre dans Tâches ; MCP `uefn`, `checkin`.
- Vérifié : `cargo test`, navigateur en local. Pas avec le vrai UEFN.
- Leçon : sous Windows, `web/dist/index.html` sort en CRLF ; le commiter en LF sinon le CI échoue.
### 2026-11-22 · #157 · l'assignation à l'initiative des membres
- Fait : `PATCH /api/tasks/:id` lit `null` comme « vider » (`double_option`) pour `assignee_id` et `planned_day` ; assigner quelqu'un d'autre ou retirer la tâche d'un autre est réservé aux propriétaires et admins (aussi à la création : `check_assignee`) ; le journal de la tâche note « X a pris #n », « X a rendu #n » ou « X a assigné #n à Y ». Le plan généré n'assigne plus personne, et une map passée en perso libère les tâches des autres au lieu de les réattribuer. Boussole : « tâches à prendre ». Web : « Je la prends » et « Je la rends » ; un membre simple ne voit plus la liste des autres.
- Vérifié : `cargo test` (5 tests `tests_assignation`), navigateur en local avec une propriétaire et une membre (plan, fiche, déplacement vers le perso).
- Leçon : axum abandonne le handler quand le client coupe la requête (changement de page). Un `UPDATE` suivi d'un `INSERT` dans le journal doit passer dans une transaction, sinon le changement reste sans trace.
