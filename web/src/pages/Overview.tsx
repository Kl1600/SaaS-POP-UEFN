import { GuidePanel } from '../components/Guide'
import { useEffect, useRef, useState } from 'react'
import {
  Clock, Ban, CheckSquare, Flag, Coins, Users, Zap, Activity, ArrowRight, Upload, FileText, PenTool, AlertTriangle, MessageSquare, CheckCircle2, Rocket,
} from 'lucide-react'
import { ago, api, euros, frDate } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead, Empty, PageHead, tagLabel } from '../components/ui'
import { tr, LOCALE, LANG, tx } from '../i18n/t'
import { useNav } from '../routes'

const ICON: Record<string, [any, string]> = {
  checkin: [Upload, 'i-g'], board: [PenTool, 'i-n'], page: [FileText, 'i-n'], blocked: [Ban, 'i-r'], done: [CheckCircle2, 'i-g'],
  playtest: [Users, 'i-b'], comment: [MessageSquare, 'i-n'], money: [Coins, 'i-o'], alert: [AlertTriangle, 'i-r'], plan: [Rocket, 'i-b'], task: [CheckSquare, 'i-n'], map: [Rocket, 'i-b'],
}

export default function Overview() {
  const { map, toast } = useApp()
  const nav = useNav()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/overview` : null, [map?.id])
  // Tâches cochées ici : restent affichées (barrées) pour pouvoir annuler.
  const [checked, setChecked] = useState<Record<number, { item: any; prev: string }>>({})
  useEffect(() => setChecked({}), [map?.id])
  const undoRef = useRef<(id: number) => void>(() => {})
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
  const undo = async (id: number) => {
    const c = checked[id]; if (!c) return
    try { await api.post(`/api/tasks/${id}/status`, { status: c.prev }); setChecked(s => { const n = { ...s }; delete n[id]; return n }); reload(); toast(tr("#{0} n’est plus marquée comme faite", [c.item.number])) } catch (e: any) { toast(e.message, true) }
  }
  const check = async (x: any) => {
    try {
      await api.post(`/api/tasks/${x.id}/status`, { status: 'done' })
      setChecked(s => ({ ...s, [x.id]: { item: x, prev: x.status } })); reload()
      toast(tr("#{0} marquée comme faite", [x.number]), false, { label: tr("Annuler"), run: () => undoRef.current(x.id) })
    } catch (e: any) { toast(e.message, true) }
  }
  undoRef.current = undo
  const todayList = [...data.today, ...Object.values(checked).map(c => c.item).filter(i => !data.today.some((x: any) => x.id === i.id))]

  return (
    <main className="content">
      <PageHead title={tr("Vue d'ensemble")} help={tr("Où en est {0}, en un coup d'œil.", [map?.name])}>
        <button className="btn pri" onClick={() => nav('/taches')}>{tr("Ouvrir les tâches")}</button>
      </PageHead>

      <section className="card"><GuidePanel compact /></section>

      <section className="tiles" aria-label={tr("Indicateurs")}>
        <div className="card"><div className="lab"><Clock size={18} strokeWidth={1.5} />{tr("Tâches en cours")}</div><div className="num">{t.doing}</div><div className="muted">{tr("{0} / {1} finies au total", [t.done, t.total])}</div></div>
        <div className="card"><div className="lab"><Ban size={18} strokeWidth={1.5} />{tr("Bloquées")}</div><div className="num">{t.blocked}</div><div className="muted">{t.blocked ? tr("voir « qui bloque qui »") : tr("rien ne bloque")}</div></div>
        <div className="card"><div className="lab"><CheckSquare size={18} strokeWidth={1.5} />{tr("Finies cette semaine")}</div><div className="num">{t.done_week}</div><div className="muted">{tr("{0}{1} vs semaine dernière", [diffWeek >= 0 ? '+' : '', diffWeek])}</div></div>
        <div className="card"><div className="lab"><Flag size={18} strokeWidth={1.5} />{tr("Prochain jalon")}</div><div className="num">{next ? tr('J-{0}', [days]) : '—'}</div><div className="muted">{next ? `${next.title} · ${frDate(next.date, { weekday: 'short', day: 'numeric', month: 'short' })}` : tr("aucun jalon à venir")}</div></div>
        <div className="card"><div className="lab"><Coins size={18} strokeWidth={1.5} />{tr("Gains du dernier mois")}</div><div className="num">{t.revenue ? euros(t.revenue.cents, 0) : '—'}</div><div className="muted">{t.revenue ? `${frDate(t.revenue.month + '-01', { month: 'long', year: 'numeric' })}${t.revenue_prev_cents ? ` · ${t.revenue.cents >= t.revenue_prev_cents ? '+' : ''}${Math.round((t.revenue.cents / t.revenue_prev_cents - 1) * 100)} %` : ''}` : tr("aucun import")}</div></div>
        <div className="card"><div className="lab"><Users size={18} strokeWidth={1.5} />{tr("Joueurs · dernier playtest")}</div><div className="num">{t.playtest ? t.playtest.players : '—'}</div><div className="muted">{t.playtest ? tr("décrochage moyen à {0} min", [Math.round(t.playtest.avg_minutes)]) : tr("aucun playtest")}</div></div>
      </section>

      <div className="two">
        <section className="card">
          <CardHead icon={<Zap size={22} strokeWidth={1.5} />} title={tr("À faire aujourd'hui")} sub={tr("Vos tâches d'une heure prévues pour aujourd'hui et demain")} />
          {todayList.length === 0 ? <Empty text={tr("Rien de prévu pour vous aujourd'hui.")} action={<button className="btn" onClick={() => nav('/taches')}>{tr("Prendre une tâche")}</button>} /> : (
            <div className="list">
              {todayList.map((x: any) => { const done = !!checked[x.id]; return (
                <div key={x.id} className={'li' + (done ? ' li-done' : '')}>
                  <input type="checkbox" className="chk" aria-label={done ? tr("Annuler : #{0} n’est pas faite", [x.number]) : tr("Marquer #{0} comme fait", [x.number])} checked={done} onChange={() => done ? undo(x.id) : check(x)} />
                  <span style={{ flex: 1, minWidth: 0 }}>
                    <span style={{ fontWeight: 500, textDecoration: done ? 'line-through' : undefined, opacity: done ? 0.55 : 1 }}><span className="mono muted">#{x.number}</span> {x.title}</span>
                    <span className="small muted" style={{ display: 'block' }}>{x.status === 'doing' ? tr("En cours") : tr("Prévu {0}", [frDate(x.planned_day)])}{x.waiting_on.length ? tr(" · attend #{0}", [x.waiting_on.join(', #')]) : ''}</span>
                  </span>
                  <span className="badge">{tagLabel(x.tag)}</span>
                  {done ? <button className="btn" onClick={() => undo(x.id)}>{tr("Annuler")}</button>
                    : x.status === 'todo' && !x.waiting_on.length && <button className="btn" onClick={() => startTask(x.id, 'doing')}>{tr("Commencer")}</button>}
                  <ArrowRight size={18} strokeWidth={1.5} className="muted" />
                </div>
              ) })}
            </div>
          )}
        </section>
        <section className="card">
          <CardHead icon={<Activity size={22} strokeWidth={1.5} />} title={tr("Journal d'activités")} sub={tr("Dernières actions et événements de l'équipe")} />
          <div className="list feed">
            {data.activity.map((a: any, i: number) => {
              const [I, cls] = ICON[a.kind] ?? [Activity, 'i-n']
              return (
                <div key={i} className="li">
                  <span className={'ico ' + cls}><I size={20} strokeWidth={1.5} /></span>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div><span>{tx(a.title)}</span> <span className="small muted">• {ago(a.at)}</span></div>
                    <div className="muted">{tx(a.detail)}{a.who ? ` · ${a.who}` : ''}</div>
                  </div>
                </div>
              )
            })}
          </div>
        </section>
      </div>

      <div className="two">
        <section className="card">
          <CardHead title={tr("Jalons · {0}", [now.toLocaleDateString(LOCALE, { month: 'long', year: 'numeric' })])} sub={tr("Aujourd'hui en noir, jalons encadrés")} />
          <div className="cal">
            {(LANG === 'fr' ? ['L', 'M', 'M', 'J', 'V', 'S', 'D'] : ['M', 'T', 'W', 'T', 'F', 'S', 'S']).map((d, i) => <span key={i} className="dn">{d}</span>)}
            {cells.map((d, i) => {
              const k = ymd(d)
              const cls = d.getMonth() !== now.getMonth() ? 'out' : k === ymd(now) ? 'today' : msDays.has(k) ? 'ms' : ''
              return <div key={i} className={'d ' + cls} title={data.milestones.filter((m: any) => m.date === k).map((m: any) => m.title).join(', ')}>{d.getDate()}</div>
            })}
          </div>
        </section>
        <section className="card">
          <CardHead title={tr("Prochains jalons")} sub={tr("Recalculés quand une tâche prend du retard")}><button className="btn" onClick={() => nav('/planifier')}>{tr("Planning")}</button></CardHead>
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
