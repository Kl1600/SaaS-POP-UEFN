import { useEffect, useState } from 'react'
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

export function ForgeAgents({ mapId }: { mapId: number }) {
  const app = useApp()
  const { data, reload } = useLoad<any[]>(`/api/maps/${mapId}/agents`, [mapId])
  const { data: res, reload: relireRes } = useLoad<any>(`/api/maps/${mapId}/resumes`, [mapId])
  const { data: taches, reload: relireTaches } = useLoad<any>(`/api/maps/${mapId}/tasks`, [mapId])
  const [journal, setJournal] = useState<number | null>(null)
  const tout = () => { reload(); relireRes() }
  useLive(['ai', 'agents', 'fil'], tout)
  useLive(['tasks'], () => { reload(); relireTaches() })
  // Chaque appel d'outil d'un agent arrive sur le WebSocket de la map (#298) : l'écran suit en direct.
  useEffect(() => app.subscribe(m => { if (m?.t === 'agent') tout() }), [mapId])
  const liste: any[] = Array.isArray(taches) ? taches : taches?.tasks ?? []
  const ouvertes = liste.filter(t => t.status !== 'done')
  const agir = async (f: () => Promise<any>, ok: string) => { try { const r = await f(); app.toast(r?.message ?? ok); reload(); relireTaches() } catch (e: any) { app.toast(e.message, true) } }
  if (!data) return null
  return <div className="stack" style={{ gap: 18 }}>
    <h1 className="forge-h1">{tr("Agents")}</h1>
    {res?.equipe?.phrase && <p style={{ margin: 0 }}>{res.equipe.phrase}</p>}
    {!data.length && <p className="muted">{tr("Aucune IA connectée à cette map. Connectez votre IA (Claude Code, Codex, Cursor…) depuis POP : IA connectées.")}</p>}
    {data.map(a => {
      const phrase = res?.agents?.find((x: any) => x.id === a.id)?.phrase
      const mode = a.scope === 'off' ? 'off' : a.scope === 'read' ? 'read' : 'write'
      return <div key={a.id} className="card" style={{ padding: 14, display: 'flex', flexDirection: 'column', gap: 8 }}>
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
        <div>{a.task ? <>#{a.task.number} {a.task.title}{a.task.step ? <span className="badge" style={{ marginLeft: 8 }}>{ETAPES[a.task.step] ?? a.task.step}</span> : null}</> : <span className="muted">{tr("Pas de tâche réservée")}</span>}</div>
        {(phrase || a.last_action) && <div className="small muted">{phrase ?? a.last_action}</div>}
        <div className="forge-ligne">
          {app.canEdit && <Selecteur label={tr("Attribuer une tâche à {0}", [a.name])} placeholder={tr("Attribuer une tâche…")}
            options={ouvertes.map(t => ({ value: String(t.number), label: `#${t.number} ${t.title}` }))}
            onChange={v => agir(() => api.post(`/api/maps/${mapId}/agents/${a.id}/assign`, { number: Number(v) }), tr("Tâche attribuée"))} />}
          <button className="linkbtn" onClick={() => setJournal(journal === a.id ? null : a.id)}>{journal === a.id ? tr("Masquer le journal") : tr("Journal")}</button>
        </div>
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
