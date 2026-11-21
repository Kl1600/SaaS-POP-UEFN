// Sélecteur de map dans la barre latérale : changer de map, créer une map ou un studio sans quitter la page.
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import { Check, ChevronsUpDown, Plus, Settings2, Building2, User, Users } from 'lucide-react'
import { api } from '../api'
import { useApp, studioName } from '../state'
import { Modal } from './ui'
import { tr } from '../i18n/t'

type Create = { kind: 'map' | 'studio'; studioId?: number }

/** Ouvre la fenêtre de création depuis n'importe où (menu du compte, page Équipe…). */
export const openCreate = (c: Create) => window.dispatchEvent(new CustomEvent('pop:create', { detail: c }))

export function MapSwitcher() {
  const app = useApp()
  const nav = useNavigate()
  const loc = useLocation()
  // Changer de map garde l'onglet en cours (Tâches → Tâches de l'autre map).
  const openMap = (id: number) => {
    const m = loc.pathname.match(/^\/map\/\d+(\/[a-z]+)?/)
    const sec = m?.[1] && !['/pages', '/tableau'].includes(m[1]) ? m[1] : m?.[1] ?? ''
    nav(`/map/${id}${m ? sec : ''}`)
  }
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
      <span className="mapsw-txt"><b>{app.map?.name ?? '—'}</b><small>{studioName(app.studio)}</small></span>
      <ChevronsUpDown size={16} strokeWidth={1.75} />
    </button>
    {open && <div ref={pop} className="mapsw-pop" role="menu" style={{ top: pos.top, left: pos.left, width: pos.width }}>
      {studios.map(s => <div key={s.id} className="mapsw-group">
        <div className="mapsw-studio">{s.personal ? <User size={14} /> : <Building2 size={14} />}{studioName(s)}</div>
        {s.maps.map(m => <button key={m.id} role="menuitem" className={'mapsw-item' + (m.id === app.map?.id ? ' on' : '')} onClick={() => go(() => openMap(m.id))}>
          <span>{m.name}</span>{m.id === app.map?.id && <Check size={15} />}
        </button>)}
        {s.personal && s.maps.length === 0 && <div className="mapsw-empty">{tr('Les maps que vous faites seul.')}</div>}
        {(s.role === 'owner' || s.role === 'admin') && <button role="menuitem" className="mapsw-item add" onClick={() => go(() => openCreate({ kind: 'map', studioId: s.id }))}><Plus size={15} />{s.personal ? tr('Nouvelle map perso') : tr('Nouvelle map dans ce studio')}</button>}
      </div>)}
      <div className="mapsw-foot">
        <button role="menuitem" className="mapsw-item add" onClick={() => go(() => openCreate({ kind: 'studio' }))}><Users size={15} />{tr('Créer un studio (équipe)')}</button>
        <button role="menuitem" className="mapsw-item" onClick={() => go(() => nav('/'))}><Settings2 size={15} />{tr('Toutes mes maps')}</button>
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
  const nav = useNavigate()
  const all = me?.studios ?? []
  const personal = all.find(s => s.personal)
  const teams = all.filter(s => !s.personal && (s.role === 'owner' || s.role === 'admin'))
  const [kind, setKind] = useState(c.kind)
  const [sid, setSid] = useState<number>(c.studioId ?? (studio && (studio.personal || teams.some(t => t.id === studio.id)) ? studio.id : personal?.id ?? teams[0]?.id ?? 0))
  const [f, setF] = useState({ name: '', genre: '', release_date: '', studio: '' })
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const solo = sid === personal?.id
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); if (busy) return
    setBusy(true); setErr('')
    try {
      const r = kind === 'map'
        ? await api.post(`/api/studios/${sid}/maps`, { name: f.name, genre: f.genre, release_date: f.release_date || null })
        : await api.post('/api/studios', f.name.trim() ? { name: f.studio, map_name: f.name, genre: f.genre, release_date: f.release_date || null } : { name: f.studio })
      await reload()
      if (r.map_id) { setMapId(r.map_id); nav(`/map/${r.map_id}`) } else nav(`/studio/${r.studio_id}/equipe`)
      toast(kind === 'map' ? tr('Map « {0} » créée', [f.name]) : tr('Studio « {0} » créé', [f.studio]))
      onClose()
    } catch (er: any) { setErr(er.message); setBusy(false) }
  }
  return <Modal onClose={onClose} label={kind === 'map' ? tr('Nouvelle map') : tr('Nouveau studio')}>
    <form className="stack" onSubmit={submit}>
      <h2 style={{ margin: 0, fontWeight: 400 }}>{kind === 'map' ? tr('Nouvelle map') : tr('Nouveau studio')}</h2>
      <div className="tabs" role="tablist" style={{ alignSelf: 'flex-start' }}>
        <button type="button" role="tab" aria-selected={kind === 'map'} className={kind === 'map' ? 'on' : ''} onClick={() => setKind('map')}>{tr('Une map')}</button>
        <button type="button" role="tab" aria-selected={kind === 'studio'} className={kind === 'studio' ? 'on' : ''} onClick={() => setKind('studio')}>{tr('Un studio')}</button>
      </div>
      {kind === 'map' ? <>
        <div className="mapsw-choice" role="radiogroup" aria-label={tr('Pour qui ?')}>
          {personal && <label className={'mapsw-opt' + (solo ? ' on' : '')}>
            <input type="radio" name="who" checked={solo} onChange={() => setSid(personal.id)} />
            <User size={20} /><span><b>{tr('Seul')}</b><small>{tr('Dans vos maps perso. Vous pourrez la passer en équipe plus tard.')}</small></span>
          </label>}
          <label className={'mapsw-opt' + (!solo ? ' on' : '') + (teams.length === 0 ? ' off' : '')}>
            <input type="radio" name="who" checked={!solo} disabled={teams.length === 0} onChange={() => teams[0] && setSid(teams.find(t => t.id === studio?.id)?.id ?? teams[0].id)} />
            <Users size={20} /><span><b>{tr('En équipe')}</b><small>{teams.length ? tr('Dans un studio : toute l’équipe y a accès.') : tr('Vous n’avez pas encore de studio : créez-en un avec l’onglet « Un studio ».')}</small></span>
          </label>
        </div>
        {!solo && teams.length > 1 && <label className="label">{tr('Studio')}<select className="field" value={sid} onChange={e => setSid(Number(e.target.value))}>{teams.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>}
      </> : <p className="muted" style={{ margin: 0 }}>{tr('Un studio = une équipe : vous invitez des membres, et toutes les maps du studio sont partagées avec eux.')}</p>}
      {kind === 'studio' && <label className="label">{tr('Nom du studio')}<input className="field" autoFocus value={f.studio} onChange={e => setF({ ...f, studio: e.target.value })} required maxLength={60} /></label>}
      <label className="label">{kind === 'map' ? tr('Nom de la map') : tr('Première map (facultatif)')}<input className="field" autoFocus={kind === 'map'} value={f.name} onChange={e => setF({ ...f, name: e.target.value })} required={kind === 'map'} maxLength={60} placeholder={kind === 'map' ? tr('Nom de la map (ex. VaultRush)') : tr('Laissez vide si vous n’avez pas encore de map')} /></label>
      {(kind === 'map' || f.name.trim() !== '') && <div className="grid2" style={{ gridTemplateColumns: '1fr 1fr' }}>
        <label className="label">{tr('Genre')}<input className="field" value={f.genre} onChange={e => setF({ ...f, genre: e.target.value })} placeholder={tr('Tycoon, box fight…')} /></label>
        <label className="label">{tr('Sortie visée')}<input className="field" type="date" value={f.release_date} onChange={e => setF({ ...f, release_date: e.target.value })} /></label>
      </div>}
      {err && <div className="err">{err}</div>}
      <div className="row" style={{ justifyContent: 'flex-end' }}>
        <button type="button" className="btn" onClick={onClose}>{tr('Annuler')}</button>
        <button className="btn pri" disabled={busy}>{busy ? tr('Création…') : kind === 'map' ? tr('Créer la map') : tr('Créer le studio')}</button>
      </div>
    </form>
  </Modal>
}
