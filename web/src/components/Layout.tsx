import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useEffect, useState } from 'react'
import {
  Home, FileText, PenTool, CalendarDays, KanbanSquare, Rocket, Coins, Users, Settings, LifeBuoy, PanelLeft,
  Search, Moon, Sun, Bell, Plus, ChevronDown, Hexagon, LogOut, FileCode2, ListTree, Bot,
} from 'lucide-react'
import { useApp } from '../state'
import { ago, api } from '../api'
import { Avatar } from './ui'
import { NewTaskModal } from '../pages/Tasks'
import { useUefnStatus, STATUS_LABEL } from '../uefnState'

const GROUPS: [string, [string, string, any][]][] = [
  ['Général', [['/', "Vue d'ensemble", Home]]],
  ['Espace', [['/pages', 'Pages', FileText], ['/tableau', 'Tableau blanc', PenTool]]],
  ['Production', [['/planifier', 'Planifier', CalendarDays], ['/taches', 'Tâches', KanbanSquare]]],
  ['UEFN', [['/verse', 'Éditeur Verse', FileCode2], ['/organiseur', 'Organiseur', ListTree], ['/ia', 'IA connectées', Bot]]],
  ['Publication', [['/lancer', 'Lancer et suivre', Rocket]]],
  ['Studio', [['/remunerer', 'Rémunérer', Coins], ['/equipe', 'Équipe et intégrations', Users]]],
]
const TITLES: Record<string, [string, string]> = {}
GROUPS.forEach(([g, items]) => items.forEach(([p, l]) => (TITLES[p] = [g, l])))

export default function Layout() {
  const app = useApp()
  const loc = useLocation()
  const nav = useNavigate()
  const [collapsed, setCollapsed] = useState(false)
  const [acct, setAcct] = useState(false)
  const [newTask, setNewTask] = useState(false)
  const key = '/' + (loc.pathname.split('/')[1] || '')
  const [group, title] = TITLES[key] ?? ['Général', '']

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); document.getElementById('gsearch')?.focus() }
    }
    window.addEventListener('keydown', h)
    return () => window.removeEventListener('keydown', h)
  }, [])
  useEffect(() => { app.send({ t: 'place', place: title }) }, [title, app.map?.id])

  const uefn = useUefnStatus()
  const others = app.presence.filter(p => p.uid !== app.me?.user.id)
  const allMaps = app.me?.studios.flatMap(s => s.maps.map(m => ({ s, m }))) ?? []

  return (
    <div className={'app' + (collapsed ? ' collapsed' : '') + (app.dark ? ' dark' : '')}>
      <aside className="side">
        <div className="brand"><img src="/logo.png" alt="POP" /></div>
        <label className="projsel lbl">
          <span className="navsec" style={{ padding: '0 4px 2px' }}>Map</span>
          <select aria-label="Map" value={app.map?.id ?? ''} onChange={e => {
            if (e.target.value === 'new') nav('/equipe?tab=maps'); else app.setMapId(Number(e.target.value))
          }}>
            {allMaps.map(({ s, m }) => <option key={m.id} value={m.id}>{m.name}{app.me!.studios.length > 1 ? ` · ${s.name}` : ''}</option>)}
            <option value="new">+ Nouvelle map</option>
          </select>
        </label>
        <nav className="nav" aria-label="Sections">
          {GROUPS.map(([g, items]) => (
            <div key={g} className="nav">
              <span className="navsec lbl">{g}</span>
              {items.map(([p, l, I]) => (
                <NavLink key={p} to={p} end={p === '/'} className={({ isActive }) => (isActive ? 'on' : '')}>
                  <I size={20} strokeWidth={1.5} /><span className="lbl">{l}</span>
                </NavLink>
              ))}
            </div>
          ))}
        </nav>
        <nav className="nav" style={{ marginTop: 'auto' }} aria-label="Compte">
          <NavLink to="/equipe?tab=compte"><Settings size={20} strokeWidth={1.5} /><span className="lbl">Paramètres du compte</span></NavLink>
          <NavLink to="/aide"><LifeBuoy size={20} strokeWidth={1.5} /><span className="lbl">Aide et support</span></NavLink>
        </nav>
      </aside>
      <div className="main">
        <header className="top">
          <button className="iconbtn" aria-label="Replier la barre latérale" onClick={() => setCollapsed(c => !c)}><PanelLeft size={20} strokeWidth={1.5} /></button>
          <nav className="crumb" aria-label="Fil d'Ariane"><span>{group}</span><span>/</span>{title}</nav>
          {others.length > 0 && (
            <div className="presence" title={others.map(o => `${o.name}${o.place ? ' · ' + o.place : ''}`).join('\n')}>
              {others.slice(0, 4).map(o => <span key={o.uid} className="pr"><Avatar name={o.name} small /></span>)}
            </div>
          )}
          <button className={'upill ' + uefn.status} onClick={() => nav('/organiseur')} title={uefn.project ? `Projet ${uefn.project}` : 'UEFN Link'}><span className="udot" />{STATUS_LABEL[uefn.status]}{uefn.pending ? ` · ${uefn.pending}` : ''}</button>
          <GlobalSearch />
          <button className="iconbtn" aria-label={app.dark ? 'Thème clair' : 'Thème sombre'} onClick={app.toggleDark}>{app.dark ? <Sun size={20} strokeWidth={1.5} /> : <Moon size={20} strokeWidth={1.5} />}</button>
          <Notifications />
          {app.canEdit && <button className="btn pri" onClick={() => setNewTask(true)}><Plus size={18} strokeWidth={1.5} />Nouvelle tâche</button>}
          <div style={{ position: 'relative' }}>
            <button className="acct" onClick={() => setAcct(a => !a)} aria-expanded={acct} aria-label="Compte et studio">
              <span className="acct-av">{(app.me?.user.name ?? '?').split(/\s+/).map(w => w[0]).join('').slice(0, 2).toUpperCase()}</span>
              <span className="acct-txt"><span className="acct-name">{app.me?.user.name}</span><span className="acct-studio">{app.studio?.name}</span></span>
              <ChevronDown size={16} strokeWidth={1.75} className="acct-chev" />
            </button>
            {acct && (
              <div className="menu" style={{ right: 0, top: 54 }}>
                <div className="small muted" style={{ padding: '6px 10px' }}>{app.me?.user.email}</div>
                <button onClick={() => { setAcct(false); nav('/equipe?tab=maps') }}><Plus size={16} />Nouveau studio ou map</button>
                <button onClick={async () => { await api.post('/api/auth/logout'); location.href = '/' }}><LogOut size={16} />Se déconnecter</button>
              </div>
            )}
          </div>
        </header>
        <div className="scroll"><Outlet /></div>
      </div>
      {newTask && <NewTaskModal onClose={() => setNewTask(false)} />}
    </div>
  )
}

