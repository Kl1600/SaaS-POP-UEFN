import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { AlertTriangle, Trash2 } from '../icons'
import { addDays, api, frDate, ymd } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { Avatar, CardHead, PageHead, Tabs, TAGS, tagLabel } from '../components/ui'
import { DEP_HELP, useMembers } from './Tasks'
import { tr } from '../i18n/t'
import { useNav } from '../routes'
import { useSearchParams } from 'react-router-dom'
import PlanCalendar from './PlanCalendar'
import { AvisFaisabilite, EquipeMap } from '../components/Faisabilite'

const GENRES: [string, string, string][] = [
  ['cr', tr("Gendarmes et voleurs"), tr("Équipes, braquages, boutique")], ['tycoon', tr("Tycoon"), tr("Production, améliorations")], ['boxfight', tr("Box fight / Zone wars"), tr("Manches courtes")],
  ['deathrun', tr("Deathrun / Parkour"), tr("Niveaux, checkpoints")], ['horreur', tr("Horreur / Histoire"), tr("Narration, ambiance")], ['course', tr("Course"), tr("Circuits, chronos")], ['party', tr("Party game"), tr("Mini-jeux")],
]

export default function Plan() {
  // Le Générateur de projet s'ouvre depuis le sélecteur de map (#194) : /planifier?vue=generateur.
  const [sp, setSp] = useSearchParams()
  const { map } = useApp()
  const wizard = sp.get('vue') === 'generateur'
  if (wizard) return (
    <main className="content">
      <PageHead title={tr("Générateur de projet")} help={tr("Découpe la map en tâches d'une heure, avec dépendances, jours, personnes et objectifs jusqu'à la sortie.")}>
        <button className="btn" onClick={() => setSp({})}>{tr("Retour au planning")}</button>
      </PageHead>
      <Wizard done={() => setSp({})} />
    </main>
  )
  const equipe = sp.get('vue') === 'equipe'
  return (
    <main className="content">
      <PageHead title={tr("Planifier")} help={tr("Objectifs, tâches et dates Fortnite sur un même calendrier. Glissez une tâche sur un jour pour la planifier.")}>
        <button className="btn" onClick={() => setSp(equipe ? {} : { vue: 'equipe' })}>{equipe ? tr("Retour au planning") : tr("Équipe et faisabilité")}</button>
      </PageHead>
      {/* Avis de faisabilité retiré du calendrier (#448) : il reste dans la vue Équipe et faisabilité. */}
      {equipe && map && <AvisFaisabilite mapId={map.id} />}
      {equipe && map ? <EquipeMap mapId={map.id} /> : <PlanCalendar />}
    </main>
  )
}

