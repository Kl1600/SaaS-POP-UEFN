import { useEffect, useRef, useState } from 'react'
import { ago, api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr } from '../i18n/t'
import { Selecteur } from './Selecteur'
import { ExternalLink, Plus, X } from 'lucide-react'

// Écran Agents de Forge (#94) : les IA connectées à la map, leur tâche et son étape, leur résumé en français (#321),
// leur journal d'appels MCP en direct ; attribuer une tâche, passer en lecture seule ou couper un agent.
const ETAPES: Record<string, string> = { ecrit: tr("écrit"), compile: tr("compilé"), teste: tr("testé"), relu: tr("relu") }
const MODES: [string, string][] = [['write', tr("Agit")], ['read', tr("Lecture seule")], ['off', tr("Coupé")]]
const ROLES: Record<string, string> = { orchestrateur: tr("Orchestrateur"), codeur_verse: tr("Codeur Verse"), level_designer: tr("Level designer"), ui: tr("Interface"), testeur: tr("Testeur"), relecteur: tr("Relecteur"), analyste: tr("Analyste") }
const actif = (d?: string | null) => !!d && Date.now() - new Date(d.replace(' ', 'T') + 'Z').getTime() < 5 * 60 * 1000

type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<any>

/** Terminaux ouverts sur la page Agents (#451), retenus par map sur ce PC ; `null` = jamais choisi (tous ouverts). */
const cleOuverts = (mapId: number) => `pop-forge-terminaux-${mapId}`
const lireOuverts = (mapId: number): number[] | null => { try { const v = JSON.parse(localStorage.getItem(cleOuverts(mapId)) ?? 'null'); return Array.isArray(v) ? v : null } catch { return null } }
const garderOuverts = (mapId: number, v: number[]) => { try { localStorage.setItem(cleOuverts(mapId), JSON.stringify(v)) } catch { /* stockage indisponible */ } }

/** Grille qui remplit la page : 1 = plein, 2 = moitiés, 3-4 = quarts, au-delà colonnes × lignes. */
export function grille(n: number): { cols: number; rows: number } {
  if (n <= 1) return { cols: 1, rows: 1 }
  if (n === 2) return { cols: 2, rows: 1 }
  const cols = Math.ceil(Math.sqrt(n))
  return { cols, rows: Math.ceil(n / cols) }
}

