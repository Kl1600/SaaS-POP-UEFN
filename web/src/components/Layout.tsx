import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useEffect, useRef, useState } from 'react'
import { Target,
  Home, FileText, PenTool, CalendarDays, KanbanSquare, Rocket, Coins, Users, Settings, LifeBuoy, PanelLeft,
  Search, Bell, Plus, ChevronDown, Hexagon, LogOut, ListTree, Bot, ChartNoAxesCombined, Layers, Lock, Sun, Moon } from 'lucide-react'
import { useApp, studioName } from '../state'
import { ago, api } from '../api'
import { Avatar } from './ui'
import { NewTaskModal } from '../pages/Tasks'
import { GuideDrawer } from './Guide'
import { Sparkles, MailCheck, ShieldCheck, Megaphone, AlertTriangle, X } from 'lucide-react'
import { useUefnStatus, STATUS_LABEL } from '../uefnState'
import { tr, tx, LOCALE } from '../i18n/t'
import { MapSwitcher, CreateHost, openCreate } from './MapSwitcher'
import { Tooltips } from './Tooltip'
import { useNav } from '../routes'
import { IdeaPop } from '../pages/Ideas'
import { AiPill, ConnectAIModal, useAccountAI } from './ConnectAI'

type Item = [string, string, any]
/** Onglets de la map (changent quand on change de map). Le premier groupe n'a pas de titre. */
const mapGroups = (mid: number): [string, Item[]][] => [
  ['', [[`/map/${mid}`, tr("Dashboard"), Home]]],
  [tr("Organiser"), [[`/map/${mid}/planifier`, tr("Planifier"), CalendarDays], [`/map/${mid}/taches`, tr("Tâches"), KanbanSquare]]],
  [tr("Créer"), [[`/map/${mid}/pages`, tr("Pages"), FileText], [`/map/${mid}/tableau`, tr("Tableau blanc"), PenTool]]],
  [tr("Publier"), [[`/map/${mid}/lancer`, tr("Lancer et suivre"), Rocket], [`/map/${mid}/remunerer`, tr("Rémunérer"), Coins]]],
]
/** Catégorie Analyse (plan du compte). */
const analyseItems = (): Item[] => [
  // Un seul salon (#316) : les pages détaillées restent joignables depuis « Maps à faire ».
  ['/analyse/a-faire', tr("Maps à faire"), Target],
]

