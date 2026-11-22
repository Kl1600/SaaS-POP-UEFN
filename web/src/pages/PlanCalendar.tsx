// Calendrier général de Planifier (#193) : objectifs, tâches planifiées et dates Fortnite sur une même
// grille, façon Calendrier d'Apple, avec un tag par type et des filtres mémorisés.
import { useMemo, useState } from 'react'
import { ChevronLeft, ChevronRight, Plus, Trash2, X } from 'lucide-react'
import { addDays, api, frDate, ymd } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { Modal } from '../components/ui'
import { tr } from '../i18n/t'
import { useNav } from '../routes'

type Tag = 'objective' | 'release' | 'task' | 'aim' | 'avoid' | 'money'
type Ev = { key: string; date: string; title: string; tag: Tag; id?: number; number?: number; status?: string; who?: string }

const TAG_INFO: Record<Tag, string> = {
  objective: tr("Objectif de la map"), release: tr("Sortie"), task: tr("Tâche planifiée"),
  aim: tr("Date Fortnite à viser"), avoid: tr("Date à éviter"), money: tr("Impact revenus"),
}
const ORDER: Tag[] = ['release', 'objective', 'avoid', 'aim', 'money', 'task']
const FKEY = 'pop.cal.filters'
const WEEKDAYS = [tr("lun."), tr("mar."), tr("mer."), tr("jeu."), tr("ven."), tr("sam."), tr("dim.")]

const monday = (d: string) => { const x = new Date(d + 'T12:00'); return addDays(d, -((x.getDay() + 6) % 7)) }

