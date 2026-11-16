import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { AlertTriangle, Trash2 } from 'lucide-react'
import { addDays, api, frDate, ymd } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { Avatar, CardHead, PageHead, Tabs, TAGS } from '../components/ui'
import { useMembers } from './Tasks'

const GENRES: [string, string, string][] = [
  ['cr', 'Gendarmes et voleurs', 'Équipes, braquages, boutique'], ['tycoon', 'Tycoon', 'Production, améliorations'], ['boxfight', 'Box fight / Zone wars', 'Manches courtes'],
  ['deathrun', 'Deathrun / Parkour', 'Niveaux, checkpoints'], ['horreur', 'Horreur / Histoire', 'Narration, ambiance'], ['course', 'Course', 'Circuits, chronos'], ['party', 'Party game', 'Mini-jeux'],
]

export default function Plan() {
  const [tab, setTab] = useState<'timeline' | 'wizard' | 'seasons'>('timeline')
  return (
    <main className="content">
      <PageHead title="Planifier" help="Découpez la map en tâches d'une heure, placez-les dans le temps et soyez prévenu en cas de retard." />
      <Tabs value={tab} onChange={setTab} items={[['timeline', 'Planning'], ['wizard', 'Générateur de projet'], ['seasons', 'Saisons et mises à jour']]} />
      {tab === 'timeline' && <Timeline />}
      {tab === 'wizard' && <Wizard done={() => setTab('timeline')} />}
      {tab === 'seasons' && <Seasons />}
    </main>
  )
}

