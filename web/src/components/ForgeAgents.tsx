import { useEffect, useRef, useState } from 'react'
import { ago, api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr } from '../i18n/t'
import { Selecteur } from './Selecteur'

// Écran Agents de Forge (#94) : les IA connectées à la map, leur tâche et son étape, leur résumé en français (#321),
// leur journal d'appels MCP en direct ; attribuer une tâche, passer en lecture seule ou couper un agent.
const ETAPES: Record<string, string> = { ecrit: tr("écrit"), compile: tr("compilé"), teste: tr("testé"), relu: tr("relu") }
const MODES: [string, string][] = [['write', tr("Agit")], ['read', tr("Lecture seule")], ['off', tr("Coupé")]]
const ROLES: Record<string, string> = { orchestrateur: tr("Orchestrateur"), codeur_verse: tr("Codeur Verse"), level_designer: tr("Level designer"), ui: tr("Interface"), testeur: tr("Testeur"), relecteur: tr("Relecteur"), analyste: tr("Analyste") }
const actif = (d?: string | null) => !!d && Date.now() - new Date(d.replace(' ', 'T') + 'Z').getTime() < 5 * 60 * 1000

export function ForgeAgents({ mapId, tache, invoke }: { mapId: number; tache?: number | null; invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<any> }) {
  const app = useApp()
  const { data, reload } = useLoad<any[]>(`/api/maps/${mapId}/agents`, [mapId])
  const { data: res, reload: relireRes } = useLoad<any>(`/api/maps/${mapId}/resumes`, [mapId])
  const { data: taches, reload: relireTaches } = useLoad<any>(`/api/maps/${mapId}/tasks`, [mapId])
  const [journal, setJournal] = useState<number | null>(null)
  const [fil, setFil] = useState<number | null>(null)
  const tout = () => { reload(); relireRes() }
  useLive(['ai', 'agents', 'fil'], tout)
  useLive(['tasks'], () => { reload(); relireTaches() })
  // Chaque appel d'outil d'un agent arrive sur le WebSocket de la map (#298) : l'écran suit en direct.
  useEffect(() => app.subscribe(m => { if (m?.t === 'agent') tout() }), [mapId])
  const liste: any[] = Array.isArray(taches) ? taches : taches?.tasks ?? []
  const ouvertes = liste.filter(t => t.status !== 'done')
  const agir = async (f: () => Promise<any>, ok: string) => { try { const r = await f(); app.toast(r?.message ?? ok); reload(); relireTaches() } catch (e: any) { app.toast(e.message, true) } }
  // Tâche ouverte depuis le site (« Ouvrir dans Forge », #297) : montrée par l'agent qui l'a prise, s'il y en a un.
  const cible = tache ? liste.find(t => t.number === tache) : null
  const porteur = cible ? data?.find(a => a.task?.number === tache) ?? data?.find(a => a.attribuees?.some((x: any) => x.number === tache)) : null
  useEffect(() => { if (porteur) document.getElementById(`agent-${porteur.id}`)?.scrollIntoView({ block: 'center' }) }, [porteur?.id])
  if (!data) return null
  return <div className="stack" style={{ gap: 18 }}>
    <h1 className="forge-h1">{tr("Agents")}</h1>
    {tache && <div className="card forge-tache" role="status">
      <b style={{ fontWeight: 500 }}>{tr("Tâche #{0}", [tache])}{cible ? ' · ' + cible.title : ''}</b>
      <div className="muted" style={{ fontSize: 13.5 }}>{!cible ? (taches ? tr("Tâche introuvable sur cette map.") : tr("Chargement…"))
        : porteur ? tr("Prise par {0} : son activité est ci-dessous.", [porteur.name])
        : cible.status === 'done' ? tr("Cette tâche est terminée.") : tr("Aucun agent ne l'a prise : attribuez-la à un agent ci-dessous.")}</div>
    </div>}
    {res?.equipe?.phrase && <p style={{ margin: 0 }}>{res.equipe.phrase}</p>}
    {invoke && app.canEdit && <Lancer mapId={mapId} invoke={invoke} ouvertes={ouvertes.filter(t => !data.some(a => a.task?.number === t.number || a.attribuees?.some((x: any) => x.number === t.number)))} fini={() => { reload(); relireTaches() }} />}
    {data.length > 1 && <Equipe mapId={mapId} />}
    {!data.length && <p className="muted">{tr("Aucune IA connectée à cette map. Connectez votre IA (Claude Code, Codex, Cursor…) depuis POP : IA connectées.")}</p>}
    {data.map(a => {
      const phrase = res?.agents?.find((x: any) => x.id === a.id)?.phrase
      const mode = a.scope === 'off' ? 'off' : a.scope === 'read' ? 'read' : 'write'
      return <div key={a.id} id={`agent-${a.id}`} className={'card' + (porteur?.id === a.id ? ' forge-cible' : '')} style={{ padding: 14, display: 'flex', flexDirection: 'column', gap: 8 }}>
        <div className="forge-ligne">
          <span className="forge-point" style={{ background: mode === 'off' ? 'var(--red)' : actif(a.last_used) ? 'var(--tg)' : 'var(--text3)' }} aria-hidden="true" />
          <b>{a.name}</b>
          <span className="small muted">{[a.role ? ROLES[a.role] ?? a.role : null, a.owner].filter(Boolean).join(' · ')}{a.last_used ? ' · ' + tr("dernier appel {0}", [ago(a.last_used)]) : ''}</span>
          <div style={{ flex: 1 }} />
          <div className="row" role="group" aria-label={tr("Mode de l’agent {0}", [a.name])} style={{ gap: 4 }}>
            {MODES.map(([k, l]) => <button key={k} className={'chip' + (mode === k ? ' on' : '')} disabled={!app.canEdit}
              onClick={() => mode !== k && (k !== 'off' || confirm(tr("Couper « {0} » ? Il ne pourra plus rien faire tant qu’un humain ne l’aura pas réactivé.", [a.name]))) && agir(() => api.post(`/api/maps/${mapId}/agents/${a.id}/mode`, { mode: k }), tr("Mode changé"))}>{l}</button>)}
          </div>
        </div>
        <div>{a.task ? <>#{a.task.number} {a.task.title}{a.task.step ? <span className="badge" style={{ marginLeft: 8 }}>{ETAPES[a.task.step] ?? a.task.step}</span> : null}</> : <span className="muted">{tr("Pas de tâche réservée")}</span>}
          {a.attribuees?.length > 0 && <small className="muted" style={{ display: 'block' }}>{tr("Attribuée(s), pas encore prise(s) : {0}", [a.attribuees.map((x: any) => `#${x.number} ${x.title}`).join(', ')])}</small>}</div>
        {(phrase || a.last_action) && <div className="small muted">{phrase ?? a.last_action}</div>}
        <div className="forge-ligne">
          {app.canEdit && <Selecteur label={tr("Attribuer une tâche à {0}", [a.name])} placeholder={tr("Attribuer une tâche…")}
            options={ouvertes.map(t => ({ value: String(t.number), label: `#${t.number} ${t.title}` }))}
            onChange={v => agir(() => api.post(`/api/maps/${mapId}/agents/${a.id}/assign`, { number: Number(v) }), tr("Tâche attribuée"))} />}
          <button className="btn" onClick={() => setFil(fil === a.id ? null : a.id)}>{fil === a.id ? tr("Fermer la discussion") : tr("Écrire")}</button>
          <button className="linkbtn" onClick={() => setJournal(journal === a.id ? null : a.id)}>{journal === a.id ? tr("Masquer le journal") : tr("Journal")}</button>
        </div>
        {fil === a.id && <Fil mapId={mapId} agent={a.id} nom={a.name} />}
        {journal === a.id && <Journal mapId={mapId} agent={a.id} />}
      </div>
    })}
  </div>
}

function Journal({ mapId, agent }: { mapId: number; agent: number }) {
  const app = useApp()
  const { data, reload } = useLoad<any[]>(`/api/maps/${mapId}/agents/activity?agent=${agent}`, [mapId, agent])
  useEffect(() => app.subscribe(m => { if (m?.t === 'agent') reload() }), [mapId, agent])
  if (!data) return null
  if (!data.length) return <p className="small muted" style={{ margin: 0 }}>{tr("Aucun appel pour l’instant.")}</p>
  return <div className="forge-maps" role="log" aria-label={tr("Journal de l’agent")}>
    {data.slice(0, 20).map(c => <div key={c.id} className="forge-map-ligne" style={{ flexDirection: 'row', alignItems: 'center', padding: '6px 12px', gap: 10 }}>
      <span className="forge-point" style={{ background: c.ok ? 'var(--tg)' : 'var(--red)' }} aria-label={c.ok ? tr("réussi") : tr("en erreur")} />
      <code style={{ flex: 1 }}>{c.tool}</code>
      {c.task && <span className="small muted">#{c.task}</span>}
      <span className="small muted">{ago(c.at)}</span>
    </div>)}
  </div>
}

/** Fil de discussion avec une IA (#381) : l'IA lit avec read_messages et répond avec send_message. */
function Fil({ mapId, agent, nom }: { mapId: number; agent: number; nom: string }) {
  const app = useApp()
  const { data, reload } = useLoad<any[]>(`/api/maps/${mapId}/agents/${agent}/messages`, [mapId, agent])
  useLive(['messages'], reload)
  const [texte, setTexte] = useState('')
  const bas = useRef<HTMLDivElement>(null)
  useEffect(() => { bas.current?.scrollIntoView({ block: 'nearest' }) }, [data?.length])
  const envoyer = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!texte.trim()) return
    try { await api.post(`/api/maps/${mapId}/agents/${agent}/messages`, { body: texte }); setTexte(''); reload() } catch (er: any) { app.toast(er.message, true) }
  }
  return <div className="forge-fil">
    <div className="forge-fil-liste" role="log" aria-label={tr("Discussion avec {0}", [nom])}>
      {!data?.length && <p className="small muted" style={{ margin: 0 }}>{tr("Écrivez à {0} : il lira votre message entre deux actions et vous répondra ici.", [nom])}</p>}
      {data?.map(m => <div key={m.id} className={'forge-bulle' + (m.from === 'ia' ? ' ia' : '')}>
        <div>{m.body}</div>
        <div className="small muted">{m.from === 'ia' ? nom : tr("Vous")}{m.team ? ' · ' + tr("à toute l’équipe") : ''} · {ago(m.at)}{m.from !== 'ia' ? ' · ' + (m.read ? tr("lu") : tr("pas encore lu")) : ''}</div>
      </div>)}
      <div ref={bas} />
    </div>
    <form className="forge-ligne" onSubmit={envoyer}>
      <textarea className="field" style={{ flex: 1, minHeight: 44, padding: 10, resize: 'vertical' }} value={texte} maxLength={4000} placeholder={tr("Votre message à {0}…", [nom])} aria-label={tr("Message à {0}", [nom])}
        onChange={e => setTexte(e.target.value)} onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); envoyer(e as any) } }} />
      <button className="btn pri" disabled={!texte.trim()}>{tr("Envoyer")}</button>
    </form>
  </div>
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