export function ForgeAgents({ mapId, tache, invoke }: { mapId: number; tache?: number | null; invoke?: Invoke }) {
  const app = useApp()
  const { data, reload } = useLoad<any[]>(`/api/maps/${mapId}/agents`, [mapId])
  const { data: res, reload: relireRes } = useLoad<any>(`/api/maps/${mapId}/resumes`, [mapId])
  const { data: taches, reload: relireTaches } = useLoad<any>(`/api/maps/${mapId}/tasks`, [mapId])
  const tout = () => { reload(); relireRes() }
  useLive(['ai', 'agents', 'fil'], tout)
  useLive(['tasks'], () => { reload(); relireTaches() })
  // Chaque appel d'outil d'un agent arrive sur le WebSocket de la map (#298) : l'écran suit en direct.
  useEffect(() => app.subscribe(m => { if (m?.t === 'agent') tout() }), [mapId])
  const [choisis, setChoisis] = useState<number[] | null>(() => lireOuverts(mapId))
  useEffect(() => setChoisis(lireOuverts(mapId)), [mapId])
  const liste: any[] = Array.isArray(taches) ? taches : taches?.tasks ?? []
  const ouvertes = liste.filter(t => t.status !== 'done')
  const agir = async (f: () => Promise<any>, ok: string) => { try { const r = await f(); app.toast(r?.message ?? ok); reload(); relireTaches() } catch (e: any) { app.toast(e.message, true) } }
  // Tâche ouverte depuis le site (« Ouvrir dans Forge », #297) : montrée par l'agent qui l'a prise, s'il y en a un.
  const cible = tache ? liste.find(t => t.number === tache) : null
  const porteur = cible ? data?.find(a => a.task?.number === tache) ?? data?.find(a => a.attribuees?.some((x: any) => x.number === tache)) : null
  if (!data) return null
  const ids = choisis ?? data.map(a => a.id)
  // L'agent de la tâche ouverte depuis le site est toujours affiché.
  const affiches = data.filter(a => ids.includes(a.id) || a.id === porteur?.id)
  const fermes = data.filter(a => !affiches.includes(a))
  const changer = (v: number[]) => { setChoisis(v); garderOuverts(mapId, v) }
  const ouvrir = (id: number) => changer([...affiches.map(a => a.id).filter(x => x !== id), id])
  const fermer = (id: number) => changer(affiches.map(a => a.id).filter(x => x !== id))
  const { cols, rows } = grille(affiches.length)
  const libres = ouvertes.filter(t => !data.some(a => a.task?.number === t.number || a.attribuees?.some((x: any) => x.number === t.number)))
  return <div className="term-page">
    <div className="term-haut">
      {app.canEdit && <PlusTerminal mapId={mapId} invoke={invoke} fermes={fermes} libres={libres} ouvrir={ouvrir} lance={id => { changer([...affiches.map(a => a.id), id]); reload(); relireTaches() }} />}
      <h1 className="forge-h1" style={{ margin: 0, fontSize: 20 }}>{tr("Agents")}</h1>
      {res?.equipe?.phrase && <span className="muted small">{res.equipe.phrase}</span>}
      <div style={{ flex: 1 }} />
      {data.length > 1 && <Equipe mapId={mapId} />}
    </div>
    {tache && <div className="card forge-tache" role="status">
      <b style={{ fontWeight: 500 }}>{tr("Tâche #{0}", [tache])}{cible ? ' · ' + cible.title : ''}</b>
      <div className="muted" style={{ fontSize: 13.5 }}>{!cible ? (taches ? tr("Tâche introuvable sur cette map.") : tr("Chargement…"))
        : porteur ? tr("Prise par {0} : son terminal est ci-dessous.", [porteur.name])
        : cible.status === 'done' ? tr("Cette tâche est terminée.") : tr("Aucun agent ne l'a prise : attribuez-la à un agent ci-dessous.")}</div>
    </div>}
    {!data.length ? <p className="muted">{tr("Aucune IA connectée à cette map. Connectez votre IA (Claude Code, Codex, Cursor…) depuis POP : IA connectées, ou lancez-en une avec « + Terminal ».")}</p>
      : !affiches.length ? <p className="muted">{tr("Aucun terminal ouvert : « + Terminal » en ouvre un.")}</p>
      : <div className="term-grille" style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${rows}, minmax(0, 1fr))` }}>
        {affiches.map((a, i) => <Fenetre key={a.id} mapId={mapId} a={a} phrase={res?.agents?.find((x: any) => x.id === a.id)?.phrase}
          cible={porteur?.id === a.id} ouvertes={ouvertes} agir={agir}
          // 3 terminaux : le dernier prend toute la largeur du bas.
          style={affiches.length === 3 && i === 2 ? { gridColumn: '1 / -1' } : undefined}
          fermer={() => fermer(a.id)}
          aPart={invoke ? () => invoke('fenetre_terminal', { map: mapId, agent: a.id, titre: a.name }).catch(() => window.open(`/forge/terminal?map=${mapId}&agent=${a.id}`, '_blank')) : () => window.open(`/forge/terminal?map=${mapId}&agent=${a.id}`, '_blank')} />)}
      </div>}
  </div>
}

/** Un terminal de la grille : barre (nom, rôle, modes, fenêtre à part, fermer), terminal, attribution. */
export function Fenetre({ mapId, a, phrase, cible, ouvertes, agir, fermer, aPart, style }: { mapId: number; a: any; phrase?: string; cible?: boolean; ouvertes: any[]; agir: (f: () => Promise<any>, ok: string) => void; fermer?: () => void; aPart?: () => void; style?: React.CSSProperties }) {
  const app = useApp()
  const mode = a.scope === 'off' ? 'off' : a.scope === 'read' ? 'read' : 'write'
  return <div id={`agent-${a.id}`} className={'term' + (cible ? ' forge-cible' : '')} style={style}>
    <div className="term-barre">
      <span className="forge-point" style={{ background: mode === 'off' ? 'var(--red)' : actif(a.last_used) ? 'var(--tg)' : '#6b6b6b' }} aria-hidden="true" />
      <b>{a.name}</b>
      <span className="term-doux term-coupe">{[a.role ? ROLES[a.role] ?? a.role : null, a.owner].filter(Boolean).join(' · ')}{a.last_used ? ' · ' + tr("dernier appel {0}", [ago(a.last_used)]) : ''}</span>
      <div style={{ flex: 1 }} />
      <div className="term-modes" role="group" aria-label={tr("Mode de l’agent {0}", [a.name])}>
        {MODES.map(([k, l]) => <button key={k} className={mode === k ? 'on' : ''} disabled={!app.canEdit}
          onClick={() => mode !== k && (k !== 'off' || confirm(tr("Couper « {0} » ? Il ne pourra plus rien faire tant qu’un humain ne l’aura pas réactivé.", [a.name]))) && agir(() => api.post(`/api/maps/${mapId}/agents/${a.id}/mode`, { mode: k }), tr("Mode changé"))}>{l}</button>)}
      </div>
      {aPart && <button className="term-bouton" title={tr("Ouvrir dans une fenêtre à part")} aria-label={tr("Ouvrir {0} dans une fenêtre à part", [a.name])} onClick={aPart}><ExternalLink size={14} /></button>}
      {fermer && <button className="term-bouton" title={tr("Fermer le terminal (l’agent continue)")} aria-label={tr("Fermer le terminal de {0}", [a.name])} onClick={fermer}><X size={14} /></button>}
    </div>
    <Terminal mapId={mapId} a={a} phrase={phrase} />
    {app.canEdit && <div className="term-pied">
      <Selecteur label={tr("Attribuer une tâche à {0}", [a.name])} placeholder={tr("Attribuer une tâche…")}
        options={ouvertes.map(t => ({ value: String(t.number), label: `#${t.number} ${t.title}` }))}
        onChange={v => agir(() => api.post(`/api/maps/${mapId}/agents/${a.id}/assign`, { number: Number(v) }), tr("Tâche attribuée"))} />
    </div>}
  </div>
}

