// Calendrier de Planifier (#193, #257), façon Calendrier d'Apple dans la DA de POP : colonne de gauche avec
// mini-mois, liste des calendriers (filtres mémorisés) et tâches à planifier ; vues Semaine, Mois et Année.
// Remplace l'ancien onglet Planning : une tâche se planifie en la glissant sur un jour.
import { useMemo, useState } from 'react'
import { AlertTriangle, ChevronLeft, ChevronRight, Plus, RefreshCw, Trash2, X } from '../icons'
import { addDays, api, frDate, ymd } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { Modal } from '../components/ui'
import { tr } from '../i18n/t'
import { TaskDrawer, useMembers } from './Tasks'

type Tag = 'objective' | 'release' | 'task' | 'aim' | 'avoid' | 'money'
type Ev = { key: string; date: string; title: string; tag: Tag; id?: number; number?: number; status?: string; who?: string; time?: string | null; detail?: string }
// Fenêtre de création ou de modification (#261) : un objectif (titre, date, heure, détail) ou une tâche.
type Draft = { kind: 'objective' | 'task'; id?: number; title: string; date: string; time: string; detail: string; who: string }
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
  const { data: tl, reload } = useLoad<any>(map ? `/api/maps/${map.id}/plan/timeline` : null, [map?.id])
  const { data: seasons } = useLoad<any[]>(map ? `/api/maps/${map.id}/seasons` : null, [map?.id])
  useLive(['tasks', 'plan'], reload)
  const today = ymd(new Date())
  const [view, setView] = useState<View>('month')
  const [anchor, setAnchor] = useState(today)
  const [mini, setMini] = useState(today)
  const [open, setOpen] = useState<Ev | null>(null)
  // Détail d'une tâche ouvert sur place (#396), avec la liste des tâches pour ses dépendances.
  const [taskId, setTaskId] = useState<number | null>(null)
  const { data: allTasks, reload: reloadTasks } = useLoad<any[]>(map && taskId ? `/api/maps/${map.id}/tasks` : null, [map?.id, !!taskId])
  const [hidden, setHidden] = useState<Set<Tag>>(() => new Set(read<Tag[]>(FKEY, [])))
  const [hideDone, setHideDone] = useState<boolean>(() => read(DKEY, true))
  const [over, setOver] = useState<string | null>(null)
  const [alerts, setAlerts] = useState<string[]>([])
  const toggle = (t: Tag) => { const h = new Set(hidden); h.has(t) ? h.delete(t) : h.add(t); setHidden(h); store(FKEY, [...h]) }
  const go = (d: string) => { setAnchor(d); setMini(d) }

  const events = useMemo(() => {
    const out: Ev[] = []
    for (const m of tl?.milestones ?? []) out.push({ key: 'm' + m.id, date: m.date, title: m.title, tag: 'objective', id: m.id, time: m.time, detail: m.detail })
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

  const [draft, setDraft] = useState<Draft | null>(null)
  const { data: team } = useMembers()
  const addObjective = (date?: string) => { if (canEdit) setDraft({ kind: 'objective', title: '', date: date ?? today, time: '', detail: '', who: '' }) }
  const saveDraft = async (d: Draft) => {
    try {
      if (d.kind === 'task') {
        await api.post(`/api/maps/${map!.id}/tasks`, { title: d.title, planned_day: d.date, assignee_id: d.who ? Number(d.who) : null })
        toast(tr("Tâche créée le {0}", [frDate(d.date, { day: 'numeric', month: 'short' })]))
      } else if (d.id) {
        await api.patch(`/api/milestones/${d.id}`, { title: d.title, date: d.date, time: d.time, detail: d.detail })
        toast(tr("Objectif modifié"))
      } else {
        await api.post(`/api/maps/${map!.id}/milestones`, { title: d.title, date: d.date, time: d.time, detail: d.detail })
        toast(tr("Objectif ajouté"))
      }
      setDraft(null); reload()
    } catch (er: any) { toast(er.message, true) }
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
    onClick={ev => { ev.stopPropagation(); setOpen(e) }} {...(e.tag === 'task' && e.id ? dragTask(e.id) : {})}><i />{e.time && <b className="cev-t">{e.time}</b>}{e.title}</button>

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
          {canEdit && <button className="btn pri sm" onClick={() => addObjective()}><Plus size={16} />{tr("Ajouter")}</button>}
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
            onClick={() => addObjective(d)} title={canEdit ? tr("Cliquer pour ajouter un objectif ou une tâche ce jour-là") : undefined} {...dropProps(d)}>
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
        <div className="muted">{frDate(open.date, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })}{open.time ? ` · ${open.time}` : ''}{open.who ? ` · ${open.who}` : ''}</div>
        {open.detail && <p style={{ margin: 0, whiteSpace: 'pre-wrap' }}>{open.detail}</p>}
        {open.tag === 'task' && canEdit && <label className="stack" style={{ gap: 4 }}><span className="small muted">{tr("Jour prévu")}</span>
          <input className="field" type="date" defaultValue={open.date} onChange={e => { if (e.target.value) { plan(open.id!, e.target.value); setOpen(null) } }} /></label>}
        <div className="row" style={{ justifyContent: 'flex-end' }}>
          {open.tag === 'task' && <button className="btn pri" onClick={() => { setTaskId(open.id!); setOpen(null) }}>{tr("Ouvrir la tâche")}</button>}
          {open.tag === 'objective' && canEdit && <button className="btn" onClick={() => { setDraft({ kind: 'objective', id: open.id, title: open.title, date: open.date, time: open.time ?? '', detail: open.detail ?? '', who: '' }); setOpen(null) }}>{tr("Modifier")}</button>}
          {open.tag === 'objective' && canEdit && <button className="btn" style={{ color: 'var(--tr)' }} onClick={async () => { if (!confirm(tr("Supprimer l’objectif « {0} » ?", [open.title]))) return; try { await api.del(`/api/milestones/${open.id}`); setOpen(null); reload() } catch (er: any) { toast(er.message, true) } }}><Trash2 size={16} />{tr("Supprimer")}</button>}
        </div>
      </div>
    </Modal>}
    {taskId && <TaskDrawer key={taskId} id={taskId} onClose={() => { setTaskId(null); reload(); reloadTasks() }} members={team?.members ?? []} tasks={allTasks ?? []} onOpen={setTaskId} />}
    {draft && <Modal onClose={() => setDraft(null)} label={draft.id ? tr("Modifier l’objectif") : tr("Nouvel événement")}>
      <form className="stack" onSubmit={e => { e.preventDefault(); if (draft.title.trim()) saveDraft(draft) }}>
        <div className="row" style={{ justifyContent: 'space-between' }}>
          <h2 style={{ margin: 0, fontWeight: 500 }}>{draft.id ? tr("Modifier l’objectif") : tr("Nouvel événement")}</h2>
          <button type="button" className="iconbtn" aria-label={tr("Fermer")} onClick={() => setDraft(null)}><X size={18} /></button>
        </div>
        {!draft.id && <div className="seg" role="tablist" aria-label={tr("Type")}>
          {([['objective', tr("Objectif")], ['task', tr("Tâche")]] as const).map(([k, l]) => <button type="button" key={k} role="tab" aria-selected={draft.kind === k} className={draft.kind === k ? 'on' : ''} onClick={() => setDraft({ ...draft, kind: k })}>{l}</button>)}
        </div>}
        <label className="stack" style={{ gap: 4 }}><span className="small muted">{tr("Titre")}</span>
          <input className="field" autoFocus required maxLength={draft.kind === 'task' ? 200 : 80} value={draft.title} onChange={e => setDraft({ ...draft, title: e.target.value })} placeholder={draft.kind === 'task' ? tr("Ex. Faire la boutique") : tr("Ex. Démo jouable au studio")} /></label>
        <div className="row" style={{ gap: 10, alignItems: 'flex-end' }}>
          <label className="stack" style={{ gap: 4, flex: 1 }}><span className="small muted">{draft.kind === 'task' ? tr("Jour prévu") : tr("Date")}</span>
            <input className="field" type="date" required value={draft.date} onChange={e => setDraft({ ...draft, date: e.target.value })} /></label>
          {draft.kind === 'objective' ? <label className="stack" style={{ gap: 4, width: 130 }}><span className="small muted">{tr("Heure (facultative)")}</span>
            <input className="field" type="time" value={draft.time} onChange={e => setDraft({ ...draft, time: e.target.value })} /></label>
            : <label className="stack" style={{ gap: 4, flex: 1 }}><span className="small muted">{tr("Personne")}</span>
              <select className="field" value={draft.who} onChange={e => setDraft({ ...draft, who: e.target.value })}>
                <option value="">{tr("Personne pour l’instant")}</option>
                {(team?.members ?? []).map((m: any) => <option key={m.id} value={m.id}>{m.name}</option>)}
              </select></label>}
        </div>
        {draft.kind === 'objective' && <label className="stack" style={{ gap: 4 }}><span className="small muted">{tr("Détail (facultatif)")}</span>
          <textarea className="field" rows={4} maxLength={2000} value={draft.detail} onChange={e => setDraft({ ...draft, detail: e.target.value })} placeholder={tr("Ce qu’il faut avoir fini, qui est concerné…")} /></label>}
        <div className="row" style={{ justifyContent: 'flex-end', gap: 8 }}>
          <button type="button" className="btn" onClick={() => setDraft(null)}>{tr("Annuler")}</button>
          <button className="btn pri" disabled={!draft.title.trim()}>{draft.id ? tr("Enregistrer") : draft.kind === 'task' ? tr("Créer la tâche") : tr("Créer l’objectif")}</button>
        </div>
      </form>
    </Modal>}
  </section>
}
