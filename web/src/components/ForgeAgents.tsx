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
/** Nom de l'IA, sans le nom technique de sa clé (#481) : Claude, Codex, ChatGPT, Cursor. */
export function famille(a: { kind?: string; name?: string }): string {
  const t = `${a.kind ?? ''} ${a.name ?? ''}`.toLowerCase()
  return t.includes('claude') ? 'Claude' : t.includes('codex') ? 'Codex' : t.includes('chatgpt') || t.includes('openai') ? 'ChatGPT' : t.includes('cursor') ? 'Cursor' : (a.name ?? 'IA')
}
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
  // Terminaux en direct (#466) : agents dont Forge fait tourner l'outil.
  const [directs, setDirects] = useState<number[]>([])
  const relireDirects = () => { invoke?.('direct_actifs').then((l: number[]) => setDirects(l ?? [])).catch(() => setDirects([])) }
  useEffect(() => { relireDirects(); const t = setInterval(relireDirects, 3000); return () => clearInterval(t) }, [invoke])
  useEffect(() => setChoisis(lireOuverts(mapId)), [mapId])
  const liste: any[] = Array.isArray(taches) ? taches : taches?.tasks ?? []
  const ouvertes = liste.filter(t => t.status !== 'done')
  const agir = async (f: () => Promise<any>, ok: string) => { try { const r = await f(); app.toast(r?.message ?? ok); reload(); relireTaches() } catch (e: any) { app.toast(e.message, true) } }
  // Tâche ouverte depuis le site (« Ouvrir dans Forge », #297) : montrée par l'agent qui l'a prise, s'il y en a un.
  const cible = tache ? liste.find(t => t.number === tache) : null
  const porteur = cible ? data?.find(a => a.task?.number === tache) ?? data?.find(a => a.attribuees?.some((x: any) => x.number === tache)) : null
  if (!data) return null
  // Seules les IA qui ont déjà appelé POP sont « connectées » (#464) : une clé jamais utilisée n'ouvre pas de terminal.
  const connectees = data.filter(a => a.last_used)
  // Par défaut, un terminal par IA (#481) : la plus récemment active de chaque famille (Claude, Codex…).
  const parDefaut = [...connectees].sort((x, y) => String(y.last_used).localeCompare(String(x.last_used)))
    .filter((a, i, l) => l.findIndex(b => famille(b) === famille(a)) === i).map(a => a.id)
  const ids = choisis ?? parDefaut
  // L'agent de la tâche ouverte depuis le site est toujours affiché.
  const affiches = data.filter(a => ids.includes(a.id) || a.id === porteur?.id)
  const changer = (v: number[]) => { setChoisis(v); garderOuverts(mapId, v) }
  const ouvrir = (id: number) => changer([...affiches.map(a => a.id).filter(x => x !== id), id])
  const fermer = (id: number) => changer(affiches.map(a => a.id).filter(x => x !== id))
  const { cols, rows } = grille(affiches.length)
  return <div className="term-page">
    <div className="term-haut">
      {app.canEdit && <PlusTerminal connectees={connectees} directs={directs} ouvrir={ouvrir} mapId={mapId} invoke={invoke} lance={id => { relireDirects(); changer([...affiches.map(a => a.id), id]); reload() }} />}
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
          direct={invoke && directs.includes(a.id) ? invoke : undefined}
          fermer={() => {
            if (invoke && directs.includes(a.id)) {
              if (!confirm(tr("Fermer ce terminal arrête {0}. Continuer ?", [a.name]))) return
              invoke('direct_fermer', { agent: a.id }).finally(relireDirects)
            }
            fermer(a.id)
          }}
          aPart={invoke ? () => invoke('fenetre_terminal', { map: mapId, agent: a.id, titre: a.name }).catch(() => app.toast(tr("Mettez Forge à jour (pop-uefn.com/forge) pour ouvrir un terminal dans une fenêtre à part."), true)) : () => window.open(`/forge/terminal?map=${mapId}&agent=${a.id}`, '_blank')} />)}
      </div>}
  </div>
}

