// Vue « Dépendances » des tâches (#163) : qui bloque qui, groupé par fonctionnalité (le lot d'une tâche).
// Ce qui débloque est à gauche : la colonne d'une tâche est la longueur de la plus longue chaîne de dépendances avant elle.
import { useMemo, useRef, useState } from 'react'
import { Check, Lock, Play, Trash2, X } from '../icons'
import { useLoad } from '../state'
import { tr } from '../i18n/t'
import { tagLabel } from './ui'

export type GraphTask = {
  id: number; number: number; title: string; tag: string; status: string; lot: string
  assignee_id: number | null; assignee_name: string | null; deps: number[]
}

export type NodeState = 'done' | 'doing' | 'ready' | 'blocked'

/** État lisible sans couleur : faite, en cours, prenable (toutes ses dépendances faites) ou bloquée. */
export function nodeState(t: GraphTask, byNum: Map<number, GraphTask>): NodeState {
  if (t.status === 'done') return 'done'
  if (t.status === 'doing') return 'doing'
  const open = t.deps.some(n => byNum.get(n) && byNum.get(n)!.status !== 'done')
  return open || t.status === 'blocked' ? 'blocked' : 'ready'
}

/** Colonne de chaque tâche : 0 sans dépendance, sinon 1 + la plus grande colonne de ses dépendances (cycles ignorés). */
export function levels(tasks: GraphTask[]): Map<number, number> {
  const byNum = new Map(tasks.map(t => [t.number, t]))
  const lv = new Map<number, number>()
  const seen = new Set<number>()
  const walk = (n: number): number => {
    if (lv.has(n)) return lv.get(n)!
    if (seen.has(n)) return 0 // cycle : on coupe
    seen.add(n)
    const t = byNum.get(n)
    const l = t ? Math.max(-1, ...t.deps.filter(d => byNum.has(d)).map(walk)) + 1 : 0
    lv.set(n, l)
    return l
  }
  tasks.forEach(t => walk(t.number))
  return lv
}

const NW = 200, NH = 66, CW = 256, RH = 80, HEAD = 34, PAD = 14, GAP = 18
const NOLOT = '\u0000'

const LABEL: Record<NodeState, string> = { done: 'Faite', doing: 'En cours', ready: 'Prenable', blocked: 'Bloquée' }

