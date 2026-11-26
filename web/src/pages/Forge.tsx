// POP Forge (#78) : interface de l'application de bureau, centrée sur la construction avec l'IA (maquettes validées
// par Kl1600 le 24/11). Servie par POP sur /forge et affichée dans Forge ; elle parle au relais UEFN Link de Forge par
// ses commandes (projets, état, liaison). Forge ne reprend aucun écran de gestion du site (décision du 24/11) :
// « Ouvrir POP » et « Voir dans POP » ouvrent le site dans le navigateur (Forge y renvoie toute page autre que /forge).
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Bell, Bot, Building2, Check, ChevronsUpDown, ExternalLink, Home, Library, Minus, Moon, RefreshCw, ShieldCheck, Square, Sun, User, Wrench, X } from 'lucide-react'
import LangPicker from '../components/LangPicker'
import { ForgeAgents } from '../components/ForgeAgents'
import { Selecteur } from '../components/Selecteur'
import { useApp, studioName, useLive, useLoad } from '../state'
import { ago, api } from '../api'
import { tr } from '../i18n/t'
import { ForgeBibliotheque } from '../components/ForgeBibliotheque'
import { ForgeVerse } from '../components/ForgeVerse'
import { ForgeVerifications } from '../components/ForgeVerifications'
import { ForgeScenarios } from '../components/ForgeScenarios'
import { cssRect } from '../zoom'

type Invoke = <T = any>(cmd: string, args?: Record<string, unknown>) => Promise<T>
const forgeInvoke = (): Invoke | null => {
  const w = window as any
  return w.__TAURI__?.core?.invoke ?? w.__TAURI_INTERNALS__?.invoke ?? null
}
type Liaison = { projet: string; map: string; serveur: string; liens?: { projet: string; map: string }[] } | null
type Section = 'accueil' | 'agents' | 'verifier' | 'bibliotheque' | 'outils' | 'reglages'

const SECTIONS: [Section, string, any][] = [
  ['accueil', tr("Accueil"), Home],
  ['agents', tr("Agents"), Bot],
  ['verifier', tr("Vérifications"), ShieldCheck],
  ['bibliotheque', tr("Bibliothèque"), Library],
  ['outils', tr("Outils"), Wrench],
]
// Écrans encore vides : ce qu'ils feront et les issues qui les rempliront (critère 7 de #78).
const A_VENIR: Partial<Record<Section, [string, string]>> = {
  bibliotheque: [tr("Tout ce que le studio réutilise : modules Verse, 3D, images et sons, installés en un clic."), '#86, #92, #382'],
  outils: [tr("Assets inutilisés, ambiances lumineuses, renommer et supprimer sans risque."), tr("issues à créer après les écrans principaux")],
}

const ROLES: Record<string, string> = { owner: tr("propriétaire"), admin: tr("admin"), member: tr("membre"), viewer: tr("invité") }
const nomDossier = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p
const memeServeur = (a: string, b: string) => a.replace(/\/+$/, '').toLowerCase() === b.replace(/\/+$/, '').toLowerCase()
/** Ouvre une page du site : Forge la renvoie vers le navigateur (garde de navigation). */
const ouvrirPOP = (chemin: string) => { window.location.href = chemin }

