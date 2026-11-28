import { LegalLinks } from './Legal'
import { PageHead } from '../components/ui'
import { tr } from '../i18n/t'

const SECTIONS: [string, string[]][] = [
  [tr("Démarrer"), [tr("Créez votre studio et votre première map, puis invitez l’équipe dans Équipe et intégrations → Membres. Chaque invité reçoit un lien (par e-mail si l’envoi est configuré, sinon à copier)."), tr("Rôles : propriétaire et admin gèrent tout ; membre travaille sur tout ; freelance travaille sur ses tâches ; invité lit et commente.")]],
  [tr("Pages"), [tr("Tapez « / » pour insérer un bloc, « # » pour un titre, « - » pour une liste, « [] » pour une case à cocher. @Prénom mentionne un membre : il reçoit une notification."), tr("Historique : chaque version est horodatée et peut être restaurée. Partage : privée, équipe, ou lien public en lecture.")]],
  [tr("Tableau blanc"), [tr("Post-it, formes, connecteurs, stylo, cadres. Votez avec des points, puis transformez les post-it gagnants en tâches et un cadre en page.")]],
  [tr("Planifier et Tâches"), [tr("Le générateur découpe la map en lots et tâches d’une heure, avec dépendances, jours et personnes suggérés. Une tâche ne passe « En cours » que si ses dépendances sont faites ; elle se débloque toute seule ensuite."), tr("Le chemin critique et les alertes d’objectif s’affichent dans la chronologie.")]],
  [tr("UEFN Link et IA connectées"), [tr("Installez POP Forge sur le PC où UEFN tourne (gratuit comme relais, Paramètres → UEFN Link). Dans UEFN : Enable Project Python Scripts, puis Model Context Protocol → Start Server."), tr("IA connectées : branchez Claude, Cursor ou Codex (IA connectées). Elles lisent et modifient les acteurs, écrivent le Verse et compilent dans UEFN ; chaque action est notée et annulable.")]],
  [tr("Lancer et suivre"), [tr("Checklist de modération, onglet Test (playtests à crédits, demandes et retours), plan heure par heure du jour J. La publication se fait dans le portail créateur d’Epic.")]],
  [tr("Rémunérer"), [tr("Règles de partage (%, fixe, par tâche), import des gains (manuel ou CSV), relevé de chacun. POP calcule, il ne fait aucun virement.")]],
  [tr("Intégrations"), [tr("Discord : rapport du soir automatique (webhook) et commandes /tache-suivante, /fini, /rapport (POP doit être en ligne). GitHub : issues ↔ tâches. Lore : webhook de check-in et verrous. IA connectée : jeton MCP pour Claude Code, Cursor…")]],
  [tr("Raccourcis"), [tr("Ctrl K : recherche globale (tâches, pages, acteurs, membres) · Échap : fermer.")]],
]

export default function Help() {
  return <main className="content help">
    <PageHead title={tr("Aide et support")} help={tr("Comment utiliser chaque partie de POP.")}>
      <button className="btn pri" onClick={() => window.dispatchEvent(new CustomEvent('boussole', { detail: '' }))}>{tr("Demander à Léo")}</button>
    </PageHead>
    <div className="grid2">
      {SECTIONS.map(([t, ps]) => <section key={t} className="card"><h3>{t}</h3>{ps.map((p, i) => <p key={i} style={{ margin: '6px 0' }}>{p}</p>)}</section>)}
      <section className="card"><h3>{tr("Informations légales")}</h3><LegalLinks className="help-legal" /></section>
      <section className="card"><h3>{tr("Un problème ?")}</h3><p>{tr("Relais UEFN : dans POP Forge, Réglages → Diagnostic, puis collez le résultat dans votre demande. Mot de passe perdu : « Mot de passe oublié » sur la page de connexion, ou demandez un lien à un admin du studio.")}</p></section>
    </div>
  </main>
}
