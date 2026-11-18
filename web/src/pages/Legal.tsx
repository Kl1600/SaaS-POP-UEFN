// Pages légales, visibles avec ou sans compte : mentions légales, confidentialité (RGPD), CGU, CGV, cookies.
// Les informations sur l'éditeur viennent de l'Administration (onglet « Infos légales »).
import { useEffect, useState, type ReactNode } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { ArrowLeft } from 'lucide-react'

export const LEGAL_PAGES: [string, string][] = [
  ['/mentions-legales', 'Mentions légales'],
  ['/confidentialite', 'Confidentialité'],
  ['/cgu', 'Conditions d’utilisation'],
  ['/cgv', 'Conditions de vente'],
  ['/cookies', 'Cookies'],
]
export const isLegalPath = (p: string) => LEGAL_PAGES.some(([x]) => x === p)

/** Liens vers les pages légales (pied de page du site, écrans de connexion, Aide). */
export function LegalLinks({ className }: { className?: string }) {
  return <nav className={'legal-links ' + (className ?? '')} aria-label="Informations légales">
    {LEGAL_PAGES.map(([p, l]) => <Link key={p} to={p}>{l}</Link>)}
  </nav>
}

type Info = Record<string, string>
const LABEL: Record<string, string> = {
  editor_name: 'nom ou raison sociale de l’éditeur', editor_form: 'forme juridique', capital: 'capital social', address: 'adresse du siège',
  siren: 'numéro SIREN / RCS', vat: 'numéro de TVA intracommunautaire', publisher: 'directeur de la publication', email: 'adresse e-mail de contact',
  phone: 'téléphone', host_name: 'hébergeur', host_address: 'adresse de l’hébergeur', host_phone: 'téléphone de l’hébergeur',
  mediator_name: 'médiateur de la consommation', mediator_url: 'site du médiateur',
}

function useInfo() {
  const [info, setInfo] = useState<Info | null>(null)
  useEffect(() => { fetch('/api/legal', { headers: { accept: 'application/json' } }).then(r => r.json()).then(setInfo).catch(() => setInfo({})) }, [])
  return info
}

export default function Legal() {
  const { pathname } = useLocation()
  const info = useInfo()
  useEffect(() => { window.scrollTo(0, 0) }, [pathname])
  const F = (k: string): ReactNode => info?.[k] ? info[k] : <mark className="legal-todo">[à compléter : {LABEL[k] ?? k}]</mark>
  const site = info?.site ?? ''
  const contact = info?.email ? <a href={`mailto:${info.email}`}>{info.email}</a> : F('email')
  const doc = ({
    '/mentions-legales': <Mentions F={F} site={site} contact={contact} />,
    '/confidentialite': <Privacy F={F} contact={contact} />,
    '/cgu': <Terms F={F} contact={contact} />,
    '/cgv': <Sales F={F} contact={contact} />,
    '/cookies': <Cookies contact={contact} />,
  } as Record<string, ReactNode>)[pathname]
  return <div className="legal">
    <header className="legal-top">
      <Link to="/" className="legal-back"><ArrowLeft size={18} />POP</Link>
      <LegalLinks />
    </header>
    <main className="legal-doc">
      {doc}
      <p className="legal-date">Dernière mise à jour : {info?.updated || '17 novembre 2026'}.</p>
    </main>
  </div>
}

type P = { F: (k: string) => ReactNode; contact: ReactNode; site?: string }

