// Panneau de choix d'icône de page (#197). Chargé seulement à l'ouverture (import dynamique) :
// toute la bibliothèque Lucide (licence ISC), ses mots-clés et la liste des emojis (unicode-emoji-json, MIT).
import { useEffect, useMemo, useRef, useState } from 'react'
import data from './iconData.json'
import { tr } from '../i18n/t'
import { lucideIcon } from './lucideAll'

const TAGS = data.tags as Record<string, string[]>
const EMOJIS = data.emojis as [string, [string, string][]][]

const NAMES = Object.keys(TAGS).filter(n => lucideIcon(n))

// Mots-clés français → termes anglais de Lucide (nom ou étiquette).
const FR: Record<string, string[]> = {
  'épée': ['sword', 'swords'], 'arme': ['sword', 'swords', 'axe', 'crosshair', 'bomb'], 'argent': ['coins', 'banknote', 'wallet', 'dollar', 'euro', 'piggy-bank', 'money'],
  'pièce': ['coins'], 'monnaie': ['coins', 'banknote'], 'trophée': ['trophy', 'award', 'medal'], 'victoire': ['trophy', 'crown', 'medal'], 'couronne': ['crown'],
  'carte': ['map', 'map-pin', 'compass'], 'monde': ['globe', 'earth', 'map'], 'manette': ['gamepad', 'joystick'], 'jeu': ['gamepad', 'dice', 'puzzle', 'joystick'],
  'dé': ['dice'], 'bouclier': ['shield'], 'cœur': ['heart'], 'coeur': ['heart'], 'vie': ['heart', 'heart-pulse'], 'étoile': ['star', 'sparkles'], 'feu': ['flame', 'fire'],
  'magie': ['wand', 'sparkles'], 'potion': ['flask'], 'clé': ['key'], 'cle': ['key'], 'coffre': ['package', 'archive', 'box'], 'boîte': ['box', 'package'],
  'arbre': ['tree', 'trees'], 'forêt': ['trees'], 'montagne': ['mountain'], 'eau': ['droplet', 'waves'], 'mer': ['waves', 'anchor', 'ship'], 'île': ['palmtree'],
  'maison': ['house', 'home'], 'château': ['castle'], 'bâtiment': ['building', 'house'], 'voiture': ['car'], 'véhicule': ['car', 'truck', 'bike', 'plane'],
  'avion': ['plane'], 'bateau': ['ship', 'sailboat'], 'temps': ['clock', 'timer', 'hourglass'], 'chrono': ['timer', 'stopwatch'], 'calendrier': ['calendar'],
  'tâche': ['check', 'list-todo', 'square-check'], 'liste': ['list'], 'idée': ['lightbulb'], 'note': ['notebook', 'sticky-note', 'file-text'], 'doc': ['file-text', 'book'],
  'livre': ['book'], 'règle': ['scale', 'gavel', 'book'], 'équipe': ['users'], 'joueur': ['user', 'users'], 'personne': ['user'], 'groupe': ['users'],
  'message': ['message', 'mail'], 'son': ['volume', 'music', 'speaker'], 'musique': ['music'], 'image': ['image'], 'photo': ['camera', 'image'], 'vidéo': ['video', 'film'],
  'code': ['code', 'terminal', 'braces'], 'bug': ['bug'], 'outil': ['wrench', 'hammer', 'settings'], 'réglage': ['settings', 'sliders'], 'paramètre': ['settings'],
  'cible': ['target', 'crosshair'], 'objectif': ['target', 'flag'], 'drapeau': ['flag'], 'lancement': ['rocket'], 'fusée': ['rocket'], 'graphique': ['chart'],
  'stat': ['chart', 'bar-chart'], 'boutique': ['store', 'shopping'], 'achat': ['shopping-cart', 'shopping-bag'], 'cadeau': ['gift'], 'éclair': ['zap'], 'énergie': ['zap', 'battery'],
  'danger': ['alert', 'skull', 'triangle-alert'], 'mort': ['skull'], 'crâne': ['skull'], 'monstre': ['ghost', 'skull', 'bug'], 'fantôme': ['ghost'], 'neige': ['snowflake'],
  'soleil': ['sun'], 'lune': ['moon'], 'nuit': ['moon'], 'météo': ['cloud', 'sun', 'snowflake'], 'porte': ['door'], 'serrure': ['lock'], 'verrou': ['lock'], 'sécurité': ['shield', 'lock'],
  'niveau': ['layers', 'signal', 'gauge'], 'vitesse': ['gauge', 'zap', 'rabbit'], 'saut': ['arrow-up', 'chevrons-up'], 'course': ['footprints', 'timer'],
}