export default function Forge() {
  const app = useApp()
  const navigate = useNavigate()
  const invoke = forgeInvoke()
  const [section, setSection] = useState<Section>('accueil')
  const [etat, setEtat] = useState('…')
  const [liaison, setLiaison] = useState<Liaison>(null)

  const lire = useCallback(async () => {
    if (!invoke) return
    try {
      setEtat(await invoke<string>('etat_relais'))
      setLiaison(await invoke<Liaison>('liaison'))
    } catch { /* relais indisponible : on réessaie au prochain tour */ }
  }, [invoke])
  useEffect(() => { lire(); const t = setInterval(lire, 2000); return () => clearInterval(t) }, [lire])
  // F5 recharge Forge, comme l'ancien menu de Windows.
  useEffect(() => {
    const f = (e: KeyboardEvent) => { if (e.key === 'F5') { e.preventDefault(); location.reload() } }
    window.addEventListener('keydown', f); return () => window.removeEventListener('keydown', f)
  }, [])

  // Ouverte dans un navigateur : page de téléchargement de Forge (#80).
  if (!invoke) return <Telecharger />

  if (app.me && app.me.forge === false) return <ForgePro />

  const relie = !!liaison && memeServeur(liaison.serveur, location.origin) && etat !== 'UEFN non relié'
  const projetCourant = (liaison?.liens ?? []).find(l => app.map && l.map.toLowerCase() === app.map.name.toLowerCase())?.projet
  const titre = section === 'reglages' ? tr("Réglages") : SECTIONS.find(s => s[0] === section)?.[1]

  return <div className="forge-shell">
  <TitleBar invoke={invoke} />
  <div className={'app forge' + (app.dark ? ' dark' : '')}>
    <aside className="side">
      <div className="brand" style={{ flexDirection: 'column', alignItems: 'center', gap: 4 }}>
        <img src="/logo.png" alt="POP" style={{ height: 40 }} />
        <span style={{ fontSize: 12, letterSpacing: '.14em', textTransform: 'uppercase', color: 'rgb(163,163,163)' }}>Forge</span>
      </div>
      <MapPicker />
      <nav className="nav" aria-label="Forge">
        {SECTIONS.map(([k, label, Icon]) =>
          <a key={k} href="#" className={section === k ? 'on' : ''} aria-current={section === k ? 'page' : undefined} onClick={e => { e.preventDefault(); setSection(k) }}>
            <Icon size={20} strokeWidth={1.5} /><span className="lbl">{label}</span>
          </a>)}
      </nav>
      <div style={{ flex: 1 }} />
      <nav className="nav">
        <a href="/" title={tr("S'ouvre dans le navigateur")} onClick={e => { e.preventDefault(); ouvrirPOP(app.map ? `/map/${app.map.id}` : '/') }}><ExternalLink size={20} strokeWidth={1.5} /><span className="lbl">{tr("Ouvrir POP")}</span></a>
      </nav>
      <ProfileMenu invoke={invoke} onReglages={() => setSection('reglages')} actif={section === 'reglages'} />
    </aside>
    <div className="main">
      <div className="top" style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
        <div style={{ fontSize: 17, display: 'flex', gap: 8 }}><span className="muted">Forge</span><span className="muted">/</span><span>{titre}</span></div>
        <span className="badge" title={tr("État du relais")}><span aria-hidden="true" style={{ width: 8, height: 8, borderRadius: '50%', background: relie ? 'var(--tg)' : 'var(--red)' }} />{relie && projetCourant ? nomDossier(projetCourant) + ' · ' + tr(etat) : relie ? tr(etat) : tr(etat)}</span>
        <div style={{ flex: 1 }} />
        <Cloche />
        <button className="iconbtn" aria-label={app.dark ? tr("Thème clair") : tr("Thème sombre")} onClick={app.toggleDark}>{app.dark ? <Sun size={20} strokeWidth={1.5} /> : <Moon size={20} strokeWidth={1.5} />}</button>
      </div>
      <main className="content" style={{ padding: '40px 28px', width: '100%', maxWidth: 920, margin: '0 auto', boxSizing: 'border-box' }}>
        {app.map && section !== 'reglages' && <Bandeau mapId={app.map.id} etat={etat} aller={setSection} />}
        {section === 'accueil' ? <Accueil invoke={invoke} etat={etat} liaison={liaison} relie={relie} relire={lire} />
          : section === 'reglages' ? <Reglages />
          : section === 'agents' ? (app.map ? <ForgeAgents mapId={app.map.id} /> : <p className="muted">{tr("Choisissez une map.")}</p>)
          : section === 'verifier' ? (app.map ? <div className="stack" style={{ gap: 18 }}><ForgeVerifications mapId={app.map.id} ouvrir={ouvrirPOP} /><ForgeScenarios mapId={app.map.id} capturer={() => captureJeu(invoke)} /></div> : <p className="muted">{tr("Choisissez une map.")}</p>)
          : section === 'bibliotheque' ? <Bibliotheque mapId={app.map?.id ?? null} studioId={app.studio?.id ?? null} />
            : <AVenir section={section} titre={titre ?? ''} />}
      </main>
    </div>
  </div>
  </div>
}

/** Barre de titre de Forge (retour de Kl1600 du 24/11) : remplace le cadre et le menu de Windows. Glisser déplace la
 * fenêtre, un double-clic l'agrandit ; boutons réduire, agrandir et fermer à droite, comme sous Windows. */
function TitleBar({ invoke }: { invoke: Invoke }) {
  const fen = (action: string) => { invoke('fenetre', { action }).catch(() => {}) }
  return <div className="forge-titlebar" onMouseDown={e => { if (e.button === 0 && e.detail === 1 && !(e.target as HTMLElement).closest('button')) fen('deplacer') }}
    onDoubleClick={e => { if (!(e.target as HTMLElement).closest('button')) fen('agrandir') }}>
    <span className="forge-titlebar-nom">POP Forge</span>
    <div style={{ flex: 1 }} />
    <button aria-label={tr("Réduire")} title={tr("Réduire")} onClick={() => fen('reduire')}><Minus size={16} strokeWidth={1.5} /></button>
    <button aria-label={tr("Agrandir")} title={tr("Agrandir")} onClick={() => fen('agrandir')}><Square size={13} strokeWidth={1.5} /></button>
    <button className="fermer" aria-label={tr("Fermer")} title={tr("Fermer")} onClick={() => fen('fermer')}><X size={17} strokeWidth={1.5} /></button>
  </div>
}

/** Sélecteur de map sur le modèle de celui du site (groupes perso et studios, map ouverte cochée). Forge ne gère pas
 * les maps (décision du 24/11) : créer, renommer ou déplacer se fait dans POP. */