function Mentions({ F, site, contact }: P) {
  return <>
    <h1>Mentions légales</h1>
    <p>Le site {site || 'POP'} et l’application POP sont édités et hébergés dans les conditions suivantes, conformément à la loi n° 2004-575 du 21 juin 2004 pour la confiance dans l’économie numérique.</p>
    <h2>Éditeur</h2>
    <ul>
      <li>{F('editor_name')}{' '}({F('editor_form')}{' '}au capital de {F('capital')})</li>
      <li>Siège : {F('address')}</li>
      <li>SIREN / RCS : {F('siren')}</li>
      <li>TVA intracommunautaire : {F('vat')}</li>
      <li>Contact : {contact}{' '}· {F('phone')}</li>
      <li>Directeur de la publication : {F('publisher')}</li>
    </ul>
    <h2>Hébergement</h2>
    <ul><li>{F('host_name')}</li><li>{F('host_address')}</li><li>{F('host_phone')}</li></ul>
    <h2>Propriété intellectuelle</h2>
    <p>Le nom POP, le logo, les textes, l’interface et le code de POP appartiennent à l’éditeur. Les contenus créés par les utilisateurs restent leur propriété (voir les conditions d’utilisation).</p>
    <h2>Marques de tiers</h2>
    <p>POP est un service indépendant. Il n’est ni édité, ni approuvé, ni sponsorisé par Epic Games. Fortnite, Unreal Editor for Fortnite (UEFN) et Unreal Engine sont des marques d’Epic Games, Inc. Les chiffres du marché proviennent de l’API publique de données Fortnite d’Epic Games.</p>
  </>
}

function Privacy({ F, contact }: P) {
  return <>
    <h1>Politique de confidentialité</h1>
    <p>Cette page explique quelles données personnelles POP utilise, pourquoi, combien de temps, et comment exercer vos droits, conformément au Règlement général sur la protection des données (RGPD) et à la loi Informatique et Libertés.</p>
    <h2>Responsable du traitement</h2>
    <p>{F('editor_name')}, {F('address')}. Contact pour toute question sur vos données : {contact}.</p>
    <h2>Les données que nous utilisons</h2>
    <table className="legal-table"><thead><tr><th>Données</th><th>Pourquoi</th><th>Base légale</th><th>Durée</th></tr></thead><tbody>
      <tr><td>Compte : nom affiché, identifiant, adresse e-mail, mot de passe (stocké chiffré de façon irréversible), date d’acceptation des conditions</td><td>Créer et faire fonctionner votre compte</td><td>Exécution du contrat</td><td>Tant que le compte existe ; supprimées à la suppression du compte</td></tr>
      <tr><td>Votre travail : maps, tâches, pages, tableaux blancs, commentaires, fichiers et acteurs synchronisés depuis UEFN, partage des gains, profil dans le studio</td><td>Fournir le service que vous utilisez</td><td>Exécution du contrat</td><td>Tant que le compte ou le studio existe</td></tr>
      <tr><td>Sessions : adresse IP, type de navigateur, dates de connexion</td><td>Vous garder connecté, afficher vos appareils, protéger le compte</td><td>Intérêt légitime (sécurité)</td><td>30 jours au plus, 14 jours sans activité</td></tr>
      <tr><td>Journal de sécurité : actions sensibles (connexion, mot de passe, administration) avec l’adresse IP</td><td>Détecter et prouver un accès frauduleux</td><td>Intérêt légitime (sécurité)</td><td>12 mois</td></tr>
      <tr><td>Demande d’accès à la bêta : nom, e-mail, studio, expérience, liens, message</td><td>Étudier votre demande et créer votre accès</td><td>Mesures précontractuelles</td><td>Jusqu’à la création du compte ; 12 mois si la demande est refusée</td></tr>
      <tr><td>Clés et jetons d’intégration que vous ajoutez (Discord, GitHub, YouTube, Apify, Lore) et jetons des IA connectées (stockés sous forme d’empreinte)</td><td>Relier POP aux outils que vous choisissez</td><td>Exécution du contrat</td><td>Jusqu’à leur retrait ou la suppression du compte</td></tr>
      <tr><td>Notifications</td><td>Vous prévenir de ce qui vous concerne</td><td>Exécution du contrat</td><td>6 mois après lecture</td></tr>
      <tr><td>Sauvegardes de la base</td><td>Restaurer le service en cas de panne</td><td>Intérêt légitime</td><td>14 jours</td></tr>
    </tbody></table>
    <p>POP n’affiche pas de publicité, ne vend aucune donnée et n’utilise aucun outil de mesure d’audience ou de suivi publicitaire.</p>
    <h2>Données publiques utilisées par l’analyse</h2>
    <p>L’analyse du marché utilise les statistiques publiques des îles Fortnite fournies par Epic Games (joueurs, temps de jeu, rétention, titres, créateurs) et des vidéos publiques TikTok et YouTube collectées avec les clés que vous ajoutez. Ce sont des informations publiées par leurs auteurs ; POP ne les relie à aucun compte POP.</p>
    <h2>Qui reçoit vos données</h2>
    <ul>
      <li>Les membres de vos studios voient ce que vous partagez dans ces studios.</li>
      <li>L’hébergeur de POP ({F('host_name')}) et, si les e-mails sont activés, le service d’envoi d’e-mails, en tant que sous-traitants.</li>
      <li>Les services que vous connectez vous-même (Discord, GitHub, l’IA de votre choix…) reçoivent ce que vous leur envoyez depuis POP, selon leurs propres règles.</li>
    </ul>
    <p>Les polices de caractères et les vignettes vidéo sont servies par POP : votre navigateur ne contacte ni Google, ni TikTok, ni YouTube en utilisant POP. Une image que vous insérez vous-même par son adresse dans une page ou un tableau est chargée depuis le site d’origine.</p>
    <h2>Mineurs</h2>
    <p>POP est ouvert à partir de 13 ans. Entre 13 et 15 ans, l’accord d’un parent ou du titulaire de l’autorité parentale est nécessaire pour créer un compte. Un parent peut nous écrire à tout moment pour faire supprimer le compte d’un enfant.</p>
    <h2>Vos droits</h2>
    <p>Vous pouvez accéder à vos données, les corriger, les faire effacer, les récupérer dans un format lisible (Paramètres → Exporter mes données), limiter ou refuser certains traitements, et donner des directives sur leur sort après votre décès. Vous pouvez supprimer votre compte vous-même dans Paramètres. Pour tout le reste, écrivez à {contact} : nous répondons dans un délai d’un mois.</p>
    <p>Si vous estimez que vos droits ne sont pas respectés, vous pouvez saisir la CNIL (cnil.fr).</p>
    <h2>Sécurité</h2>
    <p>Connexion chiffrée (HTTPS), mots de passe chiffrés de façon irréversible, cookie de session inaccessible aux scripts et réservé à POP, sessions fermées après 14 jours sans activité, liste des appareils connectés avec déconnexion à distance, et journal des actions sensibles.</p>
  </>
}