/** Un terminal de la grille : barre (nom, rôle, modes, fenêtre à part, fermer), terminal, attribution. */
export function Fenetre({ mapId, a, phrase, cible, ouvertes, agir, fermer, aPart, style, direct }: { mapId: number; a: any; phrase?: string; cible?: boolean; ouvertes: any[]; agir: (f: () => Promise<any>, ok: string) => void; fermer?: () => void; aPart?: () => void; style?: React.CSSProperties; direct?: Invoke }) {
  const app = useApp()
  const mode = a.scope === 'off' ? 'off' : a.scope === 'read' ? 'read' : 'write'
  return <div id={`agent-${a.id}`} className={'term' + (cible ? ' forge-cible' : '')} style={style}>
    <div className="term-barre">
      <span className="forge-point" style={{ background: mode === 'off' ? 'var(--red)' : actif(a.last_used) ? 'var(--tg)' : '#6b6b6b' }} aria-hidden="true" />
      <b title={a.name}>{famille(a)}</b>
      {direct && <span className="term-direct">{tr("en direct")}</span>}
      <span className="term-doux term-coupe">{[a.role ? ROLES[a.role] ?? a.role : null, a.owner].filter(Boolean).join(' · ')}{a.last_used ? ' · ' + tr("dernier appel {0}", [ago(a.last_used)]) : ''}</span>
      <div style={{ flex: 1 }} />
      <div className="term-modes" role="group" aria-label={tr("Mode de l’agent {0}", [a.name])}>
        {MODES.map(([k, l]) => <button key={k} className={mode === k ? 'on' : ''} disabled={!app.canEdit}
          onClick={() => mode !== k && (k !== 'off' || confirm(tr("Couper « {0} » ? Il ne pourra plus rien faire tant qu’un humain ne l’aura pas réactivé.", [a.name]))) && agir(() => api.post(`/api/maps/${mapId}/agents/${a.id}/mode`, { mode: k }), tr("Mode changé"))}>{l}</button>)}
      </div>
      {aPart && <button className="term-bouton" title={tr("Ouvrir dans une fenêtre à part")} aria-label={tr("Ouvrir {0} dans une fenêtre à part", [a.name])} onClick={aPart}><ExternalLink size={14} /></button>}
      {fermer && <button className="term-bouton" title={tr("Fermer le terminal (l’agent continue)")} aria-label={tr("Fermer le terminal de {0}", [a.name])} onClick={fermer}><X size={14} /></button>}
    </div>
    {direct ? <TerminalDirect a={a} invoke={direct} /> : <Terminal mapId={mapId} a={a} phrase={phrase} />}
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

/** Une ligne du flux JSON de Claude Code, en ligne de terminal (#466). */
function ligneDirecte(l: string, i: number): React.ReactNode {
  let v: any
  try { v = JSON.parse(l) } catch { return <div key={i} className="term-l term-doux">{l}</div> }
  const court = (x: any, n = 160) => { const t = typeof x === 'string' ? x : JSON.stringify(x); return t.length > n ? t.slice(0, n) + '…' : t }
  switch (v.type) {
    case 'moi': return <div key={i} className="term-l term-moi">› {v.text}</div>
    case 'erreur': return <div key={i} className="term-l term-ko">{v.text}</div>
    case 'fin': return <div key={i} className="term-l term-doux"><span className="term-etoile">✻</span> {tr("Claude Code s’est arrêté.")}</div>
    case 'system': return v.subtype === 'init' ? <div key={i} className="term-l term-doux"><span className="term-etoile">✻</span> {tr("Claude Code prêt")}{v.model ? ` · ${v.model}` : ''}</div> : null
    case 'assistant': return <div key={i}>{(v.message?.content ?? []).map((c: any, j: number) => c.type === 'text'
      ? <div key={j} className="term-l"><span className="term-ia">⏺</span> {c.text}</div>
      : c.type === 'tool_use' ? <div key={j} className="term-l"><span className="term-ok">⏺</span> <b>{String(c.name).replace(/^mcp__pop__/, '')}</b><span className="term-doux">({court(c.input, 100)})</span></div> : null)}</div>
    case 'user': return <div key={i}>{(Array.isArray(v.message?.content) ? v.message.content : []).filter((c: any) => c.type === 'tool_result').map((c: any, j: number) =>
      <div key={j} className={'term-l ' + (c.is_error ? 'term-ko' : 'term-doux')}>  ⎿  {court(Array.isArray(c.content) ? c.content.map((x: any) => x.text ?? '').join(' ') : c.content)}</div>)}</div>
    case 'result': return <div key={i} className="term-l term-doux">  {v.is_error ? tr("✗ erreur") : '✓'}{v.duration_ms ? ` · ${(v.duration_ms / 1000).toFixed(1)} s` : ''}</div>
    default: return null
  }
}

/** Terminal en direct (#466) : ce qu'on écrit part tout de suite dans Claude Code (tournant dans Forge) ; sa réponse et
 * ses outils s'affichent au fil de l'eau (lecture toutes les 250 ms). */
function TerminalDirect({ a, invoke }: { a: any; invoke: Invoke }) {
  const app = useApp()
  const [lignes, setLignes] = useState<string[]>([])
  const [vivant, setVivant] = useState(true)
  const [attend, setAttend] = useState(false)
  const [texte, setTexte] = useState('')
  const pos = useRef(0)
  const corps = useRef<HTMLDivElement>(null)
  useEffect(() => {
    let fini = false
    const lire = async () => {
      try {
        const r = await invoke('direct_lire', { agent: a.id, depuis: pos.current })
        if (fini || !r) { if (!r) setVivant(false); return }
        pos.current = r.suite; setVivant(r.vivant)
        if (r.lignes.length) {
          setLignes(l => [...l, ...r.lignes].slice(-1500))
          if (r.lignes.some((x: string) => x.includes('"type":"result"') || x.includes('"type":"fin"'))) setAttend(false)
        }
      } catch { /* Forge occupé : on relit au prochain tour */ }
    }
    lire(); const t = setInterval(lire, 250)
    return () => { fini = true; clearInterval(t) }
  }, [a.id])
  useEffect(() => { const c = corps.current; if (c) c.scrollTop = c.scrollHeight }, [lignes.length, attend])
  const envoyer = async (e: React.FormEvent) => {
    e.preventDefault()
    const t = texte.trim()
    if (!t) return
    try { await invoke('direct_ecrire', { agent: a.id, texte: t }); setTexte(''); setAttend(true) } catch (er: any) { app.toast(String(er?.message ?? er), true) }
  }
  return <>
    <div className="term-corps" ref={corps} role="log" aria-live="polite" aria-label={tr("Terminal en direct de {0}", [a.name])}>
      {!lignes.length && <div className="term-l term-doux"><span className="term-etoile">✻</span> {tr("Démarrage de Claude Code… Écrivez-lui ci-dessous.")}</div>}
      {lignes.map(ligneDirecte)}
      {attend && <div className="term-l term-doux"><span className="term-etoile term-pouls">✻</span> {tr("réfléchit…")}</div>}
    </div>
    <form className="term-invite" onSubmit={envoyer}>
      <span aria-hidden="true">›</span>
      <input value={texte} maxLength={8000} onChange={e => setTexte(e.target.value)} placeholder={vivant ? tr("Écrire à {0}…", [a.name]) : tr("Claude Code s’est arrêté : rouvrez-en un avec « + Terminal ».")} aria-label={tr("Message à {0}", [a.name])} disabled={!vivant || !app.canEdit} />
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

/** « + Terminal » (#451, #481), en haut à gauche de la page Agents : une ligne par IA connectée à POP (Claude, Codex…).
 * Claude s'ouvre en direct si Claude Code est installé sur le PC (#466), sinon dans le terminal de l'IA connectée. */
function PlusTerminal({ connectees, directs, ouvrir, mapId, invoke, lance }: { connectees: any[]; directs: number[]; ouvrir: (id: number) => void; mapId: number; invoke?: Invoke; lance: (id: number) => void }) {
  const app = useApp()
  const [ouvert, setOuvert] = useState(false)
  const [claudeCode, setClaudeCode] = useState(false)
  const [occupe, setOccupe] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!ouvert) return
    invoke?.('outils_ia').then((l: any[]) => setClaudeCode(!!l?.find(o => o.id === 'claude_code' && o.lancable))).catch(() => setClaudeCode(false))
    const f = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOuvert(false) }
    const k = (e: KeyboardEvent) => { if (e.key === 'Escape') setOuvert(false) }
    document.addEventListener('mousedown', f); document.addEventListener('keydown', k)
    return () => { document.removeEventListener('mousedown', f); document.removeEventListener('keydown', k) }
  }, [ouvert])
  // Une ligne par IA, la plus récemment active d'abord.
  const familles: { nom: string; agents: any[] }[] = []
  for (const a of [...connectees].sort((x, y) => String(y.last_used).localeCompare(String(x.last_used)))) {
    const f = familles.find(x => x.nom === famille(a))
    if (f) f.agents.push(a); else familles.push({ nom: famille(a), agents: [a] })
  }
  const enDirect = async () => {
    setOccupe(true)
    let cle: { id: number; token: string } | null = null
    try {
      cle = await api.post('/api/me/tokens', { name: 'Claude', kind: 'claude_code', scope: 'write', map: mapId })
      await invoke!('direct_ouvrir', { outil: 'claude_code', token: cle!.token, map: app.map?.name ?? '', agent: cle!.id })
      lance(cle!.id)
    } catch (e: any) {
      if (cle) await api.del(`/api/tokens/${cle.id}`).catch(() => {})
      app.toast(/not allowed|not found|unknown/i.test(String(e?.message ?? e)) ? tr("Mettez Forge à jour (pop-uefn.com/forge) pour ouvrir un terminal en direct.") : (e?.message ?? String(e)), true)
    }
    setOccupe(false)
  }
  const choisir = async (f: { nom: string; agents: any[] }) => {
    setOuvert(false)
    const vivant = f.agents.find(a => directs.includes(a.id))
    if (vivant) return ouvrir(vivant.id)
    if (f.nom === 'Claude' && invoke && claudeCode) return enDirect()
    ouvrir(f.agents[0].id)
  }
  return <div ref={ref} style={{ position: 'relative' }}>
    <button className="btn pri" aria-haspopup="dialog" aria-expanded={ouvert} disabled={occupe} onClick={() => setOuvert(o => !o)}><Plus size={15} strokeWidth={2} />{occupe ? tr("Démarrage…") : tr("Terminal")}</button>
    {ouvert && <div className="term-menu card" role="dialog" aria-label={tr("Ouvrir un terminal")}>
      {familles.length ? <div className="stack" style={{ gap: 4 }}>{familles.map(f => <button key={f.nom} className="term-choix" onClick={() => choisir(f)}>
        <span className="forge-point" style={{ background: f.agents.some(a => actif(a.last_used)) ? 'var(--tg)' : '#6b6b6b' }} aria-hidden="true" />
        <b style={{ fontWeight: 500 }}>{f.nom}</b>
        {(f.agents.some(a => directs.includes(a.id)) || (f.nom === 'Claude' && claudeCode)) && <span className="term-direct">{tr("direct")}</span>}
      </button>)}</div> : <p className="small muted" style={{ margin: 0 }}>{tr("Aucune IA connectée à POP. Connectez Claude ou Codex (POP → IA connectées), puis revenez ici.")}</p>}
    </div>}
  </div>
}