function MapPicker() {
  const app = useApp()
  const [ouvert, setOuvert] = useState(false)
  const btn = useRef<HTMLButtonElement>(null)
  const pop = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState({ top: 0, left: 0, width: 280 })
  useEffect(() => {
    if (!ouvert || !btn.current) return
    const r = cssRect(btn.current)
    setPos({ top: r.bottom + 6, left: r.left, width: Math.max(280, r.width) })
    const down = (e: MouseEvent) => { if (!pop.current?.contains(e.target as Node) && !btn.current?.contains(e.target as Node)) setOuvert(false) }
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') setOuvert(false) }
    document.addEventListener('mousedown', down); document.addEventListener('keydown', key)
    return () => { document.removeEventListener('mousedown', down); document.removeEventListener('keydown', key) }
  }, [ouvert])
  const studios = app.me?.studios ?? []
  if (!studios.length) return null
  return <div className="mapsw lbl">
    <button ref={btn} className="mapsw-btn" onClick={() => setOuvert(o => !o)} aria-haspopup="menu" aria-expanded={ouvert} title={tr("Changer de map")}>
      <span className="mapsw-txt"><b>{app.map?.name ?? tr("Choisir une map")}</b><small>{app.studio ? studioName(app.studio) : ''}</small></span>
      <ChevronsUpDown size={16} strokeWidth={1.75} />
    </button>
    {ouvert && <div ref={pop} className="mapsw-pop" role="menu" style={{ top: pos.top, left: pos.left, width: pos.width }}>
      {studios.map(s => <div key={s.id} className="mapsw-group">
        <div className="mapsw-studio">
          <span className="mapsw-gico">{s.personal ? <User size={14} /> : <Building2 size={14} />}</span>
          <span className="mapsw-gname">{s.personal ? tr('Mes maps (seul)') : s.name}</span>
          {!s.personal && <span className="mapsw-role">{ROLES[s.role] ?? s.role}</span>}
          <span className="mapsw-count">{s.maps.length}</span>
        </div>
        {s.maps.map(m => { const cur = m.id === app.map?.id; return <div key={m.id} className="mapsw-row">
          <button role="menuitem" aria-current={cur ? 'true' : undefined} className={'mapsw-item' + (cur ? ' on' : '')} onClick={() => { setOuvert(false); app.setMapId(m.id) }}>
            <span>{m.name}</span>{cur && <em className="mapsw-cur"><Check size={13} />{tr('Ouverte')}</em>}
          </button>
        </div> })}
        {s.maps.length === 0 && <div className="mapsw-empty">{s.personal ? tr('Les maps que vous faites seul.') : tr('Aucune map dans ce studio.')}</div>}
      </div>)}
      <div className="mapsw-foot">
        <button role="menuitem" className="mapsw-item" onClick={() => { setOuvert(false); ouvrirPOP('/') }}><ExternalLink size={15} /><span>{tr("Gérer les maps dans POP")}</span></button>
      </div>
    </div>}
  </div>
}

/** Profil en bas de la barre latérale, avec son menu (comme sur le site) : Réglages y est rangé. */
function ProfileMenu({ invoke, onReglages, actif }: { invoke: Invoke; onReglages: () => void; actif: boolean }) {
  const app = useApp()
  const [ouvert, setOuvert] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!ouvert) return
    const f = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOuvert(false) }
    document.addEventListener('mousedown', f); return () => document.removeEventListener('mousedown', f)
  }, [ouvert])
  const choisir = (f: () => void) => () => { setOuvert(false); f() }
  return <div ref={ref} style={{ position: 'relative' }}>
    {ouvert && <div role="menu" className="forge-menu">
      <button role="menuitem" onClick={choisir(onReglages)}>{tr("Réglages")}</button>
      <button role="menuitem" onClick={choisir(() => ouvrirPOP('/parametres?onglet=ia'))}>{tr("IA connectées")}<ExternalLink size={14} /></button>
      <button role="menuitem" onClick={choisir(app.toggleDark)}>{tr("Thème")}<span>{app.dark ? tr("Sombre") : tr("Clair")}</span></button>
      <hr />
      <button role="menuitem" onClick={choisir(() => invoke('action_forge', { id: 'etat' }).catch(() => {}))}>{tr("Page d'état du relais")}</button>
      <button role="menuitem" onClick={choisir(() => invoke('action_forge', { id: 'changer' }).catch(() => {}))}>{tr("Changer l'adresse de POP")}</button>
      <button role="menuitem" onClick={choisir(() => location.reload())}>{tr("Recharger")}<span>F5</span></button>
      <hr />
      <button role="menuitem" onClick={choisir(() => ouvrirPOP('/parametres'))}>{tr("Mon compte dans POP")}<ExternalLink size={14} /></button>
      <button role="menuitem" onClick={choisir(async () => { await api.post('/api/auth/logout'); location.reload() })}>{tr("Se déconnecter")}</button>
    </div>}
    <button className={'forge-profil' + (ouvert || actif ? ' on' : '')} aria-haspopup="menu" aria-expanded={ouvert} onClick={() => setOuvert(o => !o)}>
      <span aria-hidden="true" className="forge-avatar">{(app.me?.user.name ?? '?').slice(0, 1).toUpperCase()}</span>
      <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', lineHeight: 1.25, textAlign: 'left' }}>
        <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{app.me?.user.name}</span>
        <span style={{ fontSize: 12, color: 'rgb(163,163,163)' }}>{app.me?.plan === 'pro' ? tr("Abonnement Pro") : tr("Compte POP")}</span>
      </span>
      <ChevronsUpDown size={16} color="rgb(163,163,163)" />
    </button>
  </div>
}