function GlobalSearch() {
  const app = useApp()
  const nav = useNavigate()
  const [q, setQ] = useState('')
  const [res, setRes] = useState<any[]>([])
  const [open, setOpen] = useState(false)
  const [sel, setSel] = useState(0)
  useEffect(() => {
    if (!app.map || q.trim().length < 2) { setRes([]); return }
    const t = setTimeout(() => api.get(`/api/maps/${app.map!.id}/search?q=${encodeURIComponent(q)}`).then(r => { setRes(r); setSel(0) }).catch(() => {}), 160)
    return () => clearTimeout(t)
  }, [q, app.map?.id])
  const go = (r: any) => { setOpen(false); setQ(''); nav(r.link) }
  return <div style={{ position: 'relative' }} className="gsearch-wrap">
    <form className="search" onSubmit={e => { e.preventDefault(); if (res[sel]) go(res[sel]) }}>
      <Search size={18} strokeWidth={1.5} />
      <input id="gsearch" type="search" placeholder="Chercher" aria-label="Recherche globale" value={q} autoComplete="off"
        onChange={e => { setQ(e.target.value); setOpen(true) }} onFocus={() => setOpen(true)} onBlur={() => setTimeout(() => setOpen(false), 150)}
        onKeyDown={e => { if (e.key === 'ArrowDown') { e.preventDefault(); setSel(s => Math.min(res.length - 1, s + 1)) } if (e.key === 'ArrowUp') { e.preventDefault(); setSel(s => Math.max(0, s - 1)) } if (e.key === 'Escape') (e.target as HTMLInputElement).blur() }} />
      <span className="kbd">Ctrl K</span>
    </form>
    {open && q.trim().length >= 2 && <div className="menu gresults">
      {res.length === 0 ? <div className="small muted" style={{ padding: 10 }}>Aucun résultat pour « {q} »</div> : res.map((r, i) => (
        <button key={i} className={i === sel ? 'on' : ''} onMouseDown={e => { e.preventDefault(); go(r) }}>
          <span className="badge" style={{ minWidth: 64, justifyContent: 'center' }}>{r.kind}</span>
          <span style={{ minWidth: 0, flex: 1, textAlign: 'left' }}><span style={{ display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.title}</span>{r.detail && <span className="small muted" style={{ display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.detail}</span>}</span>
        </button>))}
    </div>}
  </div>
}

function Notifications() {
  const app = useApp()
  const nav = useNavigate()
  const [open, setOpen] = useState(false)
  const [data, setData] = useState<{ unread: number; items: any[] }>({ unread: 0, items: [] })
  const load = () => api.get('/api/notifications').then(setData).catch(() => {})
  useEffect(() => { load() }, [app.me?.user.id])
  useEffect(() => app.subscribe(m => { if (m.t === 'notif' && m.uid === app.me?.user.id) load() }), [app.subscribe, app.me?.user.id])
  return <div style={{ position: 'relative' }}>
    <button className="iconbtn" aria-label={`Notifications${data.unread ? ` (${data.unread} non lues)` : ''}`} aria-expanded={open} onClick={async () => { setOpen(o => !o); if (!open && data.unread) { await api.post('/api/notifications/read'); setTimeout(load, 300) } }}>
      <Bell size={20} strokeWidth={1.5} />{data.unread > 0 && <span className="dot" />}
    </button>
    {open && <div className="menu notif" style={{ right: 0, top: 50 }}>
      <div className="small muted" style={{ padding: '6px 10px' }}>Notifications</div>
      {data.items.length === 0 ? <div className="small muted" style={{ padding: 10 }}>Rien pour l’instant. Vous serez prévenu quand on vous assigne une tâche, qu’on vous mentionne avec @, ou qu’une tâche se débloque.</div> :
        data.items.map(n => <button key={n.id} onClick={() => { setOpen(false); if (n.map_id && n.map_id !== app.map?.id) app.setMapId(n.map_id); if (n.link) nav(n.link) }} style={{ alignItems: 'flex-start' }}>
          {!n.read && <span style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--red)', marginTop: 7, flex: 'none' }} />}
          <span style={{ flex: 1, textAlign: 'left' }}><span style={{ display: 'block' }}>{n.text}</span><span className="small muted">{ago(n.at)}</span></span>
        </button>)}
    </div>}
  </div>
}
