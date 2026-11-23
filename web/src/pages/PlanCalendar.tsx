// Calendrier de Planifier (#193, #257), façon Calendrier d'Apple dans la DA de POP : colonne de gauche avec
// mini-mois, liste des calendriers (filtres mémorisés) et tâches à planifier ; vues Semaine, Mois et Année.
// Remplace l'ancien onglet Planning : une tâche se planifie en la glissant sur un jour.
import { useMemo, useState } from 'react'
import { AlertTriangle, ChevronLeft, ChevronRight, Plus, RefreshCw, Trash2, X } from 'lucide-react'
import { addDays, api, frDate, ymd } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { Modal } from '../components/ui'
import { tr } from '../i18n/t'
import { useNav } from '../routes'

type Tag = 'objective' | 'release' | 'task' | 'aim' | 'avoid' | 'money'
type Ev = { key: string; date: string; title: string; tag: Tag; id?: number; number?: number; status?: string; who?: string }
type View = 'week' | 'month' | 'year'

const TAG_INFO: Record<Tag, string> = {
  objective: tr("Objectifs de la map"), release: tr("Sortie"), task: tr("Tâches"),
  aim: tr("Dates Fortnite à viser"), avoid: tr("Dates à éviter"), money: tr("Impact revenus"),
}
const ORDER: Tag[] = ['release', 'objective', 'avoid', 'aim', 'money', 'task']
const FKEY = 'pop.cal.filters'
const DKEY = 'pop.cal.done'
const WEEKDAYS = [tr("lun."), tr("mar."), tr("mer."), tr("jeu."), tr("ven."), tr("sam."), tr("dim.")]
const MINI = ['L', 'M', 'M', 'J', 'V', 'S', 'D']

const monday = (d: string) => { const x = new Date(d + 'T12:00'); return addDays(d, -((x.getDay() + 6) % 7)) }
const first = (d: string) => d.slice(0, 8) + '01'
const shiftMonth = (d: string, n: number) => { const x = new Date(first(d) + 'T12:00'); x.setMonth(x.getMonth() + n); return ymd(x) }
const cap = (t: string) => t.charAt(0).toUpperCase() + t.slice(1)
const monthDays = (d: string) => Array.from({ length: 42 }, (_, i) => addDays(monday(first(d)), i))
const store = (k: string, v: unknown) => { try { localStorage.setItem(k, JSON.stringify(v)) } catch { /* stockage indisponible */ } }
const read = <T,>(k: string, def: T): T => { try { const v = localStorage.getItem(k); return v === null ? def : JSON.parse(v) } catch { return def } }