/** Mini-terminal d'un agent (#432), au style de Claude Code : sa tâche, ses appels d'outils (⏺ outil, ⎿ détail) et
 * la discussion (#381), en direct ; l'invite « › » écrit à l'agent, qui lit avec read_messages et répond avec
 * send_message. */
export function Terminal({ mapId, a, phrase }: { mapId: number; a: any; phrase?: string }) {
  const app = useApp()
  const { data: appels, reload: relireAppels } = useLoad<any[]>(`/api/maps/${mapId}/agents/activity?agent=${a.id}`, [mapId, a.id])
  const { data: msgs, reload: relireMsgs } = useLoad<any[]>(`/api/maps/${mapId}/agents/${a.id}/messages`, [mapId, a.id])
  useEffect(() => app.subscribe(m => { if (m?.t === 'agent') relireAppels() }), [mapId, a.id])
  useLive(['messages'], relireMsgs)
  const [texte, setTexte] = useState('')
  const corps = useRef<HTMLDivElement>(null)
  // Une seule chronologie, du plus ancien au plus récent : appels et messages.
  const lignes: { at: string; k: string; appel?: any; msg?: any }[] = [
    // L'activité arrive du plus récent au plus ancien : on la remet dans l'ordre d'un terminal.
    ...(appels ?? []).slice(0, 30).reverse().map(c => ({ at: c.at, k: 'c' + c.id, appel: c })),
    ...(msgs ?? []).slice(-30).map(m => ({ at: m.at, k: 'm' + m.id, msg: m })),
  ].map((l, i) => ({ ...l, i })).sort((x, y) => String(x.at).localeCompare(String(y.at)) || x.i - y.i)
  useEffect(() => { const c = corps.current; if (c) c.scrollTop = c.scrollHeight }, [lignes.length])
  const envoyer = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!texte.trim()) return
    try { await api.post(`/api/maps/${mapId}/agents/${a.id}/messages`, { body: texte }); setTexte(''); relireMsgs() } catch (er: any) { app.toast(er.message, true) }
  }
  return <>
    <div className="term-corps" ref={corps} role="log" aria-label={tr("Terminal de {0}", [a.name])}>
      <div className="term-l"><span className="term-etoile">✻</span> {a.task
        ? <>{tr("Tâche #{0}", [a.task.number])} · {a.task.title}{a.task.step ? <span className="term-doux"> · {ETAPES[a.task.step] ?? a.task.step}</span> : null}</>
        : <span className="term-doux">{tr("Pas de tâche réservée")}</span>}</div>
      {a.attribuees?.length > 0 && <div className="term-l term-doux">  {tr("Attribuée(s), pas encore prise(s) : {0}", [a.attribuees.map((x: any) => `#${x.number} ${x.title}`).join(', ')])}</div>}
      {phrase && <div className="term-l term-doux">  {phrase}</div>}
      {appels && msgs && lignes.length === 0 && <div className="term-l term-doux">{tr("Aucune activité pour l’instant. Écrivez-lui ci-dessous.")}</div>}
      {lignes.map(l => l.appel
        ? <div key={l.k} className="term-l">
            <span className={l.appel.ok ? 'term-ok' : 'term-ko'} aria-label={l.appel.ok ? tr("réussi") : tr("en erreur")}>⏺</span> <b>{l.appel.tool}</b>{l.appel.task ? <span className="term-doux"> #{l.appel.task}</span> : null}<span className="term-doux"> · {ago(l.appel.at)}</span>
            {l.appel.detail && <div className="term-l term-doux">  ⎿  {l.appel.detail}</div>}
          </div>
        : l.msg.from === 'ia'
          ? <div key={l.k} className="term-l"><span className="term-ia">⏺</span> {l.msg.body}<span className="term-doux"> · {ago(l.msg.at)}</span></div>
          : <div key={l.k} className="term-l term-moi">› {l.msg.body}<span className="term-doux"> · {l.msg.team ? tr("à toute l’équipe") + ' · ' : ''}{l.msg.read ? tr("lu") : tr("pas encore lu")}</span></div>)}
    </div>
    <form className="term-invite" onSubmit={envoyer}>
      <span aria-hidden="true">›</span>
      <input value={texte} maxLength={4000} onChange={e => setTexte(e.target.value)} placeholder={tr("Écrire à {0}…", [a.name])} aria-label={tr("Message à {0}", [a.name])} disabled={!app.canEdit} />
    </form>
  </>
}

