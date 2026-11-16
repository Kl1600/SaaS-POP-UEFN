import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import {
  MousePointer2, Lasso, StickyNote, Type, Square, Circle, Spline, Pen, Highlighter, Eraser, Frame, ImagePlus, MessageSquarePlus, ThumbsUp,
  Timer, Eye, EyeOff, LayoutGrid, Presentation, Download, Plus, Minus, Maximize, UserRound, ChevronLeft, ChevronRight, X, Network,
  Expand, Shrink, Undo2, Redo2, Copy, Trash2, BringToFront, SendToBack, AlignStartVertical, AlignStartHorizontal, AlignCenterVertical, AlignCenterHorizontal, Upload, Pencil,
} from 'lucide-react'
import { api } from '../api'
import { useApp, useLoad } from '../state'
import { PageHead } from '../components/ui'
import { useMembers } from './Tasks'

type Obj = { id: string; kind: string; x: number; y: number; w?: number; h?: number; text?: string; shade?: number; votes?: Record<string, number>; author?: string; author_id?: number; task_id?: number; task_number?: number; from?: string; to?: string; style?: string; dashed?: boolean; arrow?: boolean; points?: number[][]; color?: string; width?: number; shape?: string; url?: string; locked?: boolean; reactions?: number } & Record<string, any>

const uid = () => Math.random().toString(36).slice(2, 12)
const SHADES = [['#FFFFFF', '#0A0A0A'], ['#E5E5E5', '#0A0A0A'], ['#A3A3A3', '#0A0A0A'], ['#404040', '#FFFFFF'], ['#FDF0A6', '#0A0A0A'], ['#C9EDD0', '#0A0A0A'], ['#CBE0FA', '#0A0A0A'], ['#F8D0E1', '#0A0A0A'], ['#FCD9B8', '#0A0A0A']]
const SHADE_NAMES = ['Blanc', 'Gris clair', 'Gris moyen', 'Gris foncé', 'Jaune', 'Vert', 'Bleu', 'Rose', 'Orange']
const KEYS: Record<string, string> = { v: 'select', l: 'lasso', n: 'sticky', t: 'text', r: 'rect', o: 'ellipse', c: 'connector', p: 'pen', h: 'hl', e: 'erase', f: 'frame', m: 'comment' }
const GRID = 24
const snap = (v: number, on: boolean) => (on ? Math.round(v / GRID) * GRID : v)
const UEFN: [string, string, string][] = [
  ['zone', 'Zone', 'Z'], ['spawn', 'Apparition', 'A'], ['button', 'Bouton', 'B'], ['trigger', 'Déclencheur', 'D'], ['dispenser', 'Distributeur', 'R'], ['npc', 'PNJ', 'P'],
  ['objective', 'Objectif', 'O'], ['vehicle', 'Véhicule', 'V'], ['road', 'Route', '═'], ['wall', 'Mur', '▮'], ['camera', 'Caméra', 'C'], ['door', 'Porte', '⌂'], ['loot', 'Butin', '€'], ['path', 'Parcours joueur', '→'],
]

export default function Board() {
  const { bid } = useParams()
  const nav = useNavigate()
  const { map, canEdit, toast, role } = useApp()
  const { data: list, reload } = useLoad<any[]>(map ? `/api/maps/${map.id}/boards` : null, [map?.id])
  const [tplOpen, setTplOpen] = useState(false)
  const id = bid ? Number(bid) : list?.[0]?.id
  const create = async (template?: string) => {
    try { const r = await api.post(`/api/maps/${map!.id}/boards`, { template }); setTplOpen(false); await reload(); nav(`/tableau/${r.id}`) } catch (e: any) { toast(e.message, true) }
  }
  const TPL = [['brainstorm', 'Brainstorm d’idées'], ['plan', 'Plan de map et parcours'], ['loop', 'Boucle de gameplay'], ['journey', 'Parcours du joueur'], ['mockups', 'Maquettes 16:9 et mobile'], ['retro', 'Rétrospective'], ['roadmap', 'Feuille de route par saison'], ['matrix', 'Matrice impact / effort']]
  return (
    <main className="content">
      <PageHead title={`Tableau blanc${list?.find(b => b.id === id) ? ' · ' + list.find(b => b.id === id).title : ''}`} help="Dessinez la map à plusieurs, votez les idées, puis transformez les post-it en tâches ou en pages.">
        <select className="field" value={id ?? ''} onChange={e => nav(`/tableau/${e.target.value}`)} aria-label="Tableau">{list?.map(b => <option key={b.id} value={b.id}>{b.title}</option>)}</select>
        {canEdit && <div style={{ position: 'relative' }}>
          <button className="btn" onClick={() => setTplOpen(o => !o)}>Nouveau tableau</button>
          {tplOpen && <div className="menu" style={{ right: 0, top: 48 }}><button onClick={() => create()}>Tableau vide</button>{TPL.map(([k, l]) => <button key={k} onClick={() => create(k)}>{l}</button>)}</div>}
        </div>}
        {canEdit && id && <>
          <button className="iconbtn" title="Renommer le tableau" aria-label="Renommer le tableau" onClick={async () => { const t = prompt('Nom du tableau', list?.find(b => b.id === id)?.title); if (t) { try { await api.patch(`/api/boards/${id}`, { title: t }); reload() } catch (e: any) { toast(e.message, true) } } }}><Pencil size={18} /></button>
          <button className="iconbtn" title="Dupliquer le tableau" aria-label="Dupliquer le tableau" onClick={async () => { try { const r = await api.post(`/api/boards/${id}/duplicate`); await reload(); nav(`/tableau/${r.id}`) } catch (e: any) { toast(e.message, true) } }}><Copy size={18} /></button>
          {['owner', 'admin', 'member'].includes(role) && <button className="iconbtn" title="Supprimer le tableau" aria-label="Supprimer le tableau" onClick={async () => { if (!confirm(`Mettre « ${list?.find(b => b.id === id)?.title} » à la corbeille ? (restaurable depuis la corbeille des Pages)`)) return; try { await api.del(`/api/boards/${id}`); await reload(); nav('/tableau') } catch (e: any) { toast(e.message, true) } }}><Trash2 size={18} /></button>}
        </>}
      </PageHead>
      {id ? <Canvas key={id} id={id} /> : list && <div className="card"><p className="muted">Aucun tableau. Créez-en un avec « Nouveau tableau ».</p></div>}
    </main>
  )
}

