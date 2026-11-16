import { useEffect, useMemo, useRef, useState } from 'react'
import {
  Folder, FolderOpen, ChevronRight, ChevronDown, Eye, EyeOff, Lock, Copy, Trash2, FolderInput, Pencil, Save, Archive, RotateCcw, RefreshCw, Plus, X, Star, AlertTriangle, Link2, Cpu, Box, Shapes, Code2, Check, Ban,
} from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead, PageHead, Tabs } from '../components/ui'
import { STATUS_LABEL } from '../uefnState'

type Actor = {
  id: string; name: string; type: string; class: string; folder: string; hidden: boolean; kind_label?: string; icon?: string; locked_by?: string
  transform: { location: number[]; rotation: number[]; scale: number[] }; tags: string[]; options: Record<string, any>; props: Record<string, any>
  events: { event: string; target: string; function: string }[]; schema?: { key: string; label: string; type: string; values: string[]; default: any; help: string; disabled?: string }[]
}

const norm = (s: string) => s.toLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/[’']/g, "'")
const TYPE_LABEL: Record<string, string> = { device: 'Appareil', prop: 'Accessoire', mesh: 'Maillage', verse_device: 'Appareil Verse' }
const TYPE_ICON: Record<string, any> = { device: Cpu, prop: Box, mesh: Shapes, verse_device: Code2 }

/** « tag:vault_plate type:appareil dossier:Planque 3 option:Équipe 2 texte libre » */
function matches(a: Actor, q: string) {
  if (!q.trim()) return true
  const parts = q.match(/(\w+:"[^"]+"|\w+:\S+|"[^"]+"|\S+)/g) ?? []
  return parts.every(p => {
    const m = p.match(/^(\w+):(.+)$/)
    const v = norm((m ? m[2] : p).replace(/"/g, ''))
    if (m) {
      const k = norm(m[1])
      if (k === 'tag') return a.tags.some(t => norm(t).includes(v))
      if (k === 'type') return norm(a.type).includes(v) || norm(TYPE_LABEL[a.type] ?? '').includes(v) || norm(a.kind_label ?? '').includes(v)
      if (k === 'dossier' || k === 'folder') return norm(a.folder).includes(v)
      if (k === 'classe' || k === 'class') return norm(a.class).includes(v)
      if (k === 'option') return Object.values({ ...a.options, ...a.props }).some(x => norm(String(x)).includes(v))
    }
    const hay = norm([a.name, a.class, a.kind_label, TYPE_LABEL[a.type], a.folder, ...a.tags, ...Object.values(a.options).map(String), ...Object.values(a.props).map(String)].join(' '))
    return v.split(/\s+/).every(w => hay.includes(w) || (w.length > 4 && hay.includes(w.slice(0, -1))))
  })
}

export default function Organizer() {
  const app = useApp()
  const { map, me, canEdit, isAdmin, role, toast, subscribe } = app
  const { data: st, reload } = useLoad<any>(map ? `/api/maps/${map.id}/uefn` : null, [map?.id])
  const { data: filters, reload: reloadFilters } = useLoad<any[]>(map ? `/api/maps/${map.id}/filters` : null, [map?.id])
  const { data: tasks } = useLoad<any[]>(map ? `/api/maps/${map.id}/tasks` : null, [map?.id])
  const [q, setQ] = useState(() => new URLSearchParams(location.search).get('q') ?? '')
  const [sel, setSel] = useState<string[]>([])
  const [anchor, setAnchor] = useState<string | null>(null)
  const [closed, setClosed] = useState<Set<string>>(new Set())
  const [tab, setTab] = useState<'details' | 'top' | 'journal'>('details')
  const [task, setTask] = useState<number | ''>('')
  const [showLink, setShowLink] = useState(false)
  useLive(['uefn'], reload)

  const actors: Actor[] = st?.actors ?? []
  const vis = useMemo(() => actors.filter(a => matches(a, q)), [actors, q])
  const selected = actors.filter(a => sel.includes(a.id))
  const canDelete = ['owner', 'admin', 'member'].includes(role)
  const readOnly = !canEdit || st?.status !== 'connected'

  const act = async (action: string, args: any, ok?: string) => {
    try {
      const r = await api.post(`/api/maps/${map!.id}/uefn/action`, { action, args, task: task || null })
      toast(r.queued ? 'UEFN indisponible : changement mis en attente' : ok ?? 'Envoyé à UEFN')
      reload()
      return r
    } catch (e: any) { toast(e.message, true) }
  }
  const update = (ids: string[], changes: any) => act('actor_update', { ids, changes }, `${ids.length} acteur(s) modifié(s)`)

  // Ctrl+Z : annule votre dernière action
  useEffect(() => {
    const h = async (e: KeyboardEvent) => {
      if (!((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') || (e.target as HTMLElement).tagName === 'INPUT') return
      e.preventDefault()
      const j: any[] = await api.get(`/api/maps/${map!.id}/uefn/journal`)
      const last = j.find(x => x.undoable && x.who === me?.user.name)
      if (!last) return toast('Rien à annuler.')
      try { await api.post(`/api/uefn-actions/${last.id}/undo`); toast(`Annulé : ${last.summary.slice(0, 60)}`); reload() } catch (er: any) { toast(er.message, true) }
    }
    window.addEventListener('keydown', h)
    return () => window.removeEventListener('keydown', h)
  }, [map?.id, me?.user.name])

  // Arbre des dossiers
  type Node = { name: string; path: string; kids: Map<string, Node>; actors: Actor[] }
  const root = useMemo(() => {
    const r: Node = { name: '', path: '', kids: new Map(), actors: [] }
    vis.forEach(a => {
      let n = r
      ;(a.folder || '').split('/').filter(Boolean).forEach((part, i, arr) => {
        const path = arr.slice(0, i + 1).join('/')
        if (!n.kids.has(part)) n.kids.set(part, { name: part, path, kids: new Map(), actors: [] })
        n = n.kids.get(part)!
      })
      n.actors.push(a)
    })
    return r
  }, [vis])
  const flat = useMemo(() => { const out: string[] = []; const walk = (n: Node) => { [...n.kids.values()].sort((a, b) => a.name.localeCompare(b.name)).forEach(walk); n.actors.forEach(a => out.push(a.id)) }; walk(root); return out }, [root])
  const click = (e: React.MouseEvent, id: string) => {
    if (e.shiftKey && anchor) { const [i, j] = [flat.indexOf(anchor), flat.indexOf(id)].sort((a, b) => a - b); setSel(flat.slice(i, j + 1)) }
    else if (e.metaKey || e.ctrlKey) setSel(s => (s.includes(id) ? s.filter(x => x !== id) : [...s, id]))
    else setSel([id])
    setAnchor(id)
  }
  const NodeView = ({ n, depth }: { n: Node; depth: number }) => {
    const open = !closed.has(n.path)
    const count = (x: Node): number => x.actors.length + [...x.kids.values()].reduce((s, k) => s + count(k), 0)
    return <>
      {n.path && <button className="otree-folder" style={{ paddingLeft: 8 + depth * 14 }} onClick={() => setClosed(c => { const s = new Set(c); s.has(n.path) ? s.delete(n.path) : s.add(n.path); return s })}>
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}{open ? <FolderOpen size={15} /> : <Folder size={15} />}<span>{n.name}</span><span className="small muted">{count(n)}</span>
      </button>}
      {open && <>
        {[...n.kids.values()].sort((a, b) => a.name.localeCompare(b.name)).map(k => <NodeView key={k.path} n={k} depth={depth + (n.path ? 1 : 0)} />)}
        {n.actors.map(a => { const I = TYPE_ICON[a.type] ?? Box; return (
          <button key={a.id} className={'otree-actor' + (sel.includes(a.id) ? ' on' : '')} style={{ paddingLeft: 8 + (depth + (n.path ? 1 : 0)) * 14 + 18 }} onClick={e => click(e, a.id)} title={a.class}>
            <I size={15} /><span className="oname">{a.name}</span>
            <span className="small muted">{a.kind_label ?? TYPE_LABEL[a.type]}</span>
            {a.hidden && <EyeOff size={13} />}{a.locked_by && <span title={`Verrouillé par ${a.locked_by}`}><Lock size={13} /></span>}
          </button>
        ) })}
      </>}
    </>
  }

  const bulkRename = async () => {
    if (selected.length === 1) { const n = prompt('Nouveau nom', selected[0].name); if (n) update([selected[0].id], { name: n }) }
    else { const base = prompt(`Renommer ${selected.length} acteurs : préfixe (un numéro sera ajouté)`, 'acteur_'); if (base) for (const [i, a] of selected.entries()) await update([a.id], { name: `${base}${String(i + 1).padStart(2, '0')}` }) }
  }
  const moveFolder = () => { const f = prompt('Dossier de destination (ex. Planques/Planque 3)', selected[0]?.folder ?? ''); if (f !== null) update(sel, { folder: f }) }
  const del = () => { if (confirm(`Supprimer ${selected.length} acteur(s) du niveau ?\n${selected.map(a => a.name).join(', ')}`)) act('actor_delete', { ids: sel }, 'Supprimé (annulable avec Ctrl+Z)').then(() => setSel([])) }

  if (!st) return null
  return (
    <main className="content">
      <PageHead title="Organiseur" help="L’outliner d’UEFN dans le navigateur : retrouvez, modifiez et rangez les acteurs du niveau à plusieurs.">
        <span className={'badge ' + (st.status === 'connected' ? 'b-g' : st.status === 'closed' ? 'b-o' : 'b-r')}>{STATUS_LABEL[st.status]}</span>
        <button className="btn" onClick={() => setShowLink(v => !v)}><Link2 size={16} />UEFN Link</button>
        <button className="btn" disabled={st.status !== 'connected'} onClick={() => act('refresh_actors', {}, 'Acteurs actualisés')}><RefreshCw size={16} />Actualiser</button>
      </PageHead>
      {st.status !== 'connected' && <div className="warn"><AlertTriangle size={18} /><span>{st.status === 'offline' ? 'Relais hors ligne' : 'UEFN fermé'} : lecture seule sur le dernier état connu{st.updated_at ? ` (${ago(st.updated_at)})` : ''}. Vos modifications seront mises en attente et envoyées à l’ouverture.</span></div>}
      {st.approvals.length > 0 && <div className="warn" style={{ flexWrap: 'wrap' }}><AlertTriangle size={18} /><div style={{ flex: 1 }}>{st.approvals.map((p: any) => (
        <div key={p.id} className="row" style={{ flexWrap: 'wrap', padding: '4px 0' }}><span style={{ flex: 1 }}>{p.summary}</span>
          {canDelete && <><button className="btn" onClick={async () => { await api.post(`/api/approvals/${p.id}/approve`); reload() }}><Check size={15} />Valider</button><button className="btn" onClick={async () => { await api.post(`/api/approvals/${p.id}/refuse`); reload() }}><Ban size={15} />Refuser</button></>}</div>
      ))}</div></div>}
      {(actors.length === 0 || showLink) && <Pairing status={st.status} tools={st.tools} />}

      <div className="ogrid">
        <aside className="card oleft">
          <input className="field" style={{ width: '100%' }} placeholder="Chercher : nom, type, tag:x, dossier:x, option:x" value={q} onChange={e => setQ(e.target.value)} aria-label="Recherche d’acteurs" />
          <div className="row" style={{ flexWrap: 'wrap', gap: 6, margin: '8px 0' }}>
            {filters?.map(f => <span key={f.id} className={'chip' + (q === f.query ? ' on' : '')} style={{ height: 30, paddingRight: 4 }}><button className="link" style={{ textDecoration: 'none', color: 'inherit' }} onClick={() => setQ(f.query)}><Star size={12} /> {f.name}</button><button className="iconbtn" style={{ width: 22, height: 22 }} aria-label="Supprimer le filtre" onClick={async () => { await api.del(`/api/filters/${f.id}`); reloadFilters() }}><X size={12} /></button></span>)}
            {q && canEdit && <button className="chip" style={{ height: 30 }} onClick={async () => { const n = prompt('Nom du filtre', q); if (n) { await api.post(`/api/maps/${map!.id}/filters`, { name: n, query: q }); reloadFilters() } }}><Plus size={12} />Enregistrer le filtre</button>}
          </div>
          <div className="small muted" style={{ marginBottom: 6 }}>{vis.length} / {actors.length} acteurs{sel.length ? ` · ${sel.length} sélectionné(s)` : ''}</div>
          {sel.length > 0 && <div className="row" style={{ flexWrap: 'wrap', gap: 4, marginBottom: 8 }}>
            <button className="iconbtn" title="Renommer" aria-label="Renommer" disabled={readOnly && !canEdit} onClick={bulkRename}><Pencil size={16} /></button>
            <button className="iconbtn" title="Déplacer dans un dossier" aria-label="Déplacer" disabled={!canEdit} onClick={moveFolder}><FolderInput size={16} /></button>
            <button className="iconbtn" title="Dupliquer" aria-label="Dupliquer" disabled={!canEdit} onClick={() => act('actor_duplicate', { ids: sel }, 'Dupliqué')}><Copy size={16} /></button>
            <button className="iconbtn" title="Masquer / afficher en éditeur" aria-label="Masquer" disabled={!canEdit} onClick={() => update(sel, { hidden: !selected.every(a => a.hidden) })}>{selected.every(a => a.hidden) ? <Eye size={16} /> : <EyeOff size={16} />}</button>
            {canDelete && <button className="iconbtn" title="Supprimer" aria-label="Supprimer" onClick={del}><Trash2 size={16} /></button>}
            <button className="iconbtn" title="Désélectionner" aria-label="Désélectionner" onClick={() => setSel([])}><X size={16} /></button>
          </div>}
          <div className="otree"><NodeView n={root} depth={0} /></div>
          <p className="small muted">Ctrl/Cmd+clic : ajouter · Maj+clic : plage · Ctrl+Z : annuler</p>
        </aside>

        <section className="card oright">
          <Tabs value={tab} onChange={setTab} items={[['details', 'Détails'], ['top', 'Vue du dessus'], ['journal', 'Changements et journal']]} />
          <div style={{ marginTop: 16 }}>
            {tab === 'details' && (selected.length === 0 ? <p className="muted">Sélectionnez un ou plusieurs acteurs dans l’arbre.</p> : selected.length === 1 ? <Details a={selected[0]} all={actors} tags={st.tags} update={update} canEdit={canEdit} me={me?.user.email ?? ''} connected={st.status === 'connected'} /> : <Bulk list={selected} tags={st.tags} update={update} canEdit={canEdit} connected={st.status === 'connected'} />)}
            {tab === 'top' && <TopView actors={vis} sel={sel} setSel={setSel} update={update} canEdit={canEdit} hasImage={st.has_topview} bounds={st.topview_bounds} refresh={() => act('render_topview', {}, 'Vue du dessus rendue par UEFN')} connected={st.status === 'connected'} />}
            {tab === 'journal' && <Journal st={st} task={task} setTask={setTask} tasks={tasks ?? []} act={act} reload={reload} />}
          </div>
        </section>
      </div>
    </main>
  )
}

function Field({ s, value, onSet, disabled }: { s: NonNullable<Actor['schema']>[number] | { key: string; label: string; type: string; values: string[]; default: any; help: string; disabled?: string }; value: any; onSet: (v: any) => void; disabled: boolean }) {
  const off = disabled || !!s.disabled
  const input = s.type === 'enum' ? <select className="field" value={value ?? ''} disabled={off} onChange={e => onSet(e.target.value)}>{value === undefined && <option value="">(valeurs différentes)</option>}{s.values.map(v => <option key={v}>{v}</option>)}</select>
    : s.type === 'bool' ? <select className="field" value={value === undefined ? '' : value ? '1' : '0'} disabled={off} onChange={e => onSet(e.target.value === '1')}>{value === undefined && <option value="">(valeurs différentes)</option>}<option value="1">Oui</option><option value="0">Non</option></select>
    : <input className="field" type={s.type === 'number' ? 'number' : 'text'} key={String(value)} defaultValue={value ?? ''} placeholder={value === undefined ? '(valeurs différentes)' : ''} disabled={off} onBlur={e => { const v = s.type === 'number' ? Number(e.target.value) : e.target.value; if (e.target.value !== '' && v !== value) onSet(v) }} />
  return <div className={'ofield' + (s.disabled ? ' disabled' : '')}>
    <label className="label"><span>{s.label}</span>{input}</label>
    <div className="small muted">{s.disabled ?? s.help}{s.default !== null && s.default !== undefined && s.default !== '' && <> · par défaut : {typeof s.default === 'boolean' ? (s.default ? 'Oui' : 'Non') : String(s.default)}</>}</div>
  </div>
}

function Details({ a, all, tags, update, canEdit, me, connected }: { a: Actor; all: Actor[]; tags: string[]; update: (ids: string[], ch: any) => any; canEdit: boolean; me: string; connected: boolean }) {
  const { map } = useApp()
  // Options réelles lues dans UEFN à la sélection.
  const [live, setLive] = useState<{ values: Record<string, any>; schema: any[] } | null>(null)
  const [liveErr, setLiveErr] = useState('')
  const [tick, setTick] = useState(0)
  useEffect(() => {
    setLive(null); setLiveErr('')
    if (!connected || !(a.type === 'device' || a.type === 'verse_device')) return
    let off = false
    api.get(`/api/maps/${map!.id}/uefn/props?id=${encodeURIComponent(a.id)}`).then(r => { if (!off) setLive(r) }).catch(e => { if (!off) setLiveErr(e.message) })
    return () => { off = true }
  }, [a.id, connected, tick])
  const setOpt = async (key: string, v: any) => { await update([a.id], { [isVerse ? 'props' : 'options']: { [key]: v } }); setTick(t => t + 1) }
  const schema = live?.schema ?? a.schema ?? []
  const values = live?.values ?? (a.type === 'verse_device' ? a.props : a.options)
  const locked = a.locked_by && a.locked_by.toLowerCase() !== me.toLowerCase()
  const dis = !canEdit || !!locked
  const [ev, setEv] = useState({ event: '', target: '', function: '' })
  const isVerse = a.type === 'verse_device'
  const vec = (k: 'location' | 'rotation' | 'scale', label: string, axes: string[]) => (
    <div className="ovec"><span className="small muted">{label}</span>{a.transform[k].map((v, i) => (
      <label key={i} className="row small" style={{ gap: 4 }}>{axes[i]}<input className="field mono" type="number" step="any" key={v} defaultValue={Math.round(v * 100) / 100} disabled={dis} onBlur={e => { const n = Number(e.target.value); if (n !== v) { const t = [...a.transform[k]]; t[i] = n; update([a.id], { transform: { [k]: t } }) } }} aria-label={`${label} ${axes[i]}`} /></label>
    ))}</div>
  )
  return <div className="stack" style={{ gap: 18 }}>
    <div className="row" style={{ flexWrap: 'wrap' }}>
      <input className="field" style={{ flex: 1, fontSize: 18 }} key={a.name} defaultValue={a.name} disabled={dis} onBlur={e => e.target.value !== a.name && update([a.id], { name: e.target.value })} aria-label="Nom" />
      <span className="badge">{a.kind_label ?? TYPE_LABEL[a.type]}</span><span className="small muted mono">{a.class}</span>
    </div>
    {locked && <div className="warn"><Lock size={16} /><span>Verrouillé dans Lore par {a.locked_by} : lecture seule.</span></div>}
    <div className="row small muted" style={{ flexWrap: 'wrap' }}><span>Dossier : <b style={{ fontWeight: 500, color: 'var(--text)' }}>{a.folder || '—'}</b></span><span>Visible en éditeur : {a.hidden ? 'non' : 'oui'}</span></div>
    <section><div className="dsec" style={{ marginTop: 0 }}>Transformation</div>{vec('location', 'Position', ['X', 'Y', 'Z'])}{vec('rotation', 'Rotation', ['Tangage', 'Lacet', 'Roulis'])}{vec('scale', 'Échelle', ['X', 'Y', 'Z'])}</section>
    <section><div className="dsec" style={{ marginTop: 0 }}>Tags Verse</div>
      <div className="row" style={{ flexWrap: 'wrap', gap: 6 }}>
        {a.tags.map(t => <span key={t} className="chip" style={{ height: 30 }}>{t}{!dis && <button className="iconbtn" style={{ width: 22, height: 22 }} aria-label={`Retirer ${t}`} onClick={() => update([a.id], { tags_remove: [t] })}><X size={12} /></button>}</span>)}
        {!dis && <select className="field" style={{ height: 32 }} value="" onChange={e => e.target.value && update([a.id], { tags_add: [e.target.value] })} aria-label="Ajouter un tag"><option value="">+ tag du projet</option>{tags.filter(t => !a.tags.includes(t)).map(t => <option key={t}>{t}</option>)}</select>}
      </div>
    </section>
    {(a.type === 'device' || isVerse) && <section><div className="dsec" style={{ marginTop: 0 }}>{isVerse ? 'Propriétés Verse (@editable)' : 'Options de l’appareil'}{live && <span className="small muted"> · lues dans UEFN</span>}</div>
      {schema.length > 0 ? <div className="ofields">{schema.map((s: any) => <Field key={s.key} s={s} value={values?.[s.key]} disabled={dis} onSet={v => setOpt(s.key, v)} />)}</div>
        : <p className="small muted">{liveErr || (connected ? 'Lecture des options dans UEFN…' : 'Options disponibles quand UEFN est connecté.')}</p>}
    </section>}
    {(a.type === 'device' || isVerse) && a.events.length > 0 && <section><div className="dsec" style={{ marginTop: 0 }}>Liens d’événements (qui déclenche quoi)</div>
      <div className="list">{a.events.map((e, i) => (
        <div key={i} className="li"><Link2 size={15} /><span style={{ flex: 1 }}><span className="mono small">{e.event}</span> → <b style={{ fontWeight: 500 }}>{all.find(x => x.id === e.target)?.name ?? e.target}</b>.<span className="mono small">{e.function}</span></span>
          {!dis && <button className="iconbtn" aria-label="Retirer le lien" onClick={() => update([a.id], { events: a.events.filter((_, j) => j !== i) })}><X size={14} /></button>}</div>
      ))}{a.events.length === 0 && <p className="small muted">Aucun lien.</p>}</div>
      {!dis && <form className="row" style={{ flexWrap: 'wrap', marginTop: 8 }} onSubmit={e => { e.preventDefault(); if (ev.event && ev.target && ev.function) { update([a.id], { events: [...a.events, ev] }); setEv({ event: '', target: '', function: '' }) } }}>
        <input className="field mono" placeholder="Événement (ex. InteractedWithEvent)" value={ev.event} onChange={e => setEv({ ...ev, event: e.target.value })} />
        <select className="field" value={ev.target} onChange={e => setEv({ ...ev, target: e.target.value })}><option value="">Appareil cible…</option>{all.filter(x => x.type === 'device' || x.type === 'verse_device').map(x => <option key={x.id} value={x.id}>{x.name}</option>)}</select>
        <input className="field mono" placeholder="Fonction (ex. Start)" value={ev.function} onChange={e => setEv({ ...ev, function: e.target.value })} />
        <button className="btn">Ajouter</button>
      </form>}
    </section>}
  </div>
}

function Bulk({ list: base, tags, update, canEdit, connected }: { list: Actor[]; tags: string[]; update: (ids: string[], ch: any) => any; canEdit: boolean; connected: boolean }) {
  const { map } = useApp()
  const ids = base.map(a => a.id)
  // Options réelles de chaque acteur sélectionné (20 au plus), lues dans UEFN.
  const [live, setLive] = useState<Record<string, any>>({})
  const [tick, setTick] = useState(0)
  useEffect(() => {
    if (!connected) return
    let off = false
    const want = base.filter(a => a.type === 'device' || a.type === 'verse_device').slice(0, 20)
    Promise.all(want.map(a => api.get(`/api/maps/${map!.id}/uefn/props?id=${encodeURIComponent(a.id)}`).then(r => [a.id, r] as const).catch(() => null)))
      .then(rs => { if (!off) setLive(Object.fromEntries(rs.filter(Boolean) as any)) })
    return () => { off = true }
  }, [ids.join('|'), connected, tick])
  const list: Actor[] = base.map((a): Actor => live[a.id] ? { ...a, schema: live[a.id].schema, ...(a.type === 'verse_device' ? { props: live[a.id].values } : { options: live[a.id].values }) } : a)
  const upd = async (i: string[], ch: any) => { await update(i, ch); setTick(t => t + 1) }
  const locked = list.filter(a => a.locked_by)
  // Seules les options présentes chez tous les acteurs sélectionnés.
  const common = (list[0].schema ?? []).filter(s => list.every(a => a.schema?.some(x => x.key === s.key && x.type === s.type)))
  const val = (k: string) => { const vs = list.map(a => (a.type === 'verse_device' ? a.props[k] : a.options[k])); return vs.every(v => JSON.stringify(v) === JSON.stringify(vs[0])) ? vs[0] : undefined }
  const allTags = [...new Set(list.flatMap(a => a.tags))]
  return <div className="stack" style={{ gap: 18 }}>
    <div><b style={{ fontWeight: 500, fontSize: 18 }}>{list.length} acteurs sélectionnés</b><div className="small muted">{list.map(a => a.name).join(', ').slice(0, 220)}</div></div>
    {locked.length > 0 && <div className="warn"><Lock size={16} /><span>{locked.length} acteur(s) verrouillé(s) dans Lore : la modification sera refusée s’ils sont pris par quelqu’un d’autre.</span></div>}
    <section><div className="dsec" style={{ marginTop: 0 }}>Tags Verse</div>
      <div className="row" style={{ flexWrap: 'wrap', gap: 6 }}>
        {allTags.map(t => <span key={t} className="chip" style={{ height: 30 }}>{t}{canEdit && <button className="iconbtn" style={{ width: 22, height: 22 }} aria-label={`Retirer ${t} de tous`} onClick={() => update(ids, { tags_remove: [t] })}><X size={12} /></button>}</span>)}
        {canEdit && <select className="field" style={{ height: 32 }} value="" onChange={e => e.target.value && update(ids, { tags_add: [e.target.value] })}><option value="">+ tag pour tous</option>{tags.map(t => <option key={t}>{t}</option>)}</select>}
      </div>
    </section>
    <section><div className="dsec" style={{ marginTop: 0 }}>Options communes · la nouvelle valeur s’applique à tous</div>
      {common.length === 0 ? <p className="small muted">Ces acteurs n’ont aucune option en commun.</p> :
        <div className="ofields">{common.map(s => <Field key={s.key} s={{ ...s, disabled: undefined }} value={val(s.key)} disabled={!canEdit} onSet={v => upd(ids, { [list[0].type === 'verse_device' ? 'props' : 'options']: { [s.key]: v } })} />)}</div>}
    </section>
  </div>
}

function TopView({ actors, sel, setSel, update, canEdit, hasImage, bounds, refresh, connected }: { actors: Actor[]; sel: string[]; setSel: (s: string[]) => void; update: (ids: string[], ch: any) => any; canEdit: boolean; hasImage: boolean; bounds: any; refresh: () => void; connected: boolean }) {
  const { map, send, toast } = useApp()
  const { data: img, reload } = useLoad<any>(map && hasImage ? `/api/maps/${map.id}/uefn/topview` : null, [map?.id, hasImage])
  const { data: boards } = useLoad<any[]>(map ? `/api/maps/${map.id}/boards` : null, [map?.id])
  const [view, setView] = useState({ x: 0, y: 0, z: 1 })
  const [drag, setDrag] = useState<{ id: string; x: number; y: number } | null>(null)
  const box = useRef<HTMLDivElement>(null)
  const pan = useRef<any>(null)
  useLive(['uefn'], reload)
  const b = img?.bounds ?? bounds ?? { x0: -10000, y0: -10000, x1: 10000, y1: 10000 }
  const W = 900, H = Math.round(W * (b.y1 - b.y0) / (b.x1 - b.x0))
  const toPx = (x: number, y: number) => [((x - b.x0) / (b.x1 - b.x0)) * W, ((y - b.y0) / (b.y1 - b.y0)) * H]
  const toWorld = (px: number, py: number) => [b.x0 + (px / W) * (b.x1 - b.x0), b.y0 + (py / H) * (b.y1 - b.y0)]
  const local = (e: React.PointerEvent) => { const r = box.current!.getBoundingClientRect(); return [(e.clientX - r.left - view.x) / view.z, (e.clientY - r.top - view.y) / view.z] }
  const toBoard = () => {
    if (!img?.image || !boards?.[0]) return
    send({ t: 'board', board: boards[0].id, op: 'upsert', objs: [{ id: 'plan-' + Date.now(), kind: 'image', x: -100, y: -100, w: W, h: H, url: img.image, locked: true }] })
    toast(`Vue du dessus posée en fond du tableau « ${boards[0].title} »`)
  }
  return <div className="stack">
    <div className="row" style={{ flexWrap: 'wrap' }}>
      <button className="btn" disabled={!connected} onClick={refresh}><RefreshCw size={16} />Rafraîchir</button>
      <button className="btn" disabled={!img?.image} onClick={toBoard}>Utiliser comme fond du tableau blanc</button>
      <span className="small muted">{actors.length} acteur(s) du filtre · glisser un point le déplace (x, y)</span>
      <span className="spacer" /><button className="iconbtn" aria-label="Zoomer" onClick={() => setView(v => ({ ...v, z: Math.min(5, v.z * 1.25) }))}><Plus size={16} /></button><button className="iconbtn" aria-label="Dézoomer" onClick={() => setView(v => ({ ...v, z: Math.max(0.3, v.z / 1.25) }))}>−</button>
    </div>
    <div ref={box} className="otop" onWheel={e => { const z = Math.min(5, Math.max(0.3, view.z * (e.deltaY < 0 ? 1.1 : 0.9))); setView(v => ({ ...v, z })) }}
      onPointerDown={e => { if ((e.target as HTMLElement).dataset.aid) return; pan.current = { sx: e.clientX, sy: e.clientY, vx: view.x, vy: view.y } }}
      onPointerMove={e => { if (pan.current) setView(v => ({ ...v, x: pan.current.vx + e.clientX - pan.current.sx, y: pan.current.vy + e.clientY - pan.current.sy })); if (drag) { const [x, y] = local(e); setDrag({ ...drag, x, y }) } }}
      onPointerUp={() => {
        pan.current = null
        if (drag) { const a = actors.find(x => x.id === drag.id)!; const [wx, wy] = toWorld(drag.x, drag.y); update([a.id], { transform: { location: [Math.round(wx), Math.round(wy), a.transform.location[2]] } }); setDrag(null) }
      }}>
      <div style={{ position: 'absolute', left: 0, top: 0, width: W, height: H, transform: `translate(${view.x}px,${view.y}px) scale(${view.z})`, transformOrigin: '0 0' }}>
        {img?.image ? <img src={img.image} alt="Vue du dessus de la map" width={W} height={H} draggable={false} style={{ display: 'block' }} /> : <div className="empty" style={{ width: W, height: H, display: 'grid', placeItems: 'center', border: '1px dashed var(--border)' }}>Pas encore d’image : « Rafraîchir » quand UEFN est ouvert.</div>}
        {actors.map(a => {
          const [px, py] = drag?.id === a.id ? [drag.x, drag.y] : toPx(a.transform.location[0], a.transform.location[1])
          return <button key={a.id} data-aid={a.id} className={'opoint' + (sel.includes(a.id) ? ' on' : '')} style={{ left: px, top: py, transform: `translate(-50%,-50%) scale(${1 / view.z})` }} title={a.name}
            onPointerDown={e => { e.stopPropagation(); setSel([a.id]); if (canEdit && !a.locked_by) { (e.target as HTMLElement).setPointerCapture?.(e.pointerId); setDrag({ id: a.id, x: px, y: py }) } }} aria-label={a.name} />
        })}
      </div>
    </div>
  </div>
}

function Journal({ st, task, setTask, tasks, act, reload }: { st: any; task: number | ''; setTask: (t: number | '') => void; tasks: any[]; act: (a: string, args: any, ok?: string) => Promise<any>; reload: () => void }) {
  const { map, canEdit, toast } = useApp()
  const { data, reload: reloadJ } = useLoad<any[]>(map ? `/api/maps/${map.id}/uefn/journal` : null, [map?.id])
  useLive(['uefn'], reloadJ)
  const [msg, setMsg] = useState('')
  const S: Record<string, [string, string]> = { done: ['Fait', 'b-g'], queued: ['En attente', 'b-o'], sent: ['Envoyé', ''], failed: ['Échec', 'b-r'], undone: ['Annulé', ''] }
  return <div className="stack">
    <div className="tiles" style={{ gridTemplateColumns: 'repeat(2, minmax(0, 1fr))' }}>
      <div className="card"><div className="lab">Acteurs modifiés non enregistrés</div><div className="num">{st.unsaved_actors}</div></div>
      <div className="card"><div className="lab">Changements en attente d’UEFN</div><div className="num">{st.pending}</div></div>
    </div>
    <div className="row" style={{ flexWrap: 'wrap' }}>
      <select className="field" value={task} onChange={e => setTask(e.target.value ? Number(e.target.value) : '')} aria-label="Tâche"><option value="">Sans tâche</option>{tasks.filter(t => t.status !== 'done').map(t => <option key={t.id} value={t.number}>#{t.number} {t.title}</option>)}</select>
      <button className="btn" disabled={!canEdit} onClick={() => act('save_level', {}, 'Niveau enregistré')}><Save size={16} />Enregistrer le niveau</button>
      <input className="field" placeholder="Message Lore" value={msg} onChange={e => setMsg(e.target.value)} />
      <button className="btn pri" disabled={!canEdit} onClick={() => act('lore_checkin', { message: `${task ? `#${task} ` : ''}${msg || 'Organisation du niveau'}` }, 'Archivé dans Lore')}><Archive size={16} />Archiver dans Lore</button>
    </div>
    <p className="small muted">La publication de l’île pour les joueurs (nouvelle version, certification) se fait dans le portail créateur d’Epic. POP prépare la checklist et les notes de version (Lancer et suivre).</p>
    <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Quand</th><th>Qui</th><th>Changement (avant → après)</th><th>Tâche</th><th>État</th><th /></tr></thead><tbody>
      {data?.map(r => <tr key={r.id}>
        <td className="small muted" style={{ whiteSpace: 'nowrap' }}>{ago(r.at)}</td><td className="small">{r.who}{r.source === 'mcp' ? ' (IA)' : r.source === 'undo' ? ' (annulation)' : ''}</td>
        <td className="small">{r.summary}{r.error && <div className="err small">{r.error}</div>}</td><td className="mono small">{r.task ? `#${r.task}` : ''}</td>
        <td><span className={'badge ' + (S[r.status]?.[1] ?? '')}>{S[r.status]?.[0] ?? r.status}</span></td>
        <td>{r.undoable && canEdit && <button className="iconbtn" aria-label="Annuler" title="Annuler" onClick={async () => { try { await api.post(`/api/uefn-actions/${r.id}/undo`); reload(); reloadJ() } catch (e: any) { toast(e.message, true) } }}><RotateCcw size={15} /></button>}</td>
      </tr>)}
    </tbody></table></div>
  </div>
}

function Pairing({ status, tools }: { status: string; tools: any }) {
  const { map, canEdit, toast } = useApp()
  const { data: devices, reload } = useLoad<any[]>(map ? `/api/maps/${map.id}/relay` : null, [map?.id])
  const [code, setCode] = useState<string | null>(null)
  const local = ['localhost', '127.0.0.1', '::1'].includes(location.hostname)
  const roles = tools?.roles ?? {}
  return <section className="card">
    <CardHead title="Relier UEFN avec UEFN Link" sub="Le relais tourne sur le PC Windows où UEFN est ouvert. Il ne touche qu’au projet choisi et pilote UEFN par son serveur MCP officiel." />
    <ol style={{ margin: 0, paddingLeft: 20, lineHeight: 1.9 }}>
      <li>Dans UEFN, projet ouvert : <b style={{ fontWeight: 500 }}>Projet → Paramètres du projet → Enable Project Python Scripts</b>, puis <b style={{ fontWeight: 500 }}>Édition → Préférences de l’éditeur → Agentic Workflows → Model Context Protocol → Start Server</b> (port 8000).</li>
      <li>Télécharger le relais sur ce PC : <a className="btn" style={{ height: 34 }} href="/downloads/pop-link.exe" download>pop-link.exe (Windows)</a> <span className="small muted">Windows peut afficher « Windows a protégé votre ordinateur » : Informations complémentaires → Exécuter quand même.</span></li>
      <li>Générer un code : {canEdit && <button className="btn" style={{ height: 34 }} onClick={async () => { try { setCode((await api.post(`/api/maps/${map!.id}/relay/pair`)).code) } catch (e: any) { toast(e.message, true) } }}>Obtenir un code</button>}
        {code && <span> → <b className="mono" style={{ fontSize: 18, letterSpacing: 2 }}>{code}</b> <span className="small muted">(valable 10 minutes)</span></span>}</li>
      <li>Double-cliquer sur pop-link.exe et répondre : adresse de POP <b className="mono" style={{ fontWeight: 500 }}>{local ? 'http://<adresse-réseau-du-Mac>:8080' : location.origin}</b>, le code, puis le numéro du projet UEFN. Laisser la fenêtre ouverte.
        {local && <div className="small muted">Vous êtes sur l’adresse locale du Mac : le PC ne peut pas l’utiliser. Lancez POP avec <span className="mono">./start.sh --reseau</span>, qui affiche l’adresse à taper sur le PC (même réseau Wi-Fi), ou mettez POP en ligne.</div>}</li>
      <li>En cas de souci : <span className="mono">pop-link.exe diagnostic</span> dans un terminal liste ce qu’UEFN expose.</li>
    </ol>
    <div className="row small" style={{ flexWrap: 'wrap', marginTop: 12 }}>
      <span>État : <b style={{ fontWeight: 500 }}>{STATUS_LABEL[status]}</b></span>
      {tools?.url && <span className="muted">MCP : {tools.url}</span>}
      {tools?.list && <span className="muted">{tools.list.length} outil(s) UEFN · Python : {roles.python ?? '?'} · compilation : {roles.compile ?? '?'} · session : {roles.push ?? '?'}</span>}
    </div>
    {tools?.list?.length > 0 && <details style={{ marginTop: 8 }}><summary className="small" style={{ cursor: 'pointer' }}>Voir les outils détectés</summary><div className="row small" style={{ flexWrap: 'wrap', gap: 6, marginTop: 8 }}>{tools.list.map((t: any) => <span key={t.name} className="chip mono" style={{ height: 26 }} title={t.description}>{t.name}</span>)}</div></details>}
    {devices && devices.length > 0 && <div className="list" style={{ marginTop: 12 }}>{devices.map(d => <div key={d.id} className="li"><span style={{ flex: 1 }}>{d.name} <span className="small muted">({d.owner}{d.last_seen ? ', vu ' + ago(d.last_seen) : ''})</span></span>{canEdit && <button className="iconbtn" aria-label="Retirer ce relais" onClick={async () => { if (confirm('Retirer ce relais ? Il faudra l’appairer à nouveau.')) { await api.del(`/api/relay-devices/${d.id}`); reload() } }}><Trash2 size={15} /></button>}</div>)}</div>}
  </section>
}
