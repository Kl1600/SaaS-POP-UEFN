// Sélecteur de map dans la barre latérale : changer de map, créer une map ou un studio sans quitter la page.
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import { Check, ChevronsUpDown, Building2, LayoutGrid, User, Users, Sparkles, MoreHorizontal, ArrowRight, Pencil, Trash2, Settings, LogIn } from '../icons'
import { api } from '../api'
import { useApp, useLoad, studioName } from '../state'
import { Modal, ROLE } from './ui'
import { tr, tx } from '../i18n/t'
import { ChoixEquipe, versEnvoi } from './Faisabilite'
import { ConfirmMove, isAdmin, previewMove, type Drag, type Pending } from './MoveMap'
import { cssRect } from '../zoom'
import { DeleteDialog, type DeleteTarget } from './DeleteDialog'

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
    const r = cssRect(btn.current)
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
  const personal = studios.find(s => s.personal)
  const teams = studios.filter(s => !s.personal && (s.role === 'owner' || s.role === 'admin'))
  const go = (fn: () => void) => { setOpen(false); fn() }
  // Glisser une map vers un autre groupe la déplace (perso ↔ studio), après confirmation.
  const [drag, setDrag] = useState<Drag | null>(null)
  const [over, setOver] = useState<number | null>(null)
  const [pending, setPending] = useState<Pending | null>(null)
  // Menu « ⋯ » d'une map ou d'un studio (#269), affiché sous sa ligne.
  const [menu, setMenu] = useState<string | null>(null)
  const [renaming, setRenaming] = useState<{ id: number; name: string } | null>(null)
  const [del, setDel] = useState<DeleteTarget | null>(null)
  useEffect(() => { if (!open) { setMenu(null); setRenaming(null) } }, [open])
  const rename = async () => {
    if (!renaming?.name.trim()) return
    try { await api.patch(`/api/maps/${renaming.id}`, { name: renaming.name.trim() }); await app.reload(); app.toast(tr("Map renommée")); setRenaming(null); setMenu(null) } catch (er: any) { app.toast(er.message, true) }
  }
  const moveTo = async (d: Drag, to: number) => { setOpen(false); try { setPending(await previewMove(d, to)) } catch (er: any) { app.toast(er.message, true) } }
  const drop = async (to: number) => {
    const d = drag; setDrag(null); setOver(null)
    if (!d || d.from === to) return
    setOpen(false)
    try { setPending(await previewMove(d, to)) } catch (er: any) { app.toast(er.message, true) }
  }
  return <div className="mapsw lbl">
    <span className="navsec" style={{ padding: '0 4px 2px' }}>{tr('Map')}</span>
    <button ref={btn} className="mapsw-btn" onClick={() => setOpen(o => !o)} aria-haspopup="menu" aria-expanded={open}>
      <span className="mapsw-txt"><b>{app.map?.name ?? '—'}</b><small>{studioName(app.studio)}</small></span>
      <ChevronsUpDown size={16} strokeWidth={1.75} />
    </button>
    {open && <div ref={pop} className="mapsw-pop" role="menu" style={{ top: pos.top, left: pos.left, width: Math.max(pos.width, 300) }}>
      {studios.map(s => { const canDrop = isAdmin(s) && !!drag && drag.from !== s.id; return <div key={s.id}
        className={'mapsw-group' + (canDrop ? ' drop-ok' : '') + (canDrop && over === s.id ? ' drop-over' : '')}
        onDragOver={e => { if (!canDrop) return; e.preventDefault(); e.dataTransfer.dropEffect = 'move'; if (over !== s.id) setOver(s.id) }}
        onDragLeave={e => { if (!e.currentTarget.contains(e.relatedTarget as Node | null) && over === s.id) setOver(null) }}
        onDrop={e => { if (!canDrop) return; e.preventDefault(); drop(s.id) }}>
        <div className="mapsw-studio">
          <span className="mapsw-gico">{s.personal ? <User size={14} /> : <Building2 size={14} />}</span>
          <span className="mapsw-gname">{s.personal ? tr('Mes maps (seul)') : s.name}</span>
          {!s.personal && <span className="mapsw-role">{ROLE[s.role] ?? s.role}</span>}
          <span className="mapsw-count">{s.maps.length}</span>
          {!s.personal && <button className="mapsw-more" aria-label={tr("Actions du studio {0}", [s.name])} aria-expanded={menu === 's' + s.id} onClick={() => setMenu(menu === 's' + s.id ? null : 's' + s.id)}><MoreHorizontal size={15} /></button>}
        </div>
        {menu === 's' + s.id && <div className="mapsw-menu" role="menu">
          <button role="menuitem" onClick={() => go(() => nav(`/studio/${s.id}/maps`))}><LogIn size={14} />{tr("Ouvrir le studio")}</button>
          {isAdmin(s) && <button role="menuitem" onClick={() => go(() => nav(`/studio/${s.id}/parametres`))}><Settings size={14} />{tr("Paramètres du studio")}</button>}
          {s.role === 'owner' && <button role="menuitem" className="danger" onClick={() => { setOpen(false); setDel({ kind: 'studio', id: s.id, name: s.name, studioId: s.id, maps: s.maps.length }) }}><Trash2 size={14} />{tr("Supprimer le studio")}</button>}
        </div>}
        {s.maps.map(m => { const cur = m.id === app.map?.id; const k = 'm' + m.id; const lastTeam = !s.personal && s.maps.length <= 1; return <div key={m.id} className={'mapsw-row' + (menu === k ? ' open' : '')} onContextMenu={e => { if (!isAdmin(s)) return; e.preventDefault(); setMenu(k) }}><button role="menuitem" aria-current={cur ? 'true' : undefined} className={'mapsw-item' + (cur ? ' on' : '') + (drag?.id === m.id ? ' dragging' : '')} onClick={() => go(() => openMap(m.id))}
          draggable={isAdmin(s)} title={isAdmin(s) && studios.length > 1 ? tr('Glissez la map vers un autre groupe pour la déplacer') : undefined}
          onDragStart={e => { e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', String(m.id)); setDrag({ id: m.id, name: m.name, from: s.id }) }}
          onDragEnd={() => { setDrag(null); setOver(null) }}>
          <span>{m.name}</span>{cur && <em className="mapsw-cur"><Check size={13} />{tr('Ouverte')}</em>}
        </button>
          {isAdmin(s) && <button className="mapsw-more" aria-label={tr("Actions de la map {0}", [m.name])} aria-expanded={menu === k} onClick={() => { setMenu(menu === k ? null : k); setRenaming(null) }}><MoreHorizontal size={15} /></button>}
          {menu === k && <div className="mapsw-menu" role="menu">
            {renaming?.id === m.id
              ? <form className="mapsw-rename" onSubmit={e => { e.preventDefault(); rename() }}><input className="field" autoFocus maxLength={80} value={renaming.name} onChange={e => setRenaming({ id: m.id, name: e.target.value })} aria-label={tr("Nouveau nom")} /><button className="btn sm pri">{tr("OK")}</button></form>
              : <button role="menuitem" onClick={() => setRenaming({ id: m.id, name: m.name })}><Pencil size={14} />{tr("Renommer")}</button>}
            {studios.filter(t => t.id !== s.id && isAdmin(t)).map(t => <button key={t.id} role="menuitem" onClick={() => moveTo({ id: m.id, name: m.name, from: s.id }, t.id)}><ArrowRight size={14} />{tr("Déplacer vers {0}", [t.personal ? tr('Mes maps (seul)') : t.name])}</button>)}
            {lastTeam
              ? <div className="mapsw-note">{tr("Dernière map du studio : supprimez plutôt le studio, ou créez une autre map d’abord.")}</div>
              : <button role="menuitem" className="danger" onClick={() => { setOpen(false); setDel({ kind: 'map', id: m.id, name: m.name, studioId: s.id }) }}><Trash2 size={14} />{tr("Supprimer la map")}</button>}
          </div>}
        </div> })}
        {s.maps.length === 0 && <div className="mapsw-empty">{s.personal ? tr('Les maps que vous faites seul.') : tr('Aucune map dans ce studio.')}</div>}
      </div> })}
      <div className="mapsw-foot">
        <div className="mapsw-studio"><span className="mapsw-gname">{tr('Créer')}</span></div>
        {personal && <button role="menuitem" className="mapsw-item add" onClick={() => go(() => openCreate({ kind: 'map', studioId: personal.id }))}><User size={15} /><span>{tr('Nouvelle map perso')}</span></button>}
        {teams.length > 0 && <button role="menuitem" className="mapsw-item add" onClick={() => go(() => openCreate({ kind: 'map', studioId: (teams.find(t => t.id === app.studio?.id) ?? teams[0]).id }))}><Building2 size={15} /><span>{tr('Nouvelle map dans un studio')}</span></button>}
        <button role="menuitem" className="mapsw-item add" onClick={() => go(() => openCreate({ kind: 'studio' }))}><Users size={15} /><span>{tr('Créer un studio (équipe)')}</span></button>
      </div>
      <div className="mapsw-foot">
        <button role="menuitem" className="mapsw-item" onClick={() => go(() => nav('/'))}><LayoutGrid size={15} /><span>{tr('Vue d’ensemble des maps')}</span></button>
        {app.map && <button role="menuitem" className="mapsw-item" onClick={() => go(() => nav(`/map/${app.map!.id}/planifier?vue=generateur`))}><Sparkles size={15} /><span>{tr('Générateur de projet')}</span></button>}
        {studios.some(isAdmin) && studios.length > 1 && <div className="mapsw-hint">{tr('Astuce : glissez une map vers un autre groupe pour la déplacer.')}</div>}
      </div>
    </div>}
    {pending && app.me && <ConfirmMove p={pending} studios={app.me.studios} onClose={() => setPending(null)} />}
    {del && <DeleteDialog t={del} onClose={() => setDel(null)} />}
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
  // Équipe de la map (#149) : membres du studio choisi, heures par semaine sur cette map, début, part de l'IA.
  const { data: membres } = useLoad<any>(kind === 'map' && sid ? `/api/studios/${sid}/members` : null, [sid, kind])
  const [equipe, setEquipe] = useState<Record<number, any>>({})
  const [debut, setDebut] = useState('')
  const [partIa, setPartIa] = useState(0)
  useEffect(() => {
    if (!membres || !me) return
    setEquipe({ [me.user.id]: { on: true, hours: Math.min(10, membres.members.find((m: any) => m.id === me.user.id)?.hours_per_week ?? 10), absences: [] } })
  }, [membres, me])
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); if (busy) return
    setBusy(true); setErr('')
    try {
      const r = kind === 'map'
        ? await api.post(`/api/studios/${sid}/maps`, { name: f.name, genre: f.genre, release_date: f.release_date || null, team: { start_date: debut, margin_pct: 20, ai_share_pct: partIa, staff: versEnvoi(equipe) } })
        : await api.post('/api/studios', f.name.trim() ? { name: f.studio, map_name: f.name, genre: f.genre, release_date: f.release_date || null } : { name: f.studio })
      await reload()
      if (r.map_id) { setMapId(r.map_id); nav(`/map/${r.map_id}`) } else nav(`/studio/${r.studio_id}/equipe`)
      toast(kind === 'map' ? tr('Map « {0} » créée', [f.name]) : tr('Studio « {0} » créé', [f.studio]))
      for (const o of r.avis?.overloads ?? []) toast(tx(o.text), true)
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
      {kind === 'map' && membres && <>
        <div className="grid2" style={{ gridTemplateColumns: '1fr 1fr' }}>
          <label className="label">{tr('Date de début')}<input className="field" type="date" value={debut} onChange={e => setDebut(e.target.value)} /></label>
          <label className="label">{tr("Part des tâches pour l'IA (%)")}<input className="field" type="number" min={0} max={100} value={partIa} onChange={e => setPartIa(Number(e.target.value))} /></label>
        </div>
        <div className="label">{tr('Qui travaille dessus, et combien d’heures par semaine ?')}</div>
        <ChoixEquipe compact members={membres.members.filter((m: any) => m.role !== 'viewer').map((m: any) => ({ user_id: m.id, name: m.name, studio_hours: m.hours_per_week }))} value={equipe} onChange={setEquipe} />
      </>}
      {err && <div className="err">{err}</div>}
      <div className="row" style={{ justifyContent: 'flex-end' }}>
        <button type="button" className="btn" onClick={onClose}>{tr('Annuler')}</button>
        <button className="btn pri" disabled={busy}>{busy ? tr('Création…') : kind === 'map' ? tr('Créer la map') : tr('Créer le studio')}</button>
      </div>
    </form>
  </Modal>
}
