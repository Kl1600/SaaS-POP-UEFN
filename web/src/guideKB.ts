// Base de connaissances de Boussole : comment utiliser POP. Pas d'IA : on compare les mots de la question
// (sans accents, au singulier, tolérant aux fautes de frappe) avec les mots-clés de chaque fiche.
export type Fiche = {
  id: string
  q: string                     // question type, affichée
  k: string                     // mots-clés et synonymes (séparés par des espaces)
  a: string                     // réponse courte
  steps?: string[]
  go?: [string, string]         // [route, libellé du bouton]
  pages?: string[]              // pages où la fiche est proposée en priorité
  action?: 'today'              // réponse calculée : la journée de l'utilisateur
}

export const KB: Fiche[] = [
  // ----- Journée -----
  { id: 'today', q: 'Qu’est-ce que je fais maintenant ?', k: 'quoi faire maintenant aujourd hui journee priorite suivante prochaine conseil retard urgent par ou commencer', a: 'Voici votre journée et les tâches que je vous conseille.', action: 'today' },
  // ----- Tâches -----
  { id: 'task-new', q: 'Comment créer une tâche ?', k: 'creer ajouter nouvelle tache todo ticket', a: 'Le bouton orange « Nouvelle tâche » est en haut de chaque page.', steps: ['Cliquez sur « Nouvelle tâche » en haut à droite.', 'Donnez un titre court : une tâche doit tenir en une heure.', 'Choisissez l’étiquette, la personne, le jour prévu et, si besoin, les numéros des tâches dont elle dépend.', 'Cliquez sur « Créer la tâche ».'], go: ['/taches', 'Ouvrir les tâches'], pages: ['/taches', '/'] },
  { id: 'task-status', q: 'Comment changer le statut d’une tâche ?', k: 'statut deplacer colonne en cours fait termine finir bloque kanban glisser cocher valider', a: 'Glissez la carte d’une colonne à l’autre dans le Kanban (À faire, En cours, Bloqué, Fait), ou ouvrez la tâche et changez son statut.', steps: ['Ouvrez Tâches → Kanban.', 'Glissez la carte dans la bonne colonne.', 'Depuis la Vue d’ensemble, cochez la case pour la marquer faite ; « Annuler » la remet comme avant.'], go: ['/taches', 'Ouvrir le Kanban'], pages: ['/taches', '/'] },
  { id: 'task-deps', q: 'Pourquoi je ne peux pas commencer une tâche ?', k: 'dependance depend attend bloque commencer impossible pourquoi verrouille debloquer', a: 'Une tâche ne peut passer « En cours » que lorsque toutes les tâches dont elle dépend sont faites. Elle se débloque toute seule ensuite.', steps: ['Ouvrez la tâche : la section dépendances montre ce qu’elle attend.', 'Terminez (ou faites terminer) ces tâches d’abord.', 'Pour retirer une dépendance, modifiez le champ « Dépend de ».'], go: ['/taches', 'Voir les tâches'], pages: ['/taches'] },
  { id: 'task-assign', q: 'Comment assigner une tâche à quelqu’un ?', k: 'assigner attribuer donner personne membre responsable prendre tache', a: 'Ouvrez la tâche et choisissez la personne. Pour vous la donner directement, utilisez « Prendre » dans Boussole.', steps: ['Cliquez sur la tâche pour l’ouvrir.', 'Choisissez la personne dans « Assigné ».', 'Ou, dans Boussole → Mon jour, cliquez sur « Prendre » : la tâche est à vous et passe en cours.'], go: ['/taches', 'Ouvrir les tâches'], pages: ['/taches'] },
  { id: 'task-delete', q: 'Comment supprimer une tâche ?', k: 'supprimer effacer retirer tache doublon', a: 'Ouvrez la tâche, puis « Supprimer la tâche » tout en bas de sa fiche (réservé aux admins du studio).', go: ['/taches', 'Ouvrir les tâches'], pages: ['/taches'] },
  { id: 'task-comment', q: 'Comment commenter ou mentionner quelqu’un ?', k: 'commenter commentaire mention mentionner arobase @ prevenir notifier message', a: 'Dans une tâche ou une page, écrivez @ suivi du prénom : la personne reçoit une notification (la cloche en haut).', pages: ['/taches', '/pages'] },
  // ----- Planifier -----
  { id: 'plan-gen', q: 'Comment générer le plan de ma map ?', k: 'generer generateur plan planning planifier decouper lot projet organisation', a: 'Le générateur découpe votre map en lots et en tâches d’une heure, avec dépendances, jours et personnes suggérés.', steps: ['Ouvrez Planifier.', 'Répondez aux étapes du « Générateur de projet » puis cliquez sur « Suivant ».', 'Vérifiez l’aperçu, puis cliquez sur « Générer le plan ».'], go: ['/planifier', 'Ouvrir Planifier'], pages: ['/planifier'] },
  { id: 'plan-milestone', q: 'Comment ajouter un jalon ou une date de sortie ?', k: 'jalon date sortie deadline echeance milestone beta release objectif', a: 'Les jalons se gèrent dans Planifier → Jalons. Boussole vous dit ensuite si vous êtes dans les temps.', go: ['/planifier', 'Ouvrir Planifier'], pages: ['/planifier', '/'] },
  { id: 'plan-late', q: 'Est-ce qu’on est en retard ?', k: 'retard en retard rythme temps jalon avance estimation finir a temps', a: 'Boussole compare les tâches à finir avant le prochain jalon avec votre rythme des 14 derniers jours.', action: 'today' },
  // ----- Pages -----
  { id: 'page-new', q: 'Comment créer une page ?', k: 'creer nouvelle page document doc note wiki ecrire', a: 'Ouvrez Pages puis « Nouvelle page ».', steps: ['Ouvrez Pages.', 'Cliquez sur « Nouvelle page ».', 'Tapez « / » pour insérer un bloc, « # » pour un titre, « - » pour une liste, « [] » pour une case à cocher.'], go: ['/pages', 'Ouvrir Pages'], pages: ['/pages'] },
  { id: 'page-delete', q: 'Comment supprimer ou retrouver une page ?', k: 'supprimer effacer page corbeille restaurer retrouver perdue', a: 'Survolez la page dans l’arborescence, ouvrez le menu « … » puis Supprimer. Elle va dans la Corbeille (en bas de l’arborescence), d’où vous pouvez la restaurer.', go: ['/pages', 'Ouvrir Pages'], pages: ['/pages'] },
  { id: 'page-share', q: 'Comment partager une page ?', k: 'partager page lien public prive equipe lecture', a: 'Une page peut être privée, visible par l’équipe, ou partagée par lien public en lecture.', go: ['/pages', 'Ouvrir Pages'], pages: ['/pages'] },
  { id: 'page-history', q: 'Comment revenir à une ancienne version d’une page ?', k: 'historique version ancienne restaurer annuler revenir page', a: 'Chaque version d’une page est gardée : ouvrez son historique et restaurez la version voulue.', go: ['/pages', 'Ouvrir Pages'], pages: ['/pages'] },
  // ----- Tableau blanc -----
  { id: 'board-new', q: 'Comment créer un tableau blanc ?', k: 'creer nouveau tableau blanc whiteboard miro board dessiner', a: 'Ouvrez Tableau blanc puis « Nouveau tableau » (vide ou à partir d’un modèle).', go: ['/tableau', 'Ouvrir le tableau blanc'], pages: ['/tableau'] },
  { id: 'board-format', q: 'Comment mettre en gras, en couleur ou souligner sur le tableau ?', k: 'gras couleur souligne italique barre police taille texte mise en forme couleur fond bordure tableau', a: 'Sélectionnez un objet : une barre de mise en forme apparaît au-dessus (police, taille, gras, couleurs, fond, bordure…). Double-cliquez pour ne mettre en forme qu’un morceau du texte.', steps: ['Cliquez sur l’objet.', 'Utilisez la barre au-dessus : B gras, I italique, U souligné, couleur du texte, couleur de fond, bordure.', 'Raccourcis : Ctrl+B, Ctrl+I, Ctrl+U.'], go: ['/tableau', 'Ouvrir le tableau'], pages: ['/tableau'] },
  { id: 'board-zoom', q: 'Comment zoomer ou me déplacer sur le tableau ?', k: 'zoom zoomer dezoomer deplacer naviguer molette trackpad pincer tout voir plein ecran', a: 'Molette ou pincement du trackpad pour zoomer, glisser à deux doigts pour vous déplacer. « Tout voir » (Ctrl+0) recadre sur tout le contenu, « Plein écran » agrandit le tableau.', go: ['/tableau', 'Ouvrir le tableau'], pages: ['/tableau'] },
  { id: 'board-tasks', q: 'Comment transformer des post-it en tâches ?', k: 'post-it postit transformer convertir tache vote voter idee', a: 'Sélectionnez les post-it, puis « Tâches » dans la barre au-dessus : choisissez la personne et l’étiquette, puis « Convertir ».', go: ['/tableau', 'Ouvrir le tableau'], pages: ['/tableau'] },
  { id: 'board-export', q: 'Comment exporter le tableau ?', k: 'exporter export telecharger png svg pdf image tableau', a: 'Bouton « Exporter » en haut à droite du tableau : PNG, SVG ou PDF.', go: ['/tableau', 'Ouvrir le tableau'], pages: ['/tableau'] },
  // ----- UEFN -----
  { id: 'uefn-link', q: 'Comment relier UEFN à POP ?', k: 'relier connecter uefn link lier synchroniser projet relais pop-link exe installer unreal', a: 'UEFN Link est un petit programme à lancer sur le PC Windows où UEFN est ouvert. Il importe vos projets et les garde synchronisés.', steps: ['Ouvrez Organiseur → « Relier UEFN avec UEFN Link ».', 'Cliquez sur « Télécharger UEFN Link » : il est déjà configuré pour votre compte (valable 24 h).', 'Double-cliquez sur pop-link.exe sur le PC (Windows peut demander « Exécuter quand même »).', 'Dans UEFN : activez Enable Project Python Scripts, puis Model Context Protocol → Start Server.', 'Laissez la fenêtre ouverte : le badge en haut passe à « Relais en ligne ».'], go: ['/organiseur', 'Ouvrir l’Organiseur'], pages: ['/organiseur', '/verse', '/'] },
  { id: 'uefn-offline', q: 'Pourquoi le relais est hors ligne ?', k: 'relais hors ligne deconnecte marche pas uefn probleme erreur diagnostic connexion', a: 'Le relais est hors ligne quand pop-link.exe n’est pas lancé, ou que UEFN ou son serveur MCP n’est pas démarré.', steps: ['Vérifiez que pop-link.exe tourne sur le PC.', 'Dans UEFN : Model Context Protocol → Start Server.', 'Toujours rien ? Lancez « pop-link.exe diagnostic » dans un terminal et gardez le résultat.'], go: ['/organiseur', 'Ouvrir l’Organiseur'], pages: ['/organiseur', '/verse'] },
  { id: 'verse-send', q: 'Comment envoyer mon code Verse à UEFN ?', k: 'verse code envoyer compiler sauvegarder ctrl s erreur compilation editeur', a: 'Dans l’Éditeur Verse, Ctrl+S (Cmd+S sur Mac) envoie le fichier à UEFN : écriture, compilation, et les erreurs s’affichent à la bonne ligne.', go: ['/verse', 'Ouvrir l’Éditeur Verse'], pages: ['/verse'] },
  { id: 'verse-help', q: 'Comment comprendre ou corriger une erreur Verse ?', k: 'erreur verse corriger comprendre expliquer bug assistant relire', a: 'L’assistant de l’Éditeur Verse explique, relit et corrige votre code sans service externe. Sélectionnez le code ou l’erreur et demandez une explication ou une correction.', go: ['/verse', 'Ouvrir l’Éditeur Verse'], pages: ['/verse'] },
  { id: 'organizer', q: 'Comment organiser les acteurs de ma map ?', k: 'organiser acteur device dossier renommer deplacer tag outliner organiseur filtrer chercher', a: 'L’Organiseur liste les acteurs de la map. Recherchez avec « tag:x type:x dossier:x », sélectionnez-en plusieurs (Ctrl/Maj) puis renommez, déplacez, dupliquez ou masquez. Ctrl+Z annule votre dernière action.', go: ['/organiseur', 'Ouvrir l’Organiseur'], pages: ['/organiseur'] },
  { id: 'ai-connect', q: 'Comment connecter Claude, Cursor ou une autre IA ?', k: 'connecter ia claude cursor codex windsurf gemini chatgpt mcp jeton token assistant code', a: 'Dans IA connectées, choisissez votre outil : POP crée un jeton et affiche la configuration à copier. Vous pouvez en connecter plusieurs, en lecture seule ou en écriture.', go: ['/ia', 'Ouvrir IA connectées'], pages: ['/ia'] },
  // ----- Analyse -----
  { id: 'market', q: 'Comment lire l’analyse du marché ?', k: 'analyse marche genre opportunite sature niche dispute verdict comprendre lire chiffre', a: 'Chaque genre a un verdict (bonne opportunité, disputé, dominé, saturé, niche) et le nombre de joueurs pour une map typique. Cliquez sur un genre pour le détail et un conseil.', go: ['/analyse/marche', 'Ouvrir l’analyse'], pages: ['/analyse/marche'] },
  { id: 'trends', q: 'Comment voir les maps qui marchent ?', k: 'maps qui marchent populaire top classement joueurs tendance meilleures', a: '« Maps qui marchent » classe les maps par joueurs du dernier jour, avec leur évolution. « Meilleurs démarrages » montre les nouvelles maps qui ont bien démarré.', go: ['/analyse/tendances', 'Voir les maps qui marchent'], pages: ['/analyse/tendances', '/analyse/lancements'] },
  { id: 'mymap', q: 'Comment suivre les chiffres de ma map ?', k: 'chiffres stats statistiques ma map code ile suivre joueurs retention concurrent', a: 'Dans Analyse du marché → « Votre map », reliez le code de votre île publiée (1234-5678-9012). Vous pouvez aussi suivre des maps concurrentes.', go: ['/analyse/marche', 'Ouvrir l’analyse'], pages: ['/analyse/marche'] },
  { id: 'video', q: 'Comment avoir les tendances TikTok et YouTube ?', k: 'tiktok youtube shorts video tendance hashtag son apify cle', a: 'Dans Tendances vidéo, connectez YouTube Shorts (clé Google gratuite) et/ou TikTok (jeton Apify), puis « Collecter maintenant ».', go: ['/analyse/tiktok', 'Ouvrir Tendances vidéo'], pages: ['/analyse/tiktok'] },
  // ----- Équipe -----
  { id: 'invite', q: 'Comment inviter quelqu’un dans l’équipe ?', k: 'inviter ajouter membre equipe collegue ami associe lien invitation', a: 'Dans Équipe → Membres et rôles, créez une invitation et envoyez le lien (il expire après 7 jours).', steps: ['Ouvrez Équipe.', 'Onglet « Membres et rôles » → Inviter.', 'Choisissez le rôle, puis copiez le lien et envoyez-le (e-mail, Discord).'], go: ['/equipe', 'Ouvrir Équipe'], pages: ['/equipe'] },
  { id: 'roles', q: 'Quels sont les rôles ?', k: 'role roles droit permission admin proprietaire membre freelance invite acces', a: 'Propriétaire : tout, y compris la paie. Admin : membres, intégrations, gains. Membre : pages, tableaux, tâches, planning. Freelance : ses tâches, pages et tableaux. Invité : lecture seule et commentaires.', go: ['/equipe', 'Ouvrir Équipe'], pages: ['/equipe'] },
  { id: 'new-map', q: 'Comment créer une autre map ou un studio ?', k: 'nouvelle map creer studio projet autre ajouter changer de map', a: 'Menu du compte (en haut à droite) → « Nouveau studio ou map », ou Équipe → Maps du studio. Le sélecteur « Map » à gauche permet de changer de map.', go: ['/equipe?tab=maps', 'Ouvrir les maps'], pages: ['/equipe'] },
  { id: 'discord', q: 'Comment connecter Discord ou GitHub ?', k: 'discord github integration webhook bot rapport commande issues', a: 'Équipe → Intégrations : Discord pour le rapport du soir et les commandes (/tache-suivante, /fini, /rapport), GitHub pour relier issues et tâches.', go: ['/equipe?tab=integ', 'Ouvrir les intégrations'], pages: ['/equipe'] },
  // ----- Lancer / Rémunérer -----
  { id: 'launch', q: 'Comment préparer la sortie de ma map ?', k: 'lancer sortie publier publication moderation checklist jour j playtest', a: 'Lancer et suivre contient la vérification avant publication, l’échange de playtests et la checklist heure par heure du jour J. La publication elle-même se fait dans le portail créateur d’Epic.', go: ['/lancer', 'Ouvrir Lancer et suivre'], pages: ['/lancer'] },
  { id: 'playtest', q: 'Comment avoir des testeurs pour ma map ?', k: 'playtest testeur tester test retour avis joueurs credits', a: 'Dans Lancer et suivre → « Maps à tester » : testez les maps des autres pour gagner des crédits, puis dépensez-les pour faire tester la vôtre. Les retours arrivent dans « Retours reçus ».', go: ['/lancer', 'Ouvrir Lancer et suivre'], pages: ['/lancer'] },
  { id: 'money', q: 'Comment partager les gains de la map ?', k: 'argent gains revenus payer paie partager pourcentage remunerer salaire releve csv', a: 'Rémunérer : définissez les règles de partage (%, fixe, par tâche), importez les gains (manuel ou CSV) et chacun voit son relevé. POP calcule, il ne fait aucun virement.', go: ['/remunerer', 'Ouvrir Rémunérer'], pages: ['/remunerer'] },
  // ----- Compte -----
  { id: 'password', q: 'Comment changer mon mot de passe ?', k: 'mot de passe changer modifier oublie perdu compte nom email profil', a: 'Paramètres → votre compte : nom et mot de passe. Mot de passe oublié : lien « Mot de passe oublié » sur la page de connexion, ou demandez un lien à un admin.', go: ['/parametres', 'Ouvrir Paramètres'] },
  { id: 'dark', q: 'Comment passer en mode sombre ?', k: 'mode sombre nuit dark clair theme couleur', a: 'Cliquez sur l’icône lune en haut, à côté de la recherche.' },
  { id: 'search', q: 'Comment chercher quelque chose ?', k: 'chercher rechercher trouver recherche ctrl k', a: 'Ctrl+K (Cmd+K) ouvre la recherche : tâches, pages, tableaux, Verse, acteurs, membres et actions. Majuscules et accents ne comptent pas.' },
  { id: 'notif', q: 'Où sont mes notifications ?', k: 'notification cloche alerte prevenu mention', a: 'La cloche en haut : vous êtes prévenu quand on vous assigne une tâche, qu’on vous mentionne avec @ ou qu’une tâche se débloque.' },
  { id: 'shortcuts', q: 'Quels sont les raccourcis clavier ?', k: 'raccourci clavier touche shortcut ctrl', a: 'Ctrl+K recherche · Ctrl+S envoyer le Verse à UEFN · Ctrl+Z annuler (tableau, Organiseur) · Ctrl+B/I/U gras, italique, souligné sur le tableau · Ctrl+0 tout voir · Échap fermer.' },
]