function Terms({ F, contact }: P) {
  return <>
    <h1>Conditions générales d’utilisation</h1>
    <p>Ces conditions s’appliquent à toute personne qui utilise POP, le service en ligne qui aide les créateurs de maps Fortnite (UEFN) à organiser, construire, lancer et suivre leurs maps. En créant un compte, vous les acceptez.</p>
    <h2>1. Accès au service</h2>
    <p>POP est en bêta privée : l’accès se fait sur invitation, avec un identifiant créé par l’équipe POP. Certaines fonctions peuvent changer, être ajoutées ou retirées pendant la bêta. Les fonctions payantes sont régies par les <a href="/cgv">conditions de vente</a>.</p>
    <h2>2. Âge</h2>
    <p>Il faut avoir au moins 13 ans pour utiliser POP. Entre 13 et 15 ans, l’accord d’un parent est nécessaire. Le parent accepte alors ces conditions pour l’enfant.</p>
    <h2>3. Votre compte</h2>
    <p>Vos identifiants sont personnels. Gardez votre mot de passe secret, ne le réutilisez pas ailleurs, et prévenez-nous à {contact} si vous pensez que quelqu’un d’autre a accès à votre compte. Vous pouvez fermer à distance les sessions ouvertes sur d’autres appareils dans Paramètres.</p>
    <h2>4. Ce qui est interdit</h2>
    <ul>
      <li>Accéder ou tenter d’accéder aux données d’un autre utilisateur ou studio sans y être invité.</li>
      <li>Perturber le service, contourner ses limites ou ses protections, ou l’interroger de façon automatisée en dehors des accès prévus (API et IA connectées).</li>
      <li>Publier des contenus illicites, haineux, harcelants, ou qui portent atteinte aux droits d’autrui.</li>
      <li>Utiliser POP pour fabriquer de l’engagement artificiel, du spam ou des maps contraires aux règles d’Epic Games.</li>
    </ul>
    <h2>5. Vos contenus</h2>
    <p>Les maps, tâches, pages, tableaux et fichiers que vous créez restent à vous ou à votre studio. Vous nous autorisez seulement à les stocker et à les afficher pour faire fonctionner le service. Dans un studio, le propriétaire et les admins gèrent les accès des membres.</p>
    <h2>6. IA connectées et outils tiers</h2>
    <p>Quand vous connectez une IA (Claude, Cursor, Codex…) ou un outil (Discord, GitHub…), elle agit avec les droits que vous lui donnez. Vous restez responsable de ce qu’elle fait ; chaque action est notée dans POP et peut être annulée quand c’est possible. Votre abonnement à ces services n’est pas compris dans POP.</p>
    <h2>7. Analyse du marché et idées de maps</h2>
    <p>Les chiffres, notes et idées de maps sont calculés à partir de données publiques et présentés à titre indicatif. Ils ne garantissent aucun résultat dans Fortnite. POP n’est pas lié à Epic Games et ne connaît pas l’algorithme de Discover au-delà de ce qu’Epic publie.</p>
    <h2>8. Disponibilité et responsabilité</h2>
    <p>Nous faisons de notre mieux pour que POP soit disponible et que vos données soient sauvegardées chaque jour, sans pouvoir garantir une absence totale d’interruption. Exportez régulièrement ce qui compte pour vous. Notre responsabilité est limitée aux dommages directs causés par une faute de notre part, sauf disposition légale contraire.</p>
    <h2>9. Suspension et fermeture</h2>
    <p>Vous pouvez supprimer votre compte à tout moment dans Paramètres. En cas de non-respect grave de ces conditions, nous pouvons suspendre un compte, après vous avoir prévenu sauf urgence.</p>
    <h2>10. Modifications</h2>
    <p>Si ces conditions changent, nous vous prévenons dans POP avant l’entrée en vigueur. Si vous refusez les nouvelles conditions, vous pouvez supprimer votre compte.</p>
    <h2>11. Droit applicable</h2>
    <p>Ces conditions sont soumises au droit français. En cas de litige, écrivez-nous d’abord à {contact} pour trouver une solution amiable. Éditeur : {F('editor_name')}.</p>
  </>
}

