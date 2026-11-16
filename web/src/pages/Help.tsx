import { PageHead } from '../components/ui'

const SECTIONS: [string, string[]][] = [
  ['Démarrer', ['Créez votre studio et votre première map, puis invitez l’équipe dans Équipe et intégrations → Membres. Chaque invité reçoit un lien (par e-mail si l’envoi est configuré, sinon à copier).', 'Rôles : propriétaire et admin gèrent tout ; membre travaille sur tout ; freelance travaille sur ses tâches ; invité lit et commente.']],
  ['Pages', ['Tapez « / » pour insérer un bloc, « # » pour un titre, « - » pour une liste, « [] » pour une case à cocher. @Prénom mentionne un membre : il reçoit une notification.', 'Historique : chaque version est horodatée et peut être restaurée. Partage : privée, équipe, ou lien public en lecture.']],
  ['Tableau blanc', ['Post-it, formes, connecteurs, stylo, cadres. Votez avec des points, puis transformez les post-it gagnants en tâches et un cadre en page.']],
  ['Planifier et Tâches', ['Le générateur découpe la map en lots et tâches d’une heure, avec dépendances, jours et personnes suggérés. Une tâche ne passe « En cours » que si ses dépendances sont faites ; elle se débloque toute seule ensuite.', 'Le chemin critique et les alertes de jalon s’affichent dans la chronologie.']],
  ['UEFN : Éditeur Verse et Organiseur', ['Installez UEFN Link sur le PC où UEFN tourne (Organiseur → UEFN Link). Dans UEFN : Enable Project Python Scripts, puis Model Context Protocol → Start Server.', 'Éditeur Verse : Ctrl+S envoie à UEFN (écriture, compilation, erreurs à la ligne, session de test). L’assistant explique, relit et corrige sans service externe.', 'Organiseur : recherche « tag:x type:x dossier:x option:x », multi-sélection Ctrl/Maj, Ctrl+Z annule votre dernière action. Quand UEFN est fermé, les changements attendent sa réouverture.']],
  ['Lancer et suivre', ['Checklist de modération, échange de playtests à crédits, analyse « pourquoi ils partent », plan heure par heure du jour J. La publication se fait dans le portail créateur d’Epic.']],
  ['Rémunérer', ['Règles de partage (%, fixe, par tâche), import des gains (manuel ou CSV), relevé de chacun. POP calcule, il ne fait aucun virement.']],
  ['Intégrations', ['Discord : rapport du soir automatique (webhook) et commandes /tache-suivante, /fini, /rapport (POP doit être en ligne). GitHub : issues ↔ tâches. Lore : webhook de check-in et verrous. IA connectée : jeton MCP pour Claude Code, Cursor…']],
  ['Raccourcis', ['Ctrl K : recherche globale (tâches, pages, Verse, acteurs, membres) · Ctrl S : envoyer le Verse à UEFN · Ctrl Z dans l’Organiseur : annuler · Échap : fermer.']],
]

export default function Help() {
  return <main className="content help">
    <PageHead title="Aide et support" help="Comment utiliser chaque partie de POP." />
    <div className="grid2">
      {SECTIONS.map(([t, ps]) => <section key={t} className="card"><h3>{t}</h3>{ps.map((p, i) => <p key={i} style={{ margin: '6px 0' }}>{p}</p>)}</section>)}
      <section className="card"><h3>Un problème ?</h3><p>Relais UEFN : lancez <span className="mono">pop-link.exe diagnostic</span> sur le PC et gardez le résultat. Mot de passe perdu : « Mot de passe oublié » sur la page de connexion, ou demandez un lien à un admin du studio.</p></section>
    </div>
  </main>
}