export default function Layout() {
  const app = useApp()
  const loc = useLocation()
  const nav = useNav()
  const [collapsed, setCollapsed] = useState(false)
  const [acct, setAcct] = useState(false)
  const acctBox = useOutside(acct, () => setAcct(false))
  const [newTask, setNewTask] = useState(false)
  // « Connecte ton IA » (#134) : ouverte d'office tant qu'aucune IA du compte n'a appelé POP, sauf après « Passer ».
  const [aiOpen, setAiOpen] = useState<'' | 'auto' | 'manual'>('')
  const ai = useAccountAI(!!aiOpen)
  useEffect(() => {
    if (!app.me || !ai.tokens || ai.live.length || app.me.ai_intro_skipped) return
    let closed = false
    try { closed = sessionStorage.getItem('pop-ai-intro') === 'closed' } catch {}
    if (!closed) setAiOpen(o => o || 'auto')
  }, [app.me?.user.id, !!ai.tokens])
  const closeAi = () => { if (aiOpen === 'auto') { try { sessionStorage.setItem('pop-ai-intro', 'closed') } catch {} } setAiOpen('') }
  const [guide, setGuide] = useState<false | { ask?: string }>(false)
  useEffect(() => { const h = (e: Event) => setGuide({ ask: (e as CustomEvent).detail ?? '' }); window.addEventListener('boussole', h); return () => window.removeEventListener('boussole', h) }, [])
  // Pas d'onglets de map quand le studio ouvert n'a pas cette map (ex. espace perso encore vide).
  const showMap = !!app.map && (!app.studio || app.studio.maps.some(m => m.id === app.map!.id))
  const mGroups = showMap ? mapGroups(app.map!.id) : []
  const st = app.studio
  const sLabel = st ? (st.personal ? tr("Mes outils") : st.name) : ''
  const sHome = st ? `/studio/${st.id}/${st.personal ? 'maps' : 'equipe'}` : ''
  // Onglet « Marché » du studio : seulement dans un studio d'équipe, si son créateur a le plan (cadenas pour le créateur sinon).
  const sMarket = st && !st.personal && (st.analyse || st.role === 'owner') ? `/studio/${st.id}/marche` : ''
  const onStudio = !!st && loc.pathname.startsWith(`/studio/${st.id}`) && !loc.pathname.startsWith(`/studio/${st.id}/marche`)
  const aItems = analyseItems()
  const locked = !app.me?.analyse
  const all: [string, string, string][] = [
    ...mGroups.flatMap(([, items]) => items.map(([p, l]) => [p, app.map!.name, l] as [string, string, string])),
    ...(st ? [[`/studio/${st.id}`, tr("Studio"), sLabel] as [string, string, string]] : []),
    ...(sMarket ? [[sMarket, sLabel, tr("Marché")] as [string, string, string]] : []),
    ...aItems.map(([p, l]) => [p, tr("Analyse"), l] as [string, string, string]),
    ['/', 'POP', tr("Accueil")], ['/parametres', tr("Compte"), tr("Paramètres")], ['/aide', 'POP', tr("Aide et support")], ['/admin', 'POP', tr("Administration")],
  ]
  const hit = all.filter(([p]) => p === '/' ? loc.pathname === '/' : loc.pathname === p || loc.pathname.startsWith(p + '/')).sort((x, y) => y[0].length - x[0].length)[0]
  const [, group, title] = hit ?? ['', 'POP', tr("Accueil")]

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); window.dispatchEvent(new Event('pop:search')) }
    }
    window.addEventListener('keydown', h)
    return () => window.removeEventListener('keydown', h)
  }, [])
  useEffect(() => { app.send({ t: 'place', place: title }) }, [title, app.map?.id])

  const uefn = useUefnStatus()
  const allMaps = app.me?.studios.flatMap(s => s.maps.map(m => ({ s, m }))) ?? []

  return (
    <div className={'app' + (collapsed ? ' collapsed' : '') + (app.dark ? ' dark' : '')}>
      <a href="#contenu" className="skip">{tr("Aller au contenu")}</a>
      <aside className="side">
        <div className="brand"><img src="/logo.png" alt="POP" /></div>
        <nav className="nav" aria-label={tr("Accueil")}>
        </nav>
        <MapSwitcher />
        <nav className="nav" aria-label={tr("Sections")}>
          {mGroups.map(([g, items], gi) => (
            <div key={gi} className="nav">
              {g && <span className="navsec lbl">{g}</span>}
              {items.map(([p, l, I]) => <NavLink key={p} to={p} end={p === `/map/${app.map!.id}`} className={({ isActive }) => (isActive ? 'on' : '')}><I size={20} strokeWidth={1.5} /><span className="lbl">{l}</span></NavLink>)}
            </div>
          ))}
          {st && <div className="nav">
            <span className="navsec lbl">{tr("Studio")}</span>
            <NavLink to={sHome} className={onStudio ? 'on' : ''} title={sLabel}>{st.personal ? <Layers size={20} strokeWidth={1.5} /> : <Users size={20} strokeWidth={1.5} />}<span className="lbl">{sLabel}</span></NavLink>
            {sMarket && <NavLink to={sMarket} className={({ isActive }) => (isActive ? 'on' : '')}><ChartNoAxesCombined size={20} strokeWidth={1.5} /><span className="lbl">{tr("Marché")}</span>{!st.analyse && <Lock size={14} className="nav-lock" />}</NavLink>}
          </div>}
          <div className="nav">
            <span className="navsec lbl">{tr("Analyse")}{locked && <Lock size={12} className="nav-lock" />}</span>
            {aItems.map(([p, l, I]) => <NavLink key={p} to={p} className={({ isActive }) => (isActive ? 'on' : '') + (locked ? ' locked' : '')}><I size={20} strokeWidth={1.5} /><span className="lbl">{l}</span></NavLink>)}
          </div>
        </nav>
        <div className="side-foot">
          <div className="acct-wrap" ref={acctBox}>
          <button className="acct" onClick={() => setAcct(a => !a)} aria-expanded={acct} aria-label={tr("Compte et studio")}>
            <span className="acct-av">{(app.me?.user.name ?? '?').split(/\s+/).map(w => w[0]).join('').slice(0, 2).toUpperCase()}</span>
            <span className="acct-txt"><span className="acct-name">{app.me?.user.name}</span><span className="acct-studio">{studioName(app.studio)}</span></span>
            <ChevronDown size={16} strokeWidth={1.75} className="acct-chev" />
          </button>
          {acct && (
            <div className="menu acct-menu">
              <div className="small muted acct-mail" title={app.me?.user.email}>{app.me?.user.email}</div>
              <div className="acct-plan small">{app.me?.trial_until ? tr("Essai Pro jusqu’au {0}", [new Date(app.me.trial_until.replace(' ', 'T') + 'Z').toLocaleDateString(LOCALE, { day: 'numeric', month: 'long' })]) : app.me?.plan === 'pro' ? tr("Plan Pro") : tr("Plan Starter")}</div>
              <button onClick={() => { setAcct(false); nav('/parametres') }}><Settings size={16} />{tr("Paramètres")}</button>
              <button onClick={() => { setAcct(false); nav('/aide') }}><LifeBuoy size={16} />{tr("Aide et support")}</button>
              {(app.me?.admin || app.me?.moderator) && <button onClick={() => { setAcct(false); nav('/admin') }}><ShieldCheck size={16} />{tr("Administration")}{!!app.me.admin_pending && <span className="nav-count">{app.me.admin_pending}</span>}</button>}
              <button onClick={async () => { await api.post('/api/auth/logout'); location.href = '/' }}><LogOut size={16} />{tr("Se déconnecter")}</button>
            </div>
          )}
        </div>
        </div>
      </aside>
      <div className="main">
        <header className="top">
          <button className="iconbtn" aria-label={tr("Replier la barre latérale")} onClick={() => setCollapsed(c => !c)}><PanelLeft size={20} strokeWidth={1.5} /></button>
          <nav className="crumb" aria-label={tr("Fil d'Ariane")}><span>{group}</span><span>/</span>{title}</nav>
          <button className={'upill ' + uefn.status} onClick={() => nav('/parametres?onglet=uefn')} title={uefn.project ? tr("Projet {0}", [uefn.project]) : 'UEFN Link'}><span className="udot" />{STATUS_LABEL[uefn.status]}{uefn.stale ? ` · ${tr("périmé")}` : ''}{uefn.pending ? ` · ${uefn.pending}` : ''}</button>
          <AiPill ai={ai} onOpen={() => setAiOpen('manual')} onSettings={() => nav('/parametres?onglet=ia')} />
          <span className="top-spacer" />
          <GlobalSearch />
          <Notifications />
          <button className="iconbtn" aria-label={app.dark ? tr("Passer en mode clair") : tr("Passer en mode sombre")} data-tip={app.dark ? tr("Mode clair") : tr("Mode sombre")} onClick={app.toggleDark}>{app.dark ? <Sun size={20} strokeWidth={1.5} /> : <Moon size={20} strokeWidth={1.5} />}</button>
          <CreateMenu onTask={app.map && app.canEdit ? () => setNewTask(true) : undefined} />
        </header>
        <div className="scroll" id="contenu" tabIndex={-1}>{app.me?.announcement && <Announcement a={app.me.announcement} />}{app.me?.email_verified === false && <VerifyBanner email={app.me.user.email} />}<Outlet /></div>
      </div>
      {app.map && <button className="popy-fab" aria-label={tr("Popy, assistant intégré")} data-tip={tr("Popy, votre assistant")} onClick={() => setGuide({})}><Sparkles size={24} strokeWidth={1.75} /></button>}
      {guide && <GuideDrawer key={guide.ask ?? 'day'} ask={guide.ask} onClose={() => setGuide(false)} />}
      {newTask && <NewTaskModal onClose={() => setNewTask(false)} />}
      {aiOpen && <ConnectAIModal ai={ai} onClose={closeAi} onSkip={aiOpen === 'auto' ? async () => {
        try { await api.post('/api/me/ai-intro/skip'); setAiOpen(''); app.reload() } catch (e: any) { app.toast(e.message, true) }
      } : undefined} />}
      <CreateHost />
      <Tooltips />
      <IdeaPop />
    </div>
  )
}

