import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import {
  MousePointer2, Lasso, StickyNote, Type, Square, Circle, Spline, Pen, Highlighter, Eraser, Frame, ImagePlus, MessageSquarePlus, ThumbsUp,
  Timer, Eye, EyeOff, LayoutGrid, Presentation, Download, Plus, Minus, Maximize, UserRound, ChevronLeft, ChevronRight, X, Network,
  Expand, Shrink, Undo2, Redo2, MoreHorizontal, Users, Copy, Trash2, BringToFront, SendToBack, AlignStartVertical, AlignStartHorizontal, AlignCenterVertical, AlignCenterHorizontal, Upload, Pencil,
} from 'lucide-react'
import { api } from '../api'
import { useApp, useLoad } from '../state'
import { PageHead } from '../components/ui'
import { useMembers } from './Tasks'
import { FormatBar, PenBar, Rich, RichEdit, ShapeSvg, textStyle, vAlign, strokeColor, FONTS } from './BoardFormat'
import { tr } from '../i18n/t'

type Obj = { id: string; kind: string; x: number; y: number; w?: number; h?: number; text?: string; shade?: number; votes?: Record<string, number>; author?: string; author_id?: number; task_id?: number; task_number?: number; from?: string; to?: string; style?: string; dashed?: boolean; arrow?: boolean; points?: number[][]; color?: string; width?: number; shape?: string; url?: string; locked?: boolean; reactions?: number } & Record<string, any>

