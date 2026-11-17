import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useEffect, useRef, useState } from 'react'
import {
  Home, FileText, PenTool, CalendarDays, KanbanSquare, Rocket, Coins, Users, Settings, LifeBuoy, PanelLeft,
  Search, Moon, Sun, Bell, Plus, ChevronDown, Hexagon, LogOut, ListTree, Bot, ChartNoAxesCombined, Hash, Flame, Sparkle,
} from 'lucide-react'
import { useApp, studioName } from '../state'
import { ago, api } from '../api'
import { Avatar } from './ui'
import { NewTaskModal } from '../pages/Tasks'
import { GuideDrawer } from './Guide'
import { Compass, ShieldCheck } from 'lucide-react'
import { useUefnStatus, STATUS_LABEL } from '../uefnState'
import { tr, tx } from '../i18n/t'
import LangPicker from './LangPicker'
import { MapSwitcher, CreateHost, openCreate } from './MapSwitcher'

const GROUPS: [string, [string, string, any][]][] = [
  [tr("Général"), [['/', tr("Vue d'ensemble"), Home]]],
  [tr("Espace"), [['/pages', tr("Pages"), FileText], ['/tableau', tr("Tableau blanc"), PenTool]]],
  [tr("Production"), [['/planifier', tr("Planifier"), CalendarDays], ['/taches', tr("Tâches"), KanbanSquare]]],
  ['UEFN', [['/organiseur', tr("Organiseur"), ListTree], ['/ia', tr("IA connectées"), Bot]]],
  [tr("Analyse"), [['/analyse/marche', tr("Marché"), ChartNoAxesCombined], ['/analyse/tiktok', tr("Tendances vidéo"), Hash], ['/analyse/tendances', tr("Maps qui marchent"), Flame], ['/analyse/lancements', tr("Meilleurs démarrages"), Sparkle]]],
  [tr("Publication"), [['/lancer', tr("Lancer et suivre"), Rocket]]],
  [tr("Studio"), [['/remunerer', tr("Rémunérer"), Coins], ['/equipe', tr("Équipe"), Users]]],
]
const TITLES: Record<string, [string, string]> = {}
GROUPS.forEach(([g, items]) => items.forEach(([p, l]) => (TITLES[p] = [g, l])))
TITLES['/parametres'] = [tr("Compte"), tr("Paramètres")]
TITLES['/aide'] = [tr("Compte"), tr("Aide et support")]
TITLES['/admin'] = ['POP', tr("Administration")]
TITLES['/equipe'] = [tr("Studio"), tr("Équipe et intégrations")]