// Catégories : noms (ou débuts de noms) Lucide.
const CATS: [string, string[]][] = [
  ['Jeu', ['gamepad', 'joystick', 'dice', 'puzzle', 'sword', 'swords', 'shield', 'crown', 'trophy', 'medal', 'award', 'target', 'crosshair', 'skull', 'ghost', 'heart', 'star', 'flame', 'wand', 'sparkles', 'bomb', 'axe', 'flask', 'key', 'castle', 'zap', 'gem', 'flag', 'swords', 'drama', 'party-popper', 'rocket', 'footprints', 'hourglass']],
  ['Économie', ['coins', 'banknote', 'wallet', 'piggy-bank', 'dollar', 'euro', 'badge-dollar', 'credit-card', 'receipt', 'store', 'shopping', 'gift', 'hand-coins', 'landmark', 'scale', 'trending', 'chart', 'percent', 'gem', 'package']],
  ['Carte et décor', ['map', 'compass', 'globe', 'earth', 'mountain', 'tree', 'palmtree', 'trees', 'leaf', 'flower', 'waves', 'droplet', 'sun', 'moon', 'cloud', 'snowflake', 'house', 'home', 'building', 'castle', 'tent', 'warehouse', 'factory', 'church', 'car', 'truck', 'plane', 'ship', 'sailboat', 'bike', 'train', 'anchor', 'door', 'fence', 'lamp', 'route', 'signpost', 'milestone']],
  ['Projet', ['calendar', 'clock', 'timer', 'list', 'square-check', 'check', 'clipboard', 'file', 'folder', 'book', 'notebook', 'sticky-note', 'lightbulb', 'goal', 'flag', 'milestone', 'kanban', 'gantt', 'layers', 'layout', 'code', 'terminal', 'bug', 'git', 'wrench', 'hammer', 'settings', 'sliders', 'rocket', 'bar-chart', 'pie-chart']],
  ['Équipe', ['user', 'users', 'contact', 'message', 'mail', 'phone', 'at-sign', 'handshake', 'hand', 'thumbs', 'smile', 'heart', 'megaphone', 'bell', 'share', 'video', 'mic', 'headphones', 'shield-check', 'badge']],
]
const inCat = (n: string, keys: string[]) => keys.some(k => n === k || n.startsWith(k + '-'))

const RECENT = 'pop.icons.recent'
const recent = (): string[] => { try { return JSON.parse(localStorage.getItem(RECENT) ?? '[]') } catch { return [] } }