/** Édition (#182) : onLink(de, vers) ajoute « vers dépend de de » ; onUnlink la retire. Absentes = lecture seule. */
export default function DepGraph({ mapId, tasks, match, onOpen, onLink, onUnlink }: {
  mapId: number; tasks: GraphTask[]; match: (t: GraphTask) => boolean; onOpen: (id: number) => void
  onLink?: (from: number, to: number) => void; onUnlink?: (from: number, to: number) => void
}) {
  const box = useRef<HTMLDivElement>(null)
  const [link, setLink] = useState<{ from: number; x: number; y: number } | null>(null)
  const [sel, setSel] = useState<{ from: number; to: number } | null>(null)
  // Coordonnées du graphe depuis la souris (la page peut être zoomée : on rapporte à la taille réelle).
  const at = (e: React.PointerEvent) => {
    const r = box.current!.getBoundingClientRect(), k = r.width / box.current!.offsetWidth || 1
    return { x: (e.clientX - r.left) / k, y: (e.clientY - r.top) / k }
  }
  const startLink = (e: React.PointerEvent, from: number) => {
    e.preventDefault(); e.stopPropagation(); setSel(null)
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
    setLink({ from, ...at(e) })
  }
  const moveLink = (e: React.PointerEvent) => { if (link) setLink({ ...link, ...at(e) }) }
  const endLink = (e: React.PointerEvent) => {
    if (!link) return
    const el = document.elementFromPoint(e.clientX, e.clientY)?.closest('[data-num]') as HTMLElement | null
    const to = el ? Number(el.dataset.num) : NaN
    setLink(null)
    if (to && to !== link.from && onLink) onLink(link.from, to)
  }
  const { data: tl } = useLoad<any>(`/api/maps/${mapId}/plan/timeline`, [mapId, tasks.length])
  const [hover, setHover] = useState<number | null>(null)
  const critical = useMemo(() => new Set<number>((tl?.tasks ?? []).filter((t: any) => t.critical).map((t: any) => t.id)), [tl])

  const g = useMemo(() => {
    const byNum = new Map(tasks.map(t => [t.number, t]))
    const lv = levels(tasks)
    // Lots dans l'ordre de leur première tâche ; « Sans fonctionnalité » à la fin.
    const order: string[] = []
    for (const t of [...tasks].sort((a, b) => a.number - b.number)) { const k = t.lot.trim() || NOLOT; if (!order.includes(k)) order.push(k) }
    order.sort((a, b) => (a === NOLOT ? 1 : 0) - (b === NOLOT ? 1 : 0))
    const pos = new Map<number, { x: number; y: number }>()
    const groups: { name: string; y: number; h: number }[] = []
    let y = 0
    const maxLv = Math.max(0, ...lv.values())
    for (const k of order) {
      const mine = tasks.filter(t => (t.lot.trim() || NOLOT) === k).sort((a, b) => a.number - b.number)
      const perCol = new Map<number, number>()
      for (const t of mine) {
        const c = lv.get(t.number) ?? 0
        const r = perCol.get(c) ?? 0
        perCol.set(c, r + 1)
        pos.set(t.number, { x: PAD + c * CW, y: y + HEAD + r * RH })
      }
      const rows = Math.max(1, ...perCol.values())
      const h = HEAD + rows * RH + PAD - (RH - NH)
      groups.push({ name: k === NOLOT ? tr('Sans fonctionnalité') : k, y, h })
      y += h + GAP
    }
    const width = PAD * 2 + maxLv * CW + NW
    const edges = tasks.flatMap(t => t.deps.filter(d => pos.has(d)).map(d => ({ from: d, to: t.number })))
    return { byNum, pos, groups, edges, width, height: Math.max(0, y - GAP) }
  }, [tasks])

  const near = useMemo(() => {
    if (hover === null) return null
    const s = new Set<number>([hover])
    g.edges.forEach(e => { if (e.to === hover) s.add(e.from); if (e.from === hover) s.add(e.to) })
    return s
  }, [hover, g])

  if (!tasks.length) return <p className="muted">{tr('Aucune tâche pour le moment.')}</p>

  return (
    <section className="card">
      <div className="row small muted" style={{ flexWrap: 'wrap', gap: 14, marginBottom: 12 }}>
        <span>{tr('Une flèche va de la tâche qui débloque vers celle qui l’attend. Survolez une tâche pour voir ses liens ; cliquez pour l’ouvrir.')}</span>
        <span className="dg-legend"><span className="dg-node dg-ready dg-mini" />{tr('Prenable')}</span>
        <span className="dg-legend"><span className="dg-node dg-doing dg-mini" />{tr('En cours')}</span>
        <span className="dg-legend"><span className="dg-node dg-blocked dg-mini" />{tr('Bloquée')}</span>
        <span className="dg-legend"><span className="dg-node dg-done dg-mini" />{tr('Faite')}</span>
        <span className="dg-legend"><span className="dg-node dg-ready dg-crit dg-mini" />{tr('Chemin critique')}</span>
      </div>
      {onLink && <p className="small muted" style={{ margin: '0 0 10px' }}>{tr('Pour ajouter une dépendance, tirez le rond à droite d’une tâche vers la tâche qui l’attend. Cliquez une flèche pour la retirer.')}</p>}
      {sel && onUnlink && <div className="row dg-sel" role="status">
        <span>{tr('Flèche #{0} → #{1} : #{1} attend #{0}.', [sel.from, sel.to])}</span>
        <button className="btn" onClick={() => { onUnlink(sel.from, sel.to); setSel(null) }}><Trash2 size={15} />{tr('Retirer la dépendance')}</button>
        <button className="iconbtn" aria-label={tr('Annuler')} onClick={() => setSel(null)}><X size={16} /></button>
      </div>}
      <div className="dg-wrap" tabIndex={-1} onKeyDown={e => { if (sel && onUnlink && (e.key === 'Delete' || e.key === 'Backspace')) { onUnlink(sel.from, sel.to); setSel(null) } if (e.key === 'Escape') { setSel(null); setLink(null) } }}>
        <div className="dg" ref={box} style={{ width: g.width, height: g.height }} onPointerMove={moveLink} onPointerUp={endLink}>
          {g.groups.map(gr => <div key={gr.name} className="dg-group" style={{ top: gr.y, height: gr.h, width: g.width }}><span className="dg-gname">{gr.name}</span></div>)}
          <svg className="dg-svg" width={g.width} height={g.height} aria-hidden="true">
            <defs>
              <marker id="dg-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="currentColor" /></marker>
            </defs>
            {g.edges.map(e => {
              const a = g.pos.get(e.from)!, b = g.pos.get(e.to)!
              const x1 = a.x + NW, y1 = a.y + NH / 2, x2 = b.x - 2, y2 = b.y + NH / 2
              const dx = Math.max(40, (x2 - x1) / 2)
              const on = near ? near.has(e.from) && near.has(e.to) && (e.from === hover || e.to === hover) : false
              const done = g.byNum.get(e.from)?.status === 'done'
              const d = `M${x1},${y1} C${x1 + dx},${y1} ${x2 - dx},${y2} ${x2},${y2}`
              const picked = sel?.from === e.from && sel?.to === e.to
              return <g key={`${e.from}-${e.to}`}>
                <path d={d} className={'dg-edge' + (done ? ' done' : '') + (on || picked ? ' on' : '') + (near && !on ? ' dim' : '')} markerEnd="url(#dg-arrow)" />
                {onUnlink && <path d={d} className="dg-hit" onClick={() => setSel({ from: e.from, to: e.to })}><title>{`#${e.from} → #${e.to}`}</title></path>}
              </g>
            })}
            {link && (() => { const a = g.pos.get(link.from)!; return <path d={`M${a.x + NW},${a.y + NH / 2} L${link.x},${link.y}`} className="dg-edge on dg-draft" markerEnd="url(#dg-arrow)" /> })()}
          </svg>
          {[...g.byNum.values()].map(t => {
            const p = g.pos.get(t.number); if (!p) return null
            const s = nodeState(t, g.byNum)
            const waiting = t.deps.filter(n => g.byNum.get(n) && g.byNum.get(n)!.status !== 'done')
            const dim = (near && !near.has(t.number)) || !match(t)
            return (
              <span key={t.id}>
              <button data-num={t.number} className={`dg-node dg-${s}` + (critical.has(t.id) ? ' dg-crit' : '') + (dim ? ' dim' : '') + (hover === t.number ? ' on' : '')}
                style={{ left: p.x, top: p.y, width: NW, height: NH }}
                onMouseEnter={() => setHover(t.number)} onMouseLeave={() => setHover(null)} onFocus={() => setHover(t.number)} onBlur={() => setHover(null)}
                onClick={() => onOpen(t.id)}
                title={`#${t.number} ${t.title}\n${tr(LABEL[s])}${waiting.length ? ' · ' + tr('attend') + ' #' + waiting.join(', #') : ''}${critical.has(t.id) ? ' · ' + tr('chemin critique') : ''}`}
                aria-label={`#${t.number} ${t.title}, ${tr(LABEL[s])}`}>
                <span className="dg-top">
                  <span className="mono">#{t.number}</span>
                  <span className="dg-state">{s === 'done' ? <Check size={12} /> : s === 'blocked' ? <Lock size={11} /> : s === 'doing' ? <Play size={11} /> : null}{tr(LABEL[s])}</span>
                </span>
                <span className="dg-title">{t.title}</span>
                <span className="dg-meta">{tagLabel(t.tag)}{t.assignee_name ? ' · ' + t.assignee_name : ''}</span>
              </button>
              {onLink && <span className="dg-port" style={{ left: p.x + NW - 7, top: p.y + NH / 2 - 7 }} title={tr('Tirer vers la tâche qui attend celle-ci')} onPointerDown={e => startLink(e, t.number)} />}
              </span>
            )
          })}
        </div>
      </div>
    </section>
  )
}