function Sales({ F, contact }: P) {
  return <>
    <h1>Conditions générales de vente</h1>
    <p>Pendant la bêta, POP est gratuit : aucun paiement n’est demandé. Les conditions ci-dessous s’appliqueront dès l’ouverture des paiements ; vous serez prévenu avant tout paiement.</p>
    <h2>1. Offres</h2>
    <ul>
      <li><b>Créateur</b> : gratuit.</li>
      <li><b>Studio</b> : abonnement mensuel, prix affiché sur la page Tarifs au moment de la commande. Comprend Analyse (marché, idées de maps, suivi Discover), et ses membres en profitent dans le studio.</li>
      <li><b>Studio+</b> : à venir.</li>
      <li><b>Crédits</b> : achat ponctuel, utilisés notamment pour la génération par l’IA de POP et l’échange de playtests.</li>
    </ul>
    <p>Les prix sont indiqués en euros, toutes taxes comprises pour les particuliers.</p>
    <h2>2. Commande et paiement</h2>
    <p>La commande est validée au paiement en ligne. L’abonnement est payé d’avance et se renouvelle chaque mois à la même date, jusqu’à résiliation.</p>
    <h2>3. Résiliation</h2>
    <p>Vous pouvez résilier à tout moment depuis votre compte, en quelques clics. L’abonnement reste actif jusqu’à la fin du mois déjà payé, puis n’est plus renouvelé.</p>
    <h2>4. Droit de rétractation (particuliers)</h2>
    <p>Si vous êtes un particulier, vous disposez de 14 jours à compter de la souscription pour vous rétracter, sans avoir à donner de raison, en nous écrivant à {contact} ou depuis votre compte.</p>
    <ul>
      <li><b>Abonnement</b> : si vous avez demandé à utiliser l’abonnement tout de suite, avant la fin des 14 jours, et que vous vous rétractez, vous êtes remboursé du montant payé moins la part correspondant aux jours déjà utilisés.</li>
      <li><b>Crédits</b> : vous pouvez vous rétracter pour les crédits non utilisés. Les crédits déjà utilisés ne sont pas remboursés : en les utilisant avant la fin du délai, vous avez demandé à en profiter tout de suite et reconnu perdre votre droit de rétractation pour eux.</li>
    </ul>
    <p>Le remboursement est fait sur le moyen de paiement utilisé, au plus tard 14 jours après votre demande.</p>
    <h2>5. Remboursements en dehors de la rétractation</h2>
    <p>Un mois d’abonnement commencé n’est pas remboursé, sauf si le service a été indisponible de notre fait pendant une durée importante : écrivez-nous et nous trouverons une solution. Les crédits n’expirent pas tant que le compte existe.</p>
    <h2>6. Professionnels</h2>
    <p>Si vous souscrivez pour les besoins de votre activité professionnelle, le droit de rétractation ne s’applique pas, sauf dans les cas prévus par la loi.</p>
    <h2>7. Médiation</h2>
    <p>En cas de litige non résolu avec nous, un particulier peut recourir gratuitement au médiateur de la consommation : {F('mediator_name')}{' '}({F('mediator_url')}).</p>
    <h2>8. Vendeur</h2>
    <p>{F('editor_name')}, {F('address')}, SIREN {F('siren')}.</p>
  </>
}

