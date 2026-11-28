// Didacticiel au premier passage sur un onglet (#496) : 3 ou 4 bulles qui pointent l'essentiel de la page.
// Vu une fois par onglet et par compte : la liste des onglets vus est gardée dans ui_prefs.tours (#493).
import { useEffect, useLayoutEffect, useState } from 'react'
import { X } from '../icons'
import { api } from '../api'
import { useApp } from '../state'
import { tr } from '../i18n/t'

type Etape = { sel?: string; titre: string; texte: string }

/** Étapes par onglet. `sel` vise une zone de la page (dans <main>, sauf si elle commence par « @ ») ; absente ou
 * introuvable, la bulle se centre. */
const ETAPES = (): Record<string, Etape[]> => ({
  dashboard: [
    { sel: '.phead', titre: tr("Le Dashboard de la map"), texte: tr("L’état de la map en un coup d’œil : ce qui avance, ce qui bloque, le prochain objectif.") },
    { sel: 'section.tiles', titre: tr("Les chiffres clés"), texte: tr("Tâches en cours, bloquées, finies, prochain objectif, gains et joueurs du dernier playtest.") },
    { sel: '.two', titre: tr("À faire aujourd’hui"), texte: tr("Vos tâches du jour et les prochains objectifs de la map.") },
    { sel: '@.popy-fab', titre: tr("Popy, l’assistant"), texte: tr("Une question sur POP ? Cliquez ici : Popy répond et vous dit par où commencer.") },
  ],
  planifier: [
    { sel: '.phead', titre: tr("Planifier"), texte: tr("Objectifs, tâches et dates Fortnite sur un même calendrier.") },
    { sel: '.pcal-main', titre: tr("Le calendrier"), texte: tr("Cliquez un jour pour ajouter un événement ; glissez une tâche sur un jour pour la planifier.") },
    { sel: '.pcal-side', titre: tr("Les calendriers"), texte: tr("Cochez ce que vous voulez voir : objectifs, tâches, dates Fortnite…") },
    { sel: '.phead .row', titre: tr("Équipe et faisabilité"), texte: tr("Qui travaille sur la map, combien d’heures, et si la date de sortie tient.") },
  ],
  taches: [
    { sel: '.phead', titre: tr("Les tâches"), texte: tr("La map découpée en tâches d’une heure, avec leurs dépendances.") },
    { sel: '.tabs', titre: tr("Kanban ou dépendances"), texte: tr("Le Kanban pour avancer, le graphe pour voir ce qui bloque quoi.") },
    { sel: '.phead + .row, main > .row', titre: tr("Filtrer"), texte: tr("Par membre, par étiquette ou par mot : ne gardez que ce qui vous concerne.") },
    { sel: '@.plusbtn', titre: tr("Créer une tâche"), texte: tr("Le bouton + crée une tâche, une page ou un tableau en un clic.") },
  ],
  pages: [
    { sel: '.phead', titre: tr("Les pages"), texte: tr("Les docs de la map : règles, design, idées. Vous et vos IA les lisez et les modifiez.") },
    { sel: '.phead .row', titre: tr("Créer ou importer"), texte: tr("Partez d’un modèle UEFN, importez un document ou créez une page vide.") },
    { sel: 'nav.tree', titre: tr("L’arborescence"), texte: tr("Rangez les pages en dossiers ; mettez en favori celles que vous ouvrez souvent.") },
  ],
  tableau: [
    { sel: '.phead', titre: tr("Le tableau blanc"), texte: tr("Posez les idées de la map à plusieurs, en direct.") },
    { sel: '.wbtop', titre: tr("Les outils"), texte: tr("Post-it, formes, flèches, texte ; présentez ou exportez le tableau.") },
    { sel: '.wbbody', titre: tr("Des idées aux tâches"), texte: tr("Votez pour les meilleurs post-it, puis transformez-les en tâches.") },
  ],
  lancer: [
    { sel: '.phead', titre: tr("Lancer et suivre"), texte: tr("Tout pour sortir la map : Discover, vérification, playtests et jour J.") },
    { sel: '.tabs', titre: tr("Les étapes"), texte: tr("Passez d’un onglet à l’autre au fil de la sortie.") },
    { sel: '.dphases', titre: tr("Avant, pendant, après"), texte: tr("Ce qu’il faut faire avant la sortie, pendant les 14 jours de test d’Epic, et après.") },
  ],
  remunerer: [
    { sel: '.phead', titre: tr("Rémunérer"), texte: tr("Importez ce qu’Epic vous a versé : POP calcule la part de chacun.") },
    { sel: 'section.tiles', titre: tr("Le mois en cours"), texte: tr("Ce qui a été versé, les montants fixes et la part en pourcentage.") },
    { sel: '.two', titre: tr("Règles et relevés"), texte: tr("Fixez les règles de partage, puis envoyez à chacun son relevé. POP calcule, il ne transfère jamais d’argent.") },
  ],
  // Écrans de POP Forge (#522), même style.
  'forge-accueil': [
    { sel: '.forge-h1', titre: tr("Bienvenue dans POP Forge"), texte: tr("Vos maps et leur projet UEFN : reliez une map à son projet pour que POP et vos IA pilotent UEFN.") },
    { sel: '@.forge .side nav[aria-label="Forge"]', titre: tr("Les écrans de Forge"), texte: tr("Agents, Plan 2D, Outils, Vérifications, Bibliothèque. Repliez une catégorie au clic sur son titre ; réordonnez en tenant les points ⋮⋮.") },
    { sel: '@.forge .top .badge', titre: tr("L’état du relais"), texte: tr("Vert : UEFN est relié et POP peut agir. Rouge : ouvrez UEFN et son serveur MCP, puis reliez le projet.") },
  ],
  'forge-agents': [
    { sel: '.term-haut', titre: tr("Vos agents"), texte: tr("Chaque terminal est une de vos IA connectées qui travaille sur la map : tâches, UEFN, Verse.") },
    { sel: '.term-haut button', titre: tr("+ Terminal"), texte: tr("Ouvrez le terminal d’une IA connectée. Claude répond tout de suite quand il tourne sur ce PC.") },
    { sel: '.term-grille', titre: tr("Les terminaux"), texte: tr("Écrivez à un agent en bas de son terminal ; ouvrez-le dans une fenêtre à part si vous voulez.") },
  ],
  'forge-terminal': [
    { sel: '@.term-corps', titre: tr("Le terminal de l’agent"), texte: tr("Ce que l’agent fait et vous répond, en direct.") },
    { sel: '@.term-invite', titre: tr("Lui écrire"), texte: tr("Tapez votre demande et Entrée. Ctrl+Espace pour dicter.") },
  ],
  'forge-verifier': [
    { sel: '.forge-h1, .forge-contenu > .stack > .stack > .card', titre: tr("Les vérifications"), texte: tr("POP contrôle le niveau UEFN : ce qui bloque la publication et ce qu’il vaut mieux corriger.") },
    { sel: '.verif-bloq, .verif-erreurs, .forge-contenu > .stack > .stack', titre: tr("Ce qui bloque"), texte: tr("Cliquez un problème pour le voir dans UEFN ou le confier à un agent.") },
    { sel: '.card.stack', titre: tr("Les scénarios de test"), texte: tr("Des parcours de joueur que vos agents rejouent pour vérifier la map.") },
  ],
  'forge-plan': [
    { sel: '.p2', titre: tr("Le plan 2D"), texte: tr("Le niveau UEFN vu de dessus : acteurs, zones et mesures.") },
    { sel: '.p2-apercu, .p2 canvas, .p2 svg', titre: tr("Choisir un acteur"), texte: tr("Cliquez un acteur pour voir son nom et ses réglages ; molette pour zoomer.") },
  ],
  'forge-bibliotheque': [
    { sel: '.forge-h1, .forge-contenu > .stack', titre: tr("La bibliothèque"), texte: tr("Ce que le studio réutilise : modules Verse, assets 3D, images et sons.") },
    { sel: '.tabs', titre: tr("Public ou à vous"), texte: tr("Les créations publiques de la communauté, et les vôtres.") },
  ],
  'forge-outils': [
    { sel: '.outils-grille', titre: tr("Les outils"), texte: tr("Miniature, Texte 3D, Texte 2D, Low Mesh : des outils qui préparent les assets de la map sur votre PC.") },
    { sel: '.outil-carte', titre: tr("Ouvrir un outil"), texte: tr("Cliquez une carte. Rien n’est importé dans UEFN sans votre accord, et chaque import s’annule depuis le journal.") },
  ],
  analyse: [
    { sel: '.afaire-start, .phead', titre: tr("Maps à faire"), texte: tr("POP cherche les genres qui attirent les joueurs et où il reste de la place.") },
    { sel: '.afaire-srcs', titre: tr("Les sources"), texte: tr("Connectez YouTube, TikTok, X ou Reddit pour voir aussi ce qui buzze en vidéo. Facultatif.") },
  ],
})