function Wizard({ done }: { done: () => void }) {
  const { map, isAdmin, toast } = useApp()
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
    try { const r = await api.post(`/api/maps/${map!.id}/plan/apply`, { tasks: plan.tasks, milestones: plan.milestones }); toast(tr("{0} tâches créées", [r.created])); done() } catch (e: any) { toast(e.message, true) } finally { setBusy(false) }
  }
  const setT = (k: number, p: any) => setPlan({ ...plan, tasks: plan.tasks.map((t: any) => (t.key === k ? { ...t, ...p } : t)) })
  const lots = plan ? Array.from(new Set(plan.tasks.map((t: any) => t.lot))) as string[] : []
  const name = (id: number) => mem?.members.find((m: any) => m.id === id)?.name ?? '—'

  if (plan) return <>
    <div className="tiles">
      <div className="card"><div className="lab">{tr("Tâches d'1 heure")}</div><div className="num">{plan.stats.tasks}</div></div>
      <div className="card"><div className="lab">{tr("Confiées à des agents IA")}</div><div className="num">{plan.stats.ai}</div></div>
      <div className="card"><div className="lab">{tr("Dernière tâche prévue")}</div><div className="num">{frDate(plan.stats.end)}</div><div className="muted">{plan.stats.late ? tr("après la date de soumission") : tr("avant la soumission")}</div></div>
      <div className="card"><div className="lab">{tr("Objectifs")}</div><div className="num">{plan.milestones.length}</div><div className="muted">{plan.milestones.map((m: any) => m.title).join(' · ')}</div></div>
    </div>
    <div className="row"><button className="btn" onClick={() => setPlan(null)}>{tr("Retour")}</button><span className="muted small">{tr("Tout est modifiable avant de valider.")}</span><span className="spacer" /><button className="btn pri" disabled={busy} onClick={apply}>{tr("Valider le plan")}</button></div>
    <p className="small muted" style={{ margin: 0 }}><b>{tr("Dépend de")}</b> · {tr(DEP_HELP)}</p>
    {lots.map(l => (
      <section key={l} className="card">
        <CardHead title={l} sub={tr("{0} tâches", [plan.tasks.filter((t: any) => t.lot === l).length])} />
        <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>{tr("Tâche (1 h)")}</th><th>{tr("Étiquette")}</th><th>{tr("Jour prévu")}</th><th title={tr(DEP_HELP)}>{tr("Dépend de")}</th><th>{tr("Assigné")}</th><th /></tr></thead><tbody>
          {plan.tasks.filter((t: any) => t.lot === l).map((t: any) => (
            <tr key={t.key}>
              <td><input className="field" style={{ width: '100%', minWidth: 220 }} value={t.title} onChange={e => setT(t.key, { title: e.target.value })} />{t.ai && <span className="badge" style={{ marginTop: 4 }}>IA</span>}</td>
              <td><select className="field" value={t.tag} onChange={e => setT(t.key, { tag: e.target.value })}>{TAGS.map(x => <option key={x} value={x}>{tagLabel(x)}</option>)}</select></td>
              <td><input className="field" type="date" value={t.planned_day ?? ''} onChange={e => setT(t.key, { planned_day: e.target.value })} /></td>
              <td className="muted small">{t.deps.map((d: number) => plan.tasks.find((x: any) => x.key === d)?.title).filter(Boolean).join(', ') || '—'}</td>
              <td><select className="field" value={t.assignee_id ?? ''} disabled={!isAdmin} title={isAdmin ? undefined : tr("Chaque membre prendra ses tâches lui-même")} onChange={e => setT(t.key, { assignee_id: e.target.value ? Number(e.target.value) : null })}><option value="">{tr("Personne")}</option>{mem?.members.map((m: any) => <option key={m.id} value={m.id}>{m.name}</option>)}</select></td>
              <td><button className="iconbtn" aria-label={tr("Retirer")} onClick={() => setPlan({ ...plan, tasks: plan.tasks.filter((x: any) => x.key !== t.key).map((x: any) => ({ ...x, deps: x.deps.filter((d: number) => d !== t.key) })) })}><Trash2 size={16} /></button></td>
            </tr>
          ))}
        </tbody></table></div>
      </section>
    ))}
  </>

  return (
    <section className="card">
      <CardHead title={tr("Générateur de projet")} sub={tr("Pour {0}", [map?.name])}>
        <ol className="steps">{[tr("Genre"), tr("Équipe"), tr("Agents IA"), tr("Sortie")].map((l, i) => <li key={l}><button className={'stp' + (step === i + 1 ? ' cur' : '')} onClick={() => setStep(i + 1)}><span className="stn">{i + 1}</span>{l}</button></li>)}</ol>
      </CardHead>
      {step === 1 && <>
        <h3 style={{ fontWeight: 500 }}>{tr("Quel genre de map ?")}</h3>
        <div className="opts">{GENRES.map(([k, l, h]) => <button key={k} className={'opt' + (genre === k ? ' sel' : '')} aria-pressed={genre === k} onClick={() => setGenre(k)}><b style={{ fontWeight: 500 }}>{l}</b><span className="small muted">{h}</span></button>)}</div>
        <label className="row" style={{ marginTop: 16 }}><input type="checkbox" checked={fromBoard} onChange={e => setFromBoard(e.target.checked)} />{tr("Ajouter les post-it votés du tableau blanc")}</label>
      </>}
      {step === 2 && <>
        <h3 style={{ fontWeight: 500 }}>{tr("Équipe : membres, rôles et heures par semaine")}</h3>
        <table className="tbl"><thead><tr><th /><th>{tr("Membre")}</th><th>{tr("Rôle dans le projet")}</th><th>{tr("Heures / semaine")}</th></tr></thead><tbody>
          {mem?.members.map((m: any) => { const v = team[m.id] ?? { on: true, tag: 'code', hours: 10 }; return (
            <tr key={m.id}><td><input type="checkbox" checked={v.on} onChange={e => setTeam({ ...team, [m.id]: { ...v, on: e.target.checked } })} aria-label={tr("Inclure {0}", [m.name])} /></td>
              <td><span className="row" style={{ gap: 8 }}><Avatar name={m.name} small />{m.name}</span></td>
              <td><select className="field" value={v.tag} onChange={e => setTeam({ ...team, [m.id]: { ...v, tag: e.target.value } })}><option value="code">{tr("Code Verse")}</option><option value="carte">{tr("Carte")}</option><option value="conception">{tr("Conception")}</option><option value="test">{tr("Test")}</option></select></td>
              <td><input className="field" type="number" min={0} max={80} style={{ width: 90 }} value={v.hours} onChange={e => setTeam({ ...team, [m.id]: { ...v, hours: Number(e.target.value) } })} /></td></tr>
          ) })}
        </tbody></table>
      </>}
      {step === 3 && <>
        <h3 style={{ fontWeight: 500 }}>{tr("Avec ou sans agents IA dans UEFN ?")}</h3>
        <div className="opts">
          <button className={'opt' + (ai ? ' sel' : '')} onClick={() => setAi(true)}><b style={{ fontWeight: 500 }}>{tr("Avec")}</b><span className="small muted">{tr("Les tâches répétitives (textes, habillage, traductions) sont marquées IA et relues par un humain.")}</span></button>
          <button className={'opt' + (!ai ? ' sel' : '')} onClick={() => setAi(false)}><b style={{ fontWeight: 500 }}>{tr("Sans")}</b><span className="small muted">{tr("Tout est planifié pour l'équipe.")}</span></button>
        </div>
      </>}
      {step === 4 && <>
        <h3 style={{ fontWeight: 500 }}>{tr("Date de sortie")}</h3>
        <input className="field" type="date" value={date} onChange={e => setDate(e.target.value)} aria-label={tr("Date de sortie")} />
      </>}
      <div className="row" style={{ justifyContent: 'space-between', marginTop: 24 }}>
        <button className="btn" disabled={step === 1} onClick={() => setStep(step - 1)}>{tr("Précédent")}</button>
        {step < 4 ? <button className="btn pri" onClick={() => setStep(step + 1)}>{tr("Suivant")}</button> : <button className="btn pri" disabled={busy} onClick={preview}>{tr("Générer le plan")}</button>}
      </div>
    </section>
  )
}

