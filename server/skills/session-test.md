# Tester en session et corriger en boucle

1. `uefn_status` : UEFN doit être « connecté ». Sinon, demandez à l'humain d'ouvrir le projet et de démarrer le serveur MCP d'UEFN.
2. `start_session` (peut prendre plusieurs minutes), une seule fois.
3. Après une modification du Verse : `compile_verse`, puis `push_to_session` ; l'envoi Verse seul relance les appareils Verse dans la même partie (OnBegin rejoué).
4. `session_output` : lisez les `Print` et les erreurs ; passez `cursor` (renvoyé par l'appel précédent) pour ne lire que la suite, `pattern` pour filtrer.
5. Une modification d'acteur ou de réglage demande un envoi complet (plus long) : groupez vos changements avant d'envoyer.
6. `stop_session` quand vous avez fini.
7. Notez ce que vous avez vérifié dans la tâche (`comment_task`) avant de la passer en `done`.
