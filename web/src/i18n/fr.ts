// Textes du site vitrine et de la liste d'attente — français (langue source).
// Les autres langues reprennent exactement la même forme (type Dict).
const fr = {
  meta: { title: 'POP · Le studio des créateurs Fortnite' },
  nav: { features: 'Fonctionnalités', boussole: 'Boussole', pricing: 'Tarifs', faq: 'FAQ', login: 'Se connecter', join: 'Rejoindre la bêta', home: 'POP, accueil', lang: 'Langue', search: 'Chercher un pays' },
  hero: {
    announce: 'Nouveau : Boussole, l’assistant intégré', h1a: 'Vos maps Fortnite.', h1b: 'À plusieurs. Sans le chaos.',
    subA: 'POP réunit tout le studio UEFN au même endroit : ', subB: 'tâches d’une heure', subC: ', Verse relié à UEFN, tableau blanc, analyse du marché et partage des gains.',
    cta: 'Rejoindre la liste d’attente', see: 'Voir les fonctionnalités', micro1: 'Bêta privée', micro2: 'Accès sur liste d’attente', micro3: 'Relié à UEFN',
  },
  marquee: { label: 'Fonctionne avec vos outils', epic: 'API Fortnite d’Epic' },
  pain: {
    eyebrow: 'Le problème', h2a: 'Discord, trois Google Docs,', h2b: 'et personne ne sait qui fait quoi.', without: 'Sans POP', with: 'Avec POP',
    bad: ['« Qui a touché au spawner ? »', 'Le Verse passe par Discord en .txt', 'Une liste de tâches sur un Google Doc', 'Le tableur des gains, version 7', '« On sort quand déjà ? »', 'On construit sans savoir si le genre marche'],
    good: ['Chaque action est tracée et annulable', 'Ctrl+S envoie le Verse dans UEFN', 'Des tâches d’une heure, avec dépendances', 'Un relevé clair pour chacun', 'Boussole suit le jalon et le retard', 'Un verdict par genre, avec les vrais chiffres'],
  },
  feat: {
    eyebrow: 'Fonctionnalités', h2: 'Tout le studio, de l’idée au jour J.', p: 'Ce que vous voyez ici, ce sont les vrais écrans de POP, en plus simple.',
    items: [
      { tag: 'Tâches et Boussole', h: 'Des tâches d’une heure. Un assistant chaque matin.', p: 'Le générateur découpe la map en tâches d’une heure, avec leurs dépendances. Boussole résume la journée, signale les retards et propose quoi prendre en premier.', pts: ['Kanban et dépendances', 'Retards et jalons suivis', '« Prendre » en un clic'] },
      { tag: 'Éditeur Verse', h: 'Ctrl+S, et c’est dans UEFN.', p: 'Le Verse s’écrit dans POP et part directement dans votre projet : compilation, erreurs à la bonne ligne, session de test. L’assistant explique et corrige, sans service externe.', pts: ['Compilation dans UEFN', 'Erreurs à la ligne', 'Corrections proposées'] },
      { tag: 'Organiseur', h: 'Tous vos appareils. Rangés en trois secondes.', p: 'Cherchez par tag, type ou dossier, sélectionnez-en vingt et renommez, déplacez ou dupliquez d’un coup. Ctrl+Z annule. Si UEFN est fermé, les changements attendent.', pts: ['Recherche tag:, type:, dossier:', 'Modifications en lot', 'Tout est annulable'] },
      { tag: 'Tableau blanc', h: 'Dessinez la map à plusieurs.', p: 'Post-it, formes, flèches, couleurs, gras, souligné : tout ce qu’il faut pour poser les idées. Votez, puis transformez les meilleurs post-it en tâches.', pts: ['Mise en forme complète', 'Temps réel à plusieurs', 'Post-it → tâches'] },
      { tag: 'Analyse', h: 'Sachez quoi construire avant de construire.', p: 'Quels genres attirent les joueurs, où la place est libre, quelles maps décollent dès leur sortie, et ce qui buzze en vidéo. Chiffres de l’API officielle d’Epic.', pts: ['Verdict par genre', 'Maps qui marchent', 'Tendances TikTok et Shorts'] },
      { tag: 'Rémunérer', h: 'Chacun sait ce qu’il touche.', p: 'Règles de partage au pourcentage, au fixe ou à la tâche, import des gains, relevé par personne. Plus de tableur qui traîne sur Discord.', pts: ['Pourcentage, fixe, par tâche', 'Import CSV', 'Relevé de chacun'] },
    ],
    aiB: 'Vous avez déjà une IA ?', aiText: 'Connectez Claude, Cursor, Codex ou une autre IA compatible MCP : elle prend une tâche, écrit le Verse, compile, puis ferme la tâche avec une note. Chaque action est notée et annulable.',
  },
  bou: {
    eyebrow: 'Assistant intégré', h2a: 'Posez la question.', h2b: 'Boussole répond.',
    p: 'Boussole connaît vos tâches, vos jalons et votre équipe. Il dit ce qui est en retard, qui travaille sur quoi et par quoi commencer, et il guide pas à pas ceux qui découvrent POP. Le tout sans que vos données quittent POP.',
    sub: 'assistant intégré', ask: 'Comment je… ?',
    // Lignes de réponse : [r] [o] [g] = icône rouge, orange, verte ; **gras** ; « 1. » = étape.
    demo: [
      { q: 'Qu’est-ce que je fais maintenant ?', a: ['**Bonjour Léa, on rattrape le retard ?**', '[r] 2 tâches en retard.', '[g] Hier, vous avez terminé 3 tâches.', 'À faire maintenant : **#42 Système de score**, qui débloque 3 autres tâches.'] },
      { q: 'Comment relier UEFN ?', a: ['**Comment relier UEFN à POP ?**', '1. Organiseur → Télécharger UEFN Link.', '2. Double-cliquez sur pop-link.exe sur le PC.', '3. Dans UEFN : Model Context Protocol → Start Server.'] },
      { q: 'On est en retard pour la bêta ?', a: ['[o] Jalon « Bêta » dans 5 jours : 9 tâches à finir.', 'Au rythme actuel (1,5 par jour), il faudrait 6 jours. **Retard prévu : 1 jour.**'] },
      { q: 'Sur quoi travaille Hugo ?', a: ['**Hugo** a 1 tâche en cours et 2 à faire :', '• **#38** Décor de la zone 2 · en cours depuis hier', '• **#44** Écran de fin · attend #42', '[g] 4 tâches terminées cette semaine.'] },
      { q: 'Mon IA peut fermer les tâches ?', a: ['Oui : connectée dans « IA connectées », elle utilise **next_task**, **set_task_status** et **comment_task**. Chaque action est notée « IA » dans l’historique.'] },
    ],
  },
  stats: [['1 h', 'par tâche, pour avancer chaque jour'], ['0', 'service d’IA externe obligatoire'], ['7 jours', 'de stats par map, de l’API d’Epic'], ['100 %', 'des actions UEFN annulables']],
  pricing: {
    eyebrow: 'Tarifs', h2: 'Choisissez votre plan.', p: 'POP est en bêta privée : l’accès se fait sur liste d’attente. Les tarifs s’appliquent à l’ouverture, et vous serez prévenu avant tout paiement.',
    perMonth: '/ mois', compare: 'Comparer les plans', unlimited: 'Illimité', soon: 'Bientôt',
    plans: [
      { name: 'Créateur', tag: 'Pour se lancer', desc: 'Une map, une petite équipe, tout l’essentiel.', cta: 'Rejoindre la liste' },
      { name: 'Studio', tag: 'Le plus complet', desc: 'Pour les studios qui sortent des maps en équipe.', cta: 'Rejoindre la liste' },
      { name: 'Studio+', tag: 'Bientôt', desc: 'Tout Studio, plus POP Forge : l’app de bureau qui construit dans UEFN.', cta: 'Rejoindre la liste' },
    ],
    rows: ['Organisation', 'Tâches d’une heure, Kanban, dépendances', 'Boussole, l’assistant intégré', 'Pages, tableau blanc, planning et jalons', 'Maps', 'Membres', 'UEFN', 'UEFN Link : projets synchronisés', 'Éditeur Verse relié (compilation, erreurs à la ligne)', 'Organiseur d’acteurs et modifications en lot', 'IA connectées (Claude, Cursor, Codex…)', 'POP Forge : construction assistée dans UEFN', 'Croissance', 'Analyse du marché et maps qui marchent', 'Tendances vidéo TikTok et YouTube Shorts', 'Échange de playtests et jour J', 'Partage des gains et relevés', 'Discord, GitHub, Lore'],
  },
  faq: {
    eyebrow: 'FAQ', h2: 'Les questions qu’on nous pose.',
    items: [
      ['Faut-il un PC Windows ?', 'Pour relier UEFN, oui : UEFN ne tourne que sur Windows, et UEFN Link se lance sur ce PC. Le reste de POP s’utilise depuis n’importe quel navigateur, Mac compris.'],
      ['Est-ce que POP publie ma map ?', 'Non. POP prépare tout (vérification avant publication, playtests, plan du jour J), mais la publication se fait toujours dans le portail créateur d’Epic.'],
      ['Comment marche l’assistant Boussole ?', 'Boussole est intégré à POP : il lit vos tâches, leurs dates, leurs dépendances et vos jalons pour résumer la journée, répondre à vos questions (« qu’est-ce qui est en retard ? », « sur quoi travaille Hugo ? ») et expliquer comment utiliser POP. Il tourne entièrement dans POP : vos données ne partent vers aucun service externe.'],
      ['Et si je veux utiliser mon IA ?', 'Connectez Claude, Cursor, Codex ou une autre IA compatible MCP dans « IA connectées ». Elle peut lire et modifier vos tâches, votre Verse et vos acteurs UEFN, et chaque action est notée et annulable.'],
      ['D’où viennent les chiffres de l’analyse ?', 'De l’API officielle Fortnite d’Epic : joueurs, pic de joueurs simultanés, temps de jeu et rétention de chaque map publique, sur 7 jours.'],
      ['Comment avoir accès ?', 'POP est en bêta privée. Rejoignez la liste d’attente : on valide les demandes une par une, puis on vous envoie votre identifiant et un mot de passe provisoire, que vous changez à la première connexion.'],
      ['Combien ça coûte ?', 'Les tarifs sont affichés plus haut. Ils s’appliquent à l’ouverture de POP ; les membres de la bêta sont prévenus avant tout paiement.'],
    ] as [string, string][],
  },
  final: { h2a: 'Votre prochaine map', h2b: 'commence ici.', p: 'Rejoignez la liste d’attente : on vous envoie votre accès dès qu’une place se libère.' },
  foot: { made: 'Fait par des créateurs Fortnite, pour des créateurs Fortnite.', legal: 'Fortnite et UEFN sont des marques d’Epic Games. POP n’est ni affilié à Epic Games ni approuvé par Epic Games.', product: 'Produit', help: 'Aide', account: 'Compte' },
  mock: {
    overview: 'Vue d’ensemble', nav: ['Vue d’ensemble', 'Pages', 'Tableau blanc', 'Tâches', 'Éditeur Verse', 'Organiseur', 'Marché'],
    hello: 'Bonjour Léa, je vous propose de commencer par #42.', late: '2 tâches en retard (la plus ancienne était prévue le 12 nov.).', mile: 'Jalon « Bêta » dans 5 jours : 9 tâches à finir, retard prévu de 1 jour.',
    take: 'Prendre', start: 'Commencer', unlocks: 'débloque 3 autres tâches', today: 'prévue aujourd’hui', waits: 'attend #42',
    todo: 'À faire', doing: 'En cours', done: 'Fait', tasks: ['Système de score', 'Décor de la zone 2', 'Écran de fin', 'Playtest interne', 'Spawn des joueurs', 'Tempête'],
    sent: 'envoyé à UEFN', err: 'Ligne 12 : int et float ne s’additionnent pas.', fix: 'Corriger : Points += 1',
    red: 'Équipe rouge', selected: '2 sélectionnés', rename: 'Renommer', move: 'Déplacer', dup: 'Dupliquer',
    s1: 'Mode équipe 4v4', s2a: 'Boss final', s2b: 'au centre', s3: 'Économie : 1 pièce / élimination',
    verdicts: ['Bonne opportunité', 'Populaire mais disputé', 'Saturé', 'Niche'], typical: 'joueurs / map typique',
    earnings: 'Gains d’octobre', roles: ['Code · 40 %', 'Carte · 35 %', 'Musique · fixe', 'Reste'], noTransfer: 'POP calcule, il ne fait aucun virement.',
  },
  wl: {
    eyebrow: 'Bêta privée', title: 'Rejoindre la liste d’attente', intro: 'On ouvre POP petit à petit pour accompagner chaque studio. Dites-nous qui vous êtes, on vous envoie votre accès dès que possible.',
    pageIntro: 'Les comptes POP sont créés par l’équipe pendant la bêta. Dites-nous qui vous êtes : dès que votre accès est validé, vous recevez votre identifiant et un mot de passe provisoire.',
    invalid: 'Ce lien d’accès est invalide, déjà utilisé ou expiré. Refaites une demande, ou écrivez à l’équipe POP.',
    name: 'Nom ou pseudo', email: 'E-mail', studio: 'Studio', optional: '(facultatif)', studioPh: 'Nom du studio', team: 'Taille de l’équipe',
    teams: ['Seul', '2 à 3', '4 à 10', 'Plus de 10'], exp: 'Expérience', exps: ['Débutant UEFN', 'Quelques maps publiées', 'Créateur confirmé', 'Studio professionnel'],
    links: 'Vos maps ou réseaux', linksPh: 'Codes d’îles, TikTok, YouTube…', msg: 'Ce que vous voulez faire avec POP', msgPh: 'Ex. : on est 4 sur une map tycoon et on se perd entre Discord et les Google Docs.',
    send: 'Demander mon accès', sending: 'Envoi…', nospam: 'Pas de spam : on vous écrit seulement pour vous donner votre accès.',
    okTitle: 'C’est noté, merci !', okText: 'Votre demande est arrivée chez l’équipe POP. Dès qu’une place se libère, on vous envoie votre identifiant et un mot de passe provisoire.',
    alreadyTitle: 'Vous êtes déjà sur la liste.', approvedTitle: 'Votre accès est déjà validé.', approvedText: 'Votre compte est prêt : l’équipe POP vous envoie votre identifiant et votre mot de passe provisoire. Sinon, écrivez-nous.', close: 'Fermer',
  },
}
export type Dict = typeof fr
export default fr
