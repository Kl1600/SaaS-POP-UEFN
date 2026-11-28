# Nouveautés de POP Forge

Une entrée par version de `forge/tauri.conf.json` (#437) : `## version · date`, puis 1 à 5 lignes en français.
Le workflow « relais Windows » en tire les notes de la mise à jour ; le CI refuse une version sans entrée.

## 0.3.21 · 28/11/2026
- Même thème que le site : barre latérale avec catégories repliables, onglets à réordonner en tenant ⋮⋮.
- Didacticiel à la première visite de chaque écran de Forge (Accueil, Agents, Terminal, Outils, Vérifications, Plan 2D, Bibliothèque).
- Offre Pro présentée comme sur le site : prix par jour, choix annuel, réassurance.

## 0.3.20 · 28/11/2026
- Outils → Texte 3D : une vraie 3D qu'on tourne à la souris ; épaisseur, biseau, hauteur, couleur et police changent en direct, puis import dans UEFN aux mêmes réglages.
- Outils → Texte 2D : à part, en image transparente. Nouvelle police libre : Anton.
- 3D Studio retiré des Outils : demandez un modèle 3D directement aux agents du projet.

## 0.3.19 · 28/11/2026
- Sécurité : la décompression de Blender utilise une version corrigée de la bibliothèque zip.

## 0.3.18 · 28/11/2026
- Outils → 3D Studio : décrivez un objet à votre IA ; elle le construit avec Blender sur votre PC, vous voyez l'aperçu et vous l'acceptez avant tout import dans UEFN.

## 0.3.17 · 28/11/2026
- Outils → Low Mesh : allégez un modèle 3D (FBX, OBJ, GLB) avant import, avec aperçu et nombre de triangles avant et après ; Forge prévient au-delà de 30 000 sommets.

## 0.3.16 · 28/11/2026
- Outils → Texte 3D et 2D : un titre, un panneau ou un logo en mesh 3D (FBX) ou en image transparente (PNG), fait par Blender sur votre PC, avec aperçu, puis importé dans UEFN en un clic.

## 0.3.15 · 28/11/2026
- Outils → Blender : installez Blender (logiciel libre) une seule fois, si vous le voulez ; il servira aux outils 3D. Ses scripts tournent isolés : sans Internet, sans accès à vos fichiers, 2 minutes au plus.

## 0.3.14 · 28/11/2026
- Dictée : le texte n'est plus perdu (la transcription rendait souvent un texte vide) ; le texte provisoire est gardé.
- Dictée : niveau et nom du micro affichés pendant qu'on parle ; si le micro ne capte rien, Forge dit quoi régler dans Windows.

## 0.3.13 · 28/11/2026
- Terminal en direct : si Claude Code n'est pas connecté à votre compte, Forge le dit et vous aide à le connecter, puis à le relancer.

## 0.3.12 · 28/11/2026
- Dictée bien plus rapide : le texte suit la voix et le texte final arrive dès le relâchement.

## 0.3.11 · 27/11/2026
- La dictée vocale ne ferme plus Forge : reconnaissance vocale compilée pour tous les PC récents.

## 0.3.10 · 27/11/2026
- Dictée en temps réel : le texte s'écrit pendant que vous parlez ; au relâchement de Ctrl+Espace, le texte final reste.

## 0.3.9 · 27/11/2026
- Terminal en direct : « + Terminal » → Claude Code tourne dans Forge ; votre message lui arrive tout de suite et sa réponse s'écrit en direct.

## 0.3.8 · 27/11/2026
- Installateur et désinstalleur aux couleurs de POP (logo, images, en français).
- L'outil Miniature et le terminal de l'agent ont chacun leur discussion.

## 0.3.7 · 27/11/2026
- « Fenêtre à part » d'un terminal ne bloque plus Forge.
- « + Terminal » propose seulement les IA connectées à la map.

## 0.3.6 · 27/11/2026
- « + Terminal » ouvre une IA sans choisir de tâche : donnez-lui vos instructions dans son terminal.

## 0.3.5 · 27/11/2026
- Agents : les terminaux ouverts remplissent la page (plein écran, moitiés, quarts) ; × en ferme un.
- Chaque terminal peut s'ouvrir dans une fenêtre à part.
- « + Terminal » en haut à gauche ouvre une IA connectée ou en lance une sur votre PC.

## 0.3.4 · 27/11/2026
- Dictée vocale : gardez Ctrl+Espace appuyé dans un champ pour dicter, transcrit sur votre PC (aucun son ne quitte l'ordinateur).
- Le modèle de reconnaissance (547 Mio) se télécharge au premier usage, après votre accord ; raccourci modifiable dans Réglages.

## 0.3.3 · 27/11/2026
- La version de Forge est visible dans le menu du profil et dans Réglages.
- Réglages → « Vérifier les mises à jour » et liste des nouveautés de chaque version.
- Le diagnostic à copier contient la version de Forge.

## 0.3.2 · 27/11/2026
- Un nouveau projet UEFN est détecté et relié tout seul, sans toucher aux autres liens.

## 0.3.1 · 26/11/2026
- En reliant UEFN, chaque projet va dans son espace : en solo, ou dans le studio de son équipe Creator.

## 0.3.0 · 26/11/2026
- Mises à jour de Forge proposées dans Forge et installées en un clic.
- Plan 2D et agents en mini-terminaux dans Forge.