const uid = () => Math.random().toString(36).slice(2, 12)
const SHADES = [['#FFFFFF', '#0A0A0A'], ['#E5E5E5', '#0A0A0A'], ['#A3A3A3', '#0A0A0A'], ['#404040', '#FFFFFF'], ['#FDF0A6', '#0A0A0A'], ['#C9EDD0', '#0A0A0A'], ['#CBE0FA', '#0A0A0A'], ['#F8D0E1', '#0A0A0A'], ['#FCD9B8', '#0A0A0A']]
const SHADE_NAMES = [tr("Blanc"), tr("Gris clair"), tr("Gris moyen"), tr("Gris foncé"), tr("Jaune"), tr("Vert"), tr("Bleu"), tr("Rose"), tr("Orange")]
const KEYS: Record<string, string> = { v: 'select', l: 'lasso', n: 'sticky', t: 'text', r: 'rect', o: 'ellipse', c: 'connector', p: 'pen', h: 'hl', e: 'erase', f: 'frame', m: 'comment' }
const GRID = 24
const snap = (v: number, on: boolean) => (on ? Math.round(v / GRID) * GRID : v)
const UEFN: [string, string, string][] = [
  ['zone', tr("Zone"), 'Z'], ['spawn', tr("Apparition"), 'A'], ['button', tr("Bouton"), 'B'], ['trigger', tr("Déclencheur"), 'D'], ['dispenser', tr("Distributeur"), 'R'], ['npc', 'PNJ', 'P'],
  ['objective', tr("Objectif"), 'O'], ['vehicle', tr("Véhicule"), 'V'], ['road', tr("Route"), '═'], ['wall', tr("Mur"), '▮'], ['camera', tr("Caméra"), 'C'], ['door', tr("Porte"), '⌂'], ['loot', tr("Butin"), '€'], ['path', tr("Parcours joueur"), '→'],
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
  const TPL = [['brainstorm', tr("Brainstorm d’idées")], ['plan', tr("Plan de map et parcours")], ['loop', tr("Boucle de gameplay")], ['journey', tr("Parcours du joueur")], ['mockups', tr("Maquettes 16:9 et mobile")], ['retro', tr("Rétrospective")], ['roadmap', tr("Feuille de route par saison")], ['matrix', tr("Matrice impact / effort")]]
  return (
    <main className="content">
      <PageHead title={tr("Tableau blanc{0}", [list?.find(b => b.id === id) ? ' · ' + list.find(b => b.id === id).title : ''])} help={tr("Dessinez la map à plusieurs, votez les idées, puis transformez les post-it en tâches ou en pages.")}>
        <select className="field" value={id ?? ''} onChange={e => nav(`/tableau/${e.target.value}`)} aria-label={tr("Tableau")}>{list?.map(b => <option key={b.id} value={b.id}>{b.title}</option>)}</select>
        {canEdit && <div style={{ position: 'relative' }}>
          <button className="btn" onClick={() => setTplOpen(o => !o)}>{tr("Nouveau tableau")}</button>
          {tplOpen && <div className="menu" style={{ right: 0, top: 48 }}><button onClick={() => create()}>{tr("Tableau vide")}</button>{TPL.map(([k, l]) => <button key={k} onClick={() => create(k)}>{l}</button>)}</div>}
        </div>}
        {canEdit && id && <>
          <button className="iconbtn" title={tr("Renommer le tableau")} aria-label={tr("Renommer le tableau")} onClick={async () => { const t = prompt(tr("Nom du tableau"), list?.find(b => b.id === id)?.title); if (t) { try { await api.patch(`/api/boards/${id}`, { title: t }); reload() } catch (e: any) { toast(e.message, true) } } }}><Pencil size={18} /></button>
          <button className="iconbtn" title={tr("Dupliquer le tableau")} aria-label={tr("Dupliquer le tableau")} onClick={async () => { try { const r = await api.post(`/api/boards/${id}/duplicate`); await reload(); nav(`/tableau/${r.id}`) } catch (e: any) { toast(e.message, true) } }}><Copy size={18} /></button>
          {['owner', 'admin', 'member'].includes(role) && <button className="iconbtn" title={tr("Supprimer le tableau")} aria-label={tr("Supprimer le tableau")} onClick={async () => { if (!confirm(tr("Mettre « {0} » à la corbeille ? (restaurable depuis la corbeille des Pages)", [list?.find(b => b.id === id)?.title]))) return; try { await api.del(`/api/boards/${id}`); await reload(); nav('/tableau') } catch (e: any) { toast(e.message, true) } }}><Trash2 size={18} /></button>}
        </>}
      </PageHead>
      {id ? <Canvas key={id} id={id} /> : list && <div className="card"><p className="muted">{tr("Aucun tableau. Créez-en un avec « Nouveau tableau ».")}</p></div>}
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
  const [menu, setMenu] = useState<string | null>(null)
  const [lib, setLib] = useState(false)
  const [pen, setPen] = useState({ color: '#1A1A1A', width: 3 })
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
  const objsRef = useRef(objs); objsRef.current = objs
  const editStart = useRef<{ text?: string; html?: string } | null>(null)
  useEffect(() => { editStart.current = editing && objs[editing] ? { text: objs[editing].text, html: objs[editing].html } : null }, [editing])
  const typeTimer = useRef<any>(null)
  const typeText = (o: Obj, html: string, text: string) => {
    const n = { ...(objsRef.current[o.id] ?? o), text: text.slice(0, 2000), html }
    setObjs(s => ({ ...s, [o.id]: { ...s[o.id], text: n.text, html } }))
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
    if (!file.type.startsWith('image/')) return app.toast(tr("Seules les images sont acceptées."), true)
    const r = new FileReader()
    r.onload = () => {
      const img = new window.Image()
      img.onload = () => {
        const k = Math.min(1, 1600 / Math.max(img.width, img.height)); const c = document.createElement('canvas'); c.width = Math.round(img.width * k); c.height = Math.round(img.height * k)
        c.getContext('2d')!.drawImage(img, 0, 0, c.width, c.height)
        const url = c.toDataURL('image/jpeg', 0.82)
        if (url.length > 1_300_000) return app.toast(tr("Image trop lourde, même compressée."), true)
        const w = Math.min(600, c.width), h = Math.round(w * c.height / c.width), p = at ?? center()
        push([{ id: uid(), kind: 'image', url, x: p.x - w / 2, y: p.y - h / 2, w, h, z: zBottom() }])
      }
      img.src = r.result as string
    }
    r.readAsDataURL(file)
  }
  // Plein écran : vrai plein écran du navigateur quand il est permis (Chrome, Safari, Firefox), sinon tableau sur toute la fenêtre.
  const fsEl = () => (document.fullscreenElement ?? (document as any).webkitFullscreenElement) as Element | null
  const toggleFs = async () => {
    const el: any = wbRef.current
    if (!fs) {
      setFs(true)
      try { if (el?.requestFullscreen) await el.requestFullscreen(); else if (el?.webkitRequestFullscreen) el.webkitRequestFullscreen() } catch { /* le mode fenêtre suffit */ }
    } else {
      setFs(false)
      const d: any = document
      try { if (fsEl()) { if (d.exitFullscreen) await d.exitFullscreen(); else d.webkitExitFullscreen?.() } } catch {}
    }
    setTimeout(fit, 200)
  }
  useEffect(() => {
    const h = () => { if (!fsEl()) setFs(false) }
    document.addEventListener('fullscreenchange', h); document.addEventListener('webkitfullscreenchange', h)
    return () => { document.removeEventListener('fullscreenchange', h); document.removeEventListener('webkitfullscreenchange', h) }
  }, [])
  useEffect(() => { document.body.classList.toggle('wb-fs', fs); return () => document.body.classList.remove('wb-fs') }, [fs])
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
    if (tool === 'pen' || tool === 'hl') { const o: Obj = { id: uid(), kind: 'stroke', x: 0, y: 0, points: [[p.x, p.y]], width: tool === 'hl' ? 16 : pen.width, color: tool === 'hl' ? 'hl' : pen.color }; setObjs(s => ({ ...s, [o.id]: o })); drag.current = { mode: 'draw', id: o.id }; return }
    if (tool === 'erase') { if (oid) del([oid]); drag.current = { mode: 'erase' }; return }
    if (tool === 'connector') { if (oid) { if (!connFrom) setConnFrom(oid); else if (connFrom !== oid) { push([{ id: uid(), kind: 'connector', x: 0, y: 0, from: connFrom, to: oid, style: connStyle, arrow: true }]); setConnFrom(null) } } return }
    if (['sticky', 'text', 'rect', 'ellipse', 'frame', 'comment'].includes(tool) && !oid) {
      e.preventDefault()
      const base = { x: snap(p.x, gridOn), y: snap(p.y, gridOn) }
      const o: Obj = tool === 'sticky' ? { id: uid(), kind: 'sticky', ...base, w: 160, h: 120, text: '', shade, votes: {} }
        : tool === 'text' ? { id: uid(), kind: 'text', ...base, w: 220, h: 40, text: tr("Texte") }
        : tool === 'frame' ? { id: uid(), kind: 'frame', ...base, w: 480, h: 320, text: tr("Cadre") }
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
      if (nsel.every(s => objs[s]?.lk)) return
      // Déplacer une sélection (un cadre emporte ce qu'il contient).
      const ids = new Set(nsel)
      nsel.forEach(s => { const f = objs[s]; if (f?.kind === 'frame') list.forEach(x => { if (x.kind !== 'connector' && x.kind !== 'stroke' && x.x >= f.x && x.y >= f.y && x.x + (x.w ?? 0) <= f.x + (f.w ?? 0) && x.y + (x.h ?? 0) <= f.y + (f.h ?? 0)) ids.add(x.id) }) })
      drag.current = { mode: 'move', start: p, orig: [...ids].filter(i => !objs[i]?.lk).map(i => ({ ...objs[i] })) }
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
  // Molette de souris = zoom ; trackpad : pincer = zoom, deux doigts = se déplacer.
  // Écouteur natif non passif : sinon le navigateur zoome toute la page au lieu du tableau.
  const viewRef = useRef(view); viewRef.current = view
  useEffect(() => {
    const el = box.current
    if (!el) return
    const h = (e: WheelEvent) => {
      e.preventDefault()
      const r = el.getBoundingClientRect()
      const mx = e.clientX - r.left, my = e.clientY - r.top
      const mouse = e.deltaMode !== 0 || (e.deltaX === 0 && Math.abs(e.deltaY) >= 40 && Number.isInteger(e.deltaY))
      if (e.ctrlKey || e.metaKey || mouse) {
        const k = Math.exp(-(e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY) * (e.ctrlKey && !mouse ? 0.012 : 0.0022))
        setView(v => { const z = Math.min(4, Math.max(0.1, v.z * k)); return { z, x: mx - ((mx - v.x) / v.z) * z, y: my - ((my - v.y) / v.z) * z } })
      } else {
        setView(v => ({ ...v, x: v.x - e.deltaX, y: v.y - e.deltaY }))
      }
    }
    el.addEventListener('wheel', h, { passive: false })
    return () => el.removeEventListener('wheel', h)
  }, [])
  // Zoom des boutons : centré sur le milieu de l'écran.
  const zoom = (k: number) => { const r = box.current?.getBoundingClientRect(); const mx = (r?.width ?? 0) / 2, my = (r?.height ?? 0) / 2; setView(v => { const z = Math.min(4, Math.max(0.1, v.z * k)); return { z, x: mx - ((mx - v.x) / v.z) * z, y: my - ((my - v.y) / v.z) * z } }) }
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
      if (mod && canEdit && sel.length && ['b', 'i', 'u'].includes(e.key.toLowerCase())) {
        e.preventDefault(); const k = e.key.toLowerCase(); const t = sel.map(i => objs[i]).filter(o => o && ['sticky', 'text', 'shape', 'comment', 'frame'].includes(o.kind))
        if (t.length) push(t.map(o => ({ ...o, [k]: !t.every(x => x[k]) }))); return
      }
      if (mod && e.key.toLowerCase() === 'c' && sel.length) { clip.current = withLinks(sel).map(o => ({ ...o })); app.toast(tr("{0} objet(s) copié(s)", [sel.length])); return }
      if (!canEdit) return
      if (mod && e.key.toLowerCase() === 'z') { e.preventDefault(); travel(e.shiftKey ? 'redo' : 'undo'); return }
      if (mod && e.key.toLowerCase() === 'y') { e.preventDefault(); travel('redo'); return }
      if (mod && e.key.toLowerCase() === 'v') { e.preventDefault(); paste(clip.current); return }
      if (mod && e.key.toLowerCase() === 'd') { e.preventDefault(); paste(withLinks(sel)); return }
      if (mod && e.key.toLowerCase() === 'x' && sel.length) { clip.current = withLinks(sel).map(o => ({ ...o })); del(sel); return }
      if (!mod && !e.altKey && KEYS[e.key.toLowerCase()] && !(e.key === 'Tab')) { setTool(KEYS[e.key.toLowerCase()]); setConnFrom(null); return }
      if (e.key.startsWith(tr("Arrow")) && sel.length) {
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
      app.toast(tr("{0} tâche(s) créée(s)", [r.created.length])); setSel([])
    } catch (e: any) { app.toast(e.message, true) }
  }
  const frameToPage = async (fid: string) => { try { const r = await api.post(`/api/boards/${id}/frame-to-page`, { frame_id: fid }); nav(`/pages/${r.page_id}`) } catch (e: any) { app.toast(e.message, true) } }
  const buildSvg = () => {
    const xs = list.flatMap(o => [o.x, o.x + (o.w ?? 40)]), ys = list.flatMap(o => [o.y, o.y + (o.h ?? 40)])
    const [x0, y0] = [Math.min(...xs) - 40, Math.min(...ys) - 40], [w, h] = [Math.max(...xs) - x0 + 40, Math.max(...ys) - y0 + 40]
    const esc = (s: string) => s.replace(/[<>&"]/g, c => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;' } as any)[c])
    const fam = (o: Obj) => (FONTS[o.ff ?? 'sans']?.[1] ?? 'sans-serif').replace(/"/g, "'")
    const txt = (o: Obj, x: number, y: number, w: number, h: number, def: { size: number; color: string; anchor?: string; va?: string }) => {
      const size = o.fs ?? def.size, lines = (o.text ?? '').split('\n').slice(0, 40), lh = size * 1.3
      const ta = o.ta ?? def.anchor ?? 'left', va = o.va ?? def.va ?? 'top'
      const tx = ta === 'center' ? x + w / 2 : ta === 'right' ? x + w - 10 : x + 10
      const total = lines.length * lh, ty = (va === 'middle' ? y + (h - total) / 2 : va === 'bottom' ? y + h - total - 8 : y + 8) + size
      const deco = [o.u && 'underline', o.s && 'line-through'].filter(Boolean).join(' ')
      return tr("<text x=\"{0}\" y=\"{1}\" font-size=\"{2}\" fill=\"{3}\" font-family=\"{4}\" text-anchor=\"{5}\"{6}{7}{8}>{9}</text>", [tx, ty, size, o.tc ?? def.color, fam(o), ta === 'center' ? 'middle' : ta === 'right' ? 'end' : 'start', o.b ? ' font-weight="700"' : '', o.i ? ' font-style="italic"' : '', deco ? ` text-decoration="${deco}"` : '', lines.map((l, i) => `<tspan x="${tx}" dy="${i ? lh : 0}">${esc(l)}</tspan>`).join('')])
    }
    const shp = (o: Obj) => {
      const { x, y } = o, w = o.w ?? 140, h = o.h ?? 90, X = (v: number) => x + (v / 100) * w, Y = (v: number) => y + (v / 100) * h
      const pts = (a: number[][]) => a.map(([u, v]) => `${X(u)},${Y(v)}`).join(' ')
      const fill = o.bg && o.bg !== 'none' ? o.bg : o.fill !== undefined ? SHADES[o.fill]?.[0] : 'none'
      const sw = o.bw ?? 1.5, st = tr("fill=\"{0}\" stroke=\"{1}\" stroke-width=\"{2}\"{3}", [fill, sw ? o.bc ?? '#1a1a1a' : 'none', sw, (o.bs ?? (o.dashed ? 'dashed' : '')) === 'dashed' ? ' stroke-dasharray="8 6"' : o.bs === 'dotted' ? ' stroke-dasharray="2 4"' : ''])
      const k = o.shape
      if (k === 'ellipse') return `<ellipse cx="${x + w / 2}" cy="${y + h / 2}" rx="${w / 2}" ry="${h / 2}" ${st}/>`
      const poly: Record<string, number[][]> = { diamond: [[50, 0], [100, 50], [50, 100], [0, 50]], triangle: [[50, 0], [100, 100], [0, 100]], hexagon: [[25, 0], [75, 0], [100, 50], [75, 100], [25, 100], [0, 50]], para: [[20, 0], [100, 0], [80, 100], [0, 100]], star: [[50, 2], [61, 37], [98, 37], [68, 59], [79, 96], [50, 74], [21, 96], [32, 59], [2, 37], [39, 37]], arrow: [[0, 30], [65, 30], [65, 4], [100, 50], [65, 96], [65, 70], [0, 70]], bubble: [[0, 0], [100, 0], [100, 75], [40, 75], [20, 98], [24, 75], [0, 75]] }
      if (poly[k!]) return tr("<polygon points=\"{0}\" {1}/>", [pts(poly[k!]), st])
      return tr("<rect x=\"{0}\" y=\"{1}\" width=\"{2}\" height=\"{3}\" rx=\"{4}\" {5}/>", [x, y, w, h, k === 'round' ? 16 : 4, st])
    }
    const parts = ordered.map(o => {
      const op = o.op !== undefined ? ` opacity="${o.op}"` : ''
      const g = (inner: string) => `<g${op}>${inner}</g>`
      if (o.kind === 'sticky') { const [bg, fg] = SHADES[o.shade ?? 0]; return g(tr("<rect x=\"{0}\" y=\"{1}\" width=\"{2}\" height=\"{3}\" rx=\"6\" fill=\"{4}\" stroke=\"#d4d4d4\"/>{5}", [o.x, o.y, o.w, o.h, o.bg && o.bg !== 'none' ? o.bg : bg, txt(o, o.x, o.y, o.w ?? 160, o.h ?? 120, { size: 14, color: fg })])) }
      if (o.kind === 'frame') return g(tr("<rect x=\"{0}\" y=\"{1}\" width=\"{2}\" height=\"{3}\" rx=\"10\" fill=\"{4}\" stroke=\"{5}\" stroke-width=\"{6}\"/><text x=\"{7}\" y=\"{8}\" font-size=\"13\" font-family=\"sans-serif\">{9}</text>", [o.x, o.y, o.w, o.h, o.bg && o.bg !== 'none' ? o.bg : 'none', o.bc ?? '#a3a3a3', o.bw ?? 1.5, o.x, o.y - 8, esc(o.text ?? '')]))
      if (o.kind === 'stroke') return g(tr("<polyline points=\"{0}\" fill=\"none\" stroke=\"{1}\" stroke-opacity=\"{2}\" stroke-width=\"{3}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{4}/>", [o.points?.map(p => p.join(',')).join(' '), o.color?.startsWith('#') ? o.color : '#1a1a1a', o.color === 'hl' ? 0.25 : 1, o.width, o.dashed || o.color === 'path' ? ' stroke-dasharray="10 8"' : '']))
      if (o.kind === 'connector') { const col = o.color?.startsWith('#') ? o.color : '#555', w = o.width ?? 1.6; return g(tr("<defs><marker id=\"m{0}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"{1}\" markerHeight=\"{2}\" markerUnits=\"userSpaceOnUse\" orient=\"auto-start-reverse\"><path d=\"M0 0L10 5L0 10z\" fill=\"{3}\"/></marker></defs><path d=\"{4}\" fill=\"none\" stroke=\"{5}\" stroke-width=\"{6}\"{7}{8}{9}/>", [o.id, Math.max(7, w * 3.5), Math.max(7, w * 3.5), col, path(o), col, w, o.dashed ? ` stroke-dasharray="${w * 4} ${w * 3}"` : '', o.arrow ? ` marker-end="url(#m${o.id})"` : '', o.arrowStart ? ` marker-start="url(#m${o.id})"` : ''])) }
      if (o.kind === 'shape') return g(shp(o) + txt(o, o.x, o.y, o.w ?? 140, o.h ?? 90, { size: 13, color: o.fill !== undefined ? SHADES[o.fill]?.[1] ?? '#1a1a1a' : '#1a1a1a', anchor: 'center', va: 'middle' }))
      if (o.kind === 'text' || o.kind === 'comment') return g(`${o.bg && o.bg !== 'none' ? tr("<rect x=\"{0}\" y=\"{1}\" width=\"{2}\" height=\"{3}\" fill=\"{4}\"/>", [o.x, o.y, o.w, o.h, o.bg]) : ''}${o.bw ? tr("<rect x=\"{0}\" y=\"{1}\" width=\"{2}\" height=\"{3}\" fill=\"none\" stroke=\"{4}\" stroke-width=\"{5}\"/>", [o.x, o.y, o.w, o.h, o.bc ?? '#1a1a1a', o.bw]) : ''}${txt(o, o.x - 10, o.y - 8, (o.w ?? 220) + 10, o.h ?? 40, { size: o.kind === 'text' ? 18 : 14, color: '#1a1a1a' })}`)
      if (o.kind === 'uefn') return tr("<circle cx=\"{0}\" cy=\"{1}\" r=\"19\" fill=\"#fff\" stroke=\"#1a1a1a\" stroke-width=\"1.5\"/><text x=\"{2}\" y=\"{3}\" font-size=\"11\" text-anchor=\"middle\" font-family=\"sans-serif\">{4}</text>", [o.x + 19, o.y + 19, o.x + 19, o.y + 60, esc(o.text ?? '')])
      if (o.kind === 'image' && o.url) return g(tr("<image href=\"{0}\" x=\"{1}\" y=\"{2}\" width=\"{3}\" height=\"{4}\" preserveAspectRatio=\"xMidYMid meet\"/>", [esc(o.url), o.x, o.y, o.w, o.h]))
      return ''
    })
    return tr("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{0} {1} {2} {3}\" width=\"{4}\" height=\"{5}\"><rect x=\"{6}\" y=\"{7}\" width=\"{8}\" height=\"{9}\" fill=\"#fff\"/>{10}</svg>", [x0, y0, w, h, w, h, x0, y0, w, h, parts.join('')])
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
    const edge = (o: Obj, t: { x: number; y: number }) => { const c = anchor(o), hw = (o.w ?? 38) / 2 + 4, hh = (o.h ?? 38) / 2 + 4, dx = t.x - c.x, dy = t.y - c.y; if (!dx && !dy) return c; const k = Math.min(dx ? hw / Math.abs(dx) : 1e9, dy ? hh / Math.abs(dy) : 1e9); return { x: c.x + dx * k, y: c.y + dy * k } }
    const p = edge(a, anchor(b)), q = edge(b, anchor(a))
    if (c.style === 'straight') return tr("M{0},{1} L{2},{3}", [p.x, p.y, q.x, q.y])
    if (c.style === 'elbow') return tr("M{0},{1} L{2},{3} L{4},{5} L{6},{7}", [p.x, p.y, (p.x + q.x) / 2, p.y, (p.x + q.x) / 2, q.y, q.x, q.y])
    return tr("M{0},{1} C{2},{3} {4},{5} {6},{7}", [p.x, p.y, (p.x + q.x) / 2, p.y, (p.x + q.x) / 2, q.y, q.x, q.y])
  }
  const selSticky = sel.filter(s => objs[s]?.kind === 'sticky')
  // Barre de mise en forme au-dessus de la sélection (comme Miro), sinon en dessous si pas la place.
  const barPos = (so: Obj[]) => {
    const pts = so.flatMap(o => o.kind === 'stroke' ? (o.points ?? []) : o.kind === 'connector' ? [objs[o.from!], objs[o.to!]].filter(Boolean).map(a => [a.x + (a.w ?? 38) / 2, a.y + (a.h ?? 38) / 2]) : [[o.x, o.y], [o.x + (o.w ?? 40), o.y + (o.h ?? 40)]])
    if (!pts.length) return null
    const xs = pts.map(p => p[0]), ys = pts.map(p => p[1])
    return { left: ((Math.min(...xs) + Math.max(...xs)) / 2) * view.z + view.x, top: Math.min(...ys) * view.z + view.y, bottom: Math.max(...ys) * view.z + view.y }
  }
  const selFrame = sel.length === 1 && objs[sel[0]]?.kind === 'frame' ? sel[0] : null
  const ranking = list.filter(o => o.kind === 'sticky').map(o => ({ o, v: votesOf(o) })).filter(x => x.v > 0).sort((a, b) => b.v - a.v)
  const TOOLS: [string, string, any][] = [['select', tr("Sélectionner (V)"), MousePointer2], ['sticky', tr("Post-it (N)"), StickyNote], ['text', tr("Texte (T)"), Type], ['rect', tr("Rectangle (R)"), Square], ['ellipse', tr("Cercle (O)"), Circle], ['connector', tr("Flèche entre deux objets (C)"), Spline], ['pen', tr("Stylo (P)"), Pen], ['erase', tr("Gomme (E)"), Eraser]]
  const MORE: [string, string, any][] = [['lasso', tr("Lasso (L)"), Lasso], ['hl', tr("Surligneur (H)"), Highlighter], ['frame', tr("Cadre / diapo (F)"), Frame], ['comment', tr("Commentaire épinglé (M)"), MessageSquarePlus]]

  return (
    <>
      <section ref={wbRef} className={'wb' + (fs ? ' fs' : '')} aria-label={tr("Canevas")}>
        <div className="wbtop">
          <div className="toolbar" role="toolbar" aria-label={tr("Outils")}>
            {TOOLS.map(([k, l, I]) => <button key={k} className={'tool' + (tool === k ? ' on' : '')} title={l} aria-label={l} aria-pressed={tool === k} onClick={() => { setTool(k); setConnFrom(null) }} disabled={!canEdit && k !== 'select'}><I size={18} strokeWidth={1.5} /></button>)}
            <label className={'tool' + (canEdit ? '' : ' off')} title={tr("Image (ou glissez-la sur le tableau)")} aria-label={tr("Importer une image")}><ImagePlus size={18} strokeWidth={1.5} /><input type="file" accept="image/*" hidden disabled={!canEdit} onChange={e => { const f = e.target.files?.[0]; if (f) importImage(f); e.target.value = '' }} /></label>
            <div style={{ position: 'relative' }}>
              <button className={'tool' + (MORE.some(m => m[0] === tool) ? ' on' : '')} title={tr("Plus d’outils")} aria-label={tr("Plus d’outils")} aria-expanded={menu === 'tools'} onClick={() => setMenu(menu === 'tools' ? null : 'tools')}><MoreHorizontal size={18} strokeWidth={1.5} /></button>
              {menu === 'tools' && <div className="menu" style={{ left: 0, top: 44 }} onMouseLeave={() => setMenu(null)}>
                {MORE.map(([k, l, I]) => <button key={k} disabled={!canEdit} onClick={() => { setTool(k); setConnFrom(null); setMenu(null) }}><I size={16} />{l}</button>)}
                <button disabled={!canEdit} onClick={() => { setMenu(null); const u = prompt(tr("Lien https de l’image (plan de map vu du dessus, capture…)")); if (u && /^https:\/\//.test(u)) add({ kind: 'image', url: u, w: 600, h: 400, locked: confirm(tr("Utiliser comme fond « plan de map » (verrouillé derrière) ?")) }) }}><Upload size={16} />{tr("Image par lien")}</button>
                <button onClick={() => { setLib(l => !l); setMenu(null) }}><LayoutGrid size={16} />{tr("{0} les formes UEFN", [lib ? tr("Masquer") : tr("Afficher")])}</button>
              </div>}
            </div>
            <span className="tsep" />
            <button className="tool" title={tr("Annuler (Ctrl+Z)")} aria-label={tr("Annuler")} disabled={!canEdit} onClick={() => travel('undo')}><Undo2 size={18} strokeWidth={1.5} /></button>
            <button className="tool" title={tr("Rétablir (Ctrl+Maj+Z)")} aria-label={tr("Rétablir")} disabled={!canEdit} onClick={() => travel('redo')}><Redo2 size={18} strokeWidth={1.5} /></button>
            {tool === 'sticky' && SHADES.map(([bg], i) => <button key={i} className="tool" title={SHADE_NAMES[i]} aria-label={SHADE_NAMES[i]} onClick={() => setShade(i)}><span style={{ width: 18, height: 18, borderRadius: 4, background: bg, border: shade === i ? tr("2px solid var(--text)") : tr("1px solid var(--border)") }} /></button>)}
            {(tool === 'pen') && <PenBar color={pen.color} width={pen.width} onColor={c => setPen(p => ({ ...p, color: c }))} onWidth={w => setPen(p => ({ ...p, width: w }))} />}
            {tool === 'connector' && <select className="field" style={{ height: 34 }} value={connStyle} onChange={e => setConnStyle(e.target.value)} aria-label={tr("Style")}><option value="straight">{tr("Droit")}</option><option value="elbow">{tr("Coudé")}</option><option value="curve">{tr("Courbe")}</option></select>}
            {tool === 'connector' && <span className="small muted">{connFrom ? tr("Cliquez l’objet d’arrivée") : tr("Cliquez l’objet de départ")}</span>}
          </div>
          <div className="toolbar" aria-label={tr("Actions")}>
            {timerLeft ? <button className="pill on" onClick={() => setMetaOp({ timer_end: null })} disabled={!canEdit}><Timer size={16} />{`${String(Math.floor(timerLeft / 60)).padStart(2, '0')}:${String(timerLeft % 60).padStart(2, '0')}`}</button> : null}
            {voting && <button className="pill on" onClick={() => setVoting(false)}><ThumbsUp size={16} />{tr("Vote · {0} pts · Terminer", [Math.max(0, meta.points - myVotes)])}</button>}
            <div style={{ position: 'relative' }}>
              <button className="pill" onClick={() => setMenu(menu === 'atelier' ? null : 'atelier')} aria-expanded={menu === 'atelier'}><Users size={16} />{tr("Atelier")}</button>
              {menu === 'atelier' && <div className="menu" style={{ right: 0, top: 44 }} onMouseLeave={() => setMenu(null)}>
                <button disabled={!canEdit} onClick={() => { setMetaOp({ timer_end: timerLeft ? null : Date.now() + 5 * 60000 }); setMenu(null) }}><Timer size={16} />{timerLeft ? tr("Arrêter le minuteur") : tr("Minuteur 5 min")}</button>
                <button onClick={() => { setVoting(v => !v); setMenu(null) }}><ThumbsUp size={16} />{voting ? tr("Terminer le vote") : tr("Voter pour les post-it")}</button>
                <button disabled={!canEdit} onClick={() => { setMetaOp({ hidden: !meta.hidden }); setMenu(null) }}>{meta.hidden ? <><Eye size={16} />{tr("Révéler les idées")}</> : <><EyeOff size={16} />{tr("Cacher les idées des autres")}</>}</button>
                <button disabled={!canEdit} onClick={() => { groupByShade(); setMenu(null) }}><LayoutGrid size={16} />{tr("Regrouper par couleur")}</button>
                {Object.entries(cursors).map(([u, c]) => <button key={u} onClick={() => { setFollow(Number(u)); setMenu(null) }}><UserRound size={16} />{tr("Suivre {0}", [c.name])}</button>)}
              </div>}
            </div>
            <button className="pill" onClick={() => frames.length ? goFrame(0) : app.toast(tr("Ajoutez des cadres (menu « … ») pour présenter."))}><Presentation size={16} />{tr("Présenter")}</button>
            <div style={{ position: 'relative' }}>
              <button className="pill" onClick={() => setMenu(menu === 'export' ? null : 'export')} aria-expanded={menu === 'export'}><Download size={16} />{tr("Exporter")}</button>
              {menu === 'export' && <div className="menu" style={{ right: 0, top: 44, minWidth: 160 }} onMouseLeave={() => setMenu(null)}>
                <button onClick={() => { exportPng(); setMenu(null) }}>{tr("Image PNG")}</button>
                <button onClick={() => { exportSvg(); setMenu(null) }}>{tr("Image SVG")}</button>
                <button onClick={() => { setMenu(null); window.print() }}>PDF (impression)</button>
              </div>}
            </div>
            <button className={'pill' + (fs ? ' on' : '')} onClick={toggleFs} title={tr("Plein écran (Échap pour sortir)")}>{fs ? <><Shrink size={16} />{tr("Quitter")}</> : <><Expand size={16} />{tr("Plein écran")}</>}</button>
          </div>
        </div>
        <div className="wbbody">
          {lib && <aside className="lib" aria-label={tr("Formes UEFN")}>
            <div className="row" style={{ justifyContent: 'space-between' }}><b style={{ fontWeight: 500 }}>{tr("Formes UEFN")}</b><button className="iconbtn" style={{ width: 26, height: 26 }} aria-label={tr("Masquer")} onClick={() => setLib(false)}><X size={14} /></button></div>
            {UEFN.map(([k, l, ic]) => <button key={k} className="libitem" disabled={!canEdit} onClick={() => add(k === 'zone' ? { kind: 'shape', shape: 'rect', w: 240, h: 160, text: tr("Zone"), dashed: true } : k === 'road' || k === 'wall' ? { kind: 'shape', shape: 'rect', w: k === 'road' ? 260 : 160, h: k === 'road' ? 28 : 14, text: '' } : k === 'path' ? { kind: 'stroke', x: 0, y: 0, points: [[center().x - 100, center().y], [center().x + 100, center().y]], width: 3, color: 'path' } : { kind: 'uefn', shape: k, w: 38, h: 38, text: l })}><span className="lico">{ic}</span>{l}</button>)}
            {ranking.length > 0 && <>
              <div style={{ fontWeight: 500, margin: '16px 0 8px' }}>{tr("Classement")}</div>
              {ranking.slice(0, 8).map(({ o, v }, i) => <div key={o.id} className="small" style={{ padding: '4px 0' }}><span className="mono muted">{i + 1}.</span> {meta.hidden && o.author_id !== me ? tr("Idée cachée") : o.text} <b style={{ fontWeight: 500 }}>· {v}</b></div>)}
            </>}
          </aside>}
          <div ref={box} className="canvas" style={{ backgroundSize: `${GRID * view.z}px ${GRID * view.z}px`, backgroundPosition: `${view.x}px ${view.y}px`, cursor: tool === 'select' ? 'default' : 'crosshair' }}
            onPointerDown={onDown} onPointerMove={onMove} onPointerUp={onUp}
            onDragOver={e => { if (canEdit && e.dataTransfer.types.includes(tr("Files"))) e.preventDefault() }}
            onDrop={e => { e.preventDefault(); const f = e.dataTransfer.files?.[0]; if (f && canEdit) importImage(f, toWorld(e.clientX, e.clientY)) }} onDoubleClick={e => { const t = (e.target as HTMLElement).closest('[data-oid]') as HTMLElement | null; if (t && canEdit && !objs[t.dataset.oid!]?.lk && !['image', 'stroke', 'connector', 'uefn'].includes(objs[t.dataset.oid!]?.kind)) { setSel([t.dataset.oid!]); setEditing(t.dataset.oid!) } }}>
            <div className="layer" style={{ transform: tr("translate({0}px,{1}px) scale({2})", [view.x, view.y, view.z]) }}>
              <svg style={{ position: 'absolute', left: 0, top: 0, overflow: 'visible', pointerEvents: 'none' }} width="1" height="1">
                                {list.filter(o => o.kind === 'connector').map(c => { const w = c.width ?? 1.6; const col = strokeColor(c, 'var(--text2)'); return <g key={c.id} opacity={c.op}>
                  <defs><marker id={'arr-' + c.id} viewBox="0 0 10 10" refX="9" refY="5" markerWidth={Math.max(7, w * 3.5)} markerHeight={Math.max(7, w * 3.5)} markerUnits="userSpaceOnUse" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill={col} /></marker></defs>
                  {sel.includes(c.id) && <path d={path(c)} fill="none" stroke="#ED753E" strokeOpacity={0.35} strokeWidth={w + 8} />}
                  <path data-oid={c.id} d={path(c)} fill="none" stroke={col} strokeWidth={w} strokeDasharray={c.dashed ? `${w * 4} ${w * 3}` : undefined} markerEnd={c.arrow ? `url(#arr-${c.id})` : undefined} markerStart={c.arrowStart ? `url(#arr-${c.id})` : undefined} style={{ pointerEvents: 'stroke' }} />
                  <path data-oid={c.id} d={path(c)} fill="none" stroke="transparent" strokeWidth={Math.max(12, w + 8)} style={{ pointerEvents: 'stroke' }} />
                </g> })}
                {list.filter(o => o.kind === 'stroke').map(s => <g key={s.id} opacity={s.op}>
                  {sel.includes(s.id) && <polyline points={s.points?.map(p => p.join(',')).join(' ')} fill="none" stroke="#ED753E" strokeOpacity={0.35} strokeWidth={(s.width ?? 2) + 8} strokeLinecap="round" strokeLinejoin="round" />}
                  <polyline data-oid={s.id} points={s.points?.map(p => p.join(',')).join(' ')} fill="none" stroke={s.color === 'hl' ? 'var(--text)' : strokeColor(s)} strokeOpacity={s.color === 'hl' ? 0.25 : 1} strokeWidth={s.width} strokeDasharray={s.color === 'path' || s.dashed ? '10 8' : undefined} strokeLinecap="round" strokeLinejoin="round" style={{ pointerEvents: 'stroke' }} />
                </g>)}
              </svg>
              {ordered.filter(o => !['connector', 'stroke'].includes(o.kind)).map(o => {
                const isSel = sel.includes(o.id) ? ' sel' : ''
                const st = { left: o.x, top: o.y, width: o.w, height: o.h }
                const ed = editing === o.id
                const ts = textStyle(o)
                const input = <RichEdit o={o} style={ts} onType={(h, t) => typeText(o, h, t)} onDone={(h, t) => { const cur = objsRef.current[o.id]; setEditing(e => e === o.id ? null : e); if (!cur) return; push([{ ...cur, text: t.slice(0, 2000), html: h }], [{ ...cur, text: editStart.current?.text, html: editStart.current?.html }]) }} />
                const bx = (def = 'flex-start'): React.CSSProperties => ({ ...st, opacity: o.op, justifyContent: vAlign(o, def) })
                const border = (o.bw ?? 0) > 0 ? { border: `${o.bw}px ${o.bs ?? 'solid'} ${o.bc ?? 'var(--text)'}` } : {}
                const lk = o.lk ? ' locked' : ''
                if (o.kind === 'image') return <img key={o.id} data-oid={o.id} className={'obj' + isSel + lk} src={o.url} alt="" draggable={false} style={{ ...st, opacity: o.locked ? 0.5 : o.op ?? 1, pointerEvents: o.locked ? 'none' : 'auto', objectFit: 'contain' }} />
                if (o.kind === 'frame') return <div key={o.id} data-oid={o.id} className={'obj frame' + isSel + lk} style={{ ...st, opacity: o.op, background: o.bg && o.bg !== 'none' ? o.bg : undefined, ...(o.bw !== undefined ? { border: o.bw ? `${o.bw}px ${o.bs ?? 'solid'} ${o.bc ?? 'var(--text3)'}` : '0' } : {}) }}><span className="fname" style={ts}>{ed ? <input autoFocus defaultValue={o.text} onPointerDown={e => e.stopPropagation()} onBlur={e => { push([{ ...o, text: e.target.value }]); setEditing(null) }} className="field" style={{ height: 26 }} /> : o.text}</span></div>
                if (o.kind === 'sticky') {
                  const [sbg, sfg] = SHADES[o.shade ?? 0]; const bg = o.bg && o.bg !== 'none' ? o.bg : sbg; const hide = meta.hidden && o.author_id !== me; const t = taskOf(o.task_number)
                  return <div key={o.id} data-oid={o.id} className={'obj sticky' + isSel + lk} style={{ ...bx(), background: bg, color: sfg }}>
                    <div className="tbody" style={{ ...ts, justifyContent: vAlign(o) }}>{ed ? input : hide ? <div className="hiddenidea">{tr("Idée cachée")}<div className="small">de {o.author?.split(' ')[0]}</div></div> : <Rich o={o} placeholder={tr("Double-clic pour écrire")} />}</div>
                    <div className="row" style={{ justifyContent: 'space-between', gap: 6, flex: 'none' }}><span className="small" style={{ opacity: 0.7 }}>{o.author?.split(' ')[0]}</span>{votesOf(o) > 0 && <span className="votes">{votesOf(o)} vote{votesOf(o) > 1 ? 's' : ''}</span>}</div>
                    {o.task_number && <span className="tasklink badge" style={{ background: 'var(--surface)' }}>#{o.task_number} · {t ? ({ todo: tr("À faire"), doing: tr("En cours"), blocked: tr("Bloqué"), done: tr("Fait") } as any)[t.status] : '…'}</span>}
                  </div>
                }
                if (o.kind === 'uefn') return <div key={o.id} data-oid={o.id} className={'obj uefn' + isSel + lk} style={{ left: o.x, top: o.y, opacity: o.op }} title={o.text}>{UEFN.find(u => u[0] === o.shape)?.[2] ?? '•'}<span className="ulab">{o.text}</span></div>
                if (o.kind === 'shape') {
                  const so = { ...o, bg: o.bg ?? (o.fill !== undefined ? SHADES[o.fill]?.[0] : undefined) }
                  return <div key={o.id} data-oid={o.id} className={'obj shape' + isSel + lk} style={{ ...bx('center'), color: o.tc ?? (o.fill !== undefined ? SHADES[o.fill]?.[1] : undefined) }}>
                    <ShapeSvg o={so} />
                    <div className="tbody" style={{ ...ts, textAlign: o.ta ?? 'center', justifyContent: vAlign(o, 'center') }}>{ed ? input : <Rich o={o} />}</div>
                  </div>
                }
                if (o.kind === 'comment') return <div key={o.id} data-oid={o.id} className={'obj sticky' + isSel + lk} style={{ ...bx(), background: o.bg && o.bg !== 'none' ? o.bg : 'var(--surface)', color: 'var(--text)', border: tr("1px solid var(--border)"), ...border }}><span className="small muted">{tr("Commentaire · {0}", [o.author?.split(' ')[0]])}</span><div className="tbody" style={ts}>{ed ? input : <Rich o={o} />}</div></div>
                return <div key={o.id} data-oid={o.id} className={'obj wtext' + isSel + lk} style={{ ...bx(), background: o.bg && o.bg !== 'none' ? o.bg : undefined, ...border, padding: o.bg || o.bw ? 8 : undefined }}><div className="tbody" style={ts}>{ed ? input : <Rich o={o} />}</div></div>
              })}
              {canEdit && sel.length === 1 && objs[sel[0]] && !['connector', 'stroke', 'uefn'].includes(objs[sel[0]].kind) && !objs[sel[0]].lk && editing !== sel[0] && (() => { const o = objs[sel[0]]; return <div data-handle={o.id} className="rhandle" title={tr("Redimensionner (Maj : garder les proportions)")} style={{ left: o.x + (o.w ?? 40) - 6, top: o.y + (o.h ?? 40) - 6, transform: `scale(${1 / view.z})` }} /> })()}
              {lasso && <div style={{ position: 'absolute', left: Math.min(lasso.x0, lasso.x1), top: Math.min(lasso.y0, lasso.y1), width: Math.abs(lasso.x1 - lasso.x0), height: Math.abs(lasso.y1 - lasso.y0), border: tr("1px dashed var(--text)"), background: 'rgba(127,127,127,.08)' }} />}
              {Object.entries(cursors).filter(([, c]) => Date.now() - c.t < 15000).map(([u, c]) => (
                <div key={u} className="wcursor" style={{ left: c.x, top: c.y, transform: `scale(${1 / view.z})`, transformOrigin: '0 0' }}><svg width="16" height="16" viewBox="0 0 24 24"><path d="M3 2l7 19 2.5-8.5L21 10z" fill="var(--text)" /></svg><span>{c.name.split(' ')[0]}</span></div>
              ))}
            </div>

            {canEdit && sel.length > 0 && (() => {
              const so = sel.map(i => objs[i]).filter(Boolean)
              if (!so.length) return null
              const ed = editing && sel.includes(editing)
              return <FormatBar sel={so} editing={!!ed} push={l => push(l as Obj[])} pos={barPos(so)} extra={<>
                {selSticky.length > 0 && selSticky.length === sel.length && <>
                  <div className="fpop"><button className={'fbtn' + (menu === 'shade' ? ' on' : '')} title={tr("Couleur du post-it")} aria-label={tr("Couleur du post-it")} onClick={() => setMenu(menu === 'shade' ? null : 'shade')}><span className="sw" style={{ width: 16, height: 16, background: objs[selSticky[0]]?.bg ?? SHADES[objs[selSticky[0]]?.shade ?? 0][0] }} /></button>
                    {menu === 'shade' && <div className="fmenu"><div className="row" style={{ gap: 6 }}>{SHADES.map(([bg], i) => <button key={i} className="sw" style={{ background: bg, width: 22, height: 22 }} title={SHADE_NAMES[i]} aria-label={SHADE_NAMES[i]} onClick={() => { push(selSticky.map(s => ({ ...objs[s], shade: i, bg: undefined }))); setMenu(null) }} />)}</div></div>}</div>
                  <div className="fpop"><button className={'fbtn' + (menu === 'conv' ? ' on' : '')} title={tr("Convertir en tâches")} onClick={() => setMenu(menu === 'conv' ? null : 'conv')} style={{ width: 'auto', padding: '0 8px' }}>{tr("Tâches")}</button>
                    {menu === 'conv' && <div className="fmenu" style={{ display: 'grid', gap: 8, width: 220 }}>
                      <select className="field" style={{ height: 34 }} value={convert.assignee} onChange={e => setConvert({ ...convert, assignee: e.target.value })} aria-label={tr("Assigné")}><option value="">{tr("Assigné…")}</option>{mem?.members.map((m: any) => <option key={m.id} value={m.id}>{m.name}</option>)}</select>
                      <select className="field" style={{ height: 34 }} value={convert.tag} onChange={e => setConvert({ ...convert, tag: e.target.value })} aria-label={tr("Étiquette")}>{['code', 'carte', 'conception', 'test'].map(t => <option key={t} value={t}>{tr(t)}</option>)}</select>
                      <button className="btn pri" style={{ height: 34 }} onClick={() => { doConvert(); setMenu(null) }}>{tr("Convertir {0} post-it", [selSticky.length])}</button>
                    </div>}</div>
                </>}
                {selFrame && <button className="fbtn" style={{ width: 'auto', padding: '0 8px' }} onClick={() => frameToPage(selFrame)}>{tr("Cadre → page")}</button>}
                {sel.length > 1 && <>
                  <button className="fbtn" title={tr("Aligner à gauche")} aria-label={tr("Aligner à gauche")} onClick={() => align('left')}><AlignStartVertical size={16} /></button>
                  <button className="fbtn" title={tr("Centrer horizontalement")} aria-label={tr("Centrer horizontalement")} onClick={() => align('hcenter')}><AlignCenterVertical size={16} /></button>
                  <button className="fbtn" title={tr("Aligner en haut")} aria-label={tr("Aligner en haut")} onClick={() => align('top')}><AlignStartHorizontal size={16} /></button>
                  <button className="fbtn" title={tr("Centrer verticalement")} aria-label={tr("Centrer verticalement")} onClick={() => align('vcenter')}><AlignCenterHorizontal size={16} /></button>
                  {sel.length > 2 && <button className="fbtn" style={{ width: 'auto', padding: '0 8px' }} onClick={() => align('hdist')}>{tr("Répartir")}</button>}
                </>}
                <button className="fbtn" title={tr("Premier plan")} aria-label={tr("Premier plan")} onClick={() => push(sel.map(i => ({ ...objs[i], z: zTop() })))}><BringToFront size={16} /></button>
                <button className="fbtn" title={tr("Arrière-plan")} aria-label={tr("Arrière-plan")} onClick={() => push(sel.map(i => ({ ...objs[i], z: zBottom() })))}><SendToBack size={16} /></button>
                <button className="fbtn" title={tr("Dupliquer (Ctrl+D)")} aria-label={tr("Dupliquer")} onClick={() => paste(withLinks(sel))}><Copy size={16} /></button>
                <button className="fbtn" title={tr("Supprimer (Suppr)")} aria-label={tr("Supprimer")} onClick={() => del(sel)}><Trash2 size={16} /></button>
              </>} />
            })()}
            {present !== null && <div className="selbar" onPointerDown={e => e.stopPropagation()}><button className="iconbtn" aria-label={tr("Cadre précédent")} onClick={() => goFrame(Math.max(0, present - 1))}><ChevronLeft size={18} /></button><span>{frames[present]?.text} · {present + 1}/{frames.length}</span><button className="iconbtn" aria-label={tr("Cadre suivant")} onClick={() => goFrame(Math.min(frames.length - 1, present + 1))}><ChevronRight size={18} /></button><button className="iconbtn" aria-label={tr("Quitter")} onClick={() => setPresent(null)}><X size={18} /></button></div>}
            <div className="zoombox" onPointerDown={e => e.stopPropagation()}>
              <button className="iconbtn" aria-label={tr("Dézoomer")} onClick={() => zoom(0.85)}><Minus size={16} /></button>
              <span className="small mono" style={{ minWidth: 46, textAlign: 'center' }}>{Math.round(view.z * 100)} %</span>
              <button className="iconbtn" aria-label={tr("Zoomer")} onClick={() => zoom(1.15)}><Plus size={16} /></button>
              <button className="pill" title={tr("Recadrer pour voir tout le tableau (Ctrl+0)")} onClick={fit}><Maximize size={14} />{tr("Tout voir")}</button>
              <button className={'pill' + (gridOn ? ' on' : '')} title={tr("Aligner les objets sur la grille en les déplaçant")} onClick={() => setGridOn(g => !g)}>{tr("Grille")}</button>
              {follow && <button className="pill on" onClick={() => setFollow(null)}><UserRound size={14} />{tr("Ne plus suivre")}</button>}
            </div>
            <MiniMap objs={list} view={view} box={box} />
            {fs && <button className="btn fsexit" onPointerDown={e => e.stopPropagation()} onClick={toggleFs}><Shrink size={16} />{tr("Quitter le plein écran")}</button>}
          </div>
        </div>
      </section>
      <p className="small muted">{tr("Astuces : molette de la souris ou pincer sur le trackpad pour zoomer, deux doigts ou Espace + glisser pour se déplacer, raccourcis V L N T R O C P H E F M pour les outils, Ctrl+Z / Ctrl+Maj+Z, Ctrl+C / V / D / X, flèches pour déplacer, poignée en bas à droite pour redimensionner, glisser une image pour l’importer, double-clic pour écrire, Maj+clic pour sélectionner plusieurs objets, Tab / Entrée sur un post-it pour la carte mentale, Suppr pour effacer. En mode vote, cliquez un post-it pour voter (Maj+clic pour retirer).")}</p>
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
      {items.map(o => <div key={o.id} style={{ position: 'absolute', left: (o.x - x0) * k, top: (o.y - y0) * k, width: Math.max(2, (o.w ?? 30) * k), height: Math.max(2, (o.h ?? 30) * k), background: o.kind === 'frame' ? 'transparent' : 'var(--text3)', border: o.kind === 'frame' ? tr("1px solid var(--text3)") : 0 }} />)}
      <div style={{ position: 'absolute', left: (-view.x / view.z - x0) * k, top: (-view.y / view.z - y0) * k, width: (r.width / view.z) * k, height: (r.height / view.z) * k, border: tr("1.5px solid var(--text)"), borderRadius: 3 }} />
    </div>
  )
}