export default function IconPicker({ onPick, onClose }: { onPick: (v: string) => void; onClose: () => void }) {
  const [q, setQ] = useState('')
  const [tab, setTab] = useState<'icons' | 'emojis' | 'recent'>('icons')
  const [cat, setCat] = useState('Toutes')
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const off = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) onClose() }
    const esc = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    document.addEventListener('mousedown', off); document.addEventListener('keydown', esc)
    return () => { document.removeEventListener('mousedown', off); document.removeEventListener('keydown', esc) }
  }, [onClose])
  const pick = (v: string) => {
    try { localStorage.setItem(RECENT, JSON.stringify([v, ...recent().filter(x => x !== v)].slice(0, 32))) } catch { /* stockage indisponible */ }
    onPick(v)
  }
  const query = q.trim().toLowerCase()
  const terms = useMemo(() => {
    if (!query) return []
    const out = [query]
    for (const [fr, en] of Object.entries(FR)) if (fr.startsWith(query) || query.startsWith(fr)) out.push(...en)
    return out
  }, [query])
  const iconList = useMemo(() => {
    let l = NAMES
    if (cat !== 'Toutes' && !query) l = l.filter(n => inCat(n, CATS.find(c => c[0] === cat)![1]))
    if (query) {
      // Classement : nom exact, nom qui commence par le mot, mot du nom, puis étiquettes (début de mot seulement).
      const score = (n: string) => {
        let best = 99
        for (const t of terms) {
          const parts = n.split('-')
          if (n === t) best = Math.min(best, 0)
          else if (n.startsWith(t)) best = Math.min(best, 1)
          else if (parts.some(w => w.startsWith(t))) best = Math.min(best, 2)
          else if (TAGS[n].some(g => g === t)) best = Math.min(best, 3)
          else if (TAGS[n].some(g => g.split(' ').some(w => w.startsWith(t)))) best = Math.min(best, 4)
        }
        return best
      }
      l = l.map(n => [n, score(n)] as const).filter(x => x[1] < 99).sort((a, b) => a[1] - b[1]).map(x => x[0])
    }
    return l
  }, [cat, query, terms])
  const emojiList = useMemo(() => EMOJIS.map(([g, es]) => [g, query ? es.filter(([e, n]) => e === query || terms.some(t => n.split(' ').some(w => w.startsWith(t)))).sort((a, b) => Number(!terms.some(t => a[1] === t)) - Number(!terms.some(t => b[1] === t))) : es] as const).filter(([, es]) => es.length), [query, terms])

  const Ico = ({ v }: { v: string }) => {
    if (v.startsWith('lucide:')) { const C = lucideIcon(v.slice(7)); return C ? <C size={18} /> : null }
    return <span style={{ fontSize: 18 }}>{v}</span>
  }
  return (
    <div className="ipick card" ref={ref} role="dialog" aria-label={tr("Choisir une icône")}>
      <div className="row" style={{ gap: 6 }}>
        <input className="field" autoFocus placeholder={tr("Rechercher (épée, argent, carte…)")} value={q} onChange={e => setQ(e.target.value)} style={{ flex: 1, height: 36 }} />
        <button className="btn sm" onClick={() => onPick('')}>{tr("Retirer")}</button>
      </div>
      <div className="row ipick-tabs">
        {([['icons', tr("Icônes")], ['emojis', tr("Emojis")], ['recent', tr("Récentes")]] as const).map(([k, l]) => <button key={k} className={tab === k ? 'on' : ''} onClick={() => setTab(k)}>{l}</button>)}
      </div>
      {tab === 'icons' && !query && <div className="row ipick-cats">{['Toutes', ...CATS.map(c => c[0])].map(c => <button key={c} className={cat === c ? 'on' : ''} onClick={() => setCat(c)}>{tr(c)}</button>)}</div>}
      <div className="ipick-body">
        {tab === 'icons' && <>
          <div className="small muted" style={{ margin: '2px 0 6px' }}>{tr("{0} icônes", [iconList.length])}</div>
          <div className="ipick-grid">{iconList.slice(0, 600).map(n => { const C = lucideIcon(n); return <button key={n} title={n} aria-label={n} onClick={() => pick('lucide:' + n)}><C size={18} /></button> })}</div>
          {iconList.length > 600 && <div className="small muted" style={{ marginTop: 6 }}>{tr("Affinez la recherche pour voir les autres.")}</div>}
          {iconList.length === 0 && <div className="small muted">{tr("Aucune icône.")}</div>}
        </>}
        {tab === 'emojis' && emojiList.map(([g, es]) => <div key={g}>
          <div className="small muted" style={{ margin: '6px 0 4px' }}>{tr(g)}</div>
          <div className="ipick-grid">{es.map(([e, n]) => <button key={e} title={n} aria-label={n} onClick={() => pick(e)}><span style={{ fontSize: 18 }}>{e}</span></button>)}</div>
        </div>)}
        {tab === 'recent' && <div className="ipick-grid">{recent().map(v => <button key={v} onClick={() => pick(v)}><Ico v={v} /></button>)}</div>}
        {tab === 'recent' && recent().length === 0 && <div className="small muted">{tr("Aucune icône récente.")}</div>}
      </div>
    </div>
  )
}