export default function PlanCalendar() {
  const { map, canEdit, toast } = useApp()
  const nav = useNav()
  const { data: tl, reload } = useLoad<any>(map ? `/api/maps/${map.id}/plan/timeline` : null, [map?.id])
  const { data: seasons } = useLoad<any[]>(map ? `/api/maps/${map.id}/seasons` : null, [map?.id])
  useLive(['tasks', 'plan'], reload)
  const today = ymd(new Date())
  const [view, setView] = useState<View>('month')
  const [anchor, setAnchor] = useState(today)
  const [mini, setMini] = useState(today)
  const [open, setOpen] = useState<Ev | null>(null)
  const [hidden, setHidden] = useState<Set<Tag>>(() => new Set(read<Tag[]>(FKEY, [])))
  const [hideDone, setHideDone] = useState<boolean>(() => read(DKEY, true))
  const [over, setOver] = useState<string | null>(null)
  const [alerts, setAlerts] = useState<string[]>([])
  const toggle = (t: Tag) => { const h = new Set(hidden); h.has(t) ? h.delete(t) : h.add(t); setHidden(h); store(FKEY, [...h]) }
  const go = (d: string) => { setAnchor(d); setMini(d) }

  const events = useMemo(() => {
    const out: Ev[] = []
    for (const m of tl?.milestones ?? []) out.push({ key: 'm' + m.id, date: m.date, title: m.title, tag: 'objective', id: m.id })
    if (tl?.release_date) out.push({ key: 'release', date: tl.release_date, title: tr("Sortie de {0}", [map?.name ?? '']), tag: 'release' })
    for (const t of tl?.tasks ?? []) if (t.planned_day && !(hideDone && t.status === 'done')) out.push({ key: 't' + t.id, date: t.planned_day, title: `#${t.number} ${t.title}`, tag: 'task', id: t.id, number: t.number, status: t.status, who: t.assignee ?? undefined })
    for (const [i, e] of (seasons ?? []).entries()) if (e.kind !== 'map') out.push({ key: 's' + i, date: e.date, title: e.title, tag: e.kind as Tag })
    return out
  }, [tl, seasons, map?.name, hideDone])
  const byDay = useMemo(() => {
    const m = new Map<string, Ev[]>()
    for (const e of events) if (!hidden.has(e.tag)) m.set(e.date, [...(m.get(e.date) ?? []), e])
    for (const l of m.values()) l.sort((a, b) => ORDER.indexOf(a.tag) - ORDER.indexOf(b.tag))
    return m
  }, [events, hidden])
  const counts = useMemo(() => Object.fromEntries(ORDER.map(t => [t, events.filter(e => e.tag === t).length])), [events])
  const unplanned = (tl?.tasks ?? []).filter((t: any) => !t.planned_day && t.status !== 'done')

  const move = (dir: number) => {
    if (view === 'week') return go(addDays(anchor, dir * 7))
    if (view === 'year') return go(shiftMonth(anchor, dir * 12))
    go(shiftMonth(anchor, dir))
  }
  const [main, sub] = view === 'year' ? [anchor.slice(0, 4), '']
    : view === 'week' ? [cap(frDate(monday(anchor), { month: 'long' })), tr("semaine du {0}", [frDate(monday(anchor), { day: 'numeric', month: 'short' })])]
    : [cap(frDate(first(anchor), { month: 'long' })), anchor.slice(0, 4)]

  const addObjective = async (date?: string) => {
    if (!canEdit) return
    const t = prompt(date ? tr("Nouvel objectif le {0}", [frDate(date, { day: 'numeric', month: 'long' })]) : tr("Nouvel objectif"))?.trim(); if (!t) return
    const d = date ?? prompt(tr("Date (AAAA-MM-JJ)"), today)?.trim(); if (!d) return
    try { await api.post(`/api/maps/${map!.id}/milestones`, { title: t, date: d }); reload(); toast(tr("Objectif ajouté")) } catch (er: any) { toast(er.message, true) }
  }
  // Glisser une tâche sur un jour = la planifier ce jour-là (le serveur recalcule les tâches qui en dépendent).
  const plan = async (taskId: number, day: string) => {
    try {
      const r = await api.post(`/api/maps/${map!.id}/plan/move`, { task_id: taskId, planned_day: day })
      setAlerts(r.alerts ?? []); reload()
      toast(r.shifted?.length ? tr("Tâche planifiée le {0} · #{1} décalées pour respecter les dépendances", [frDate(day, { day: 'numeric', month: 'short' }), r.shifted.join(', #')]) : tr("Tâche planifiée le {0}", [frDate(day, { day: 'numeric', month: 'short' })]))
    } catch (er: any) { toast(er.message, true) }
  }
  const dropProps = (d: string) => canEdit ? {
    onDragOver: (e: React.DragEvent) => { if (e.dataTransfer.types.includes('text/pop-task')) { e.preventDefault(); if (over !== d) setOver(d) } },
    onDragLeave: (e: React.DragEvent) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver(o => o === d ? null : o) },
    onDrop: (e: React.DragEvent) => { e.preventDefault(); setOver(null); const id = Number(e.dataTransfer.getData('text/pop-task')); if (id) plan(id, d) },
  } : {}
  const dragTask = (id: number) => canEdit ? {
    draggable: true,
    onDragStart: (e: React.DragEvent) => { e.dataTransfer.setData('text/pop-task', String(id)); e.dataTransfer.effectAllowed = 'move' },
  } : {}

  const Pill = ({ e }: { e: Ev }) => <button className={'cev t-' + e.tag + (e.status === 'done' ? ' done' : '')} title={`${TAG_INFO[e.tag]} · ${e.title}`}
    onClick={ev => { ev.stopPropagation(); setOpen(e) }} {...(e.tag === 'task' && e.id ? dragTask(e.id) : {})}><i />{e.title}</button>

  const Mini = ({ m, onPick, big }: { m: string; onPick: (d: string) => void; big?: boolean }) => <div className={'mcal' + (big ? ' big' : '')}>
    {MINI.map((w, i) => <span key={i} className="mcal-wd">{w}</span>)}
    {monthDays(m).map(d => {
      const out = d.slice(0, 7) !== m.slice(0, 7), has = (byDay.get(d) ?? []).length > 0
      return <button key={d} className={'mcal-d' + (out ? ' out' : '') + (d === today ? ' today' : '') + (!big && d === anchor ? ' sel' : '')} onClick={() => onPick(d)} aria-label={frDate(d, { day: 'numeric', month: 'long' })}>
        {Number(d.slice(8))}{has && !out && <i />}
      </button>
    })}
  </div>

  if (!tl) return null
  return <section className="card pcal">
    <aside className="pcal-side">
      <div className="mcal-head">
        <b>{cap(frDate(first(mini), { month: 'long', year: 'numeric' }))}</b>
        <span className="row" style={{ gap: 0 }}>
          <button className="iconbtn" aria-label={tr("Mois précédent")} onClick={() => setMini(shiftMonth(mini, -1))}><ChevronLeft size={16} /></button>
          <button className="iconbtn" aria-label={tr("Mois suivant")} onClick={() => setMini(shiftMonth(mini, 1))}><ChevronRight size={16} /></button>
        </span>
      </div>
      <Mini m={mini} onPick={d => setAnchor(d)} />
      <div className="pcal-sec">{tr("Calendriers")}</div>
      <div className="pcal-cals" role="group" aria-label={tr("Filtres")}>
        {ORDER.map(t => <label key={t} className={'pcal-cal t-' + t}><input type="checkbox" checked={!hidden.has(t)} onChange={() => toggle(t)} /><i aria-hidden="true" /><span>{TAG_INFO[t]}</span><em>{counts[t]}</em></label>)}
        <label className="pcal-cal t-task sub"><input type="checkbox" checked={!hideDone} onChange={e => { setHideDone(!e.target.checked); store(DKEY, !e.target.checked) }} /><i aria-hidden="true" /><span>{tr("Tâches finies")}</span></label>
      </div>
      {canEdit && <>
        <div className="pcal-sec">{tr("À planifier · {0}", [unplanned.length])}</div>
        {unplanned.length === 0 ? <p className="small muted" style={{ margin: 0 }}>{tr("Toutes les tâches ont un jour.")}</p>
          : <div className="pcal-todo">{unplanned.slice(0, 30).map((t: any) => <div key={t.id} className="cev t-task" {...dragTask(t.id)} title={tr("Glissez sur un jour pour la planifier")}><i />#{t.number} {t.title}</div>)}</div>}
        <p className="small muted" style={{ margin: 0 }}>{tr("Glissez une tâche sur un jour pour la planifier.")}</p>
      </>}
    </aside>

    <div className="pcal-main">
      <div className="cal-bar">
        <h2 className="cal-title">{main}{sub && <span> {sub}</span>}</h2>
        <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
          <div className="seg" role="tablist" aria-label={tr("Vue")}>
            {([['week', tr("Semaine")], ['month', tr("Mois")], ['year', tr("Année")]] as [View, string][]).map(([k, l]) => <button key={k} role="tab" aria-selected={view === k} className={view === k ? 'on' : ''} onClick={() => setView(k)}>{l}</button>)}
          </div>
          <div className="row" style={{ gap: 0 }}>
            <button className="iconbtn" aria-label={tr("Précédent")} onClick={() => move(-1)}><ChevronLeft size={18} /></button>
            <button className="btn sm" onClick={() => go(today)}>{tr("Aujourd’hui")}</button>
            <button className="iconbtn" aria-label={tr("Suivant")} onClick={() => move(1)}><ChevronRight size={18} /></button>
          </div>
          {canEdit && <button className="iconbtn" aria-label={tr("Recalculer le planning")} data-tip={tr("Recalculer le planning")} onClick={async () => { try { const r = await api.post(`/api/maps/${map!.id}/plan/recalc`); setAlerts(r.alerts ?? []); reload(); toast(r.shifted?.length ? tr("#{0} décalées pour respecter les dépendances", [r.shifted.join(', #')]) : tr("Planning à jour")) } catch (er: any) { toast(er.message, true) } }}><RefreshCw size={16} /></button>}
          {canEdit && <button className="btn pri sm" onClick={() => addObjective()}><Plus size={16} />{tr("Objectif")}</button>}
        </div>
      </div>
      {alerts.length > 0 && <div className="warn" role="alert" style={{ marginBottom: 12 }}><AlertTriangle size={18} /><div>{alerts.map((a, i) => <div key={i}>{a}</div>)}</div></div>}

      {view === 'year' ? <div className="pcal-year">
        {Array.from({ length: 12 }, (_, i) => `${anchor.slice(0, 4)}-${String(i + 1).padStart(2, '0')}-01`).map(m => <div key={m}>
          <button className={'pcal-ym' + (m.slice(0, 7) === today.slice(0, 7) ? ' now' : '')} onClick={() => { go(m); setView('month') }}>{cap(frDate(m, { month: 'long' }))}</button>
          <Mini m={m} big onPick={d => { go(d); setView('week') }} />
        </div>)}
      </div> : <div className={'cal-grid ' + view}>
        {view === 'month' && WEEKDAYS.map(w => <div key={w} className="cal-wd">{w}</div>)}
        {(view === 'month' ? monthDays(anchor) : Array.from({ length: 7 }, (_, i) => addDays(monday(anchor), i))).map((d, i) => {
          const evs = byDay.get(d) ?? []
          const out = view === 'month' && d.slice(0, 7) !== anchor.slice(0, 7)
          const max = view === 'month' ? 3 : 40
          const we = i % 7 >= 5
          return <div key={d} className={'cal-day' + (out ? ' out' : '') + (we ? ' we' : '') + (d === today ? ' today' : '') + (canEdit ? ' can' : '') + (over === d ? ' drop' : '')}
            onClick={() => addObjective(d)} title={canEdit ? tr("Cliquer pour ajouter un objectif ce jour-là") : undefined} {...dropProps(d)}>
            {view === 'week'
              ? <div className="cal-wh"><span>{WEEKDAYS[i]}</span><span className="cal-num">{Number(d.slice(8))}</span></div>
              : <span className="cal-num">{d.slice(8) === '01' ? frDate(d, { day: 'numeric', month: 'short' }) : Number(d.slice(8))}</span>}
            <div className="cal-evs">{evs.slice(0, max).map(e => <Pill key={e.key} e={e} />)}{evs.length > max && <button className="cev more" onClick={ev => { ev.stopPropagation(); go(d); setView('week') }}>{tr("+{0} autres", [evs.length - max])}</button>}</div>
          </div>
        })}
      </div>}
      <p className="small muted" style={{ marginBottom: 0 }}>{tr("Les dates Fortnite sont indicatives : vérifiez les annonces d’Epic.")}</p>
    </div>

    {open && <Modal onClose={() => setOpen(null)} label={open.title}>
      <div className="stack">
        <div className="row" style={{ justifyContent: 'space-between' }}>
          <span className={'pcal-cal t-' + open.tag} style={{ padding: 0 }}><i />{TAG_INFO[open.tag]}</span>
          <button className="iconbtn" aria-label={tr("Fermer")} onClick={() => setOpen(null)}><X size={18} /></button>
        </div>
        <h2 style={{ margin: 0, fontWeight: 500 }}>{open.title}</h2>
        <div className="muted">{frDate(open.date, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })}{open.who ? ` · ${open.who}` : ''}</div>
        {open.tag === 'task' && canEdit && <label className="stack" style={{ gap: 4 }}><span className="small muted">{tr("Jour prévu")}</span>
          <input className="field" type="date" defaultValue={open.date} onChange={e => { if (e.target.value) { plan(open.id!, e.target.value); setOpen(null) } }} /></label>}
        <div className="row" style={{ justifyContent: 'flex-end' }}>
          {open.tag === 'task' && <button className="btn pri" onClick={() => nav(`/taches?q=%23${open.number}`)}>{tr("Ouvrir la tâche")}</button>}
          {open.tag === 'objective' && canEdit && <button className="btn" style={{ color: 'var(--tr)' }} onClick={async () => { if (!confirm(tr("Supprimer l’objectif « {0} » ?", [open.title]))) return; try { await api.del(`/api/milestones/${open.id}`); setOpen(null); reload() } catch (er: any) { toast(er.message, true) } }}><Trash2 size={16} />{tr("Supprimer")}</button>}
        </div>
      </div>
    </Modal>}
  </section>
}
