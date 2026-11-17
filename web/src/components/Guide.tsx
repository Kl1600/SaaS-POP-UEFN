// Boussole : guide virtuel du jour (règles simples côté serveur, pas d'IA).
import { useNavigate } from 'react-router-dom'
import { Compass, AlertCircle, AlertTriangle, CheckCircle2, Info, Play, ArrowRight, X } from 'lucide-react'
import { api } from '../api'
import { useApp, useLive, useLoad } from '../state'

const TONE: Record<string, [any, string]> = {
  bad: [AlertCircle, 'var(--tr)'], warn: [AlertTriangle, 'var(--to)'], good: [CheckCircle2, 'var(--tg)'], info: [Info, 'var(--text3)'],
}

export function GuidePanel({ compact, onNavigate }: { compact?: boolean; onNavigate?: () => void }) {
  const { map, me, toast } = useApp()
  const nav = useNavigate()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/guide` : null, [map?.id])
  useLive(['tasks', 'plan'], reload)
  if (!data) return null
  const today = new Date().toISOString().slice(0, 10)
  const take = async (s: any) => {
    try {
      if (!s.mine) await api.patch(`/api/tasks/${s.id}`, { assignee_id: me!.user.id, ...(s.planned_day ? {} : { planned_day: today }) })
      await api.post(`/api/tasks/${s.id}/status`, { status: 'doing' })
      toast(`#${s.number} est à vous, c’est parti`); reload()
    } catch (e: any) { toast(e.message, true) }
  }
  const open = (id: number) => { onNavigate?.(); nav(`/taches?t=${id}`) }
  return <div className="guide">
    <div className="guide-head">
      <span className="guide-ico"><Compass size={20} strokeWidth={1.75} /></span>
      <div style={{ minWidth: 0 }}>
        <div className="guide-name">Boussole <span className="small muted">· votre guide du jour</span></div>
        <div className="guide-hl">{data.headline}</div>
      </div>
    </div>
    {data.points.length > 0 && <ul className="guide-pts">{data.points.slice(0, compact ? 4 : 8).map((p: any, i: number) => { const [I, c] = TONE[p.tone] ?? TONE.info; return <li key={i}><I size={16} style={{ color: c, flex: 'none', marginTop: 2 }} /><span>{p.text}</span></li> })}</ul>}
    {data.pace && <div className="guide-pace" title={`${data.pace.remaining} tâche(s) restante(s), ${data.pace.per_day} finie(s) par jour en moyenne`}>
      <div className="row small" style={{ justifyContent: 'space-between' }}><span>Prochain jalon : <b style={{ fontWeight: 500 }}>{data.pace.title}</b></span><span className={'badge ' + (data.pace.on_track ? 'b-g' : 'b-r')}>{data.pace.on_track ? 'Dans les temps' : 'En retard'}</span></div>
    </div>}
    <div className="dsec" style={{ marginTop: 14 }}>À faire maintenant</div>
    {data.suggestions.length === 0 ? <p className="small muted" style={{ margin: 0 }}>Aucune tâche disponible pour vous. Créez-en une ou demandez à l’équipe.</p> :
      <div className="guide-sugs">{data.suggestions.map((s: any, i: number) => <div key={s.id} className={'guide-sug' + (i === 0 ? ' first' : '')}>
        <button className="guide-sug-main" onClick={() => open(s.id)}>
          <span className="guide-sug-t"><span className="mono muted">#{s.number}</span> {s.title}</span>
          <span className="small muted">{s.why.join(' · ')}</span>
        </button>
        {s.action === 'continue'
          ? <button className="btn" onClick={() => open(s.id)}><ArrowRight size={15} />Continuer</button>
          : <button className={'btn' + (i === 0 ? ' pri' : '')} onClick={() => take(s)}><Play size={14} />{s.mine ? 'Commencer' : 'Prendre'}</button>}
      </div>)}</div>}
    <p className="small muted" style={{ margin: '12px 0 0' }}>Boussole n’est pas une IA : elle lit vos tâches, leurs dates, leurs dépendances et vos jalons.</p>
  </div>
}

export function GuideDrawer({ onClose }: { onClose: () => void }) {
  return <>
    <div className="scrim" onClick={onClose} />
    <aside className="drawer" role="dialog" aria-label="Boussole">
      <div className="row" style={{ justifyContent: 'flex-end', marginBottom: -8 }}><button className="iconbtn" aria-label="Fermer" onClick={onClose}><X size={18} /></button></div>
      <GuidePanel onNavigate={onClose} />
    </aside>
  </>
}