export default function PlanCalendar() {
  const { map, canEdit, toast } = useApp()
  const nav = useNav()
  const { data: tl, reload } = useLoad<any>(map ? `/api/maps/${map.id}/plan/timeline` : null, [map?.id])
  const { data: seasons } = useLoad<any[]>(map ? `/api/maps/${map.id}/seasons` : null, [map?.id])
  useLive(['tasks', 'plan'], reload)
  const today = ymd(new Date())
  const [view, setView] = useState<'month' | 'week'>('month')
  const [anchor, setAnchor] = useState(today)
  const [open, setOpen] = useState<Ev | null>(null)
  const [hidden, setHidden] = useState<Set<Tag>>(() => { try { return new Set(JSON.parse(localStorage.getItem(FKEY) ?? '[]')) } catch { return new Set() } })
  const toggle = (t: Tag) => { const h = new Set(hidden); h.has(t) ? h.delete(t) : h.add(t); setHidden(h); try { localStorage.setItem(FKEY, JSON.stringify([...h])) } catch { /* stockage indisponible */ } }

  const events = useMemo(() => {
    const out: Ev[] = []
    for (const m of tl?.milestones ?? []) out.push({ key: 'm' + m.id, date: m.date, title: m.title, tag: 'objective', id: m.id })
    if (tl?.release_date) out.push({ key: 'release', date: tl.release_date, title: tr("Sortie de {0}", [map?.name ?? '']), tag: 'release' })
    for (const t of tl?.tasks ?? []) if (t.planned_day) out.push({ key: 't' + t.id, date: t.planned_day, title: `#${t.number} ${t.title}`, tag: 'task', id: t.id, number: t.number, status: t.status, who: t.assignee ?? undefined })
    for (const [i, e] of (seasons ?? []).entries()) if (e.kind !== 'map') out.push({ key: 's' + i, date: e.date, title: e.title, tag: e.kind as Tag })
    return out
  }, [tl, seasons, map?.name])
  const byDay = useMemo(() => {
    const m = new Map<string, Ev[]>()
    for (const e of events) if (!hidden.has(e.tag)) m.set(e.date, [...(m.get(e.date) ?? []), e])
    for (const l of m.values()) l.sort((a, b) => ORDER.indexOf(a.tag) - ORDER.indexOf(b.tag))
    return m
  }, [events, hidden])
  const counts = useMemo(() => Object.fromEntries(ORDER.map(t => [t, events.filter(e => e.tag === t).length])), [events])

  // Jours affichés : 6 semaines pour le mois, 7 jours pour la semaine.
  const monthStart = anchor.slice(0, 8) + '01'
  const days = view === 'month'
    ? Array.from({ length: 42 }, (_, i) => addDays(monday(monthStart), i))
    : Array.from({ length: 7 }, (_, i) => addDays(monday(anchor), i))
  const move = (dir: number) => {
    if (view === 'week') return setAnchor(addDays(anchor, dir * 7))
    const d = new Date(monthStart + 'T12:00'); d.setMonth(d.getMonth() + dir); setAnchor(ymd(d))
  }
  const title = view === 'month'
    ? (t => t.charAt(0).toUpperCase() + t.slice(1))(frDate(monthStart, { month: 'long', year: 'numeric' }))
    : tr("Semaine du {0}", [frDate(days[0], { day: 'numeric', month: 'long' })])

  const addObjective = async (date?: string) => {
    if (!canEdit) return
    const t = prompt(date ? tr("Nouvel objectif le {0}", [frDate(date, { day: 'numeric', month: 'long' })]) : tr("Nouvel objectif"))?.trim(); if (!t) return
    const d = date ?? prompt(tr("Date (AAAA-MM-JJ)"), today)?.trim(); if (!d) return
    try { await api.post(`/api/maps/${map!.id}/milestones`, { title: t, date: d }); reload(); toast(tr("Objectif ajouté")) } catch (er: any) { toast(er.message, true) }
  }
  const Pill = ({ e }: { e: Ev }) => <button className={'cev t-' + e.tag + (e.status === 'done' ? ' done' : '')} title={`${TAG_INFO[e.tag]} · ${e.title}`} onClick={ev => { ev.stopPropagation(); setOpen(e) }}><i />{e.title}</button>
  const max = view === 'month' ? 3 : 20

  if (!tl) return null
  return <section className="card pcal">
    <div className="cal-bar">
      <div className="row" style={{ gap: 6 }}>
        <h2 className="cal-title">{title}</h2>
        <button className="iconbtn" aria-label={view === 'month' ? tr("Mois précédent") : tr("Semaine précédente")} onClick={() => move(-1)}><ChevronLeft size={18} /></button>
        <button className="btn sm" onClick={() => setAnchor(today)}>{tr("Aujourd’hui")}</button>
        <button className="iconbtn" aria-label={view === 'month' ? tr("Mois suivant") : tr("Semaine suivante")} onClick={() => move(1)}><ChevronRight size={18} /></button>
      </div>
      <div className="row" style={{ gap: 8 }}>
        <div className="tabs" role="tablist">
          <button role="tab" aria-selected={view === 'month'} className={view === 'month' ? 'on' : ''} onClick={() => setView('month')}>{tr("Mois")}</button>
          <button role="tab" aria-selected={view === 'week'} className={view === 'week' ? 'on' : ''} onClick={() => setView('week')}>{tr("Semaine")}</button>
        </div>
        {canEdit && <button className="btn pri sm" onClick={() => addObjective()}><Plus size={16} />{tr("Objectif")}</button>}
      </div>
    </div>
    <div className="cal-filters" role="group" aria-label={tr("Filtres")}>
      {ORDER.map(t => <label key={t} className={'cal-f t-' + t + (hidden.has(t) ? ' off' : '')}><input type="checkbox" checked={!hidden.has(t)} onChange={() => toggle(t)} /><i />{TAG_INFO[t]}<span className="muted">{counts[t]}</span></label>)}
    </div>
    <div className={'cal-grid ' + view}>
      {WEEKDAYS.map(w => <div key={w} className="cal-wd">{w}</div>)}
      {days.map(d => {
        const evs = byDay.get(d) ?? []
        const out = view === 'month' && d.slice(0, 7) !== monthStart.slice(0, 7)
        return <div key={d} className={'cal-day' + (out ? ' out' : '') + (d === today ? ' today' : '') + (canEdit ? ' can' : '')} onClick={() => addObjective(d)} title={canEdit ? tr("Cliquer pour ajouter un objectif ce jour-là") : undefined}>
          <span className="cal-num">{view === 'week' ? frDate(d, { weekday: 'short', day: 'numeric' }) : Number(d.slice(8))}</span>
          <div className="cal-evs">{evs.slice(0, max).map(e => <Pill key={e.key} e={e} />)}{evs.length > max && <button className="cev more" onClick={ev => { ev.stopPropagation(); setView('week'); setAnchor(d) }}>{tr("+{0} autres", [evs.length - max])}</button>}</div>
        </div>
      })}
    </div>
    <p className="small muted" style={{ marginBottom: 0 }}>{tr("Les dates Fortnite sont indicatives : vérifiez les annonces d’Epic.")}</p>
    {open && <Modal onClose={() => setOpen(null)} label={open.title}>
      <div className="stack">
        <div className="row" style={{ justifyContent: 'space-between' }}>
          <span className={'cal-f t-' + open.tag}><i />{TAG_INFO[open.tag]}</span>
          <button className="iconbtn" aria-label={tr("Fermer")} onClick={() => setOpen(null)}><X size={18} /></button>
        </div>
        <h2 style={{ margin: 0, fontWeight: 500 }}>{open.title}</h2>
        <div className="muted">{frDate(open.date, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })}{open.who ? ` · ${open.who}` : ''}</div>
        <div className="row" style={{ justifyContent: 'flex-end' }}>
          {open.tag === 'task' && <button className="btn pri" onClick={() => nav(`/taches?q=%23${open.number}`)}>{tr("Ouvrir la tâche")}</button>}
          {open.tag === 'objective' && canEdit && <button className="btn" style={{ color: 'var(--tr)' }} onClick={async () => { if (!confirm(tr("Supprimer l’objectif « {0} » ?", [open.title]))) return; try { await api.del(`/api/milestones/${open.id}`); setOpen(null); reload() } catch (er: any) { toast(er.message, true) } }}><Trash2 size={16} />{tr("Supprimer")}</button>}
          {(open.tag === 'objective' || open.tag === 'release') && <button className="btn" onClick={() => { setOpen(null); nav('/planifier') }}>{tr("Voir dans le planning")}</button>}
        </div>
      </div>
    </Modal>}
  </section>
}