/** Cloche : les notifications du compte (les mêmes que sur le site), lues dans Forge ; seul le lien d'une
 * notification ouvre POP dans le navigateur (retour de Kl1600 du 24/11). */
function Cloche() {
  const { data, reload } = useLoad<{ unread: number; items: { id: number; text: string; link: string; read: boolean; at: string }[] }>('/api/notifications', [])
  const [ouvert, setOuvert] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => { const t = setInterval(reload, 30000); return () => clearInterval(t) }, [reload])
  useEffect(() => {
    if (!ouvert) return
    reload()
    const f = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOuvert(false) }
    const k = (e: KeyboardEvent) => { if (e.key === 'Escape') setOuvert(false) }
    document.addEventListener('mousedown', f); document.addEventListener('keydown', k)
    return () => { document.removeEventListener('mousedown', f); document.removeEventListener('keydown', k) }
  }, [ouvert])
  const n = data?.unread ?? 0
  const toutLu = async () => { await api.post('/api/notifications/read').catch(() => {}); reload() }
  return <div ref={ref} style={{ position: 'relative' }}>
    <button className="iconbtn" style={{ position: 'relative' }} aria-haspopup="dialog" aria-expanded={ouvert} aria-label={n ? tr("Notifications : {0} nouvelle(s)", [n]) : tr("Notifications")} onClick={() => setOuvert(o => !o)}>
      <Bell size={20} strokeWidth={1.5} />
      {n > 0 && <span aria-hidden="true" style={{ position: 'absolute', top: 8, right: 9, width: 8, height: 8, borderRadius: '50%', background: '#ED753E' }} />}
    </button>
    {ouvert && <div className="forge-notifs" role="dialog" aria-label={tr("Notifications")}>
      <div className="forge-notifs-tete"><b>{tr("Notifications")}</b>{n > 0 && <button className="linkbtn" onClick={toutLu}>{tr("Tout marquer comme lu")}</button>}</div>
      {!data?.items.length ? <p className="muted small" style={{ margin: 0, padding: '18px 14px' }}>{tr("Aucune notification.")}</p>
        : <div className="forge-notifs-liste">{data.items.slice(0, 30).map(x => <div key={x.id} className={'forge-notif' + (x.read ? '' : ' nouv')}>
          <span aria-hidden="true" className="forge-notif-point" />
          <span style={{ flex: 1, minWidth: 0 }}><span>{x.text}</span><span className="small muted" style={{ display: 'block' }}>{ago(x.at)}</span></span>
          {x.link && <button className="iconbtn" title={tr("Ouvrir dans POP (navigateur)")} aria-label={tr("Ouvrir dans POP (navigateur)")} onClick={() => ouvrirPOP(x.link)}><ExternalLink size={15} /></button>}
        </div>)}</div>}
    </div>}
  </div>
}

function AVenir({ section, titre }: { section: Section; titre: string }) {
  const [texte, issues] = A_VENIR[section] ?? ['', '']
  return <div className="stack">
    <h1 className="forge-h1">{titre}</h1>
    <p className="muted" style={{ margin: 0 }}>{texte}</p>
    <p className="small muted" style={{ margin: 0 }}>{tr("Écran à venir")} · {issues}</p>
  </div>
}

function Reglages() {
  const app = useApp()
  const ligne = (label: string, detail: string, action: React.ReactNode) =>
    <div className="li"><span style={{ flex: 1, display: 'flex', flexDirection: 'column', lineHeight: 1.35 }}><span>{label}</span>{detail && <span className="small muted">{detail}</span>}</span>{action}</div>
  return <div className="stack" style={{ gap: 28 }}>
    <h1 className="forge-h1">{tr("Réglages")}</h1>
    <section>
      <h2 className="forge-h2">{tr("Compte")}</h2>
      {ligne(app.me?.user.name ?? '', app.me?.user.email ?? '', <button className="linkbtn" onClick={() => ouvrirPOP('/parametres')}>{tr("Gérer dans POP")}</button>)}
      {ligne(tr("IA connectées"), tr("Les mêmes que dans POP : clés, rôles, lecture seule"), <button className="linkbtn" onClick={() => ouvrirPOP('/parametres?onglet=ia')}>{tr("Gérer dans POP")}</button>)}
    </section>
    <section>
      <h2 className="forge-h2">Forge</h2>
      {ligne(tr("Langue"), '', <LangPicker app />)}
      {ligne(tr("Thème"), '', <button className="btn" onClick={app.toggleDark}>{app.dark ? tr("Sombre") : tr("Clair")}</button>)}
      {ligne(tr("Mises à jour"), tr("Proposées automatiquement, à venir (#79)"), null)}
      {ligne(tr("Diagnostic"), tr("État de Forge, du relais et de la map, à coller dans une demande d’aide"), <button className="btn" onClick={async () => {
        const d = app.map ? await api.get(`/api/maps/${app.map.id}/diagnostic`).catch(() => null) : null
        const txt = [`POP Forge · ${navigator.userAgent}`, `Serveur : ${location.origin}`, `Map : ${app.map?.name ?? '—'}`, ...(d?.problems ?? []).map((p: any) => `- ${p.title}`)].join('\n')
        try { await navigator.clipboard.writeText(txt); app.toast(tr("Diagnostic copié")) } catch { app.toast(txt) }
      }}>{tr("Copier le diagnostic")}</button>)}
    </section>
  </div>
}