/** Annonce des administrateurs (#67) : en haut du contenu, sous la barre du haut ; fermée, elle ne revient pas. */
function Announcement({ a }: { a: { id: number; text: string; level: string } }) {
  const [closed, setClosed] = useState(false)
  if (closed) return null
  return (
    <div className={'warn announce' + (a.level === 'important' ? ' important' : '')} role={a.level === 'important' ? 'alert' : 'status'} style={{ margin: '16px 24px 0' }}>
      {a.level === 'important' ? <AlertTriangle size={18} /> : <Megaphone size={18} />}
      <span style={{ flex: 1, overflowWrap: 'anywhere' }}>{a.text}</span>
      <button className="iconbtn" aria-label={tr("Fermer l’annonce")} onClick={() => { setClosed(true); api.post(`/api/announcement/${a.id}/dismiss`).catch(() => {}) }}><X size={18} strokeWidth={1.5} /></button>
    </div>
  )
}

/** Ferme un menu quand on clique ailleurs ou qu'on appuie sur Échap. */
/** Adresse e-mail pas encore confirmée (#11) : le compte n'a pas accès à l'administration tant que le lien n'est pas ouvert. */
function VerifyBanner({ email }: { email: string }) {
  const [msg, setMsg] = useState('')
  const [busy, setBusy] = useState(false)
  return (
    <div className="warn" role="status" style={{ margin: '16px 24px 0' }}>
      <MailCheck size={18} />
      <span style={{ flex: 1 }}>
        <b>Confirmez votre adresse e-mail.</b> Nous avons envoyé un lien à {email}. Tant qu’elle n’est pas confirmée, l’administration de la plateforme reste fermée.
        {msg && <> {msg}</>}
      </span>
      <button className="btn" disabled={busy} onClick={async () => {
        setBusy(true)
        try { await api.post('/api/me/verify-email/resend'); setMsg('Nouveau lien envoyé.') } catch (e: any) { setMsg(e.message) } finally { setBusy(false) }
      }}>Renvoyer le lien</button>
    </div>
  )
}

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
  const nav = useNav()
  const [q, setQ] = useState('')
  const [res, setRes] = useState<any[]>([])
  const [open, setOpen] = useState(false)
  const [sel, setSel] = useState(0)
  // Fermée : une loupe ; ouverte (clic, Ctrl K) : le champ s'élargit. Se referme vide quand on en sort.
  const [wide, setWide] = useState(false)
  const input = useRef<HTMLInputElement>(null)
  const openSearch = () => { setWide(true); setTimeout(() => input.current?.focus(), 0) }
  useEffect(() => { window.addEventListener('pop:search', openSearch); return () => window.removeEventListener('pop:search', openSearch) }, [])
  useEffect(() => {
    if (!app.map || q.trim().length < 2) { setRes([]); return }
    const t = setTimeout(() => api.get(`/api/maps/${app.map!.id}/search?q=${encodeURIComponent(q)}`).then((r: any[]) => { const order: string[] = []; r.forEach(x => { if (!order.includes(x.kind)) order.push(x.kind) }); setRes(order.flatMap(k => r.filter(x => x.kind === k))); setSel(0) }).catch(() => {}), 160)
    return () => clearTimeout(t)
  }, [q, app.map?.id])
  const go = (r: any) => { setOpen(false); setQ(''); setWide(false); nav(r.link) }
  if (!wide) return <button className="iconbtn" aria-label={tr("Chercher (Ctrl K)")} data-tip={tr("Chercher (Ctrl K)")} onClick={openSearch}><Search size={20} strokeWidth={1.5} /></button>
  return <div style={{ position: 'relative' }} className="gsearch-wrap open">
    <form className="search" onSubmit={e => { e.preventDefault(); if (res[sel]) go(res[sel]) }}>
      <Search size={18} strokeWidth={1.5} />
      <input id="gsearch" ref={input} type="search" placeholder={tr("Chercher")} aria-label={tr("Recherche globale")} value={q} autoComplete="off"
        onChange={e => { setQ(e.target.value); setOpen(true) }} onFocus={() => setOpen(true)} onBlur={() => setTimeout(() => { setOpen(false); if (!input.current?.value) setWide(false) }, 150)}
        onKeyDown={e => { if (e.key === 'ArrowDown') { e.preventDefault(); setSel(s => Math.min(res.length - 1, s + 1)) } if (e.key === 'ArrowUp') { e.preventDefault(); setSel(s => Math.max(0, s - 1)) } if (e.key === 'Escape') { setQ(''); setWide(false) } if (e.key === 'Escape') (e.target as HTMLInputElement).blur() }} />
      <span className="kbd">{tr("Ctrl K")}</span>
    </form>
    {open && q.trim().length >= 2 && <div className="menu gresults">
      {res.length === 0 ? <div className="small muted" style={{ padding: 10 }}>{tr("Aucun résultat pour « {0} »", [q])}</div> : res.map((r, i) => (<div key={i}>
        {(i === 0 || res[i - 1].kind !== r.kind) && <div className="gr-head">{({ Tâche: tr("Tâches"), Page: tr("Pages"), Tableau: tr("Tableaux"), Verse: tr("Fichiers Verse"), Acteur: tr("Acteurs UEFN"), Objectif: tr("Objectifs"), Membre: tr("Membres"), Action: tr("Actions récentes") } as any)[r.kind] ?? r.kind}</div>}
        <button className={i === sel ? 'on' : ''} onMouseDown={e => { e.preventDefault(); go(r) }} onMouseEnter={() => setSel(i)}>
          <span style={{ minWidth: 0, flex: 1, textAlign: 'left' }}><span style={{ display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.title}</span>{r.detail && <span className="small muted" style={{ display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{tx(r.detail)}</span>}</span>
        </button></div>))}
    </div>}
  </div>
}

/** Bouton « + » de la barre du haut : nouvelle tâche (si une map est ouverte et modifiable), map ou studio. */
function CreateMenu({ onTask }: { onTask?: () => void }) {
  const [open, setOpen] = useState(false)
  const box = useOutside(open, () => setOpen(false))
  const pick = (fn: () => void) => { setOpen(false); fn() }
  return <div style={{ position: 'relative' }} ref={box}>
    <button className="btn pri plusbtn" aria-label={tr("Créer")} data-tip={tr("Créer")} aria-expanded={open} aria-haspopup="menu" onClick={() => setOpen(o => !o)}><Plus size={20} strokeWidth={1.75} /></button>
    {open && <div className="menu" role="menu" style={{ right: 0, top: 50, minWidth: 220 }}>
      {onTask && <button role="menuitem" onClick={() => pick(onTask)}><KanbanSquare size={16} />{tr("Nouvelle tâche")}</button>}
      <button role="menuitem" onClick={() => pick(() => openCreate({ kind: 'map' }))}><Plus size={16} />{tr("Nouvelle map")}</button>
      <button role="menuitem" onClick={() => pick(() => openCreate({ kind: 'studio' }))}><Users size={16} />{tr("Nouveau studio")}</button>
    </div>}
  </div>
}

function Notifications() {
  const app = useApp()
  const nav = useNav()
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
        data.items.map(n => <button key={n.id} onClick={() => { setOpen(false); if (n.link) nav(n.link, { map: n.map_id || undefined }); else if (n.map_id) nav(`/map/${n.map_id}`) }} style={{ alignItems: 'flex-start' }}>
          {!n.read && <span style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--red)', marginTop: 7, flex: 'none' }} />}
          <span style={{ flex: 1, textAlign: 'left' }}><span style={{ display: 'block' }}>{n.text}</span><span className="small muted">{ago(n.at)}</span></span>
        </button>)}
    </div>}
  </div>
}