function MilestoneRow({ items }: { items: { id: number; title: string; date: string; x: number }[] }) {
  const track = useRef<HTMLDivElement>(null)
  const [lay, setLay] = useState<{ left: number; row: number }[] | null>(null)
  const [width, setWidth] = useState(0)
  useEffect(() => {
    const el = track.current; if (!el || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(() => setWidth(el.clientWidth)); ro.observe(el); return () => ro.disconnect()
  }, [])
  const key = items.map(m => `${m.id}:${m.title}:${m.x}`).join('|')
  useLayoutEffect(() => {
    const el = track.current; if (!el) return
    const tw = el.clientWidth
    const labs = Array.from(el.querySelectorAll<HTMLElement>('.dlab'))
    const gap = 8
    const placed = items.map((m, i) => {
      const w = labs[i]?.offsetWidth ?? 0
      return { i, w, x: m.x, left: Math.max(0, Math.min(tw - w, m.x * tw - w / 2)) }
    }).sort((a, b) => a.x - b.x || a.left - b.left) // ordre des dates : les rangées se lisent de gauche à droite
    const ends: number[] = []
    const out: { left: number; row: number }[] = new Array(items.length)
    for (const p of placed) {
      let r = ends.findIndex(e => p.left >= e + gap)
      if (r < 0) { r = ends.length; ends.push(0) }
      ends[r] = p.left + p.w
      out[p.i] = { left: p.left, row: r }
    }
    if (JSON.stringify(out) !== JSON.stringify(lay)) setLay(out)
  }, [key, width])
  const rows = lay ? Math.max(1, ...lay.map(l => l.row + 1)) : 1
  return (
    <div className="grow" style={{ minHeight: 34 + rows * 18 }}><span className="gname" style={{ fontWeight: 500 }}>{tr("Objectifs")}</span>
      <div className="gtrack" ref={track} style={{ height: 30 + rows * 18 }}>
        {items.map((m, i) => {
          const tip = `${m.title} · ${frDate(m.date)}`
          const l = lay?.[i]
          return <span key={m.id}>
            <span className="diamond" style={{ left: `${m.x * 100}%` }} title={tip} aria-label={tip} />
            <span className="dlab" title={tip} style={l ? { left: l.left, top: 26 + l.row * 18, transform: 'none' } : { left: `${m.x * 100}%`, visibility: 'hidden' }}>{m.title}</span>
          </span>
        })}
      </div>
    </div>
  )
}
