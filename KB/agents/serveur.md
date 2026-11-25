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
EN COURS | #91 Forge F17 · Serveur : bibliothèque d'assets dans un stockage objet à part | 21/11 02:10
Owner : Kl1600 | Sur la tâche : Claude Code, pour Kl1600
Fait : migration 0030, assets.rs (envoi direct vers R2 par lien signé SigV4, quotas, droits), variables deploy/, tests
Reste : créer le bucket R2 et sa clé (humain), test réel
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
### 2026-11-22 · #160 · une dépendance = impossible à faire ou tester sans
- Fait : la définition est écrite dans l'outil MCP `create_task` (description et `depends_on`), dans la consigne du plan IA (`aiplan.rs`, « after ») et dans le web (`DEP_HELP` de `Tasks.tsx`, sous « Dépend de » dans la fiche, la création de tâche et l'aperçu du plan). 8 dépendances des squelettes de `plan.rs` retirées parce qu'elles suivaient l'ordre du jeu ; les tests de fin de lot dépendent maintenant directement de tout ce qu'ils testent.
- Vérifié : `cargo test` (`tests_dependances` dans `mcp.rs` et `plan.rs`), navigateur en local en français et en anglais.

```
FAIT | #161 Tâches UEFN : check-in Lore vérifié, un check-in pour plusieurs tâches | 22/11 18:00
Owner : benybens | Sur la tâche : Claude Code, pour benybens
Fait : checkin accepté seulement s'il est archivé sur la map ; « #3 #5 … » ferme les tâches UEFN en cours citées ; check-in d'une autre tâche = « Rattachée » ; webhook Lore compté
Reste : test avec un vrai check-in Lore (accord humain)
Bloque : rien
```
### 2026-11-22 · #145 · un jeton MCP par compte, pas par map
- Fait : migration 0034 (`api_tokens.map_id` facultatif = map de travail, `ON DELETE SET NULL`) ; outils MCP `list_maps` et `use_map` (permis en lecture seule) ; appartenance vérifiée à chaque appel ; `use_map` libère une tâche réservée dans l'autre map. Routes `/api/me/tokens` (les routes de jetons par map sont retirées). Web : Paramètres → IA connectées (onglet de #205) gère les jetons du compte et montre les IA de la map ouverte.
- Vérifié : `cargo test` (7 tests `tests_compte`, dont la migration sur des données d'avant), navigateur en local, `claude mcp add` réel puis révocation.
- Leçon : sqlx 0.8 exécute chaque migration SQLite dans une transaction (`-- no-transaction` ignoré) : impossible d'y couper les clés étrangères, et `DROP TABLE` déclenche les `ON DELETE` des tables filles. Garder les colonnes filles de côté et les remettre.

### 2026-11-23 · #274 · e-mails en HTML aux couleurs de POP
- Fait : `mail::send` envoie texte brut + HTML (`multipart/alternative`). `mail::render_html` construit le HTML depuis le texte : bandeau noir avec `logo.png` (servi par le site), carte blanche, titre = objet sans « POP · », une adresse seule sur sa ligne devient un bouton orange (libellé selon la page : `/reinitialiser`, `/activer`, `/invitation`…), lien de secours en bas, signature en gris, tout échappé. Textes harmonisés (signature, lien sur sa propre ligne).
- Convention : pour un nouvel e-mail, mettre le lien d'action **seul sur sa ligne** et finir par « L’équipe POP » ; ajouter son chemin dans `button_label` si besoin.
- Vérifié : 3 tests `tests_mail`, rendu navigateur ordinateur et téléphone.

### 2026-11-24 · #285 · connexion OAuth des IA au serveur MCP
- Fait : `server/src/oauth.rs` selon la spécification d'autorisation MCP 2025-06-18 : `/.well-known/oauth-protected-resource[/mcp]` (RFC 9728), `/.well-known/oauth-authorization-server` (RFC 8414), `/oauth/register` (RFC 7591, clients publics, 20/h par IP), `/oauth/authorize` (vérifie puis renvoie vers la page web `/autoriser`), `/oauth/token` (code + PKCE S256 obligatoire, `resource` RFC 8707, refresh avec rotation et révocation si un ancien refresh revient), `/oauth/revoke` (RFC 7009). 401 de `/mcp` avec `WWW-Authenticate: Bearer resource_metadata=…`. CORS `*` sans cookie sur ces seuls points (`cors_public`), CSRF levé sur `/oauth/token|register|revoke`. Migration 0036.
- Convention : un jeton d'accès OAuth **est une ligne `api_tokens`** (`expires_at` 1 h, `oauth_client`) : même liste, lecture seule, corbeille (qui coupe aussi le refresh par cascade). Les clés classiques ont `expires_at` NULL et restent valables. Ménage horaire dans `backup_loop` (`oauth::cleanup`).
- Web : page `/autoriser` (Authorize.tsx, avant onboarding/paywall ; si déconnecté, la page de connexion s'affiche sur la même adresse puis revient) ; Paramètres → IA connectées : bloc « collez cette adresse », cartes renommées « Clé API ».
- Vérifié : 4 tests `tests_oauth` ; en local avec Claude Code 2.1.331 (`claude mcp add --transport http` puis `claude mcp login` → « Connected ») et le client OAuth du SDK officiel MCP 1.32.2 (outils listés, `list_maps` appelé, lecture seule respectée : 16 outils au lieu de 39).
- Piège : `claude mcp login --no-browser` exige un vrai terminal (pty) et affiche l'adresse avec un lien OSC 8 (la couper avant `\x1b`).

### 2026-11-24 · #111 F25 · rôles des agents IA
- Fait : migration 0037 (`api_tokens.role`), `server/src/roles.rs` (7 rôles : consignes + outils permis ; sans rôle = tous les outils), MCP : consignes du rôle à `initialize`, `tools/list` filtré, refus clair à `tools/call` ; routes `POST /api/tokens/{id}/role`, `GET /api/agent-roles` ; choix du rôle dans Paramètres → IA connectées.
- Vérifié : `cargo test` (`roles::droits`, `mcp::roles_des_agents`) ; en local avec curl (refus du Relecteur, consignes) et capture de la liste « Vos IA ».
- Choix : le Relecteur et l'Analyste gardent `comment_task` (c'est leur façon de rendre leur avis) ; aucun autre outil qui modifie.

### 2026-11-24 · #114 F28 · reprise d'une tâche interrompue
- Fait : migration 0038 (`task_progress`), outil MCP `save_progress` (fait, reste, fichiers), note rendue par `claim_task` et `next_task` (`ta_tache`, `a_reprendre`) ; une réservation sans aucun appel MCP depuis 2 h se libère seule avec une note automatique (vérifié à la demande : claim_task, next_task, liste des agents ; pas de tâche de fond).
- Vérifié : `cargo test` (`agents::reprise_d_une_tache`) et scénario MCP réel en local (agent A note puis libère, agent B reprend et reçoit la note).

### 2026-11-24 · #298 · fil d'activité des agents et temps réel avec le kanban
- Fait : migration 0039 (`agent_calls`, sans arguments, gardés 7 jours) ; chaque appel MCP d'un agent sur sa map est noté et poussé sur le WebSocket de la map (`{"t":"agent"}`) ; `GET /api/maps/{mid}/agents/activity` ; étape de la tâche dans `/agents` (écrit, compilé, testé, relu) ; `claim_task`, `release_task`, `save_progress` écrivent dans l'historique (via « IA ») et rafraîchissent le kanban ; notifications : compilation qui passe en échec, tâche bloquée par un agent, validation demandée (texte et lien vers Paramètres → UEFN Link, plus l'Organiseur retiré).
- Vérifié : `cargo test` (`agents::fil_d_activite`, `mcp::next_task_respecte_les_dependances`) ; `next_task` respectait déjà les dépendances (`guide.rs`).

### 2026-11-24 · #112 F26 · Orchestrateur : découper une demande et répartir les tâches
- Fait : migration 0042 (`tasks.agent_id` : agent à qui la tâche est attribuée) ; `server/src/orchestre.rs` ; outils MCP `plan_tasks` (jusqu'à 30 tâches d'un coup, dépendances par numéro ou par `key` d'une tâche plus haut, agent par identifiant ou nom ; tout est vérifié avant de créer), `assign_task` (agent ou libre ; refus si un autre agent travaille dessus), `list_agents` (tous les rôles) ; `next_task` rend `attribuees` (avec ce qu'elles attendent) ; `claim_task` refuse une tâche attribuée à un autre agent ; `list_tasks` et `GET /api/maps/{mid}/agents` disent l'agent attribué et l'Orchestrateur. Historique de la tâche : « Attribuée à l'agent … par l'Orchestrateur », via « IA ».
- Règle (décision du 24/11) : Orchestrateur = l'agent qui a le rôle « Orchestrateur » (le plus récemment actif), sinon le premier agent Claude lancé, sans rôle, actif depuis moins de 2 h ; s'il s'arrête, le Claude suivant reprend ; sinon personne (Forge demandera, #94). L'Orchestrateur par défaut garde tous ses outils et reçoit en plus `plan_tasks` / `assign_task` et une consigne à `initialize`. Un agent sans rôle qui n'est pas l'Orchestrateur ne voit pas ces deux outils.
- Vérifié : `cargo test` (`mcp::orchestrateur_planifie_et_attribue`, `mcp::qui_est_orchestrateur`, `roles::droits`) par le vrai point d'entrée `/mcp`.

### 2026-11-24 · #147 · tableau blanc, documentation et jalons depuis le MCP
- Fait : outils MCP `list_boards`, `get_board`, `create_board`, `upsert_board_objects` (200 objets, types de `boards::KINDS`, texte ≤ 2 000, image ≤ 200 Ko, `stroke` refusé, id généré si absent ; les refus sont comptés et expliqués), `delete_board_objects` ; `list_pages`, `read_page` (Markdown), `write_page` (Markdown → blocs par `pages::from_markdown` ; une version gardée à chaque écriture d'IA) ; `list_milestones`, `add_milestone`. Tout passe par les fonctions des routes web (mêmes contrôles) ; `boards::apply_op` rend le message diffusé.
- Rôles : les lectures sont dans `COMMUNS` et `READ_TOOLS` ; les écritures (`CONCEPTION`) sont données à l'Orchestrateur, et à tout agent sans rôle.
- Vérifié : `cargo test` (`mcp::conception_par_l_ia` : diffusion WebSocket, modification, retrait, autre map refusée, versions de page, jalon, lecture seule).

### 2026-11-24 · #152 · « Aucun projet UEFN relié » au lieu de « Relais hors ligne »
- Fait : `uefn::statut` (état `unlinked` quand la map n'a ni relais appairé ni projet connu ; `offline` reste pour une map reliée), utilisé par `GET /api/maps/{mid}/uefn` (sans `updated_at` pour une map jamais reliée, et `relay_elsewhere` si un relais du studio est connecté), `uefn_status` (MCP, avec une phrase) et Popy ; l'appairage prévient l'interface (événement `uefn`/`status`). Web : pastille grise « Aucun projet UEFN relié », encadré qui explique quoi faire, code d'appairage ouvert d'office.
- Vérifié : `cargo test` (`uefn::tests_statut::trois_cas`, `mcp::statut_map_non_reliee`) ; Chrome sans tête avec le vrai pop-link (pair puis run, sans UEFN) : « Aucun projet UEFN relié » → « UEFN déconnecté » → « UEFN fermé » sans recharger, message « Votre relais est connecté… » sur une autre map du studio, « UEFN déconnecté » après l'arrêt du relais.

### 2026-11-24 · #66 · journal des actions des admins et modérateurs
- Fait : `GET /api/admin/journal` (admins seulement ; filtres `who` et `type` : attente, comptes, annonces, sécurité ; 50 par page, `more`) qui lit `audit_log` (actions `admin_*` et `annonce_*`), libellé lisible et compte ou demande visé ; nouvelles lignes : demande remise en attente, note sur une demande, crédits IA donnés ; refus et suppression d'une demande notent l'adresse au lieu du numéro. Web : onglet Administration → Journal (filtres, pages).
- Vérifié : `cargo test` (`admin::journal_tests`) ; Chrome sans tête (onglet Journal après une invitation).

### 2026-11-24 · #316 · sources X et Reddit pour l'analyse
- Fait : `analytics.rs` : `collect_reddit` (API officielle, identifiants d'application `client_id` + `key`, jeton « client_credentials », r/FortniteCreative + recherches de la semaine) et `collect_x` (recherche récente v2 avec le Bearer de l'utilisateur, 3 recherches de 50 au plus, seulement avec la collecte TikTok car X facture chaque message lu). Rangés dans `tt_videos` (plateformes `reddit` et `x`), donc pris en compte par les tendances et le badge `tendance_video` des niches. `sources_on` donne l'état des 4 sources ; intégrations `reddit` et `x` ajoutées dans `studio.rs` (secrets masqués). Variables de test : `POP_REDDIT_AUTH`, `POP_REDDIT_API`, `POP_X_API`.
- Pièges : Reddit ne délivre plus de nouveaux accès API depuis le 31/10/2026 (seuls les identifiants déjà approuvés marchent) ; X n'a plus d'accès gratuit en lecture (paiement à l'usage, environ 0,005 $ par message lu).
- Vérifié : test `tests_sources` (lecture des réponses Reddit et X) ; collecte de bout en bout contre des serveurs simulés.

### 2026-11-25 · #375 · une seule adresse MCP pour toutes les IA
- Fait : ancien transport HTTP + SSE (`GET /sse`, `POST /messages?session_id=…`, `mcp_sse.rs`) qui passe par le même traitement que `POST /mcp` ; `/.well-known/oauth-protected-resource/sse` et `resource` accepté pour `/sse` ; CORS et CSRF ouverts pour ces adresses comme pour `/mcp`. Types d'IA `chatgpt`, `lechat`, `gemini_cli` (`kind_of` : ChatGPT/OpenAI → chatgpt, Codex → codex, Mistral/Le Chat → lechat). `tools/list` ajoute les annotations MCP standard (`readOnlyHint`, `destructiveHint`).
- Vérifié avec de vrais clients sur un serveur local : Claude Code 2.1 (OAuth, `claude mcp login`, « Connected »), Codex CLI 0.172 (OAuth, `codex mcp login`, son jeton appelle list_maps), Gemini CLI 0.71 (clé en en-tête, « Connected »), SDK MCP TypeScript officiel (OAuth complet + Streamable HTTP + SSE avec clé). Révocation : 401 tout de suite, refresh supprimés en cascade.
- Piège : `codex mcp add --url` lance seul la connexion OAuth quand le serveur la propose (il attend le navigateur) ; Claude Code refuse `mcp login` hors terminal interactif.

### 2026-11-25 · #384 · crédits IA retirés
- Fait : `aipop.rs` et `aiplan.rs` supprimés (IA de POP payante et plan de production par l'IA) avec leurs routes (`/api/maps/{mid}/ai/plan`, `/api/admin/ai`, don de crédits) ; Stripe : plus de packs (`POP_CREDIT_PACKS` inutile), plus d'achat `kind: credits`, plus de crédit au webhook ; `/api/me` et `/api/billing` ne renvoient plus `ai_credits`. La colonne `ai_credits` et la table `ai_ledger` restent (migrations jamais modifiées). Crédits de playtest inchangés. Décision du 25/11 dans KB/DECISIONS.md, AGENTS.md §5 à jour.
- Vérifié : cargo test (122 ok) ; serveur local : route du plan IA inconnue (405), plus de `ai_credits`.
