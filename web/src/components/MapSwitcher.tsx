// Sélecteur de map dans la barre latérale : changer de map, créer une map ou un studio sans quitter la page.
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Check, ChevronsUpDown, Plus, Settings2, Building2 } from 'lucide-react'
import { api } from '../api'
import { useApp } from '../state'
import { Modal } from './ui'
import { tr } from '../i18n/t'

type Create = { kind: 'map' | 'studio'; studioId?: number }

/** Ouvre la fenêtre de création depuis n'importe où (menu du compte, page Équipe…). */
export const openCreate = (c: Create) => window.dispatchEvent(new CustomEvent('pop:create', { detail: c }))

export function MapSwitcher() {
  const app = useApp()
  const nav = useNavigate()
  const [open, setOpen] = useState(false)
  const btn = useRef<HTMLButtonElement>(null)
  const pop = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState({ top: 0, left: 0, width: 280 })
  useLayoutEffect(() => {
    if (!open || !btn.current) return
    const r = btn.current.getBoundingClientRect()
    setPos({ top: r.bottom + 6, left: r.left, width: Math.max(280, r.width) })
  }, [open])
  useEffect(() => {
    if (!open) return
    const down = (e: MouseEvent) => { if (!pop.current?.contains(e.target as Node) && !btn.current?.contains(e.target as Node)) setOpen(false) }
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false) }
    document.addEventListener('mousedown', down); document.addEventListener('keydown', key)
    return () => { document.removeEventListener('mousedown', down); document.removeEventListener('keydown', key) }
  }, [open])
  const studios = app.me?.studios ?? []
  const go = (fn: () => void) => { setOpen(false); fn() }
  return <div className="mapsw lbl">
    <span className="navsec" style={{ padding: '0 4px 2px' }}>{tr('Map')}</span>
    <button ref={btn} className="mapsw-btn" onClick={() => setOpen(o => !o)} aria-haspopup="menu" aria-expanded={open}>
      <span className="mapsw-txt"><b>{app.map?.name ?? '—'}</b><small>{app.studio?.name}</small></span>
      <ChevronsUpDown size={16} strokeWidth={1.75} />
    </button>
    {open && <div ref={pop} className="mapsw-pop" role="menu" style={{ top: pos.top, left: pos.left, width: pos.width }}>
      {studios.map(s => <div key={s.id} className="mapsw-group">
        <div className="mapsw-studio"><Building2 size={14} />{s.name}</div>
        {s.maps.map(m => <button key={m.id} role="menuitem" className={'mapsw-item' + (m.id === app.map?.id ? ' on' : '')} onClick={() => go(() => app.setMapId(m.id))}>
          <span>{m.name}</span>{m.id === app.map?.id && <Check size={15} />}
        </button>)}
        {(s.role === 'owner' || s.role === 'admin') && <button role="menuitem" className="mapsw-item add" onClick={() => go(() => openCreate({ kind: 'map', studioId: s.id }))}><Plus size={15} />{tr('Nouvelle map dans ce studio')}</button>}
      </div>)}
      <div className="mapsw-foot">
        <button role="menuitem" className="mapsw-item add" onClick={() => go(() => openCreate({ kind: 'studio' }))}><Plus size={15} />{tr('Nouveau studio')}</button>
        <button role="menuitem" className="mapsw-item" onClick={() => go(() => nav('/equipe?tab=maps'))}><Settings2 size={15} />{tr('Gérer les maps')}</button>
      </div>
    </div>}
  </div>
}

/** Fenêtre de création (map ou studio), affichée par le Layout. */
export function CreateHost() {
  const [c, setC] = useState<Create | null>(null)
  useEffect(() => {
    const h = (e: Event) => setC((e as CustomEvent).detail)
    window.addEventListener('pop:create', h); return () => window.removeEventListener('pop:create', h)
  }, [])
  if (!c) return null
  return <CreateModal c={c} onClose={() => setC(null)} />
}