/** Accueil (retour de Kl1600 du 24/11) : toutes les maps du compte, celles du site, chacune avec son projet UEFN ;
 * relier une map à un projet trouvé tout seul (Documents, projets récents d'UEFN, projet ouvert) ou choisi avec
 * « Parcourir… » ; plusieurs projets reliés à la fois. Forge ne crée pas de map : « Créer la map dans POP » ouvre le
 * site (critère 7 de #78). */
function Accueil({ invoke, liaison, relire }: { invoke: Invoke; etat: string; liaison: Liaison; relie: boolean; relire: () => Promise<void> }) {
  const app = useApp()
  const [ouvert, setOuvert] = useState<string | null>(null)
  const [projets, setProjets] = useState<string[]>([])
  const [occupe, setOccupe] = useState<string | null>(null)
  const [msg, setMsg] = useState<{ ok: boolean; texte: string } | null>(null)
  const [choix, setChoix] = useState<Record<number, string>>({})
  const [ouverte, setOuverte] = useState<number | null>(null)

  const chercher = useCallback(async () => {
    try {
      const [o, l] = await Promise.all([invoke<string | null>('projet_ouvert'), invoke<string[]>('projets')])
      setOuvert(o); setProjets([...new Set([...(o ? [o] : []), ...l])])
    } catch (e: any) { setMsg({ ok: false, texte: String(e) }) }
  }, [invoke])
  useEffect(() => { chercher() }, [chercher])

  const studios = (app.me?.studios ?? []).filter(s => ['member', 'admin', 'owner'].includes(s.role))
  const maps = studios.flatMap(s => s.maps.map(m => ({ id: m.id, nom: m.name, studio: studioName(s) })))
  const liens = (liaison && memeServeur(liaison.serveur, location.origin) ? liaison.liens ?? [] : [])
  const projetDe = (nom: string) => liens.find(l => l.map.toLowerCase() === nom.toLowerCase())?.projet
  const relies = new Set(liens.map(l => l.projet.toLowerCase()))
  const meme = (p: string, nom: string) => nomDossier(p).toLowerCase() === nom.toLowerCase()
  // Projets trouvés sans map POP du même nom ni liaison : on propose de créer leur map.
  const orphelins = projets.filter(p => !relies.has(p.toLowerCase()) && !maps.some(m => meme(p, m.nom)))
  const proposer = (nom: string) => choix[maps.find(m => m.nom === nom)?.id ?? -1] ?? projets.find(p => meme(p, nom)) ?? (ouvert && !relies.has(ouvert.toLowerCase()) ? ouvert : '') ?? ''

  const relier = async (mapId: number, projet: string) => {
    setOccupe(String(mapId)); setMsg(null)
    try {
      const { code } = await api.post<{ code: string }>(`/api/maps/${mapId}/relay/pair`)
      const e = await invoke<string>('relier', { code, projet })
      setMsg({ ok: true, texte: tr("Relié") + ' : ' + nomDossier(projet) + ' · ' + tr(e) })
      setOuverte(null); await relire()
    } catch (e: any) { setMsg({ ok: false, texte: e?.message ?? String(e) }) } finally { setOccupe(null) }
  }
  const parcourir = async (mapId: number) => {
    const d = await invoke<string | null>('choisir_dossier').catch(() => null)
    if (d) setChoix(c => ({ ...c, [mapId]: d }))
  }

  return <div className="stack" style={{ gap: 32 }}>
    <h1 className="forge-h1">{tr("Bonjour")} {app.me?.user.name}</h1>

    <section className="stack" style={{ gap: 10 }}>
      <div className="row" style={{ justifyContent: 'space-between', flexWrap: 'wrap', gap: 8 }}>
        <h2 className="forge-h2" style={{ margin: 0 }}>{tr("Vos maps et leurs projets UEFN")}</h2>
        <div className="row" style={{ gap: 8 }}>
          <button className="btn" onClick={() => { chercher(); app.reload(); relire() }}><RefreshCw size={16} strokeWidth={1.5} />{tr("Actualiser")}</button>
          <button className="btn" title={tr("S'ouvre dans le navigateur")} onClick={() => ouvrirPOP('/')}><ExternalLink size={16} strokeWidth={1.5} />{tr("Créer la map dans POP")}</button>
        </div>
      </div>
      <p className="small muted" style={{ margin: 0 }}>{tr("Les mêmes maps que sur le site. Reliez chacune à son projet UEFN : Forge cherche les projets dans vos Documents et parmi ceux ouverts récemment dans UEFN.")}</p>
      <div className="forge-maps">
        {maps.length === 0 && <p className="muted" style={{ margin: 0, padding: 14 }}>{tr("Aucune map pour l'instant : créez-la dans POP, elle apparaîtra ici.")}</p>}
        {maps.map(m => {
          const p = projetDe(m.nom)
          const sel = proposer(m.nom)
          return <div key={m.id} className="forge-map-ligne">
            <div className="row" style={{ gap: 12, flexWrap: 'wrap' }}>
              <span aria-hidden="true" className="forge-point" style={{ background: p ? 'var(--tg)' : 'var(--text3)' }} />
              <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
                <span>{m.nom} <span className="small muted">· {m.studio}</span></span>
                <span className="small muted">{p ? tr("Reliée à {0}", [nomDossier(p)]) : tr("Pas encore reliée à un projet UEFN")}</span>
              </span>
              <button className={'btn' + (p ? '' : ' pri')} onClick={() => setOuverte(ouverte === m.id ? null : m.id)}>{p ? tr("Changer de projet") : tr("Relier")}</button>
            </div>
            {ouverte === m.id && <div className="forge-ligne">
              <Selecteur label={tr("Projet UEFN pour {0}", [m.nom])} placeholder={tr("Choisir le projet UEFN…")} value={sel || undefined}
                options={[...new Set([...(sel ? [sel] : []), ...projets])].map(x => ({ value: x, label: nomDossier(x) + (ouvert === x ? ' · ' + tr("ouvert dans UEFN") : '') + (relies.has(x.toLowerCase()) ? ' · ' + tr("déjà relié") : '') }))}
                onChange={v => setChoix(c => ({ ...c, [m.id]: v }))} />
              <button className="btn" onClick={() => parcourir(m.id)}>{tr("Parcourir…")}</button>
              <button className="btn pri" disabled={!sel || occupe !== null} onClick={() => relier(m.id, sel)}>{occupe === String(m.id) ? tr("Liaison…") : tr("Relier")}</button>
            </div>}
          </div>
        })}
      </div>
      {orphelins.length > 0 && <div className="stack" style={{ gap: 6 }}>
        <span className="small muted">{tr("Projets UEFN trouvés sur ce PC sans map POP : créez leur map dans POP, puis reliez-la ici.")}</span>
        {orphelins.map(o => <div key={o} className="forge-map-ligne row" style={{ gap: 12, flexWrap: 'wrap' }}>
          <span style={{ flex: 1, minWidth: 0 }}>{nomDossier(o)}{ouvert === o && <span className="small muted"> · {tr("ouvert dans UEFN")}</span>}</span>
          <button className="btn" title={tr("S'ouvre dans le navigateur")} onClick={() => ouvrirPOP('/')}><ExternalLink size={16} strokeWidth={1.5} />{tr("Créer la map dans POP")}</button>
        </div>)}
      </div>}
      {projets.length === 0 && <p className="small muted" style={{ margin: 0 }}>{tr("Aucun projet UEFN trouvé sur ce PC. Ouvrez votre projet dans UEFN, puis « Actualiser », ou reliez une map avec « Parcourir… ».")}</p>}
      {msg && <p className="small" style={{ margin: 0, color: msg.ok ? 'var(--text)' : 'var(--red)' }}>{msg.texte}</p>}
    </section>

    {app.map && <EnCeMoment mapId={app.map.id} />}
  </div>
}