function cible(sel?: string): HTMLElement | null {
  if (!sel) return null
  for (const s of sel.split(',').map(x => x.trim())) {
    const el = s.startsWith('@') ? document.querySelector(s.slice(1)) : document.querySelector(`main ${s}`) ?? document.querySelector(s)
    if (el instanceof HTMLElement && el.getClientRects().length > 0) return el
  }
  return null
}

const zoom = () => parseFloat(getComputedStyle(document.documentElement).zoom || '1') || 1
const MARGE = 8

/** Contour d'un rectangle arrondi pour le voile (règle evenodd : la zone reste nette). */
function trou(W: number, H: number, r: DOMRect | null) {
  const cadre = `M0 0H${W}V${H}H0Z`
  if (!r) return `path(evenodd, '${cadre}')`
  const x = r.left - MARGE, y = r.top - MARGE, w = r.width + 2 * MARGE, h = r.height + 2 * MARGE, k = Math.min(14, w / 2, h / 2)
  return `path(evenodd, '${cadre} M${x + k} ${y}H${x + w - k}A${k} ${k} 0 0 1 ${x + w} ${y + k}V${y + h - k}A${k} ${k} 0 0 1 ${x + w - k} ${y + h}H${x + k}A${k} ${k} 0 0 1 ${x} ${y + h - k}V${y + k}A${k} ${k} 0 0 1 ${x + k} ${y}Z')`
}