function Wizard({ done }: { done: () => void }) {
  const { map, toast } = useApp()
  const { data: mem } = useMembers()
  const { data: boards } = useLoad<any[]>(map ? `/api/maps/${map.id}/boards` : null, [map?.id])
  const [step, setStep] = useState(1)
  const [genre, setGenre] = useState('cr')
  const [team, setTeam] = useState<Record<number, { on: boolean; tag: string; hours: number }>>({})
  const [ai, setAi] = useState(true)
  const [date, setDate] = useState(map?.release_date ?? addDays(ymd(new Date()), 45))
  const [fromBoard, setFromBoard] = useState(true)
  const [plan, setPlan] = useState<any>(null)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    if (!mem) return
    const guess = (s: string) => /verse|code/i.test(s) ? 'code' : /carte|level/i.test(s) ? 'carte' : /test|qa/i.test(s) ? 'test' : 'conception'
    setTeam(Object.fromEntries(mem.members.map((m: any) => [m.id, { on: m.role !== 'viewer', tag: guess(m.specialty), hours: m.hours_per_week }])))
  }, [mem])
  const preview = async () => {
    setBusy(true)
    try {
      setPlan(await api.post(`/api/maps/${map!.id}/plan/preview`, {
        genre, ai, release_date: date, board_id: fromBoard ? boards?.[0]?.id : null,
        members: Object.entries(team).filter(([, v]) => v.on).map(([k, v]) => ({ user_id: Number(k), tag: v.tag, hours: v.hours })),
      }))
    } catch (e: any) { toast(e.message, true) } finally { setBusy(false) }
  }
  const apply = async () => {
    setBusy(true)
    try { const r = await api.post(`/api/maps/${map!.id}/plan/apply`, { tasks: plan.tasks, milestones: plan.milestones }); toast(`${r.created} tâches créées`); done() } catch (e: any) { toast(e.message, true) } finally { setBusy(false) }
  }
  const setT = (k: number, p: any) => setPlan({ ...plan, tasks: plan.tasks.map((t: any) => (t.key === k ? { ...t, ...p } : t)) })
  const lots = plan ? Array.from(new Set(plan.tasks.map((t: any) => t.lot))) as string[] : []
  const name = (id: number) => mem?.members.find((m: any) => m.id === id)?.name ?? '—'

  if (plan) return <>
    <div className="tiles">
      <div className="card"><div className="lab">Tâches d'1 heure</div><div className="num">{plan.stats.tasks}</div></div>
      <div className="card"><div className="lab">Confiées à des agents IA</div><div className="num">{plan.stats.ai}</div></div>
      <div className="card"><div className="lab">Dernière tâche prévue</div><div className="num">{frDate(plan.stats.end)}</div><div className="muted">{plan.stats.late ? 'après la date de soumission' : 'avant la soumission'}</div></div>
      <div className="card"><div className="lab">Jalons</div><div className="num">{plan.milestones.length}</div><div className="muted">{plan.milestones.map((m: any) => m.title).join(' · ')}</div></div>
    </div>
    <div className="row"><button className="btn" onClick={() => setPlan(null)}>Retour</button><span className="muted small">Tout est modifiable avant de valider.</span><span className="spacer" /><button className="btn pri" disabled={busy} onClick={apply}>Valider le plan</button></div>
    {lots.map(l => (
      <section key={l} className="card">
        <CardHead title={l} sub={`${plan.tasks.filter((t: any) => t.lot === l).length} tâches`} />
        <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Tâche (1 h)</th><th>Étiquette</th><th>Jour prévu</th><th>Dépend de</th><th>Assigné suggéré</th><th /></tr></thead><tbody>
          {plan.tasks.filter((t: any) => t.lot === l).map((t: any) => (
            <tr key={t.key}>
              <td><input className="field" style={{ width: '100%', minWidth: 220 }} value={t.title} onChange={e => setT(t.key, { title: e.target.value })} />{t.ai && <span className="badge" style={{ marginTop: 4 }}>IA</span>}</td>
              <td><select className="field" value={t.tag} onChange={e => setT(t.key, { tag: e.target.value })}>{TAGS.map(x => <option key={x}>{x}</option>)}</select></td>
              <td><input className="field" type="date" value={t.planned_day ?? ''} onChange={e => setT(t.key, { planned_day: e.target.value })} /></td>
              <td className="muted small">{t.deps.map((d: number) => plan.tasks.find((x: any) => x.key === d)?.title).filter(Boolean).join(', ') || '—'}</td>
              <td><select className="field" value={t.assignee_id ?? ''} onChange={e => setT(t.key, { assignee_id: Number(e.target.value) })}>{mem?.members.map((m: any) => <option key={m.id} value={m.id}>{m.name}</option>)}</select></td>
              <td><button className="iconbtn" aria-label="Retirer" onClick={() => setPlan({ ...plan, tasks: plan.tasks.filter((x: any) => x.key !== t.key).map((x: any) => ({ ...x, deps: x.deps.filter((d: number) => d !== t.key) })) })}><Trash2 size={16} /></button></td>
            </tr>
          ))}
        </tbody></table></div>
      </section>
    ))}
  </>

  return (
    <section className="card">
      <CardHead title="Générateur de projet" sub={`Pour ${map?.name}`}>
        <ol className="steps">{['Genre', 'Équipe', 'Agents IA', 'Sortie'].map((l, i) => <li key={l}><button className={'stp' + (step === i + 1 ? ' cur' : '')} onClick={() => setStep(i + 1)}><span className="stn">{i + 1}</span>{l}</button></li>)}</ol>
      </CardHead>
      {step === 1 && <>
        <h3 style={{ fontWeight: 500 }}>Quel genre de map ?</h3>
        <div className="opts">{GENRES.map(([k, l, h]) => <button key={k} className={'opt' + (genre === k ? ' sel' : '')} aria-pressed={genre === k} onClick={() => setGenre(k)}><b style={{ fontWeight: 500 }}>{l}</b><span className="small muted">{h}</span></button>)}</div>
        <label className="row" style={{ marginTop: 16 }}><input type="checkbox" checked={fromBoard} onChange={e => setFromBoard(e.target.checked)} />Ajouter les post-it votés du tableau blanc</label>
      </>}
      {step === 2 && <>
        <h3 style={{ fontWeight: 500 }}>Équipe : membres, rôles et heures par semaine</h3>
        <table className="tbl"><thead><tr><th /><th>Membre</th><th>Rôle dans le projet</th><th>Heures / semaine</th></tr></thead><tbody>
          {mem?.members.map((m: any) => { const v = team[m.id] ?? { on: true, tag: 'code', hours: 10 }; return (
            <tr key={m.id}><td><input type="checkbox" checked={v.on} onChange={e => setTeam({ ...team, [m.id]: { ...v, on: e.target.checked } })} aria-label={`Inclure ${m.name}`} /></td>
              <td><span className="row" style={{ gap: 8 }}><Avatar name={m.name} small />{m.name}</span></td>
              <td><select className="field" value={v.tag} onChange={e => setTeam({ ...team, [m.id]: { ...v, tag: e.target.value } })}><option value="code">Code Verse</option><option value="carte">Carte</option><option value="conception">Conception</option><option value="test">Test</option></select></td>
              <td><input className="field" type="number" min={0} max={80} style={{ width: 90 }} value={v.hours} onChange={e => setTeam({ ...team, [m.id]: { ...v, hours: Number(e.target.value) } })} /></td></tr>
          ) })}
        </tbody></table>
      </>}
      {step === 3 && <>
        <h3 style={{ fontWeight: 500 }}>Avec ou sans agents IA dans UEFN ?</h3>
        <div className="opts">
          <button className={'opt' + (ai ? ' sel' : '')} onClick={() => setAi(true)}><b style={{ fontWeight: 500 }}>Avec</b><span className="small muted">Les tâches répétitives (textes, habillage, traductions) sont marquées IA et relues par un humain.</span></button>
          <button className={'opt' + (!ai ? ' sel' : '')} onClick={() => setAi(false)}><b style={{ fontWeight: 500 }}>Sans</b><span className="small muted">Tout est planifié pour l'équipe.</span></button>
        </div>
      </>}
      {step === 4 && <>
        <h3 style={{ fontWeight: 500 }}>Date de sortie</h3>
        <input className="field" type="date" value={date} onChange={e => setDate(e.target.value)} aria-label="Date de sortie" />
      </>}
      <div className="row" style={{ justifyContent: 'space-between', marginTop: 24 }}>
        <button className="btn" disabled={step === 1} onClick={() => setStep(step - 1)}>Précédent</button>
        {step < 4 ? <button className="btn pri" onClick={() => setStep(step + 1)}>Suivant</button> : <button className="btn pri" disabled={busy} onClick={preview}>Générer le plan</button>}
      </div>
    </section>
  )
}