/** Les agents de la map et leur tâche (registre #93), avec « Voir dans POP » sur chaque tâche. */
function EnCeMoment({ mapId }: { mapId: number }) {
  const { data, reload } = useLoad<any[]>(`/api/maps/${mapId}/agents`, [mapId])
  // Résumés en français fabriqués par POP à partir du journal MCP (#321) : équipe, agents, tâches terminées.
  const { data: res, reload: relireRes } = useLoad<any>(`/api/maps/${mapId}/resumes`, [mapId])
  useLive(['agents', 'tasks', 'fil'], () => { reload(); relireRes() })
  if (!data) return null
  const phraseDe = (id: number) => res?.agents?.find((x: any) => x.id === id)?.phrase
  return <section>
    <h2 className="forge-h2">{tr("En ce moment")}</h2>
    {res?.equipe?.phrase && <p style={{ margin: '0 0 8px' }}>{res.equipe.phrase}</p>}
    {data.length === 0 && <p className="muted" style={{ margin: 0 }}>{tr("Aucune IA ne travaille sur cette map.")}</p>}
    {data.map(a => <div key={a.id} className="li">
      <span aria-hidden="true" style={{ width: 8, height: 8, borderRadius: '50%', background: a.task ? 'var(--tg)' : 'var(--text3)' }} />
      <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', lineHeight: 1.35 }}>
        <span>{a.task ? `#${a.task.number} ${a.task.title}` : tr("Pas de tâche réservée")}</span>
        <span className="small muted">{a.name}{phraseDe(a.id) ? ' · ' + phraseDe(a.id) : a.last_action ? ' · ' + a.last_action : ''}</span>
      </span>
      {a.task && <button className="linkbtn" onClick={() => ouvrirPOP(`/map/${mapId}/taches?q=%23${a.task.number}`)}>{tr("Voir dans POP")}</button>}
    </div>)}
    {(res?.taches ?? []).slice(0, 3).map((x: any) => <div key={x.number} className="li">
      <span aria-hidden="true" style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--text3)' }} />
      <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', lineHeight: 1.35 }}>
        <span>#{x.number} {x.title}</span>
        <span className="small muted">{x.par ? x.par + ' · ' : ''}{x.phrase}</span>
      </span>
    </div>)}
  </section>
}