function Canvas({ id }: { id: number }) {
  const app = useApp()
  const nav = useNavigate()
  const { data: mem } = useMembers()
  const { data: tasks, reload: reloadTasks } = useLoad<any[]>(app.map ? `/api/maps/${app.map.id}/tasks` : null, [app.map?.id])
  const [objs, setObjs] = useState<Record<string, Obj>>({})
  const [meta, setMeta] = useState<any>({ points: 3, hidden: false, timer_end: null })
  const [me, setMe] = useState(0)
  const [editable, setEditable] = useState(false)
  const [tool, setTool] = useState('select')
  const [sel, setSel] = useState<string[]>([])
  const [editing, setEditing] = useState<string | null>(null)
  const [view, setView] = useState({ x: 40, y: 60, z: 0.8 })
  const [gridOn, setGridOn] = useState(true)
  const [voting, setVoting] = useState(false)
  const [lasso, setLasso] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null)
  const [cursors, setCursors] = useState<Record<number, { name: string; x: number; y: number; t: number }>>({})
  const [follow, setFollow] = useState<number | null>(null)
  const [present, setPresent] = useState<number | null>(null)
  const [connFrom, setConnFrom] = useState<string | null>(null)
  const [connStyle, setConnStyle] = useState('curve')
  const [shade, setShade] = useState(0)
  const [convert, setConvert] = useState({ assignee: '', tag: 'conception' })
  const [now, setNow] = useState(Date.now())
  const [fs, setFs] = useState(false)
  const wbRef = useRef<HTMLElement>(null)
  const hist = useRef<{ undo: [string, Obj | null][][]; redo: [string, Obj | null][][] }>({ undo: [], redo: [] })
  const clip = useRef<Obj[]>([])
  const space = useRef(false)
  const box = useRef<HTMLDivElement>(null)
  const drag = useRef<any>(null)
  const lastSend = useRef(0)
  const canEdit = app.canEdit && editable

  useEffect(() => {
    api.get(`/api/boards/${id}`).then(b => {
      const { __meta, ...rest } = b.objects
      setObjs(rest); setMeta(__meta ?? meta); setMe(b.me); setEditable(b.can_edit)
    }).catch(e => app.toast(e.message, true))
    app.send({ t: 'place', place: `board:${id}` })
  }, [id])
  useEffect(() => { const t = setInterval(() => setNow(Date.now()), 1000); return () => clearInterval(t) }, [])
  useEffect(() => app.subscribe(m => {
    if (m.t === 'board' && m.board === id) {
      if (m.reload) { api.get(`/api/boards/${id}`).then(b => { const { __meta, ...rest } = b.objects; setObjs(rest); setMeta(__meta) }); return }
      if (m.objs) setObjs(o => { const n = { ...o }; m.objs.forEach((x: Obj) => (n[x.id] = x)); return n })
      if (m.ids) setObjs(o => { const n = { ...o }; m.ids.forEach((i: string) => delete n[i]); return n })
      if (m.meta) setMeta(m.meta)
    }
    if (m.t === 'cursor' && m.place === `board:${id}` && m.uid !== app.me?.user.id) setCursors(c => ({ ...c, [m.uid]: { name: m.name, x: m.x, y: m.y, t: Date.now() } }))
    if (m.t === 'refresh' && m.what === 'tasks') reloadTasks()
  }), [id, app.subscribe])
  useEffect(() => { if (follow && cursors[follow] && box.current) { const r = box.current.getBoundingClientRect(); setView(v => ({ ...v, x: r.width / 2 - cursors[follow].x * v.z, y: r.height / 2 - cursors[follow].y * v.z })) } }, [follow, cursors])

  // Historique local : chaque changement garde l'état précédent des objets touchés (Ctrl+Z / Ctrl+Maj+Z).
  const record = (entries: [string, Obj | null][]) => { hist.current.undo.push(entries); if (hist.current.undo.length > 200) hist.current.undo.shift(); hist.current.redo = [] }
  const raw = (list: Obj[], ids: string[]) => {
    setObjs(o => { const n = { ...o }; list.forEach(x => (n[x.id] = x)); ids.forEach(i => delete n[i]); return n })
    if (list.length) app.send({ t: 'board', board: id, op: 'upsert', objs: list })
    if (ids.length) app.send({ t: 'board', board: id, op: 'delete', ids })
  }
  const push = (list: Obj[], prev?: Obj[]) => {
    record(list.map(x => [x.id, prev?.find(p => p.id === x.id) ?? (objs[x.id] ? { ...objs[x.id] } : null)]))
    raw(list, [])
  }
  const travel = (from: 'undo' | 'redo') => {
    const e = hist.current[from].pop(); if (!e) return
    const back: [string, Obj | null][] = e.map(([i]) => [i, objs[i] ? { ...objs[i] } : null])
    hist.current[from === 'undo' ? 'redo' : 'undo'].push(back)
    raw(e.filter(([, o]) => o).map(([, o]) => o!), e.filter(([, o]) => !o).map(([i]) => i))
    setSel([])
  }
  const typeTimer = useRef<any>(null)
  const typeText = (o: Obj, text: string) => {
    const n = { ...o, text: text.slice(0, 2000) }
    setObjs(s => ({ ...s, [o.id]: { ...s[o.id], text: n.text } }))
    clearTimeout(typeTimer.current)
    typeTimer.current = setTimeout(() => app.send({ t: 'board', board: id, op: 'upsert', objs: [n] }), 250)
  }
  const del = (ids: string[]) => { if (!ids.length) return; const linked = Object.values(objs).filter(o => o.kind === 'connector' && (ids.includes(o.from!) || ids.includes(o.to!))).map(o => o.id); const all = [...new Set([...ids, ...linked])]; record(all.filter(i => objs[i]).map(i => [i, { ...objs[i] }])); raw([], all); setSel([]) }
  const paste = (src: Obj[], dx = 24, dy = 24) => {
    if (!src.length) return
    const map: Record<string, string> = {}; src.forEach(o => (map[o.id] = uid()))
    const out = src.filter(o => o.kind !== 'connector' || (map[o.from!] && map[o.to!])).map(o => ({ ...o, id: map[o.id], x: o.x + (o.kind === 'stroke' ? 0 : dx), y: o.y + (o.kind === 'stroke' ? 0 : dy), points: o.points?.map(p => [p[0] + dx, p[1] + dy]), from: o.from ? map[o.from] : undefined, to: o.to ? map[o.to] : undefined, votes: {}, task_id: undefined, task_number: undefined }))
    push(out); setSel(out.filter(o => o.kind !== 'connector').map(o => o.id))
  }
  const withLinks = (ids: string[]) => [...ids.map(i => objs[i]).filter(Boolean), ...Object.values(objs).filter(o => o.kind === 'connector' && ids.includes(o.from!) && ids.includes(o.to!))]
  const zTop = () => Math.max(0, ...Object.values(objs).map(o => o.z ?? 0)) + 1
  const zBottom = () => Math.min(0, ...Object.values(objs).map(o => o.z ?? 0)) - 1
  const align = (how: 'left' | 'top' | 'hcenter' | 'vcenter' | 'hdist') => {
    const it = sel.map(i => objs[i]).filter(o => o && o.kind !== 'connector' && o.kind !== 'stroke'); if (it.length < 2) return
    const minX = Math.min(...it.map(o => o.x)), minY = Math.min(...it.map(o => o.y))
    const cx = (minX + Math.max(...it.map(o => o.x + (o.w ?? 38)))) / 2, cy = (minY + Math.max(...it.map(o => o.y + (o.h ?? 38)))) / 2
    let out: Obj[]
    if (how === 'hdist') {
      const sorted = [...it].sort((a, b) => a.x - b.x); const total = sorted.reduce((s, o) => s + (o.w ?? 38), 0)
      const span = Math.max(...it.map(o => o.x + (o.w ?? 38))) - minX; const gap = (span - total) / (sorted.length - 1); let x = minX
      out = sorted.map(o => { const n = { ...o, x }; x += (o.w ?? 38) + gap; return n })
    } else out = it.map(o => ({ ...o, x: how === 'left' ? minX : how === 'hcenter' ? cx - (o.w ?? 38) / 2 : o.x, y: how === 'top' ? minY : how === 'vcenter' ? cy - (o.h ?? 38) / 2 : o.y }))
    push(out)
  }
  const importImage = (file: File, at?: { x: number; y: number }) => {
    if (!file.type.startsWith('image/')) return app.toast('Seules les images sont acceptées.', true)
    const r = new FileReader()
    r.onload = () => {
      const img = new window.Image()
      img.onload = () => {
        const k = Math.min(1, 1600 / Math.max(img.width, img.height)); const c = document.createElement('canvas'); c.width = Math.round(img.width * k); c.height = Math.round(img.height * k)
        c.getContext('2d')!.drawImage(img, 0, 0, c.width, c.height)
        const url = c.toDataURL('image/jpeg', 0.82)
        if (url.length > 1_300_000) return app.toast('Image trop lourde, même compressée.', true)
        const w = Math.min(600, c.width), h = Math.round(w * c.height / c.width), p = at ?? center()
        push([{ id: uid(), kind: 'image', url, x: p.x - w / 2, y: p.y - h / 2, w, h, z: zBottom() }])
      }
      img.src = r.result as string
    }
    r.readAsDataURL(file)
  }
  const toggleFs = async () => {
    const el = wbRef.current
    if (!fs) { setFs(true); try { await el?.requestFullscreen?.() } catch {} } else { setFs(false); if (document.fullscreenElement) document.exitFullscreen?.().catch(() => {}) }
    setTimeout(fit, 120)
  }
  useEffect(() => { const h = () => { if (!document.fullscreenElement) setFs(false) }; document.addEventListener('fullscreenchange', h); return () => document.removeEventListener('fullscreenchange', h) }, [])
  const setMetaOp = (p: any) => app.send({ t: 'board', board: id, op: 'meta', ...p })

  const toWorld = (cx: number, cy: number) => { const r = box.current!.getBoundingClientRect(); return { x: (cx - r.left - view.x) / view.z, y: (cy - r.top - view.y) / view.z } }
  const center = () => { const r = box.current!.getBoundingClientRect(); return { x: (r.width / 2 - view.x) / view.z, y: (r.height / 2 - view.y) / view.z } }
  const add = (o: Partial<Obj>) => { const c = center(); const n = { id: uid(), x: snap(c.x - 80, gridOn), y: snap(c.y - 60, gridOn), ...o } as Obj; push([n]); setSel([n.id]); return n }

  const list = Object.values(objs)
  const rank = (o: Obj) => (o.kind === 'image' && o.locked ? -1e6 : o.kind === 'frame' ? -1e5 : 0) + (o.z ?? 0)
  const ordered = useMemo(() => [...list].sort((a, b) => rank(a) - rank(b)), [objs])
  const taskOf = (n?: number) => tasks?.find(t => t.number === n)
  const myVotes = list.reduce((s, o) => s + (o.votes?.[String(me)] ?? 0), 0)
  const votesOf = (o: Obj) => Object.values(o.votes ?? {}).reduce((a, b) => a + b, 0)
  const timerLeft = meta.timer_end ? Math.max(0, Math.round((meta.timer_end - now) / 1000)) : null
  const frames = list.filter(o => o.kind === 'frame').sort((a, b) => a.y - b.y || a.x - b.x)

  const onDown = (e: React.PointerEvent) => {
    const hd = (e.target as HTMLElement).closest('[data-handle]') as HTMLElement | null
    if (hd && canEdit) { const o = objs[hd.dataset.handle!]; drag.current = { mode: 'resize', start: toWorld(e.clientX, e.clientY), orig: { ...o } }; (e.target as HTMLElement).setPointerCapture?.(e.pointerId); return }
    if (space.current) return startPan(e)
    if (e.button === 1 || (e.button === 0 && (e.target as HTMLElement).closest('[data-oid]') === null && (tool === 'select' && !e.shiftKey && e.altKey))) return startPan(e)
    const p = toWorld(e.clientX, e.clientY)
    const target = (e.target as HTMLElement).closest('[data-oid]') as HTMLElement | null
    const oid = target?.dataset.oid
    if (tool === 'select' && !oid) { setSel([]); setEditing(null); return startPan(e) }
    if (!canEdit && tool !== 'select') return
    if (tool === 'lasso') { setLasso({ x0: p.x, y0: p.y, x1: p.x, y1: p.y }); drag.current = { mode: 'lasso' }; return }
    if (tool === 'pen' || tool === 'hl') { const o: Obj = { id: uid(), kind: 'stroke', x: 0, y: 0, points: [[p.x, p.y]], width: tool === 'hl' ? 16 : 2.5, color: tool === 'hl' ? 'hl' : 'pen' }; setObjs(s => ({ ...s, [o.id]: o })); drag.current = { mode: 'draw', id: o.id }; return }
    if (tool === 'erase') { if (oid) del([oid]); drag.current = { mode: 'erase' }; return }
    if (tool === 'connector') { if (oid) { if (!connFrom) setConnFrom(oid); else if (connFrom !== oid) { push([{ id: uid(), kind: 'connector', x: 0, y: 0, from: connFrom, to: oid, style: connStyle, arrow: true }]); setConnFrom(null) } } return }
    if (['sticky', 'text', 'rect', 'ellipse', 'frame', 'comment'].includes(tool) && !oid) {
      e.preventDefault()
      const base = { x: snap(p.x, gridOn), y: snap(p.y, gridOn) }
      const o: Obj = tool === 'sticky' ? { id: uid(), kind: 'sticky', ...base, w: 160, h: 120, text: '', shade, votes: {} }
        : tool === 'text' ? { id: uid(), kind: 'text', ...base, w: 220, h: 40, text: 'Texte' }
        : tool === 'frame' ? { id: uid(), kind: 'frame', ...base, w: 480, h: 320, text: 'Cadre' }
        : tool === 'comment' ? { id: uid(), kind: 'comment', ...base, w: 220, h: 80, text: '' }
        : { id: uid(), kind: 'shape', shape: tool, ...base, w: 140, h: 90, text: '' }
      push([o]); setSel([o.id]); setEditing(o.kind === 'frame' ? null : o.id); setTool('select'); return
    }
    if (oid) {
      const o = objs[oid]
      if (voting && o.kind === 'sticky') { app.send({ t: 'board', board: id, op: 'vote', id: oid, delta: e.shiftKey ? -1 : 1 }); return }
      const nsel = e.shiftKey ? (sel.includes(oid) ? sel.filter(s => s !== oid) : [...sel, oid]) : sel.includes(oid) ? sel : [oid]
      setSel(nsel)
      if (!canEdit || editing === oid) return
      // Déplacer une sélection (un cadre emporte ce qu'il contient).
      const ids = new Set(nsel)
      nsel.forEach(s => { const f = objs[s]; if (f?.kind === 'frame') list.forEach(x => { if (x.kind !== 'connector' && x.kind !== 'stroke' && x.x >= f.x && x.y >= f.y && x.x + (x.w ?? 0) <= f.x + (f.w ?? 0) && x.y + (x.h ?? 0) <= f.y + (f.h ?? 0)) ids.add(x.id) }) })
      drag.current = { mode: 'move', start: p, orig: [...ids].map(i => ({ ...objs[i] })) }
      ;(e.target as HTMLElement).setPointerCapture?.(e.pointerId)
    }
  }
  const startPan = (e: React.PointerEvent) => { drag.current = { mode: 'pan', sx: e.clientX, sy: e.clientY, vx: view.x, vy: view.y } }
  const onMove = (e: React.PointerEvent) => {
    const p = toWorld(e.clientX, e.clientY)
    if (Date.now() - lastSend.current > 60) { lastSend.current = Date.now(); app.send({ t: 'cursor', place: `board:${id}`, x: p.x, y: p.y }) }
    const d = drag.current
    if (!d) return
    if (d.mode === 'pan') setView(v => ({ ...v, x: d.vx + e.clientX - d.sx, y: d.vy + e.clientY - d.sy }))
    if (d.mode === 'resize') {
      const o = d.orig; const nw = Math.max(30, snap((o.w ?? 40) + p.x - d.start.x, gridOn)), nh = Math.max(24, snap((o.h ?? 40) + p.y - d.start.y, gridOn))
      const r = e.shiftKey && o.w && o.h ? { w: nw, h: Math.round(nw * o.h / o.w) } : { w: nw, h: nh }
      setObjs(s => ({ ...s, [o.id]: { ...s[o.id], ...r } })); d.cur = { ...o, ...r }
      if (Date.now() - (d.last ?? 0) > 90) { d.last = Date.now(); app.send({ t: 'board', board: id, op: 'upsert', objs: [d.cur] }) }
    }
    if (d.mode === 'lasso') setLasso(l => l && { ...l, x1: p.x, y1: p.y })
    if (d.mode === 'draw') setObjs(s => ({ ...s, [d.id]: { ...s[d.id], points: [...(s[d.id].points ?? []), [Math.round(p.x), Math.round(p.y)]] } }))
    if (d.mode === 'erase') { const t = (e.target as HTMLElement).closest('[data-oid]') as HTMLElement | null; if (t?.dataset.oid) del([t.dataset.oid]) }
    if (d.mode === 'move') {
      const dx = p.x - d.start.x, dy = p.y - d.start.y
      const moved = d.orig.map((o: Obj) => ({ ...o, x: snap(o.x + dx, gridOn), y: snap(o.y + dy, gridOn) }))
      setObjs(s => { const n = { ...s }; moved.forEach((m: Obj) => (n[m.id] = { ...n[m.id], x: m.x, y: m.y })); return n })
      d.moved = moved
      if (Date.now() - (d.last ?? 0) > 90) { d.last = Date.now(); app.send({ t: 'board', board: id, op: 'upsert', objs: moved }) }
    }
  }
  const onUp = () => {
    const d = drag.current
    drag.current = null
    if (!d) return
    if (d.mode === 'move' && d.moved) push(d.moved, d.orig)
    if (d.mode === 'resize' && d.cur) push([d.cur], [d.orig])
    if (d.mode === 'draw') { const o = objs[d.id]; if (o && (o.points?.length ?? 0) > 1) push([o], []) }
    if (d.mode === 'lasso' && lasso) {
      const [x0, x1] = [Math.min(lasso.x0, lasso.x1), Math.max(lasso.x0, lasso.x1)], [y0, y1] = [Math.min(lasso.y0, lasso.y1), Math.max(lasso.y0, lasso.y1)]
      setSel(list.filter(o => o.kind !== 'connector' && o.kind !== 'stroke' && o.x >= x0 && o.y >= y0 && o.x + (o.w ?? 40) <= x1 && o.y + (o.h ?? 40) <= y1).map(o => o.id))
      setLasso(null); setTool('select')
    }
  }
  const onWheel = (e: React.WheelEvent) => {
    if (!(e.ctrlKey || e.metaKey)) { setView(v => ({ ...v, x: v.x - e.deltaX, y: v.y - e.deltaY })); return }
    const r = box.current!.getBoundingClientRect()
    const mx = e.clientX - r.left, my = e.clientY - r.top
    setView(v => { const z = Math.min(4, Math.max(0.1, v.z * Math.exp(-e.deltaY * 0.01))); return { z, x: mx - ((mx - v.x) / v.z) * z, y: my - ((my - v.y) / v.z) * z } })
  }
  const zoom = (k: number) => setView(v => ({ ...v, z: Math.min(3, Math.max(0.2, v.z * k)) }))
  const fit = () => {
    if (!list.length || !box.current) return
    const xs = list.filter(o => o.kind !== 'connector').flatMap(o => [o.x, o.x + (o.w ?? 40)]), ys = list.filter(o => o.kind !== 'connector').flatMap(o => [o.y, o.y + (o.h ?? 40)])
    const r = box.current.getBoundingClientRect(); const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)]
    const z = Math.min(1.5, Math.min(r.width / (x1 - x0 + 120), r.height / (y1 - y0 + 120)))
    setView({ z, x: (r.width - (x1 - x0) * z) / 2 - x0 * z, y: (r.height - (y1 - y0) * z) / 2 - y0 * z })
  }
  const goFrame = (i: number) => {
    const f = frames[i]; if (!f || !box.current) return
    const r = box.current.getBoundingClientRect(); const z = Math.min(r.width / ((f.w ?? 400) + 80), r.height / ((f.h ?? 300) + 80))
    setView({ z, x: (r.width - (f.w ?? 0) * z) / 2 - f.x * z, y: (r.height - (f.h ?? 0) * z) / 2 - f.y * z }); setPresent(i)
  }

  // Clavier : suppression, carte mentale (Tab = enfant, Entrée = frère), présentation.
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (editing || (e.target as HTMLElement).tagName === 'INPUT' || (e.target as HTMLElement).tagName === 'TEXTAREA' || (e.target as HTMLElement).isContentEditable) return
      if (present !== null) { if (e.key === 'ArrowRight') goFrame(Math.min(frames.length - 1, present + 1)); if (e.key === 'ArrowLeft') goFrame(Math.max(0, present - 1)); if (e.key === 'Escape') setPresent(null); return }
      const mod = e.metaKey || e.ctrlKey
      if (e.key === 'Escape' && fs) { toggleFs(); return }
      if (mod && e.key === '0') { e.preventDefault(); fit(); return }
      if (mod && (e.key === '=' || e.key === '+')) { e.preventDefault(); zoom(1.2); return }
      if (mod && e.key === '-') { e.preventDefault(); zoom(1 / 1.2); return }
      if (mod && e.key.toLowerCase() === 'a') { e.preventDefault(); setSel(list.filter(o => o.kind !== 'connector').map(o => o.id)); return }
      if (mod && e.key.toLowerCase() === 'c' && sel.length) { clip.current = withLinks(sel).map(o => ({ ...o })); app.toast(`${sel.length} objet(s) copié(s)`); return }
      if (!canEdit) return
      if (mod && e.key.toLowerCase() === 'z') { e.preventDefault(); travel(e.shiftKey ? 'redo' : 'undo'); return }
      if (mod && e.key.toLowerCase() === 'y') { e.preventDefault(); travel('redo'); return }
      if (mod && e.key.toLowerCase() === 'v') { e.preventDefault(); paste(clip.current); return }
      if (mod && e.key.toLowerCase() === 'd') { e.preventDefault(); paste(withLinks(sel)); return }
      if (mod && e.key.toLowerCase() === 'x' && sel.length) { clip.current = withLinks(sel).map(o => ({ ...o })); del(sel); return }
      if (!mod && !e.altKey && KEYS[e.key.toLowerCase()] && !(e.key === 'Tab')) { setTool(KEYS[e.key.toLowerCase()]); setConnFrom(null); return }
      if (e.key.startsWith('Arrow') && sel.length) {
        e.preventDefault(); const k = e.shiftKey ? GRID : 2
        const dx = e.key === 'ArrowLeft' ? -k : e.key === 'ArrowRight' ? k : 0, dy = e.key === 'ArrowUp' ? -k : e.key === 'ArrowDown' ? k : 0
        push(sel.map(i => objs[i]).filter(o => o && o.kind !== 'connector').map(o => ({ ...o, x: o.x + dx, y: o.y + dy, points: o.points?.map(p => [p[0] + dx, p[1] + dy]) })))
        return
      }
      if ((e.key === 'Delete' || e.key === 'Backspace') && sel.length) { e.preventDefault(); del(sel) }
      const s = sel.length === 1 ? objs[sel[0]] : null
      if (s && (s.kind === 'sticky' || s.kind === 'shape') && (e.key === 'Tab' || e.key === 'Enter')) {
        e.preventDefault()
        const parentConn = list.find(c => c.kind === 'connector' && c.to === s.id)
        const parent = e.key === 'Tab' ? s : parentConn ? objs[parentConn.from!] : null
        const n: Obj = { ...s, id: uid(), text: '', votes: {}, task_id: undefined, task_number: undefined, x: e.key === 'Tab' ? s.x + 220 : s.x, y: e.key === 'Tab' ? s.y : s.y + (s.h ?? 120) + 30 }
        push(parent ? [n, { id: uid(), kind: 'connector', x: 0, y: 0, from: parent.id, to: n.id, style: 'elbow' }] : [n]); setSel([n.id]); setEditing(n.id)
      }
    }
    const sd = (e: KeyboardEvent) => { if (e.code === 'Space' && !(e.target as HTMLElement).closest('input,textarea,[contenteditable]')) { space.current = e.type === 'keydown'; if (e.type === 'keydown') e.preventDefault() } }
    window.addEventListener('keydown', h); window.addEventListener('keydown', sd); window.addEventListener('keyup', sd)
    return () => { window.removeEventListener('keydown', h); window.removeEventListener('keydown', sd); window.removeEventListener('keyup', sd) }
  })

  const groupByShade = () => {
    const st = list.filter(o => o.kind === 'sticky'); const c = center(); const moved: Obj[] = []
    ;SHADES.forEach((_, sh) => st.filter(o => (o.shade ?? 0) === sh).forEach((o, i) => moved.push({ ...o, x: c.x - 900 + sh * 200, y: c.y - 200 + i * 140 })))
    push(moved)
  }
  const doConvert = async () => {
    try {
      const r = await api.post(`/api/boards/${id}/convert`, { ids: sel, tag: convert.tag, assignee_id: convert.assignee ? Number(convert.assignee) : null })
      app.toast(`${r.created.length} tâche(s) créée(s)`); setSel([])
    } catch (e: any) { app.toast(e.message, true) }
  }
  const frameToPage = async (fid: string) => { try { const r = await api.post(`/api/boards/${id}/frame-to-page`, { frame_id: fid }); nav(`/pages/${r.page_id}`) } catch (e: any) { app.toast(e.message, true) } }
  const buildSvg = () => {
    const xs = list.flatMap(o => [o.x, o.x + (o.w ?? 40)]), ys = list.flatMap(o => [o.y, o.y + (o.h ?? 40)])
    const [x0, y0] = [Math.min(...xs) - 40, Math.min(...ys) - 40], [w, h] = [Math.max(...xs) - x0 + 40, Math.max(...ys) - y0 + 40]
    const esc = (s: string) => s.replace(/[<>&"]/g, c => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;' } as any)[c])
    const parts = ordered.map(o => {
      if (o.kind === 'sticky') { const [bg, fg] = SHADES[o.shade ?? 0]; return `<rect x="${o.x}" y="${o.y}" width="${o.w}" height="${o.h}" rx="6" fill="${bg}" stroke="#d4d4d4"/><text x="${o.x + 10}" y="${o.y + 24}" font-size="14" fill="${fg}" font-family="sans-serif">${esc(o.text ?? '')}</text>` }
      if (o.kind === 'frame') return `<rect x="${o.x}" y="${o.y}" width="${o.w}" height="${o.h}" rx="10" fill="none" stroke="#a3a3a3"/><text x="${o.x}" y="${o.y - 8}" font-size="13" font-family="sans-serif">${esc(o.text ?? '')}</text>`
      if (o.kind === 'stroke') return `<polyline points="${o.points?.map(p => p.join(',')).join(' ')}" fill="none" stroke="#000" stroke-opacity="${o.color === 'hl' ? 0.25 : 1}" stroke-width="${o.width}" stroke-linecap="round"/>`
      if (o.kind === 'text' || o.kind === 'shape' || o.kind === 'uefn') return `<text x="${o.x}" y="${o.y + 20}" font-size="14" font-family="sans-serif">${esc(o.text ?? '')}</text>`
      return ''
    })
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${x0} ${y0} ${w} ${h}" width="${w}" height="${h}"><rect x="${x0}" y="${y0}" width="${w}" height="${h}" fill="#fff"/>${parts.join('')}</svg>`
  }
  const download = (blob: Blob, name: string) => { const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = name; a.click() }
  const exportSvg = () => { if (list.length) download(new Blob([buildSvg()], { type: 'image/svg+xml' }), 'tableau.svg') }
  const exportPng = () => {
    if (!list.length) return
    const img = new window.Image()
    img.onload = () => { const c = document.createElement('canvas'); c.width = img.width; c.height = img.height; c.getContext('2d')!.drawImage(img, 0, 0); c.toBlob(b => b && download(b, 'tableau.png')) }
    img.src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(buildSvg())
  }

  const anchor = (o: Obj) => ({ x: o.x + (o.w ?? 38) / 2, y: o.y + (o.h ?? 38) / 2 })
  const path = (c: Obj) => {
    const a = objs[c.from!], b = objs[c.to!]; if (!a || !b) return ''
    const p = anchor(a), q = anchor(b)
    if (c.style === 'straight') return `M${p.x},${p.y} L${q.x},${q.y}`
    if (c.style === 'elbow') return `M${p.x},${p.y} L${(p.x + q.x) / 2},${p.y} L${(p.x + q.x) / 2},${q.y} L${q.x},${q.y}`
    return `M${p.x},${p.y} C${(p.x + q.x) / 2},${p.y} ${(p.x + q.x) / 2},${q.y} ${q.x},${q.y}`
  }
  const selSticky = sel.filter(s => objs[s]?.kind === 'sticky')
  const selFrame = sel.length === 1 && objs[sel[0]]?.kind === 'frame' ? sel[0] : null
  const ranking = list.filter(o => o.kind === 'sticky').map(o => ({ o, v: votesOf(o) })).filter(x => x.v > 0).sort((a, b) => b.v - a.v)
  const TOOLS: [string, string, any][] = [['select', 'Sélection', MousePointer2], ['lasso', 'Lasso', Lasso], ['sticky', 'Post-it', StickyNote], ['text', 'Texte', Type], ['rect', 'Rectangle', Square], ['ellipse', 'Ellipse', Circle], ['connector', 'Connecteur', Spline], ['pen', 'Stylo', Pen], ['hl', 'Surligneur', Highlighter], ['erase', 'Gomme', Eraser], ['frame', 'Cadre (diapo)', Frame], ['comment', 'Commentaire épinglé', MessageSquarePlus]]

  return (
    <>
      <section ref={wbRef} className={'wb' + (fs ? ' fs' : '')} aria-label="Canevas">
        <div className="wbtop">
          <div className="toolbar" role="toolbar" aria-label="Outils">
            {TOOLS.map(([k, l, I]) => <button key={k} className={'tool' + (tool === k ? ' on' : '')} title={l} aria-label={l} aria-pressed={tool === k} onClick={() => { setTool(k); setConnFrom(null) }} disabled={!canEdit && k !== 'select'}><I size={18} strokeWidth={1.5} /></button>)}
            <label className={'tool' + (canEdit ? '' : ' off')} title="Importer une image (ou glissez-la sur le tableau)" aria-label="Importer une image"><Upload size={18} strokeWidth={1.5} /><input type="file" accept="image/*" hidden disabled={!canEdit} onChange={e => { const f = e.target.files?.[0]; if (f) importImage(f); e.target.value = '' }} /></label>
            <button className="tool" title="Image par lien" aria-label="Image par lien" disabled={!canEdit} onClick={() => { const u = prompt('Lien https de l’image (plan de map vu du dessus, capture…)'); if (u && /^https:\/\//.test(u)) add({ kind: 'image', url: u, w: 600, h: 400, locked: confirm('Utiliser comme fond « plan de map » (verrouillé derrière) ?') }) }}><ImagePlus size={18} strokeWidth={1.5} /></button>
            <span className="tsep" />
            <button className="tool" title="Annuler (Ctrl+Z)" aria-label="Annuler" disabled={!canEdit} onClick={() => travel('undo')}><Undo2 size={18} strokeWidth={1.5} /></button>
            <button className="tool" title="Rétablir (Ctrl+Maj+Z)" aria-label="Rétablir" disabled={!canEdit} onClick={() => travel('redo')}><Redo2 size={18} strokeWidth={1.5} /></button>
            {tool === 'sticky' && SHADES.map(([bg], i) => <button key={i} className="tool" title={SHADE_NAMES[i]} aria-label={SHADE_NAMES[i]} onClick={() => setShade(i)}><span style={{ width: 18, height: 18, borderRadius: 4, background: bg, border: shade === i ? '2px solid var(--text)' : '1px solid var(--border)' }} /></button>)}
            {tool === 'connector' && <select className="field" style={{ height: 34 }} value={connStyle} onChange={e => setConnStyle(e.target.value)} aria-label="Style"><option value="straight">Droit</option><option value="elbow">Coudé</option><option value="curve">Courbe</option></select>}
            {tool === 'connector' && <span className="small muted">{connFrom ? 'Cliquez l’objet d’arrivée' : 'Cliquez l’objet de départ'}</span>}
          </div>
          <div className="toolbar" aria-label="Brainstorming">
            <button className={'pill' + (timerLeft ? ' on' : '')} onClick={() => setMetaOp({ timer_end: timerLeft ? null : Date.now() + 5 * 60000 })} disabled={!canEdit}><Timer size={16} />{timerLeft ? `${String(Math.floor(timerLeft / 60)).padStart(2, '0')}:${String(timerLeft % 60).padStart(2, '0')}` : 'Minuteur 5 min'}</button>
            <button className={'pill' + (voting ? ' on' : '')} onClick={() => setVoting(v => !v)}><ThumbsUp size={16} />Vote · {Math.max(0, meta.points - myVotes)} pts</button>
            <button className={'pill' + (meta.hidden ? ' on' : '')} onClick={() => setMetaOp({ hidden: !meta.hidden })} disabled={!canEdit}>{meta.hidden ? <><Eye size={16} />Révéler</> : <><EyeOff size={16} />Idées cachées</>}</button>
            <button className="pill" onClick={groupByShade} disabled={!canEdit}><LayoutGrid size={16} />Regrouper</button>
            <select className="pill" value={follow ?? ''} onChange={e => setFollow(e.target.value ? Number(e.target.value) : null)} aria-label="Suivre un membre"><option value="">Suivre…</option>{Object.entries(cursors).map(([u, c]) => <option key={u} value={u}>{c.name}</option>)}</select>
            <button className="pill" onClick={() => frames.length ? goFrame(0) : app.toast('Ajoutez des cadres pour présenter.')}><Presentation size={16} />Présenter</button>
            <button className="pill" onClick={exportPng}><Download size={16} />PNG</button>
            <button className="pill" onClick={exportSvg}>SVG</button>
            <button className="pill" onClick={() => window.print()}>PDF</button>
            <button className={'pill' + (fs ? ' on' : '')} onClick={toggleFs} title="Plein écran (Échap pour sortir)">{fs ? <><Shrink size={16} />Quitter</> : <><Expand size={16} />Plein écran</>}</button>
          </div>
        </div>
        <div className="wbbody">
          <aside className="lib" aria-label="Formes UEFN">
            <div style={{ fontWeight: 500, marginBottom: 8 }}>Formes UEFN</div>
            {UEFN.map(([k, l, ic]) => <button key={k} className="libitem" disabled={!canEdit} onClick={() => add(k === 'zone' ? { kind: 'shape', shape: 'rect', w: 240, h: 160, text: 'Zone', dashed: true } : k === 'road' || k === 'wall' ? { kind: 'shape', shape: 'rect', w: k === 'road' ? 260 : 160, h: k === 'road' ? 28 : 14, text: '' } : k === 'path' ? { kind: 'stroke', x: 0, y: 0, points: [[center().x - 100, center().y], [center().x + 100, center().y]], width: 3, color: 'path' } : { kind: 'uefn', shape: k, w: 38, h: 38, text: l })}><span className="lico">{ic}</span>{l}</button>)}
            {ranking.length > 0 && <>
              <div style={{ fontWeight: 500, margin: '16px 0 8px' }}>Classement</div>
              {ranking.slice(0, 8).map(({ o, v }, i) => <div key={o.id} className="small" style={{ padding: '4px 0' }}><span className="mono muted">{i + 1}.</span> {meta.hidden && o.author_id !== me ? 'Idée cachée' : o.text} <b style={{ fontWeight: 500 }}>· {v}</b></div>)}
            </>}
          </aside>
          <div ref={box} className="canvas" style={{ backgroundSize: `${GRID * view.z}px ${GRID * view.z}px`, backgroundPosition: `${view.x}px ${view.y}px`, cursor: tool === 'select' ? 'default' : 'crosshair' }}
            onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp} onWheel={onWheel}
            onDragOver={e => { if (canEdit && e.dataTransfer.types.includes('Files')) e.preventDefault() }}
            onDrop={e => { e.preventDefault(); const f = e.dataTransfer.files?.[0]; if (f && canEdit) importImage(f, toWorld(e.clientX, e.clientY)) }} onDoubleClick={e => { const t = (e.target as HTMLElement).closest('[data-oid]') as HTMLElement | null; if (t && canEdit) setEditing(t.dataset.oid!) }}>
            <div className="layer" style={{ transform: `translate(${view.x}px,${view.y}px) scale(${view.z})` }}>
              <svg style={{ position: 'absolute', left: 0, top: 0, overflow: 'visible', pointerEvents: 'none' }} width="1" height="1">
                <defs><marker id="arr" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="currentColor" /></marker></defs>
                {list.filter(o => o.kind === 'connector').map(c => <path key={c.id} data-oid={c.id} d={path(c)} fill="none" stroke="var(--text2)" strokeWidth={sel.includes(c.id) ? 3 : 1.6} strokeDasharray={c.dashed ? '8 6' : undefined} markerEnd={c.arrow ? 'url(#arr)' : undefined} style={{ color: 'var(--text2)', pointerEvents: 'stroke' }} />)}
                {list.filter(o => o.kind === 'stroke').map(s => <polyline key={s.id} data-oid={s.id} points={s.points?.map(p => p.join(',')).join(' ')} fill="none" stroke="var(--text)" strokeOpacity={s.color === 'hl' ? 0.25 : 1} strokeWidth={s.width} strokeDasharray={s.color === 'path' ? '10 8' : undefined} strokeLinecap="round" strokeLinejoin="round" style={{ pointerEvents: 'stroke' }} />)}
              </svg>
              {ordered.filter(o => !['connector', 'stroke'].includes(o.kind)).map(o => {
                const isSel = sel.includes(o.id) ? ' sel' : ''
                const st = { left: o.x, top: o.y, width: o.w, height: o.h }
                const ed = editing === o.id
                const input = <textarea className="edit-in" autoFocus defaultValue={o.text} onPointerDown={e => e.stopPropagation()} onChange={e => typeText(o, e.target.value)} onBlur={e => { push([{ ...o, text: e.target.value.slice(0, 2000) }]); setEditing(null) }} onKeyDown={e => { if (e.key === 'Escape' || (e.key === 'Enter' && (e.metaKey || e.ctrlKey))) (e.target as HTMLTextAreaElement).blur() }} />
                if (o.kind === 'image') return <img key={o.id} data-oid={o.id} className={'obj' + isSel} src={o.url} alt="" draggable={false} style={{ ...st, opacity: o.locked ? 0.5 : 1, pointerEvents: o.locked ? 'none' : 'auto', objectFit: 'contain' }} />
                if (o.kind === 'frame') return <div key={o.id} data-oid={o.id} className={'obj frame' + isSel} style={st}><span className="fname">{ed ? <input autoFocus defaultValue={o.text} onPointerDown={e => e.stopPropagation()} onBlur={e => { push([{ ...o, text: e.target.value }]); setEditing(null) }} className="field" style={{ height: 26 }} /> : o.text}</span></div>
                if (o.kind === 'sticky') {
                  const [bg, fg] = SHADES[o.shade ?? 0]; const hide = meta.hidden && o.author_id !== me; const t = taskOf(o.task_number)
                  return <div key={o.id} data-oid={o.id} className={'obj sticky' + isSel} style={{ ...st, background: bg, color: fg }}>
                    {ed ? input : hide ? <div className="hiddenidea">Idée cachée<div className="small">de {o.author?.split(' ')[0]}</div></div> : <div style={{ fontWeight: 500, lineHeight: 1.3, overflow: 'hidden' }}>{o.text || <span style={{ opacity: 0.4 }}>Double-clic pour écrire</span>}</div>}
                    <div className="row" style={{ justifyContent: 'space-between', marginTop: 'auto', gap: 6 }}><span className="small" style={{ opacity: 0.7 }}>{o.author?.split(' ')[0]}</span>{votesOf(o) > 0 && <span className="votes">{votesOf(o)} vote{votesOf(o) > 1 ? 's' : ''}</span>}</div>
                    {o.task_number && <span className="tasklink badge" style={{ background: 'var(--surface)' }}>#{o.task_number} · {t ? ({ todo: 'À faire', doing: 'En cours', blocked: 'Bloqué', done: 'Fait' } as any)[t.status] : '…'}</span>}
                  </div>
                }
                if (o.kind === 'uefn') return <div key={o.id} data-oid={o.id} className={'obj uefn' + isSel} style={{ left: o.x, top: o.y }} title={o.text}>{UEFN.find(u => u[0] === o.shape)?.[2] ?? '•'}<span className="ulab">{o.text}</span></div>
                if (o.kind === 'shape') return <div key={o.id} data-oid={o.id} className={'obj shape' + isSel} style={{ ...st, borderRadius: o.shape === 'ellipse' ? '50%' : 6, borderStyle: o.dashed ? 'dashed' : 'solid', background: o.fill !== undefined ? SHADES[o.fill]?.[0] : undefined, color: o.fill !== undefined ? SHADES[o.fill]?.[1] : undefined }}>{ed ? input : o.text}</div>
                if (o.kind === 'comment') return <div key={o.id} data-oid={o.id} className={'obj sticky' + isSel} style={{ ...st, background: 'var(--surface)', color: 'var(--text)', border: '1px solid var(--border)' }}><span className="small muted">Commentaire · {o.author?.split(' ')[0]}</span>{ed ? input : <div>{o.text}</div>}</div>
                return <div key={o.id} data-oid={o.id} className={'obj wtext' + isSel} style={st}>{ed ? input : o.text}</div>
              })}
              {canEdit && sel.length === 1 && objs[sel[0]] && !['connector', 'stroke', 'uefn'].includes(objs[sel[0]].kind) && editing !== sel[0] && (() => { const o = objs[sel[0]]; return <div data-handle={o.id} className="rhandle" title="Redimensionner (Maj : garder les proportions)" style={{ left: o.x + (o.w ?? 40) - 6, top: o.y + (o.h ?? 40) - 6, transform: `scale(${1 / view.z})` }} /> })()}
              {lasso && <div style={{ position: 'absolute', left: Math.min(lasso.x0, lasso.x1), top: Math.min(lasso.y0, lasso.y1), width: Math.abs(lasso.x1 - lasso.x0), height: Math.abs(lasso.y1 - lasso.y0), border: '1px dashed var(--text)', background: 'rgba(127,127,127,.08)' }} />}
              {Object.entries(cursors).filter(([, c]) => Date.now() - c.t < 15000).map(([u, c]) => (
                <div key={u} className="wcursor" style={{ left: c.x, top: c.y, transform: `scale(${1 / view.z})`, transformOrigin: '0 0' }}><svg width="16" height="16" viewBox="0 0 24 24"><path d="M3 2l7 19 2.5-8.5L21 10z" fill="var(--text)" /></svg><span>{c.name.split(' ')[0]}</span></div>
              ))}
            </div>

            {sel.length > 0 && selSticky.length === 0 && !selFrame && canEdit && (
              <div className="selbar" onPointerDown={e => e.stopPropagation()}>
                <span>{sel.length} objet(s)</span>
                {sel.some(i => objs[i]?.kind === 'shape') && <select className="field" style={{ height: 34 }} value="" onChange={e => push(sel.filter(i => objs[i]?.kind === 'shape').map(i => ({ ...objs[i], fill: e.target.value === 'none' ? undefined : Number(e.target.value) })))} aria-label="Couleur"><option value="">Couleur…</option><option value="none">Aucune</option>{SHADE_NAMES.map((l, i) => <option key={i} value={i}>{l}</option>)}</select>}
                {sel.length > 1 && <>
                  <button className="iconbtn" title="Aligner à gauche" aria-label="Aligner à gauche" onClick={() => align('left')}><AlignStartVertical size={16} /></button>
                  <button className="iconbtn" title="Centrer horizontalement" aria-label="Centrer horizontalement" onClick={() => align('hcenter')}><AlignCenterVertical size={16} /></button>
                  <button className="iconbtn" title="Aligner en haut" aria-label="Aligner en haut" onClick={() => align('top')}><AlignStartHorizontal size={16} /></button>
                  <button className="iconbtn" title="Centrer verticalement" aria-label="Centrer verticalement" onClick={() => align('vcenter')}><AlignCenterHorizontal size={16} /></button>
                  {sel.length > 2 && <button className="pill" onClick={() => align('hdist')}>Répartir</button>}
                </>}
                <button className="iconbtn" title="Premier plan" aria-label="Premier plan" onClick={() => push(sel.map(i => ({ ...objs[i], z: zTop() })))}><BringToFront size={16} /></button>
                <button className="iconbtn" title="Arrière-plan" aria-label="Arrière-plan" onClick={() => push(sel.map(i => ({ ...objs[i], z: zBottom() })))}><SendToBack size={16} /></button>
                <button className="iconbtn" title="Dupliquer (Ctrl+D)" aria-label="Dupliquer" onClick={() => paste(withLinks(sel))}><Copy size={16} /></button>
                <button className="iconbtn" title="Supprimer (Suppr)" aria-label="Supprimer" onClick={() => del(sel)}><Trash2 size={16} /></button>
              </div>
            )}
            {selSticky.length > 0 && canEdit && (
              <div className="selbar" onPointerDown={e => e.stopPropagation()}>
                <span>{selSticky.length} post-it</span>
                <select className="field" style={{ height: 34 }} value={convert.assignee} onChange={e => setConvert({ ...convert, assignee: e.target.value })} aria-label="Assigné"><option value="">Assigné…</option>{mem?.members.map((m: any) => <option key={m.id} value={m.id}>{m.name}</option>)}</select>
                <select className="field" style={{ height: 34 }} value={convert.tag} onChange={e => setConvert({ ...convert, tag: e.target.value })} aria-label="Étiquette">{['code', 'carte', 'conception', 'test'].map(t => <option key={t}>{t}</option>)}</select>
                <button className="btn pri" style={{ height: 34 }} onClick={doConvert}>Convertir en tâches</button>
                <select className="field" style={{ height: 34 }} value="" onChange={e => { push(selSticky.map(s => ({ ...objs[s], shade: Number(e.target.value) }))) }} aria-label="Nuance"><option value="">Couleur…</option>{SHADE_NAMES.map((l, i) => <option key={i} value={i}>{l}</option>)}</select>
                <button className="iconbtn" title="Dupliquer (Ctrl+D)" aria-label="Dupliquer" onClick={() => paste(withLinks(sel))}><Copy size={16} /></button>
                <button className="btn" style={{ height: 34 }} onClick={() => del(sel)}>Supprimer</button>
              </div>
            )}
            {selFrame && canEdit && <div className="selbar" onPointerDown={e => e.stopPropagation()}><span>Cadre</span><button className="btn" style={{ height: 34 }} onClick={() => frameToPage(selFrame)}>Cadre → page</button><button className="btn" style={{ height: 34 }} onClick={() => del([selFrame])}>Supprimer</button></div>}
            {present !== null && <div className="selbar" onPointerDown={e => e.stopPropagation()}><button className="iconbtn" aria-label="Cadre précédent" onClick={() => goFrame(Math.max(0, present - 1))}><ChevronLeft size={18} /></button><span>{frames[present]?.text} · {present + 1}/{frames.length}</span><button className="iconbtn" aria-label="Cadre suivant" onClick={() => goFrame(Math.min(frames.length - 1, present + 1))}><ChevronRight size={18} /></button><button className="iconbtn" aria-label="Quitter" onClick={() => setPresent(null)}><X size={18} /></button></div>}
            <div className="zoombox" onPointerDown={e => e.stopPropagation()}>
              <button className="iconbtn" aria-label="Dézoomer" onClick={() => zoom(0.85)}><Minus size={16} /></button>
              <span className="small mono" style={{ minWidth: 46, textAlign: 'center' }}>{Math.round(view.z * 100)} %</span>
              <button className="iconbtn" aria-label="Zoomer" onClick={() => zoom(1.15)}><Plus size={16} /></button>
              <button className="iconbtn" aria-label="Tout afficher" onClick={fit}><Maximize size={16} /></button>
              <button className={'pill' + (gridOn ? ' on' : '')} onClick={() => setGridOn(g => !g)}>Magnétisme</button>
              {follow && <button className="pill on" onClick={() => setFollow(null)}><UserRound size={14} />Ne plus suivre</button>}
            </div>
            <MiniMap objs={list} view={view} box={box} />
          </div>
        </div>
      </section>
      <p className="small muted">Astuces : molette ou deux doigts pour se déplacer, Ctrl/⌘ + molette ou pincer pour zoomer, Espace + glisser pour se déplacer, raccourcis V L N T R O C P H E F M pour les outils, Ctrl+Z / Ctrl+Maj+Z, Ctrl+C / V / D / X, flèches pour déplacer, poignée en bas à droite pour redimensionner, glisser une image pour l’importer, double-clic pour écrire, Maj+clic pour sélectionner plusieurs objets, Tab / Entrée sur un post-it pour la carte mentale, Suppr pour effacer. En mode vote, cliquez un post-it pour voter (Maj+clic pour retirer).</p>
    </>
  )
}

function MiniMap({ objs, view, box }: { objs: Obj[]; view: { x: number; y: number; z: number }; box: React.RefObject<HTMLDivElement> }) {
  const items = objs.filter(o => !['connector', 'stroke'].includes(o.kind))
  if (!items.length || !box.current) return null
  const xs = items.flatMap(o => [o.x, o.x + (o.w ?? 40)]), ys = items.flatMap(o => [o.y, o.y + (o.h ?? 40)])
  const [x0, y0] = [Math.min(...xs) - 50, Math.min(...ys) - 50], [w, h] = [Math.max(...xs) - x0 + 50, Math.max(...ys) - y0 + 50]
  const k = Math.min(180 / w, 110 / h)
  const r = box.current.getBoundingClientRect()
  return (
    <div className="minimap" aria-hidden="true">
      {items.map(o => <div key={o.id} style={{ position: 'absolute', left: (o.x - x0) * k, top: (o.y - y0) * k, width: Math.max(2, (o.w ?? 30) * k), height: Math.max(2, (o.h ?? 30) * k), background: o.kind === 'frame' ? 'transparent' : 'var(--text3)', border: o.kind === 'frame' ? '1px solid var(--text3)' : 0 }} />)}
      <div style={{ position: 'absolute', left: (-view.x / view.z - x0) * k, top: (-view.y / view.z - y0) * k, width: (r.width / view.z) * k, height: (r.height / view.z) * k, border: '1.5px solid var(--text)', borderRadius: 3 }} />
    </div>
  )
}