function CreateModal({ c, onClose }: { c: Create; onClose: () => void }) {
  const { me, studio, reload, setMapId, toast } = useApp()
  const admins = (me?.studios ?? []).filter(s => s.role === 'owner' || s.role === 'admin')
  const [kind, setKind] = useState(c.kind === 'map' && admins.length === 0 ? 'studio' : c.kind)
  const [sid, setSid] = useState<number>(c.studioId ?? (admins.find(s => s.id === studio?.id)?.id ?? admins[0]?.id ?? 0))
  const [f, setF] = useState({ name: '', genre: '', release_date: '', studio: '' })
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); if (busy) return
    setBusy(true); setErr('')
    try {
      const r = kind === 'map'
        ? await api.post(`/api/studios/${sid}/maps`, { name: f.name, genre: f.genre, release_date: f.release_date || null })
        : await api.post('/api/studios', { name: f.studio, map_name: f.name, genre: f.genre, release_date: f.release_date || null })
      await reload(); setMapId(r.map_id)
      toast(kind === 'map' ? tr('Map « {0} » créée', [f.name]) : tr('Studio « {0} » créé', [f.studio]))
      onClose()
    } catch (er: any) { setErr(er.message); setBusy(false) }
  }
  return <Modal onClose={onClose} label={kind === 'map' ? tr('Nouvelle map') : tr('Nouveau studio')}>
    <form className="stack" onSubmit={submit}>
      <h2 style={{ margin: 0, fontWeight: 400 }}>{kind === 'map' ? tr('Nouvelle map') : tr('Nouveau studio')}</h2>
      {admins.length > 0 && <div className="tabs" role="tablist" style={{ alignSelf: 'flex-start' }}>
        <button type="button" role="tab" aria-selected={kind === 'map'} className={kind === 'map' ? 'on' : ''} onClick={() => setKind('map')}>{tr('Une map')}</button>
        <button type="button" role="tab" aria-selected={kind === 'studio'} className={kind === 'studio' ? 'on' : ''} onClick={() => setKind('studio')}>{tr('Un studio')}</button>
      </div>}
      <p className="muted" style={{ margin: 0 }}>{kind === 'map'
        ? tr('Une map = un projet UEFN, avec ses tâches, ses pages, son planning et ses gains. L’équipe du studio y a accès.')
        : tr('Un studio = une équipe. Créez-en un autre seulement si vous travaillez avec d’autres personnes sur d’autres maps.')}</p>
      {kind === 'map' && admins.length > 1 && <label className="label">{tr('Studio')}<select className="field" value={sid} onChange={e => setSid(Number(e.target.value))}>{admins.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>}
      {kind === 'studio' && <label className="label">{tr('Nom du studio')}<input className="field" autoFocus value={f.studio} onChange={e => setF({ ...f, studio: e.target.value })} required maxLength={60} /></label>}
      <label className="label">{kind === 'map' ? tr('Nom de la map') : tr('Première map')}<input className="field" autoFocus={kind === 'map'} value={f.name} onChange={e => setF({ ...f, name: e.target.value })} required maxLength={60} placeholder={tr('Nom de la map (ex. VaultRush)')} /></label>
      <div className="grid2" style={{ gridTemplateColumns: '1fr 1fr' }}>
        <label className="label">{tr('Genre')}<input className="field" value={f.genre} onChange={e => setF({ ...f, genre: e.target.value })} placeholder={tr('Tycoon, box fight…')} /></label>
        <label className="label">{tr('Sortie visée')}<input className="field" type="date" value={f.release_date} onChange={e => setF({ ...f, release_date: e.target.value })} /></label>
      </div>
      {err && <div className="err">{err}</div>}
      <div className="row" style={{ justifyContent: 'flex-end' }}>
        <button type="button" className="btn" onClick={onClose}>{tr('Annuler')}</button>
        <button className="btn pri" disabled={busy}>{busy ? tr('Création…') : kind === 'map' ? tr('Créer la map') : tr('Créer le studio')}</button>
      </div>
    </form>
  </Modal>
}
