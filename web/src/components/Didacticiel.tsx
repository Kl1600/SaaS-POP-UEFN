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

/** Affiche le didacticiel de l'onglet `page` s'il n'a pas encore été vu par ce compte. */
export function Didacticiel({ page }: { page: string }) {
  const { me } = useApp()
  const etapes = ETAPES()[page]
  const vus: string[] = me?.ui_prefs?.tours ?? []
  const [i, setI] = useState(0)
  const [pret, setPret] = useState(false)
  const [fini, setFini] = useState<string[]>([])
  const [r, setR] = useState<DOMRect | null>(null)
  const actif = !!me && !!etapes && !vus.includes(page) && !fini.includes(page)
  // On attend que la page ait dessiné ses zones avant la première bulle.
  useEffect(() => { setI(0); setPret(false); if (!actif) return; const t = setTimeout(() => setPret(true), 900); return () => clearTimeout(t) }, [page, actif])
  useLayoutEffect(() => {
    if (!actif || !pret) return
    const el = cible(etapes[i]?.sel)
    if (el) el.scrollIntoView({ block: 'nearest' })
    // L'interface est zoomée à 90 % sur grand écran (html { zoom }) : on ramène la zone en pixels CSS.
    const maj = () => {
      const b = cible(etapes[i]?.sel)?.getBoundingClientRect()
      const z = parseFloat(getComputedStyle(document.documentElement).zoom || '1') || 1
      setR(b ? new DOMRect(b.left / z, b.top / z, b.width / z, b.height / z) : null)
    }
    maj(); const t = setTimeout(maj, 350)
    window.addEventListener('resize', maj); window.addEventListener('scroll', maj, true)
    return () => { clearTimeout(t); window.removeEventListener('resize', maj); window.removeEventListener('scroll', maj, true) }
  }, [i, pret, actif, page])
  if (!actif || !pret) return null
  const fin = () => {
    setFini(f => [...f, page])
    const all = { ...(me!.ui_prefs ?? {}), tours: [...vus, page] }
    me!.ui_prefs = all
    api.put('/api/me/ui-prefs', all).catch(() => {})
  }
  const e = etapes[i]
  // Bulle sous la zone visée, ou au-dessus s'il n'y a pas la place ; centrée sans zone.
  const W = 340
  const z = parseFloat(getComputedStyle(document.documentElement).zoom || '1') || 1
  const vw = window.innerWidth / z, vh = window.innerHeight / z
  const style: React.CSSProperties = r
    ? { left: Math.max(12, Math.min(vw - W - 12, r.left + Math.min(r.width, 400) / 2 - W / 2)), ...(r.bottom + 190 < vh ? { top: r.bottom + 12 } : { top: Math.max(12, r.top - 190) }) }
    : { left: '50%', top: '40%', transform: 'translate(-50%, -50%)' }
  return <>
    {r && <div className="tuto-zone" style={{ left: r.left - 6, top: r.top - 6, width: r.width + 12, height: r.height + 12 }} aria-hidden="true" />}
    <div className="tuto" style={{ ...style, width: W }} role="dialog" aria-label={tr("Didacticiel")}>
      <div className="row" style={{ justifyContent: 'space-between', alignItems: 'center' }}>
        <span className="small muted">{tr("{0} sur {1}", [String(i + 1), String(etapes.length)])}</span>
        <button className="iconbtn" aria-label={tr("Passer le didacticiel")} onClick={fin}><X size={16} /></button>
      </div>
      <b>{e.titre}</b>
      <p className="small" style={{ margin: '6px 0 12px' }}>{e.texte}</p>
      <div className="row" style={{ gap: 8, justifyContent: 'flex-end' }}>
        {i === 0 ? <button className="btn" onClick={fin}>{tr("Passer")}</button> : <button className="btn" onClick={() => setI(i - 1)}>{tr("Précédent")}</button>}
        <button className="btn pri" onClick={() => i + 1 < etapes.length ? setI(i + 1) : fin()}>{i + 1 < etapes.length ? tr("Suivant") : tr("Terminer")}</button>
      </div>
    </div>
  </>
}