/** Forge réservé à l'abonnement Pro, essai compris (#102) : se débloque sans redémarrer dès que l'abonnement est actif. */
function ForgePro() {
  const app = useApp()
  useEffect(() => { const t = setInterval(() => app.reload(), 10000); return () => clearInterval(t) }, [])
  return <div className={'app forge' + (app.dark ? ' dark' : '')}>
    <div className="main" style={{ margin: 12, padding: 32, display: 'grid', placeItems: 'center' }}>
      <div className="card" style={{ maxWidth: 560, padding: 28 }}>
        <h1 className="forge-h1">{tr("Forge est inclus dans l’abonnement Pro")}</h1>
        <p className="muted">{tr("POP Forge construit votre map dans UEFN avec vos IA : agents, liaison au projet, outils. Il est réservé à l’abonnement Pro, essai gratuit compris. Vos IA connectées au site continuent de marcher sans Pro.")}</p>
        <div className="forge-ligne">
          <button className="btn pri" onClick={() => ouvrirPOP('/parametres')}>{tr("Voir l’abonnement Pro")}</button>
          <button className="btn" onClick={() => app.reload()}>{tr("J’ai changé d’abonnement")}</button>
        </div>
        <p className="small muted" style={{ marginBottom: 0 }}>{tr("Forge se débloque tout seul dès que l’abonnement est actif.")}</p>
      </div>
    </div>
  </div>
}

/** États et dépannage (#300) : chaque problème de la map avec son action et l'aide en un clic. */
function Bandeau({ mapId, etat, aller }: { mapId: number; etat: string; aller: (s: any) => void }) {
  const app = useApp()
  const { data, reload } = useLoad<any>(`/api/maps/${mapId}/diagnostic`, [mapId, etat])
  const [aide, setAide] = useState<string | null>(null)
  useEffect(() => app.subscribe(m => { if (m?.t === 'uefn' || m?.t === 'agent' || m?.t === 'relay') reload() }), [mapId])
  useEffect(() => { const t = setInterval(reload, 30000); return () => clearInterval(t) }, [mapId])
  const pbs: any[] = data?.problems ?? []
  if (!pbs.length) return null
  const agir = (p: any) => {
    if (p.action === 'accueil') aller('accueil')
    else if (p.action === 'verifier' || p.action === 'construire') aller('verifier')
    else if (p.action === 'agents') aller('agents')
    else if (p.action === 'reessayer') reload()
    else setAide(aide === p.code ? null : p.code)
  }
  return <div className="stack" style={{ gap: 8, marginBottom: 22 }} role="status" aria-label={tr("État de Forge")}>
    {pbs.map((p, i) => <div key={i} className="card" style={{ padding: '10px 14px', borderColor: 'var(--red)' }}>
      <div className="forge-ligne">
        <span className="forge-point" style={{ background: 'var(--red)' }} aria-hidden="true" />
        <span style={{ flex: 1 }}>{tr(p.title)}</span>
        <button className="btn" onClick={() => agir(p)}>{tr(p.label)}</button>
        <button className="linkbtn" onClick={() => setAide(aide === p.code + i ? null : p.code + i)}>{tr("Pourquoi ?")}</button>
      </div>
      {(aide === p.code + i || aide === p.code) && <p className="small" style={{ margin: '8px 0 0' }}>{tr(p.help)}</p>}
    </div>)}
  </div>
}