export default function Layout() {
  const app = useApp()
  const loc = useLocation()
  const nav = useNavigate()
  const [collapsed, setCollapsed] = useState(false)
  const [acct, setAcct] = useState(false)
  const acctBox = useOutside(acct, () => setAcct(false))
  const [newTask, setNewTask] = useState(false)
  const [guide, setGuide] = useState<false | { ask?: string }>(false)
  useEffect(() => { const h = (e: Event) => setGuide({ ask: (e as CustomEvent).detail ?? '' }); window.addEventListener('boussole', h); return () => window.removeEventListener('boussole', h) }, [])
  const key = '/' + (loc.pathname.split('/')[1] || '')
  const [group, title] = TITLES[loc.pathname] ?? TITLES[key] ?? [tr("Général"), '']

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
        <MapSwitcher />
        <nav className="nav" aria-label={tr("Sections")}>
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
        <nav className="nav" style={{ marginTop: 'auto' }} aria-label={tr("Compte")}>
          <NavLink to="/parametres" className={({ isActive }) => (isActive ? 'on' : '')}><Settings size={20} strokeWidth={1.5} /><span className="lbl">{tr("Paramètres")}</span></NavLink>
          <NavLink to="/aide"><LifeBuoy size={20} strokeWidth={1.5} /><span className="lbl">{tr("Aide et support")}</span></NavLink>
          {app.me?.admin && <NavLink to="/admin" className={({ isActive }) => (isActive ? 'on' : '')}><ShieldCheck size={20} strokeWidth={1.5} /><span className="lbl">{tr("Administration")}</span>{!!app.me.admin_pending && <span className="nav-count">{app.me.admin_pending}</span>}</NavLink>}
        </nav>
      </aside>
      <div className="main">
        <header className="top">
          <button className="iconbtn" aria-label={tr("Replier la barre latérale")} onClick={() => setCollapsed(c => !c)}><PanelLeft size={20} strokeWidth={1.5} /></button>
          <nav className="crumb" aria-label={tr("Fil d'Ariane")}><span>{group}</span><span>/</span>{title}</nav>
          {others.length > 0 && (
            <div className="presence" title={others.map(o => `${o.name}${o.place ? ' · ' + o.place : ''}`).join('\n')}>
              {others.slice(0, 4).map(o => <span key={o.uid} className="pr"><Avatar name={o.name} small /></span>)}
            </div>
          )}
          <button className={'upill ' + uefn.status} onClick={() => nav('/organiseur')} title={uefn.project ? tr("Projet {0}", [uefn.project]) : 'UEFN Link'}><span className="udot" />{STATUS_LABEL[uefn.status]}{uefn.pending ? ` · ${uefn.pending}` : ''}</button>
          <GlobalSearch />
          <button className="iconbtn" aria-label={app.dark ? tr("Thème clair") : tr("Thème sombre")} onClick={app.toggleDark}>{app.dark ? <Sun size={20} strokeWidth={1.5} /> : <Moon size={20} strokeWidth={1.5} />}</button>
          <LangPicker app />
          {app.map && <button className="iconbtn" aria-label={tr("Boussole, assistant intégré")} title={tr("Boussole : assistant intégré")} onClick={() => setGuide({})}><Compass size={20} strokeWidth={1.5} /></button>}
          <Notifications />
          {app.canEdit && <button className="btn pri" onClick={() => setNewTask(true)}><Plus size={18} strokeWidth={1.5} />{tr("Nouvelle tâche")}</button>}
          <div style={{ position: 'relative' }} ref={acctBox}>
            <button className="acct" onClick={() => setAcct(a => !a)} aria-expanded={acct} aria-label={tr("Compte et studio")}>
              <span className="acct-av">{(app.me?.user.name ?? '?').split(/\s+/).map(w => w[0]).join('').slice(0, 2).toUpperCase()}</span>
              <span className="acct-txt"><span className="acct-name">{app.me?.user.name}</span><span className="acct-studio">{studioName(app.studio)}</span></span>
              <ChevronDown size={16} strokeWidth={1.75} className="acct-chev" />
            </button>
            {acct && (
              <div className="menu" style={{ right: 0, top: 54 }}>
                <div className="small muted" style={{ padding: '6px 10px' }}>{app.me?.user.email}</div>
                <button onClick={() => { setAcct(false); openCreate({ kind: 'map' }) }}><Plus size={16} />{tr("Nouvelle map")}</button>
                <button onClick={() => { setAcct(false); openCreate({ kind: 'studio' }) }}><Plus size={16} />{tr("Nouveau studio")}</button>
                <button onClick={() => { setAcct(false); nav('/parametres') }}><Settings size={16} />{tr("Paramètres")}</button>
                <button onClick={async () => { await api.post('/api/auth/logout'); location.href = '/' }}><LogOut size={16} />{tr("Se déconnecter")}</button>
              </div>
            )}
          </div>
        </header>
        <div className="scroll"><Outlet /></div>
      </div>
      {guide && <GuideDrawer key={guide.ask ?? 'day'} ask={guide.ask} onClose={() => setGuide(false)} />}
      {newTask && <NewTaskModal onClose={() => setNewTask(false)} />}
      <CreateHost />
    </div>
  )
}

