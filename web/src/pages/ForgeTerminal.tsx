// Terminal d'un agent dans une fenêtre à part de Forge (#451) : /forge/terminal?map=…&agent=…, ouverte par le bouton
// « Fenêtre à part » de la page Agents. Même terminal, même discussion, en direct.
import { useEffect, useState } from 'react'
import { api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr } from '../i18n/t'
import { Fenetre } from '../components/ForgeAgents'
import { Didacticiel } from '../components/Didacticiel'

export default function ForgeTerminal() {
  const app = useApp()
  const [q] = useState(() => new URLSearchParams(location.search))
  const mapId = Number(q.get('map')) || 0, agent = Number(q.get('agent')) || 0
  useEffect(() => { if (mapId) app.setMapId(mapId) }, [mapId])
  const { data, reload } = useLoad<any[]>(`/api/maps/${mapId}/agents`, [mapId])
  const { data: taches, reload: relireTaches } = useLoad<any>(`/api/maps/${mapId}/tasks`, [mapId])
  useLive(['ai', 'agents', 'tasks'], () => { reload(); relireTaches() })
  useEffect(() => app.subscribe(m => { if (m?.t === 'agent') reload() }), [mapId])
  const a = data?.find(x => x.id === agent)
  useEffect(() => { if (a) document.title = `${a.name} · POP Forge` }, [a?.name])
  if (!data) return null
  if (!a) return <p className="muted" style={{ padding: 24 }}>{tr("Cet agent n’est plus connecté à la map.")}</p>
  const liste: any[] = Array.isArray(taches) ? taches : taches?.tasks ?? []
  const agir = async (f: () => Promise<any>, ok: string) => { try { const r = await f(); app.toast(r?.message ?? ok); reload(); relireTaches() } catch (e: any) { app.toast(e.message, true) } }
  return <div className="term-seul-fond"><Didacticiel page="forge-terminal" /><Fenetre mapId={mapId} a={a} ouvertes={liste.filter(t => t.status !== 'done')} agir={agir} style={{ height: '100vh', borderRadius: 0, border: 0 }} /></div>
}