/** Message à toutes ses IA de la map (#381). */
function Equipe({ mapId }: { mapId: number }) {
  const app = useApp()
  const [texte, setTexte] = useState('')
  const envoyer = async (e: React.FormEvent) => {
    e.preventDefault()
    try { const r = await api.post(`/api/maps/${mapId}/agents/messages`, { body: texte }); setTexte(''); app.toast(tr("Envoyé à {0} IA", [r.agents])) } catch (er: any) { app.toast(er.message, true) }
  }
  return <form className="forge-ligne" onSubmit={envoyer}>
    <input className="field" style={{ flex: 1 }} value={texte} maxLength={4000} placeholder={tr("Écrire à toute l’équipe…")} aria-label={tr("Message à toute l’équipe")} onChange={e => setTexte(e.target.value)} />
    <button className="btn" disabled={!texte.trim()}>{tr("Envoyer à toutes")}</button>
  </form>
}

/** « + Terminal » (#451), en haut à gauche de la page Agents : ouvrir le terminal d'une IA connectée à la map, ou lancer
 * un outil sur ce PC (« Lancer avec… » de #95 et #420, déplacé ici). */
function PlusTerminal({ mapId, invoke, fermes, libres, ouvrir, lance }: { mapId: number; invoke?: Invoke; fermes: any[]; libres: any[]; ouvrir: (id: number) => void; lance: (id: number) => void }) {
  const [ouvert, setOuvert] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!ouvert) return
    const f = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOuvert(false) }
    const k = (e: KeyboardEvent) => { if (e.key === 'Escape') setOuvert(false) }
    document.addEventListener('mousedown', f); document.addEventListener('keydown', k)
    return () => { document.removeEventListener('mousedown', f); document.removeEventListener('keydown', k) }
  }, [ouvert])
  return <div ref={ref} style={{ position: 'relative' }}>
    <button className="btn pri" aria-haspopup="dialog" aria-expanded={ouvert} onClick={() => setOuvert(o => !o)}><Plus size={15} strokeWidth={2} />{tr("Terminal")}</button>
    {ouvert && <div className="term-menu card" role="dialog" aria-label={tr("Ouvrir un terminal")}>
      <div className="small muted" style={{ marginBottom: 6 }}>{tr("IA connectées à la map")}</div>
      {fermes.length ? <div className="stack" style={{ gap: 4 }}>{fermes.map(a => <button key={a.id} className="term-choix" onClick={() => { ouvrir(a.id); setOuvert(false) }}>
        <span className="forge-point" style={{ background: actif(a.last_used) ? 'var(--tg)' : '#6b6b6b' }} aria-hidden="true" />
        <b style={{ fontWeight: 500 }}>{a.name}</b><span className="small muted">{[a.role ? ROLES[a.role] ?? a.role : null, a.owner].filter(Boolean).join(' · ')}</span>
      </button>)}</div> : <p className="small muted" style={{ margin: 0 }}>{tr("Toutes les IA connectées ont déjà leur terminal.")}</p>}
      {invoke && <><hr style={{ margin: '12px 0', border: 0, borderTop: '1px solid var(--line, #e5e5e5)' }} />
        <Lancer mapId={mapId} invoke={invoke} ouvertes={libres} fini={id => { if (id) lance(id); setOuvert(false) }} /></>}
    </div>}
  </div>
}