const STOP = new Set('a au aux avec ce ces cette comment de des du en est et faire fait je j la le les leur ma me mes moi mon ne on ou par pas peut peux pour quand que quel quelle quels qui sa se ses son sur ta te tes ton tu un une vos votre vous y il elle dans d l qu c s n t m est-ce ca cela svp stp merci bonjour salut hey aide aider besoin veux voudrais faut dois peut-on possible'.split(' '))
export const fold = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/œ/g, 'oe').replace(/[^a-z0-9@+ ]+/g, ' ')
const stem = (w: string) => w.length > 4 ? w.replace(/(ements|ement|ations|ation|euses|euse|eurs|eur|ees|ee|es|er|ez|s|x|e)$/, '') : w
const words = (s: string) => fold(s).split(/\s+/).filter(w => w && !STOP.has(w)).map(stem)
function near(a: string, b: string) {
  if (a === b) return true
  if (a.length >= 4 && b.length >= 4 && (a.startsWith(b) || b.startsWith(a))) return true
  if (Math.min(a.length, b.length) < 5 || Math.abs(a.length - b.length) > 1) return false
  // distance d'édition ≤ 1 (une faute de frappe)
  let i = 0, j = 0, d = 0
  while (i < a.length && j < b.length) { if (a[i] === b[j]) { i++; j++; continue } if (++d > 1) return false; if (a.length > b.length) i++; else if (b.length > a.length) j++; else { i++; j++ } }
  return d + (a.length - i) + (b.length - j) <= 1
}
const index = KB.map(f => ({ f, kw: [...new Set(words(f.k + ' ' + f.q))] }))

/** Les fiches qui répondent le mieux à la question (score décroissant). */
export function ask(question: string, page = ''): { f: Fiche; score: number }[] {
  const qs = [...new Set(words(question))]
  if (!qs.length) return []
  return index.map(({ f, kw }) => {
    let s = 0
    for (const w of qs) { if (kw.includes(w)) s += 3; else if (kw.some(k => near(w, k))) s += 2 }
    if (s && f.pages?.some(p => page === p || (p !== '/' && page.startsWith(p)))) s += 1.5
    return { f, score: s / Math.sqrt(qs.length + 1) }
  }).filter(x => x.score >= 1.2).sort((a, b) => b.score - a.score)
}
/** Questions proposées selon la page ouverte. */
export function suggested(page: string): Fiche[] {
  const here = KB.filter(f => f.pages?.some(p => page === p || (p !== '/' && page.startsWith(p))))
  const base = ['today', 'task-new', 'uefn-link', 'invite']
  return [...here, ...base.map(id => KB.find(f => f.id === id)!)].filter((f, i, a) => a.indexOf(f) === i).slice(0, 5)
}