/** Page de téléchargement de POP Forge (#80), quand /forge s'ouvre dans un navigateur. */
function Telecharger() {
  const app = useApp()
  const navigate = useNavigate()
  const [info, setInfo] = useState<any>(null)
  const [refus, setRefus] = useState('')
  useEffect(() => { api.get('/api/forge/download').then(setInfo).catch((e: any) => setRefus(e.message)) }, [])
  return <div className={'app' + (app.dark ? ' dark' : '')}>
    <div className="main" style={{ margin: 12, padding: 32, overflow: 'auto' }}>
      <div className="stack" style={{ maxWidth: 680, margin: '0 auto', gap: 18 }}>
        <h1 style={{ margin: 0, fontWeight: 500 }}>{tr("Télécharger POP Forge")}</h1>
        <p className="muted" style={{ margin: 0 }}>{tr("L’application Windows de POP : vos IA construisent la map dans UEFN, vous suivez et pilotez tout au même endroit.")}</p>
        <section className="card" style={{ padding: 20 }}>
          {refus ? <>
            <p style={{ marginTop: 0 }}>{refus}</p>
            <button className="btn pri" onClick={() => navigate('/parametres')}>{tr("Voir l’abonnement Pro")}</button>
          </> : !info ? <p className="muted" style={{ margin: 0 }}>{tr("Chargement…")}</p> : !info.available ? <p style={{ margin: 0 }}>{tr("Aucune version publiée pour l’instant : revenez bientôt.")}</p> : <>
            <div className="forge-ligne" style={{ justifyContent: 'space-between' }}>
              <div><b>{tr("POP Forge {0}", [info.version ?? ''])}</b><div className="small muted">{tr("Windows 10 ou 11, 64 bits · {0} Mo · publiée le {1}", [Math.max(1, Math.round(info.size / 1e6)), info.date ?? '—'])}</div></div>
              <a className="btn pri" href={info.url} download>{tr("Télécharger l’installateur")}</a>
            </div>
          </>}
        </section>
        <section className="card" style={{ padding: 20 }}>
          <h2 style={{ marginTop: 0, fontSize: 17, fontWeight: 500 }}>{tr("Configuration requise")}</h2>
          <ul style={{ margin: 0, paddingLeft: 18, lineHeight: 1.7 }}>
            <li>{tr("Windows 10 ou 11, 64 bits.")}</li>
            <li>{tr("Unreal Editor for Fortnite (UEFN) installé, avec son serveur MCP pour piloter le projet.")}</li>
            <li>{tr("Un compte POP avec l’abonnement Pro (essai gratuit compris).")}</li>
          </ul>
        </section>
        <section className="card" style={{ padding: 20 }}>
          <h2 style={{ marginTop: 0, fontSize: 17, fontWeight: 500 }}>{tr("Si Windows affiche « Windows a protégé votre ordinateur »")}</h2>
          <p style={{ marginTop: 0 }}>{tr("Pendant la bêta, l’installateur n’est pas encore signé : Windows SmartScreen le signale comme une application inconnue. C’est normal.")}</p>
          <ol style={{ margin: 0, paddingLeft: 18, lineHeight: 1.7 }}>
            <li>{tr("Cliquez sur « Informations complémentaires ».")}</li>
            <li>{tr("Vérifiez que l’éditeur indiqué est « POP » ou « Éditeur inconnu » et que le fichier s’appelle pop-forge-setup.exe.")}</li>
            <li>{tr("Cliquez sur « Exécuter quand même », puis suivez l’installation.")}</li>
            <li>{tr("Au premier lancement, entrez l’adresse de POP : pop-uefn.com, puis connectez-vous.")}</li>
          </ol>
        </section>
        <p className="small muted" style={{ margin: 0 }}>{tr("Sans Forge, vous pouvez toujours relier UEFN avec le relais seul : POP → IA connectées → pop-link.exe.")}</p>
        <div><button className="btn" onClick={() => navigate('/')}>{tr("Retour à POP")}</button></div>
      </div>
    </div>
  </div>
}

/** Bibliothèque de Forge : modules Verse (#86) et assets (#92, #382). */
function Bibliotheque({ mapId, studioId }: { mapId: number | null; studioId: number | null }) {
  const [vue, setVue] = useState<'verse' | 'assets'>('verse')
  return <div className="stack" style={{ gap: 18 }}>
    <div className="tabs" role="tablist" aria-label={tr("Bibliothèque")}>
      <button role="tab" aria-selected={vue === 'verse'} className={vue === 'verse' ? 'on' : ''} onClick={() => setVue('verse')}>{tr("Modules Verse")}</button>
      <button role="tab" aria-selected={vue === 'assets'} className={vue === 'assets' ? 'on' : ''} onClick={() => setVue('assets')}>{tr("Assets")}</button>
    </div>
    {vue === 'verse' ? <ForgeVerse mapId={mapId} studioId={studioId} /> : <ForgeBibliotheque mapId={mapId} studioId={studioId} />}
  </div>
}

/** Photo de l'écran du jeu par Forge (#350) : PNG en base64 → Blob prêt à envoyer au stockage. */
async function captureJeu(invoke: Invoke): Promise<Blob> {
  const b64 = await invoke('capture_jeu') as string
  const bin = atob(b64)
  const o = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) o[i] = bin.charCodeAt(i)
  return new Blob([o], { type: 'image/png' })
}