/** « Lancer avec… » (#95, #420) : un outil en ligne de commande (Claude Code, Codex) démarré par Forge avec une clé
 * d'agent pour cette map, ou « Mon IA » (app Claude, ChatGPT, Cursor…), déjà branchée à POP par MCP : la consigne est
 * copiée et Forge ouvre l'IA. Le dernier choix est retenu sur ce PC. POP n'appelle aucune IA. */
const CHOIX = 'pop-forge-lancer-avec'
function Lancer({ mapId, invoke, ouvertes, fini }: { mapId: number; invoke: (cmd: string, args?: Record<string, unknown>) => Promise<any>; ouvertes: any[]; fini: () => void }) {
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
  }
  const lancer = async () => {
    const t = libres.find(x => String(x.number) === num)
    if (!t) return
    setOccupe(true)
    if (outil === 'mon_ia') {
      try { await monIa(t); setNum(''); fini() } catch (e: any) { app.toast(e?.message ?? String(e), true) }
      setOccupe(false); return
    }
    const o = outils.find(x => x.id === outil)
    let cle: { id: number; token: string } | null = null
    try {
      cle = await api.post('/api/me/tokens', { name: `${o.nom} · Forge`, kind: o.id, scope: 'write', map: mapId })
      await api.post(`/api/maps/${mapId}/agents/${cle!.id}/assign`, { number: t.number })
      await invoke('lancer_agent', { outil: o.id, token: cle!.token, map: app.map?.name ?? '', tache: t.number, titre: t.title })
      app.toast(tr("{0} démarre sur la tâche #{1} dans une nouvelle fenêtre.", [o.nom, t.number])); setNum(''); fini()
    } catch (e: any) {
      // Lancement raté : la clé créée pour lui est révoquée tout de suite.
      if (cle) await api.del(`/api/tokens/${cle.id}`).catch(() => {})
      app.toast(e?.message ?? String(e), true); fini()
    }
    setOccupe(false)
  }
  const sansIa = outil === 'mon_ia' && branchees !== null && branchees.length === 0
  const choix = [...outils.map(o => ({ ...o, nom: `${o.nom} (${tr("terminal")})` })), { id: 'mon_ia', nom: tr("Mon IA (app Claude, ChatGPT, Cursor…)"), lancable: true }]
  return <section className="card stack" style={{ padding: 16, gap: 10 }} aria-label={tr("Lancer un agent")}>
    <b style={{ fontWeight: 500 }}>{tr("Lancer avec…")}</b>
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