function Timeline() {
  const { map, canEdit, toast } = useApp()
  const nav = useNavigate()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/plan/timeline` : null, [map?.id])
  const [alerts, setAlerts] = useState<string[]>([])
  const [shifted, setShifted] = useState<number[]>([])
  const [hideDone, setHideDone] = useState(true)
  const [ms, setMs] = useState({ title: '', date: '' })
  const drag = useRef<any>(null)
  const [ghost, setGhost] = useState<{ id: number; dx: number } | null>(null)
  useLive(['tasks', 'plan'], reload)

  const days = useMemo(() => {
    if (!data) return []
    const start = addDays(data.today, -7)
    const end = [data.release_date, ...data.tasks.map((t: any) => t.planned_day), ...data.milestones.map((m: any) => m.date)].filter(Boolean).sort().pop() ?? addDays(data.today, 21)
    const n = Math.min(120, Math.max(21, Math.round((new Date(end + 'T12:00').getTime() - new Date(start + 'T12:00').getTime()) / 864e5) + 4))
    return Array.from({ length: n }, (_, i) => addDays(start, i))
  }, [data])
  if (!data) return null
  const W = 100 / days.length
  const pos = (d: string) => days.indexOf(d)
  const tasks = data.tasks.filter((t: any) => t.planned_day && (!hideDone || t.status !== 'done'))

  const onDown = (e: React.PointerEvent, t: any) => {
    if (!canEdit) return
    const track = (e.currentTarget.parentElement as HTMLElement).getBoundingClientRect()
    drag.current = { id: t.id, x0: e.clientX, px: track.width / days.length, day: t.planned_day }
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  }
  const onMove = (e: React.PointerEvent) => { const d = drag.current; if (d) setGhost({ id: d.id, dx: Math.round((e.clientX - d.x0) / d.px) }) }
  const onUp = async () => {
    const d = drag.current; drag.current = null
    if (!d || !ghost || ghost.dx === 0) { setGhost(null); return }
    try {
      const r = await api.post(`/api/maps/${map!.id}/plan/move`, { task_id: d.id, planned_day: addDays(d.day, ghost!.dx) })
      setAlerts(r.alerts); setShifted(r.shifted); reload()
    } catch (e: any) { toast(e.message, true) } finally { setGhost(null) }
  }

  return <>
    {alerts.length > 0 && <div className="warn" role="alert"><AlertTriangle size={18} /><div>{alerts.map((a, i) => <div key={i}>{a}</div>)}</div></div>}
    {shifted.length > 0 && alerts.length === 0 && <div className="warn"><span>Recalcul : #{shifted.join(', #')} décalées pour respecter les dépendances. Aucun jalon dépassé.</span></div>}
    <section className="card">
      <CardHead title="Chronologie" sub="Glissez une barre pour la déplacer : les tâches qui en dépendent et les jalons se recalculent.">
        <label className="row small"><input type="checkbox" checked={hideDone} onChange={e => setHideDone(e.target.checked)} />Masquer les tâches faites</label>
        {canEdit && <button className="btn" onClick={async () => { const r = await api.post(`/api/maps/${map!.id}/plan/recalc`); setAlerts(r.alerts); setShifted(r.shifted); reload() }}>Recalculer</button>}
      </CardHead>
      <div className="gwrap">
        <div className="gantt" style={{ minWidth: Math.max(900, days.length * 26 + 250) }} onPointerMove={onMove} onPointerUp={onUp}>
          <div className="grow"><span className="gname" /><div className="gtrack" style={{ height: 20 }}>{days.map((d, i) => (new Date(d + 'T12:00').getDay() === 1 || i === 0) && <span key={d} className="gday" style={{ left: `${i * W}%` }}>{frDate(d)}</span>)}</div></div>
          {tasks.map((t: any) => {
            const i = pos(t.planned_day) + (ghost && ghost.id === t.id ? ghost.dx : 0)
            return (
              <div key={t.id} className="grow">
                <span className="gname" title={t.title}><span className="mono muted small">#{t.number}</span> {t.title}</span>
                <div className="gtrack">
                  {days.map((d, k) => <span key={d} className={'gcell' + ([0, 6].includes(new Date(d + 'T12:00').getDay()) ? ' we' : '')} style={{ left: `${k * W}%`, width: `${W}%` }} />)}
                  <span className="today" style={{ left: `${(pos(data.today) + 0.5) * W}%` }} />
                  {i >= 0 && <div className={'gbar' + (t.critical ? ' crit' : '') + (t.status === 'done' ? ' done' : '')} style={{ left: `${i * W}%`, width: `max(${W}%, 26px)`, outline: shifted.includes(t.number) ? '2px dashed var(--text)' : undefined }}
                    onPointerDown={e => onDown(e, t)} onDoubleClick={() => nav(`/taches?q=%23${t.number}`)} title={`${t.title} · ${t.assignee ?? '—'} · ${frDate(t.planned_day)}${t.deps.length ? ' · dépend de ' + t.deps.length + ' tâche(s)' : ''}`}>
                    {t.assignee && <span className="mono">{t.assignee.split(' ').map((w: string) => w[0]).join('')}</span>}
                  </div>}
                </div>
              </div>
            )
          })}
          <div className="grow" style={{ minHeight: 50 }}><span className="gname" style={{ fontWeight: 500 }}>Jalons</span><div className="gtrack">
            {data.milestones.map((m: any) => pos(m.date) >= 0 && <span key={m.id}><span className="diamond" style={{ left: `${(pos(m.date) + 0.5) * W}%` }} /><span className="dlab" style={{ left: `${(pos(m.date) + 0.5) * W}%` }}>{m.title}</span></span>)}
          </div></div>
        </div>
      </div>
      <p className="small muted">Barres pleines : chemin critique. Ligne rouge : aujourd'hui. Double-clic sur une barre : ouvrir la tâche.</p>
    </section>
    <div className="two">
      <section className="card">
        <CardHead title="Charge par personne · cette semaine" sub="Heures planifiées face aux heures déclarées" />
        <div className="stack">
          {data.load.map((l: any) => (
            <div key={l.id} className="row" style={{ flexWrap: 'wrap' }}>
              <span className="row" style={{ gap: 8, width: 170 }}><Avatar name={l.name} small />{l.name}</span>
              <div className="bar" style={{ flex: 1, height: 8 }}><span style={{ width: `${Math.min(100, (l.planned_h / Math.max(1, l.capacity_h)) * 100)}%` }} /></div>
              <span className="mono small" style={{ width: 70, textAlign: 'right' }}>{l.planned_h} / {l.capacity_h} h</span>
              {l.overload ? <span className="badge b-r">Surcharge</span> : <span className="badge">OK</span>}
            </div>
          ))}
        </div>
      </section>
      <section className="card">
        <CardHead title="Jalons" sub="Fin d'étape, tests à plusieurs, soumission, sortie" />
        <div className="list">{data.milestones.map((m: any) => <div key={m.id} className="li"><span className="mono small" style={{ width: 90 }}>{frDate(m.date)}</span><span style={{ flex: 1 }}>{m.title}</span>{canEdit && <button className="iconbtn" aria-label="Supprimer" onClick={async () => { await api.del(`/api/milestones/${m.id}`); reload() }}><Trash2 size={16} /></button>}</div>)}</div>
        {canEdit && <form className="row" style={{ marginTop: 12, flexWrap: 'wrap' }} onSubmit={async e => { e.preventDefault(); try { await api.post(`/api/maps/${map!.id}/milestones`, ms); setMs({ title: '', date: '' }); reload() } catch (er: any) { toast(er.message, true) } }}>
          <input className="field" placeholder="Nouveau jalon" value={ms.title} onChange={e => setMs({ ...ms, title: e.target.value })} required /><input className="field" type="date" value={ms.date} onChange={e => setMs({ ...ms, date: e.target.value })} required /><button className="btn">Ajouter</button>
        </form>}
      </section>
    </div>
  </>
}

function Seasons() {
  const { map } = useApp()
  const { data } = useLoad<any[]>(map ? `/api/maps/${map.id}/seasons` : null, [map?.id])
  const months = useMemo(() => {
    const m = new Map<string, any[]>()
    data?.forEach(e => { const k = e.date.slice(0, 7); m.set(k, [...(m.get(k) ?? []), e]) })
    return [...m.entries()]
  }, [data])
  const label: Record<string, [string, string]> = { map: ['Votre map', ''], aim: ['À viser', 'b-g'], avoid: ['À éviter', 'b-r'], money: ['Impact revenus', 'b-o'] }
  return (
    <section className="card">
      <CardHead title="Calendrier des mises à jour et des saisons" sub="Vos jalons face aux dates Fortnite à viser ou à éviter. Les dates Epic sont indicatives : vérifiez les annonces officielles." />
      <div className="grid2" style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))' }}>
        {months.map(([k, evs]) => (
          <div key={k} style={{ border: '1px solid var(--border)', borderRadius: 14, padding: 14 }}>
            <div style={{ fontWeight: 500, marginBottom: 10 }}>{frDate(k + '-01', { month: 'long', year: 'numeric' })}</div>
            <div className="stack" style={{ gap: 8 }}>{evs.map((e, i) => <div key={i}><div className="row" style={{ gap: 6 }}><span className="mono small">{frDate(e.date)}</span><span className={'badge ' + label[e.kind][1]}>{label[e.kind][0]}</span></div><div>{e.title}</div></div>)}</div>
          </div>
        ))}
      </div>
    </section>
  )
}
