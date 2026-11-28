import { OuvrirForge } from './OuvrirForge'
import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useEffect, useRef, useState } from 'react'
import { Target,
  Home, FileText, PenTool, CalendarDays, KanbanSquare, Rocket, Coins, Users, Settings, LifeBuoy, PanelLeft,
  Search, Bell, Plus, ChevronDown, Hexagon, LogOut, ListTree, Bot, ChartNoAxesCombined, Layers, Lock, Sun, Moon, GripVertical } from 'lucide-react'
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
import { AiPill } from './ConnectAI'
import { Didacticiel } from './Didacticiel'

type Item = [string, string, any]
/** Onglets de la map (changent quand on change de map). Le premier groupe n'a pas de titre. */
const mapGroups = (mid: number): [string, Item[]][] => [
  ['', [[`/map/${mid}`, tr("Dashboard"), Home]]],
  [tr("Organiser"), [[`/map/${mid}/planifier`, tr("Planifier"), CalendarDays], [`/map/${mid}/taches`, tr("Tâches"), KanbanSquare]]],
  [tr("Créer"), [[`/map/${mid}/pages`, tr("Pages"), FileText], [`/map/${mid}/tableau`, tr("Tableau blanc"), PenTool]]],
  [tr("Publier"), [[`/map/${mid}/lancer`, tr("Lancer et suivre"), Rocket], [`/map/${mid}/remunerer`, tr("Rémunérer"), Coins]]],
]
/** Clés stables des catégories de la map (#493), dans l'ordre de mapGroups sans le premier groupe. */
const GROUP_KEYS = ['organiser', 'creer', 'publier']
/** Clé stable d'un onglet de map : la fin de son adresse (« taches », « tableau »…). */
const itemKey = (p: string) => p.split('/').pop() ?? p
type NavPrefs = { order?: Record<string, string[]>; collapsed?: string[] }

/** Range les onglets de la map selon les préférences du compte : ordre et catégorie choisis, puis les onglets
 * absents des préférences (nouveaux) à leur place d'origine. */
function arrange(groups: [string, Item[]][], prefs: NavPrefs): [string, string, Item[]][] {
  const head = groups[0]
  const cats = groups.slice(1).map(([t, items], i) => ({ key: GROUP_KEYS[i], title: t, items }))
  const byKey = new Map(cats.flatMap(c => c.items.map(it => [itemKey(it[0]), it] as [string, Item])))
  const used = new Set<string>()
  const out = cats.map(c => {
    const keys = (prefs.order?.[c.key] ?? []).filter(k => byKey.has(k) && !used.has(k))
    keys.forEach(k => used.add(k))
    return { ...c, items: keys.map(k => byKey.get(k)!) }
  })
  cats.forEach((c, i) => c.items.forEach(it => { const k = itemKey(it[0]); if (!used.has(k)) { used.add(k); out[i].items.push(it) } }))
  return [['', head[0], head[1]], ...out.map(c => [c.key, c.title, c.items] as [string, string, Item[]])]
}
/** Catégorie Analyse (plan du compte). */
const analyseItems = (): Item[] => [
  // Un seul salon (#316) : les pages détaillées restent joignables depuis « Maps à faire ».
  ['/analyse/a-faire', tr("Maps à faire"), Target],
]

function useMq(q: string) {
  const [m, setM] = useState(() => window.matchMedia(q).matches)
  useEffect(() => { const mq = window.matchMedia(q); const h = () => setM(mq.matches); mq.addEventListener('change', h); return () => mq.removeEventListener('change', h) }, [q])
  return m
}

