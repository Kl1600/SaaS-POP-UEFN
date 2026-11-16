import { useNavigate } from 'react-router-dom'
import {
  Clock, Ban, CheckSquare, Flag, Coins, Users, Zap, Activity, ArrowRight, Upload, FileText, PenTool, AlertTriangle, MessageSquare, CheckCircle2, Rocket,
} from 'lucide-react'
import { ago, api, euros, frDate } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead, Empty, PageHead } from '../components/ui'

const ICON: Record<string, [any, string]> = {
  checkin: [Upload, 'i-g'], board: [PenTool, 'i-n'], page: [FileText, 'i-n'], blocked: [Ban, 'i-r'], done: [CheckCircle2, 'i-g'],
  playtest: [Users, 'i-b'], comment: [MessageSquare, 'i-n'], money: [Coins, 'i-o'], alert: [AlertTriangle, 'i-r'], plan: [Rocket, 'i-b'], task: [CheckSquare, 'i-n'], map: [Rocket, 'i-b'],
}

export default function Overview() {
  const { map, toast } = useApp()
  const nav = useNavigate()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/overview` : null, [map?.id])
  useLive(['tasks', 'activity', 'plan'], reload)
  if (!data) return null
  const t = data.tiles
  const next = t.next_milestone
  const days = next ? Math.max(0, Math.ceil((new Date(next.date + 'T18:00:00').getTime() - Date.now()) / 864e5)) : null
  const diffWeek = t.done_week - t.done_prev_week
  const msDays = new Set(data.milestones.map((m: any) => m.date))
  const now = new Date()
  const first = new Date(now.getFullYear(), now.getMonth(), 1)
  const start = new Date(first); start.setDate(1 - ((first.getDay() + 6) % 7))
  const cells = Array.from({ length: 42 }, (_, i) => { const d = new Date(start); d.setDate(start.getDate() + i); return d })
  const ymd = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`

  const startTask = async (id: number, status: string) => {
    try { await api.post(`/api/tasks/${id}/status`, { status }); reload() } catch (e: any) { toast(e.message, true) }
  }

  return (
    <main className="content">
      <PageHead title="Vue d'ensemble" help={`Où en est ${map?.name}, en un coup d'œil.`}>
        <button className="btn pri" onClick={() => nav('/taches')}>Ouvrir les tâches</button>
      </PageHead>

      <section className="tiles" aria-label="Indicateurs">
        <div className="card"><div className="lab"><Clock size={18} strokeWidth={1.5} />Tâches en cours</div><div className="num">{t.doing}</div><div className="muted">{t.done} / {t.total} finies au total</div></div>
        <div className="card"><div className="lab"><Ban size={18} strokeWidth={1.5} />Bloquées</div><div className="num">{t.blocked}</div><div className="muted">{t.blocked ? 'voir « qui bloque qui »' : 'rien ne bloque'}</div></div>
        <div className="card"><div className="lab"><CheckSquare size={18} strokeWidth={1.5} />Finies cette semaine</div><div className="num">{t.done_week}</div><div className="muted">{diffWeek >= 0 ? '+' : ''}{diffWeek} vs semaine dernière</div></div>
        <div className="card"><div className="lab"><Flag size={18} strokeWidth={1.5} />Prochain jalon</div><div className="num">{next ? `J-${days}` : '—'}</div><div className="muted">{next ? `${next.title} · ${frDate(next.date, { weekday: 'short', day: 'numeric', month: 'short' })}` : 'aucun jalon à venir'}</div></div>
        <div className="card"><div className="lab"><Coins size={18} strokeWidth={1.5} />Gains du dernier mois</div><div className="num">{t.revenue ? euros(t.revenue.cents, 0) : '—'}</div><div className="muted">{t.revenue ? `${frDate(t.revenue.month + '-01', { month: 'long', year: 'numeric' })}${t.revenue_prev_cents ? ` · ${t.revenue.cents >= t.revenue_prev_cents ? '+' : ''}${Math.round((t.revenue.cents / t.revenue_prev_cents - 1) * 100)} %` : ''}` : 'aucun import'}</div></div>
        <div className="card"><div className="lab"><Users size={18} strokeWidth={1.5} />Joueurs · dernier playtest</div><div className="num">{t.playtest ? t.playtest.players : '—'}</div><div className="muted">{t.playtest ? `décrochage moyen à ${Math.round(t.playtest.avg_minutes)} min` : 'aucun playtest'}</div></div>
      </section>

      <div className="two">
        <section className="card">
          <CardHead icon={<Zap size={22} strokeWidth={1.5} />} title="À faire aujourd'hui" sub="Vos tâches d'une heure prévues pour aujourd'hui et demain" />
          {data.today.length === 0 ? <Empty text="Rien de prévu pour vous aujourd'hui." action={<button className="btn" onClick={() => nav('/taches')}>Prendre une tâche</button>} /> : (
            <div className="list">
              {data.today.map((x: any) => (
                <div key={x.id} className="li">
                  <input type="checkbox" className="chk" aria-label={`Marquer #${x.number} comme fait`} checked={false} onChange={() => startTask(x.id, 'done')} />
                  <span style={{ flex: 1, minWidth: 0 }}>
                    <span style={{ fontWeight: 500 }}><span className="mono muted">#{x.number}</span> {x.title}</span>
                    <span className="small muted" style={{ display: 'block' }}>{x.status === 'doing' ? 'En cours' : `Prévu ${frDate(x.planned_day)}`}{x.waiting_on.length ? ` · attend #${x.waiting_on.join(', #')}` : ''}</span>
                  </span>
                  <span className="badge">{x.tag}</span>
                  {x.status === 'todo' && !x.waiting_on.length && <button className="btn" onClick={() => startTask(x.id, 'doing')}>Commencer</button>}
                  <ArrowRight size={18} strokeWidth={1.5} className="muted" />
                </div>
              ))}
            </div>
          )}
        </section>
        <section className="card">
          <CardHead icon={<Activity size={22} strokeWidth={1.5} />} title="Journal d'activités" sub="Dernières actions et événements de l'équipe" />
          <div className="list feed">
            {data.activity.map((a: any, i: number) => {
              const [I, cls] = ICON[a.kind] ?? [Activity, 'i-n']
              return (
                <div key={i} className="li">
                  <span className={'ico ' + cls}><I size={20} strokeWidth={1.5} /></span>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div><span>{a.title}</span> <span className="small muted">• {ago(a.at)}</span></div>
                    <div className="muted">{a.detail}{a.who ? ` · ${a.who}` : ''}</div>
                  </div>
                </div>
              )
            })}
          </div>
        </section>
      </div>

      <div className="two">
        <section className="card">
          <CardHead title={`Jalons · ${now.toLocaleDateString('fr-FR', { month: 'long', year: 'numeric' })}`} sub="Aujourd'hui en noir, jalons encadrés" />
          <div className="cal">
            {['L', 'M', 'M', 'J', 'V', 'S', 'D'].map((d, i) => <span key={i} className="dn">{d}</span>)}
            {cells.map((d, i) => {
              const k = ymd(d)
              const cls = d.getMonth() !== now.getMonth() ? 'out' : k === ymd(now) ? 'today' : msDays.has(k) ? 'ms' : ''
              return <div key={i} className={'d ' + cls} title={data.milestones.filter((m: any) => m.date === k).map((m: any) => m.title).join(', ')}>{d.getDate()}</div>
            })}
          </div>
        </section>
        <section className="card">
          <CardHead title="Prochains jalons" sub="Recalculés quand une tâche prend du retard"><button className="btn" onClick={() => nav('/planifier')}>Planning</button></CardHead>
          <div className="list">
            {data.milestones.filter((m: any) => m.date >= ymd(now)).map((m: any) => (
              <div key={m.id} className="li">
                <div style={{ width: 52, textAlign: 'center' }}><div style={{ fontSize: 22, fontWeight: 500 }}>{new Date(m.date + 'T12:00:00').getDate()}</div><div className="small muted">{frDate(m.date, { month: 'short' })}</div></div>
                <div style={{ flex: 1 }}>{m.title}</div>
              </div>
            ))}
          </div>
        </section>
      </div>
    </main>
  )
}
