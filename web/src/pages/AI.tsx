// IA qui travaillent sur la map ouverte (#145) : celles dont c'est la map de travail, avec leur tâche réservée et
// leur dernière action. Affiché dans Paramètres → IA connectées, sous les IA du compte.
import { Lock, Unlock } from 'lucide-react'
import { ago } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead } from '../components/ui'
import { ClientLogo, clientName } from '../components/AiTokens'
import { tr } from '../i18n/t'

export function MapAgents() {
  const { map } = useApp()
  const { data, reload } = useLoad<any[]>(map ? `/api/maps/${map.id}/agents` : null, [map?.id])
  useLive(['ai'], reload)
  if (!map) return null
  return (
    <section className="card">
      <CardHead title={tr("IA sur « {0} »", [map.name])} sub={tr("Les IA dont c’est la map de travail en ce moment, avec la tâche réservée et la dernière action. Pour en mettre une sur cette map, demandez-lui de travailler sur « {0} » : elle la choisit elle-même.", [map.name])} />
      {data?.length === 0 && <p className="muted">{tr("Aucune IA ne travaille sur cette map pour l’instant.")}</p>}
      <div className="list">{data?.map(t => (
        <div key={t.id} className="li" style={{ flexWrap: 'wrap' }}>
          <ClientLogo kind={t.kind} />
          <span style={{ flex: 1, minWidth: 180 }}>{t.name}<span className="small muted"> · {clientName(t.kind)} · {t.owner}</span>{(t.task || t.last_action) && <span className="small muted" style={{ display: 'block' }}>{t.task ? tr("Tâche #{0} · {1}", [t.task.number, t.task.title]) : ''}{t.task && t.last_action ? ' · ' : ''}{t.last_action ?? ''}</span>}</span>
          <span className={'badge ' + (t.last_used ? 'b-g' : '')}>{t.last_used ? tr("Utilisée {0} · {1} appel{2}", [ago(t.last_used), t.calls, t.calls > 1 ? 's' : '']) : tr("Jamais utilisée")}</span>
          <span className="badge">{t.scope === 'read' ? <><Lock size={12} /> {tr("Lecture seule")}</> : <><Unlock size={12} /> {tr("Peut modifier")}</>}</span>
        </div>
      ))}</div>
    </section>
  )
}