export default function Layout() {
  const app = useApp()
  const loc = useLocation()
  const nav = useNav()
  // Téléphone et tablette (#360) : la barre reste à gauche, repliée en icônes ; sur téléphone elle s'ouvre par-dessus.
  const phone = useMq('(max-width:760px)')
  const tablet = useMq('(max-width:1024px)')
  const [collapsed, setCollapsed] = useState(() => window.matchMedia('(max-width:1024px)').matches)
  const [drawer, setDrawer] = useState(false)
  useEffect(() => { setCollapsed(tablet) }, [tablet])
  useEffect(() => { setDrawer(false) }, [loc.pathname, phone])
  const rail = phone ? !drawer : collapsed
  const [acct, setAcct] = useState(false)
  const acctBox = useOutside(acct, () => setAcct(false))
  const [newTask, setNewTask] = useState(false)
  const [guide, setGuide] = useState<false | { ask?: string }>(false)
  useEffect(() => { const h = (e: Event) => setGuide({ ask: (e as CustomEvent).detail ?? '' }); window.addEventListener('boussole', h); return () => window.removeEventListener('boussole', h) }, [])
  // Pas d'onglets de map quand le studio ouvert n'a pas cette map (ex. espace perso encore vide).
  const showMap = !!app.map && (!app.studio || app.studio.maps.some(m => m.id === app.map!.id))
  const mGroups = showMap ? mapGroups(app.map!.id) : []
  // Barre latérale personnalisable (#493) : catégories repliées et ordre des onglets, gardés sur le compte.
  const [navP, setNavP] = useState<NavPrefs>(() => app.me?.ui_prefs?.nav ?? {})
  useEffect(() => { setNavP(app.me?.ui_prefs?.nav ?? {}) }, [app.me?.user.id, JSON.stringify(app.me?.ui_prefs?.nav ?? {})])
  const saveNav = (p: NavPrefs) => {
    setNavP(p)
    const all = { ...(app.me?.ui_prefs ?? {}), nav: p }
    if (app.me) app.me.ui_prefs = all
    api.put('/api/me/ui-prefs', all).catch(() => {})
  }
  const folded = (k: string) => !rail && (navP.collapsed ?? []).includes(k)
  const fold = (k: string) => { const c = navP.collapsed ?? []; saveNav({ ...navP, collapsed: c.includes(k) ? c.filter(x => x !== k) : [...c, k] }) }
  const arranged = showMap ? arrange(mGroups, navP) : []
  const [drag, setDrag] = useState<string | null>(null)
  const [dragCat, setDragCat] = useState<string | null>(null)
  /** Dépose l'onglet tiré avant `before` (ou à la fin de la catégorie `cat`). */
  const drop = (cat: string, before: string | null) => {
    if (!drag || cat !== dragCat) return
    const order: Record<string, string[]> = {}
    for (const [k, , items] of arranged.slice(1)) order[k] = items.map(it => itemKey(it[0])).filter(x => x !== drag)
    const list = order[cat] ?? []
    const at = before ? list.indexOf(before) : -1
    list.splice(at < 0 ? list.length : at, 0, drag)
    order[cat] = list
    setDrag(null)
    saveNav({ ...navP, order })
  }
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
  // Écran ouvert, en clé stable, pour la scène du studio (#176) : /map/12/taches → « taches », tableau de bord → « dashboard ».
  const ecran = (() => {
    const m = loc.pathname.match(/^\/map\/\d+\/?([^/]*)/)
    if (m) return m[1] || 'dashboard'
    if (loc.pathname.startsWith('/parametres') && loc.search.includes('onglet=uefn')) return 'uefn'
    return loc.pathname.split('/')[1] || 'accueil'
  })()
  useEffect(() => { app.send({ t: 'place', place: title, page: ecran }) }, [title, ecran, app.map?.id])

  const uefn = useUefnStatus()
  const allMaps = app.me?.studios.flatMap(s => s.maps.map(m => ({ s, m }))) ?? []

  return (
    <div className={'app' + (rail ? ' collapsed' : '') + (phone && drawer ? ' drawer-open' : '') + (app.dark ? ' dark' : '')}>
      {phone && drawer && <div className="side-scrim" onClick={() => setDrawer(false)} aria-hidden="true" />}
      {showMap || ecran === 'analyse' ? <Didacticiel key={ecran} page={ecran} /> : null}
      <a href="#contenu" className="skip">{tr("Aller au contenu")}</a>
      <aside className="side">
        <div className="brand"><img src="/logo.png" alt="POP" /></div>
        <nav className="nav" aria-label={tr("Accueil")}>
        </nav>
        <MapSwitcher />
        <nav className="nav" aria-label={tr("Sections")}>
          {arranged.map(([k, g, items]) => (
            <div key={k || 'tete'} className={'nav' + (drag && k && k === dragCat ? ' nav-drop' : '')} onDragOver={k && drag && k === dragCat ? e => e.preventDefault() : undefined} onDrop={k && drag && k === dragCat ? e => { e.preventDefault(); drop(k, null) } : undefined}>
              {g && <button type="button" className="navsec navsec-btn lbl" aria-expanded={!folded(k)} onClick={() => fold(k)}>{g}<ChevronDown size={14} className={'navsec-chev' + (folded(k) ? ' off' : '')} /></button>}
              {!folded(k) && items.map(([p, l, I]) => <NavLink key={p} to={p} end={p === `/map/${app.map!.id}`} draggable={false}
                onDragOver={k && drag && k === dragCat ? e => e.preventDefault() : undefined} onDrop={k && drag && k === dragCat ? e => { e.preventDefault(); e.stopPropagation(); drop(k, itemKey(p)) } : undefined}
                className={({ isActive }) => (isActive ? 'on' : '') + (drag === itemKey(p) ? ' dragging' : '')}><I size={20} strokeWidth={1.5} /><span className="lbl">{l}</span>
                {k && !rail ? <span className="nav-grip" draggable title={tr('Glisser pour déplacer dans la catégorie')} aria-hidden
                  onClick={e => { e.preventDefault(); e.stopPropagation() }}
                  onDragStart={e => { setDrag(itemKey(p)); setDragCat(k); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', itemKey(p)); const a = (e.currentTarget as HTMLElement).closest('a'); if (a) e.dataTransfer.setDragImage(a, 20, 16) }}
                  onDragEnd={() => { setDrag(null); setDragCat(null) }}><GripVertical size={14} strokeWidth={1.75} /></span> : null}</NavLink>)}
            </div>
          ))}
          {st && <div className="nav">
            <button type="button" className="navsec navsec-btn lbl" aria-expanded={!folded('studio')} onClick={() => fold('studio')}>{tr("Studio")}<ChevronDown size={14} className={'navsec-chev' + (folded('studio') ? ' off' : '')} /></button>
            {!folded('studio') && <>
            <NavLink to={sHome} className={onStudio ? 'on' : ''} title={sLabel}>{st.personal ? <Layers size={20} strokeWidth={1.5} /> : <Users size={20} strokeWidth={1.5} />}<span className="lbl">{sLabel}</span></NavLink>
            {sMarket && <NavLink to={sMarket} className={({ isActive }) => (isActive ? 'on' : '')}><ChartNoAxesCombined size={20} strokeWidth={1.5} /><span className="lbl">{tr("Marché")}</span>{!st.analyse && <Lock size={14} className="nav-lock" />}</NavLink>}
            </>}
          </div>}
          <div className="nav">
            <button type="button" className="navsec navsec-btn lbl" aria-expanded={!folded('analyse')} onClick={() => fold('analyse')}>{tr("Analyse")}{locked && <Lock size={12} className="nav-lock" />}<ChevronDown size={14} className={'navsec-chev' + (folded('analyse') ? ' off' : '')} /></button>
            {!folded('analyse') && aItems.map(([p, l, I]) => <NavLink key={p} to={p} className={({ isActive }) => (isActive ? 'on' : '') + (locked ? ' locked' : '')}><I size={20} strokeWidth={1.5} /><span className="lbl">{l}</span></NavLink>)}
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
          <button className="iconbtn" aria-label={tr("Replier la barre latérale")} onClick={() => phone ? setDrawer(d => !d) : setCollapsed(c => !c)}><PanelLeft size={20} strokeWidth={1.5} /></button>
          <nav className="crumb" aria-label={tr("Fil d'Ariane")}><span>{group}</span><span>/</span>{title}</nav>
          <button className={'upill ' + uefn.status} onClick={() => nav('/parametres?onglet=uefn')} title={uefn.project ? tr("Projet {0}", [uefn.project]) : 'UEFN Link'}><span className="udot" />{STATUS_LABEL[uefn.status]}{uefn.stale ? ` · ${tr("périmé")}` : ''}{uefn.pending ? ` · ${uefn.pending}` : ''}</button>
          <AiPill />
          <span className="top-spacer" />
          {app.map && <OuvrirForge mapId={app.map.id} compact />}
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