/** Ferme un menu quand on clique ailleurs ou qu'on appuie sur Échap. */
function useOutside(open: boolean, close: () => void) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const down = (e: PointerEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) close() }
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') close() }
    document.addEventListener('pointerdown', down, true); document.addEventListener('keydown', key)
    return () => { document.removeEventListener('pointerdown', down, true); document.removeEventListener('keydown', key) }
  }, [open])
  return ref
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
    const t = setTimeout(() => api.get(`/api/maps/${app.map!.id}/search?q=${encodeURIComponent(q)}`).then((r: any[]) => { const order: string[] = []; r.forEach(x => { if (!order.includes(x.kind)) order.push(x.kind) }); setRes(order.flatMap(k => r.filter(x => x.kind === k))); setSel(0) }).catch(() => {}), 160)
    return () => clearTimeout(t)
  }, [q, app.map?.id])
  const go = (r: any) => { setOpen(false); setQ(''); nav(r.link) }
  return <div style={{ position: 'relative' }} className="gsearch-wrap">
    <form className="search" onSubmit={e => { e.preventDefault(); if (res[sel]) go(res[sel]) }}>
      <Search size={18} strokeWidth={1.5} />
      <input id="gsearch" type="search" placeholder={tr("Chercher")} aria-label={tr("Recherche globale")} value={q} autoComplete="off"
        onChange={e => { setQ(e.target.value); setOpen(true) }} onFocus={() => setOpen(true)} onBlur={() => setTimeout(() => setOpen(false), 150)}
        onKeyDown={e => { if (e.key === 'ArrowDown') { e.preventDefault(); setSel(s => Math.min(res.length - 1, s + 1)) } if (e.key === 'ArrowUp') { e.preventDefault(); setSel(s => Math.max(0, s - 1)) } if (e.key === 'Escape') (e.target as HTMLInputElement).blur() }} />
      <span className="kbd">{tr("Ctrl K")}</span>
    </form>
    {open && q.trim().length >= 2 && <div className="menu gresults">
      {res.length === 0 ? <div className="small muted" style={{ padding: 10 }}>{tr("Aucun résultat pour « {0} »", [q])}</div> : res.map((r, i) => (<div key={i}>
        {(i === 0 || res[i - 1].kind !== r.kind) && <div className="gr-head">{({ Tâche: tr("Tâches"), Page: tr("Pages"), Tableau: tr("Tableaux"), Verse: tr("Fichiers Verse"), Acteur: tr("Acteurs UEFN"), Jalon: tr("Jalons"), Membre: tr("Membres"), Action: tr("Actions récentes") } as any)[r.kind] ?? r.kind}</div>}
        <button className={i === sel ? 'on' : ''} onMouseDown={e => { e.preventDefault(); go(r) }} onMouseEnter={() => setSel(i)}>
          <span style={{ minWidth: 0, flex: 1, textAlign: 'left' }}><span style={{ display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.title}</span>{r.detail && <span className="small muted" style={{ display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{tx(r.detail)}</span>}</span>
        </button></div>))}
    </div>}
  </div>
}

function Notifications() {
  const app = useApp()
  const nav = useNavigate()
  const [open, setOpen] = useState(false)
  const [data, setData] = useState<{ unread: number; items: any[] }>({ unread: 0, items: [] })
  const load = () => api.get('/api/notifications').then(setData).catch(() => {})
  const box = useOutside(open, () => setOpen(false))
  useEffect(() => { load() }, [app.me?.user.id])
  useEffect(() => app.subscribe(m => { if (m.t === 'notif' && m.uid === app.me?.user.id) load() }), [app.subscribe, app.me?.user.id])
  return <div style={{ position: 'relative' }} ref={box}>
    <button className="iconbtn" aria-label={tr("Notifications{0}", [data.unread ? tr(" ({0} non lues)", [data.unread]) : ''])} aria-expanded={open} onClick={async () => { setOpen(o => !o); if (!open && data.unread) { await api.post('/api/notifications/read'); setTimeout(load, 300) } }}>
      <Bell size={20} strokeWidth={1.5} />{data.unread > 0 && <span className="dot" />}
    </button>
    {open && <div className="menu notif" style={{ right: 0, top: 50 }}>
      <div className="small muted" style={{ padding: '6px 10px' }}>{tr("Notifications")}</div>
      {data.items.length === 0 ? <div className="small muted" style={{ padding: 10 }}>{tr("Rien pour l’instant. Vous serez prévenu quand on vous assigne une tâche, qu’on vous mentionne avec @, ou qu’une tâche se débloque.")}</div> :
        data.items.map(n => <button key={n.id} onClick={() => { setOpen(false); if (n.map_id && n.map_id !== app.map?.id) app.setMapId(n.map_id); if (n.link) nav(n.link) }} style={{ alignItems: 'flex-start' }}>
          {!n.read && <span style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--red)', marginTop: 7, flex: 'none' }} />}
          <span style={{ flex: 1, textAlign: 'left' }}><span style={{ display: 'block' }}>{n.text}</span><span className="small muted">{ago(n.at)}</span></span>
        </button>)}
    </div>}
  </div>
}