/** Touches qui font défiler la page : bloquées pendant le didacticiel. */
const TOUCHES = new Set([' ', 'PageUp', 'PageDown', 'Home', 'End', 'ArrowUp', 'ArrowDown'])

/** Affiche le didacticiel de l'onglet `page` s'il n'a pas encore été vu par ce compte. */
export function Didacticiel({ page }: { page: string }) {
  const { me } = useApp()
  const etapes = ETAPES()[page]
  const vus: string[] = me?.ui_prefs?.tours ?? []
  const [i, setI] = useState(0)
  const [pret, setPret] = useState(false)
  const [fini, setFini] = useState<string[]>([])
  const [r, setR] = useState<DOMRect | null>(null)
  const [, setTaille] = useState(0)
  const actif = !!me && !!etapes && !vus.includes(page) && !fini.includes(page)
  const fin = () => {
    setFini(f => [...f, page])
    const all = { ...(me!.ui_prefs ?? {}), tours: [...vus, page] }
    me!.ui_prefs = all
    api.put('/api/me/ui-prefs', all).catch(() => {})
  }
  // On attend que la page ait dessiné ses zones avant la première bulle.
  useEffect(() => { setI(0); setPret(false); if (!actif) return; const t = setTimeout(() => setPret(true), 900); return () => clearTimeout(t) }, [page, actif])
  // Pas de défilement à la main pendant le didacticiel (molette, tactile, clavier) ; ← → et Échap le pilotent.
  useEffect(() => {
    if (!actif || !pret) return
    const bloque = (e: Event) => { if (!(e.target as HTMLElement)?.closest?.('.tuto')) e.preventDefault() }
    const touche = (e: KeyboardEvent) => {
      if (TOUCHES.has(e.key) && !(e.target as HTMLElement)?.closest?.('input, textarea, [contenteditable]')) e.preventDefault()
    }
    const o = { passive: false, capture: true } as AddEventListenerOptions
    window.addEventListener('wheel', bloque, o); window.addEventListener('touchmove', bloque, o); window.addEventListener('keydown', touche, o)
    return () => { window.removeEventListener('wheel', bloque, o); window.removeEventListener('touchmove', bloque, o); window.removeEventListener('keydown', touche, o) }
  }, [actif, pret])
  useLayoutEffect(() => {
    if (!actif || !pret) return
    // La page défile toute seule jusqu'à la zone si elle est coupée ou hors de l'écran.
    const el = cible(etapes[i]?.sel)
    if (el) {
      const b = el.getBoundingClientRect(), H = window.innerHeight
      if (b.top < 70 || b.bottom > H - 20) el.scrollIntoView({ block: b.height > H * 0.6 ? 'start' : 'center', inline: 'nearest', behavior: 'smooth' })
    }
    // L'interface est zoomée à 90 % sur grand écran (html { zoom }) : on ramène la zone en pixels CSS.
    const maj = () => {
      const b = cible(etapes[i]?.sel)?.getBoundingClientRect()
      const z = zoom()
      // Zone gardée à l'intérieur de l'écran, avec la même marge que le contour.
      const vw = window.innerWidth / z, vh = window.innerHeight / z, m = MARGE + 4
      if (!b) { setR(null) } else {
        const l = Math.max(m, b.left / z), t = Math.max(m, b.top / z), rr = Math.min(vw - m, b.right / z), bb = Math.min(vh - m, b.bottom / z)
        setR(new DOMRect(l, t, Math.max(0, rr - l), Math.max(0, bb - t)))
      }
      setTaille(window.innerWidth * 10000 + window.innerHeight)
    }
    maj(); const t1 = setTimeout(maj, 300), t2 = setTimeout(maj, 700)
    window.addEventListener('resize', maj); window.addEventListener('scroll', maj, true)
    return () => { clearTimeout(t1); clearTimeout(t2); window.removeEventListener('resize', maj); window.removeEventListener('scroll', maj, true) }
  }, [i, pret, actif, page])
  if (!actif || !pret) return null
  const e = etapes[i]
  const avant = () => setI(Math.max(0, i - 1))
  const apres = () => i + 1 < etapes.length ? setI(i + 1) : fin()
  // Bulle sous la zone visée, sinon au-dessus, sinon en bas de l'écran ; centrée sans zone.
  const W = 340, HB = 200
  const z = zoom()
  const vw = window.innerWidth / z, vh = window.innerHeight / z
  const style: React.CSSProperties = r
    ? { left: Math.max(12, Math.min(vw - W - 12, r.left + Math.min(r.width, 400) / 2 - W / 2)),
        top: r.bottom + MARGE + 12 + HB < vh ? r.bottom + MARGE + 12 : r.top - MARGE - 12 - HB > 0 ? r.top - MARGE - 12 - HB : vh - HB - 16 }
    : { left: '50%', top: '40%', transform: 'translate(-50%, -50%)' }
  return <>
    <div className="tuto-voile" style={{ clipPath: trou(vw, vh, r) }} aria-hidden="true" onClick={ev => ev.stopPropagation()} />
    {r && <div className="tuto-zone" style={{ left: r.left - MARGE, top: r.top - MARGE, width: r.width + 2 * MARGE, height: r.height + 2 * MARGE }} aria-hidden="true" />}
    <div className="tuto" style={{ ...style, width: W }} role="dialog" aria-modal="true" aria-label={tr("Didacticiel")}
      onKeyDown={ev => { if (ev.key === 'ArrowRight') apres(); else if (ev.key === 'ArrowLeft') avant(); else if (ev.key === 'Escape') fin() }}>
      <div className="row" style={{ justifyContent: 'space-between', alignItems: 'center' }}>
        <span className="tuto-pas">{etapes.map((_, k) => <i key={k} className={k === i ? 'on' : k < i ? 'vu' : ''} />)}<span className="small muted">{tr("{0} sur {1}", [String(i + 1), String(etapes.length)])}</span></span>
        <button className="iconbtn" aria-label={tr("Passer le didacticiel")} onClick={fin}><X size={16} /></button>
      </div>
      <b>{e.titre}</b>
      <p className="small" style={{ margin: '6px 0 12px' }}>{e.texte}</p>
      <div className="row" style={{ gap: 8, alignItems: 'center' }}>
        <button className="btn ghost" onClick={fin}>{tr("Passer")}</button>
        <span style={{ flex: 1 }} />
        <button className="btn" onClick={avant} disabled={i === 0}>{tr("Précédent")}</button>
        <button className="btn pri" autoFocus onClick={apres}>{i + 1 < etapes.length ? tr("Suivant") : tr("Terminer")}</button>
      </div>
    </div>
  </>
}