function Cookies({ contact }: { contact: ReactNode }) {
  return <>
    <h1>Politique de cookies</h1>
    <p>POP n’utilise que les cookies et le stockage local indispensables au fonctionnement du service ou à des réglages que vous choisissez vous-même. Aucun cookie publicitaire, aucune mesure d’audience, aucun réseau social, aucun service tiers. C’est pourquoi POP ne vous demande pas de consentement et n’affiche pas de bannière : ces traceurs en sont exemptés par la loi (article 82 de la loi Informatique et Libertés et lignes directrices de la CNIL).</p>
    <table className="legal-table"><thead><tr><th>Nom</th><th>Type</th><th>À quoi il sert</th><th>Durée</th></tr></thead><tbody>
      <tr><td><code>__Host-pop_session</code></td><td>Cookie</td><td>Vous garder connecté. Inaccessible aux scripts, envoyé seulement à POP et seulement en HTTPS.</td><td>30 jours au plus, 14 jours sans activité, supprimé à la déconnexion</td></tr>
      <tr><td><code>pop-lang</code></td><td>Stockage local</td><td>Retenir la langue que vous avez choisie</td><td>Jusqu’à ce que vous le supprimiez</td></tr>
      <tr><td><code>pop.dark</code></td><td>Stockage local</td><td>Retenir le thème clair ou sombre</td><td>Jusqu’à ce que vous le supprimiez</td></tr>
      <tr><td><code>pop.map</code></td><td>Stockage local</td><td>Rouvrir la dernière map utilisée</td><td>Jusqu’à ce que vous le supprimiez</td></tr>
      <tr><td><code>pop-idea-pop</code></td><td>Stockage local</td><td>Ne pas vous montrer deux fois la même alerte d’idée de map</td><td>Jusqu’à ce que vous le supprimiez</td></tr>
    </tbody></table>
    <p>Vous pouvez supprimer ces éléments dans les réglages de votre navigateur ; vous serez alors déconnecté et vos réglages d’affichage seront perdus. Si POP ajoutait un jour un traceur non indispensable, il ne serait déposé qu’après votre accord, et cette page serait mise à jour.</p>
    <p>Questions : {contact}.</p>
  </>
}