/** « Lancer avec… » (#95, #420) : un outil en ligne de commande (Claude Code, Codex) démarré par Forge avec une clé
 * d'agent pour cette map, ou « Mon IA » (app Claude, ChatGPT, Cursor…), déjà branchée à POP par MCP : la consigne est
 * copiée et Forge ouvre l'IA. Le dernier choix est retenu sur ce PC. POP n'appelle aucune IA. */
const CHOIX = 'pop-forge-lancer-avec'
function Lancer({ mapId, invoke, ouvertes, fini }: { mapId: number; invoke: Invoke; ouvertes: any[]; fini: (agent?: number) => void }) {
  const app = useApp()
  const [outils, setOutils] = useState<any[] | null>(null)
  const [apps, setApps] = useState<any[]>([])
  const [branchees, setBranchees] = useState<any[] | null>(null)
  const memo = (() => { try { return JSON.parse(localStorage.getItem(CHOIX) || '{}') } catch { return {} } })()
  const [outil, setOutil] = useState<string>('')
  const [appIa, setAppIa] = useState<string>(memo.app || 'claude_desktop')
  const [num, setNum] = useState('')
  const [occupe, setOccupe] = useState(false)
  useEffect(() => {
    Promise.all([invoke('outils_ia').catch(() => []), invoke('apps_ia').catch(() => [])]).then(([o, a]: any[]) => {
      setOutils(o); setApps(a)
      const dispo = (id: string) => id === 'mon_ia' || o.some((x: any) => x.id === id && x.lancable)
      setOutil(memo.outil && dispo(memo.outil) ? memo.outil : o.find((x: any) => x.lancable)?.id ?? 'mon_ia')
    })
    // IA branchées à POP par l'utilisateur (hors clés créées par Forge pour ses agents lancés).
    api.get('/api/me/tokens').then((l: any[]) => setBranchees(l.filter(k => !String(k.name).endsWith(' · Forge')))).catch(() => setBranchees([]))
  }, [])
  if (!outils) return null
  const libres = ouvertes
  // Le choix est retenu dès qu'il change (par utilisateur, sur ce PC).
  const retenir = (o: string, a: string) => { try { localStorage.setItem(CHOIX, JSON.stringify({ outil: o, app: a })) } catch { /* stockage indisponible */ } }
  const monIa = async (t: any) => {
    const a = apps.find(x => x.id === appIa)
    const consigne = await invoke('consigne_mon_ia', { map: app.map?.name ?? '', mapId, tache: t.number, titre: t.title })
    await navigator.clipboard.writeText(consigne)
    await api.post(`/api/tasks/${t.id}/status`, { status: 'doing' })
    // Attribuée à l'IA branchée si elle travaille déjà sur cette map ; sinon elle la prendra après use_map.
    const ia = branchees?.find(k => k.kind === appIa && k.map?.id === mapId)
    if (ia) await api.post(`/api/maps/${mapId}/agents/${ia.id}/assign`, { number: t.number }).catch(() => {})
    await invoke('ouvrir_ia', { id: appIa })
    app.toast(tr("Consigne copiée : collez-la dans {0}.", [a?.nom ?? tr("votre IA")]))
    return ia?.id as number | undefined
  }
  const lancer = async () => {
    const t = libres.find(x => String(x.number) === num)
    if (!t) return
    setOccupe(true)
    if (outil === 'mon_ia') {
      try { const id = await monIa(t); setNum(''); fini(id) } catch (e: any) { app.toast(e?.message ?? String(e), true) }
      setOccupe(false); return
    }
    const o = outils.find(x => x.id === outil)
    let cle: { id: number; token: string } | null = null
    try {
      cle = await api.post('/api/me/tokens', { name: `${o.nom} · Forge`, kind: o.id, scope: 'write', map: mapId })
      await api.post(`/api/maps/${mapId}/agents/${cle!.id}/assign`, { number: t.number })
      await invoke('lancer_agent', { outil: o.id, token: cle!.token, map: app.map?.name ?? '', tache: t.number, titre: t.title })
      app.toast(tr("{0} démarre sur la tâche #{1} dans une nouvelle fenêtre.", [o.nom, t.number])); setNum(''); fini(cle!.id)
    } catch (e: any) {
      // Lancement raté : la clé créée pour lui est révoquée tout de suite.
      if (cle) await api.del(`/api/tokens/${cle.id}`).catch(() => {})
      app.toast(e?.message ?? String(e), true); fini()
    }
    setOccupe(false)
  }
  const sansIa = outil === 'mon_ia' && branchees !== null && branchees.length === 0
  const choix = [...outils.map(o => ({ ...o, nom: `${o.nom} (${tr("terminal")})` })), { id: 'mon_ia', nom: tr("Mon IA (app Claude, ChatGPT, Cursor…)"), lancable: true }]
  return <section className="stack" style={{ gap: 10 }} aria-label={tr("Lancer un agent")}>
    <div className="small muted">{tr("Lancer sur ce PC")}</div>
    <div className="forge-outils" role="radiogroup" aria-label={tr("Outil d'IA")}>
      {choix.map(o => <label key={o.id} className={'forge-outil' + (o.lancable ? '' : ' off') + (outil === o.id ? ' on' : '')} title={o.raison ?? o.chemin ?? ''}>
        <input type="radio" name="outil-ia" value={o.id} checked={outil === o.id} disabled={!o.lancable} onChange={() => { setOutil(o.id); retenir(o.id, appIa) }} />
        <span><b style={{ fontWeight: 500 }}>{o.nom}</b>{!o.lancable && <small className="muted" style={{ display: 'block' }}>{o.raison}</small>}</span>
      </label>)}
    </div>
    {outil === 'mon_ia' && !sansIa && <div className="forge-outils" role="radiogroup" aria-label={tr("Votre IA")}>
      {apps.map(a => { const ok = !!a.app || !!a.site
        return <label key={a.id} className={'forge-outil' + (ok ? '' : ' off') + (appIa === a.id ? ' on' : '')}>
          <input type="radio" name="app-ia" value={a.id} checked={appIa === a.id} disabled={!ok} onChange={() => { setAppIa(a.id); retenir(outil, a.id) }} />
          <span><b style={{ fontWeight: 500 }}>{a.nom}</b><small className="muted" style={{ display: 'block' }}>{a.app ? tr("app installée") : a.site ? tr("dans le navigateur") : tr("pas installé sur ce PC")}</small></span>
        </label> })}
    </div>}
    {sansIa ? <div className="row" style={{ gap: 10, flexWrap: 'wrap' }}>
      <span className="muted" style={{ flex: 1 }}>{tr("Aucune IA n'est branchée à POP. Branchez la vôtre une fois (POP → IA connectées), puis revenez ici.")}</span>
      <button className="btn pri" onClick={() => { window.location.href = '/parametres?onglet=ia' }}>{tr("Brancher mon IA")}</button>
    </div> : <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
      <Selecteur label={tr("Tâche à faire")} placeholder={libres.length ? tr("Choisir une tâche…") : tr("Aucune tâche libre")} value={num} disabled={!libres.length}
        options={libres.map(t => ({ value: String(t.number), label: `#${t.number} · ${t.title}` }))} onChange={setNum} />
      <button className="btn pri" disabled={!outil || !num || occupe} onClick={lancer}>{occupe ? tr("Lancement…") : tr("Lancer")}</button>
    </div>}
    <small className="muted">{outil === 'mon_ia' ? tr("La consigne de la tâche est copiée : collez-la dans votre IA, qui lit la tâche dans POP par sa connexion MCP.") : tr("L'outil tourne sur votre PC avec votre abonnement. Sa clé POP lui est passée en mémoire, jamais écrite dans un fichier.")}</small>
  </section>
}
