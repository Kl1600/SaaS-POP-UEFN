// Équipe d'une map et avis de faisabilité (#149) : « Faisable », « Tendu » ou « Pas faisable », avec les heures
// disponibles jusqu'à la sortie, le travail restant et des corrections chiffrées. Calcul fait par le serveur.
import { useEffect, useState } from 'react'
import { AlertTriangle, CheckCircle2, Gauge, Plus, Trash2, XCircle } from '../icons'
import { api, frDate } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead } from './ui'
import { tr, tx } from '../i18n/t'

const ETAT: Record<string, [string, any, string]> = {
  faisable: [tr("Faisable"), CheckCircle2, 'var(--tg)'],
  tendu: [tr("Tendu"), AlertTriangle, '#ED753E'],
  pas_faisable: [tr("Pas faisable"), XCircle, 'var(--red)'],
}

/** Carte de l'avis, mise à jour quand les tâches ou l'équipe changent. */
export function AvisFaisabilite({ mapId, onEdit }: { mapId: number; onEdit?: () => void }) {
  const { data, reload } = useLoad<any>(`/api/maps/${mapId}/faisabilite`, [mapId])
  useLive(['tasks', 'plan'], reload)
  if (!data) return null
  const a = data.avis
  const etat = ETAT[a.status]
  const Icone = etat?.[1]
  return (
    <section className="card">
      <CardHead icon={<Gauge size={22} strokeWidth={1.5} />} title={tr("Avis de faisabilité")}
        sub={etat ? tr("{0} h disponibles jusqu'au {1} · {2} h de travail, {3} h avec la marge de {4} %", [a.available_hours, frDate(a.release), a.needed_hours, a.with_margin, a.margin_pct]) : tx(a.message)} />
      {etat && <div className="row" style={{ gap: 8, alignItems: 'center', marginBottom: 8 }}>
        <Icone size={20} strokeWidth={1.5} style={{ color: etat[2] }} />
        <b style={{ fontWeight: 500 }}>{etat[0]}</b>
        <span className="small muted">{a.source === 'gabarit' ? tr("estimé sur le gabarit du genre ({0} tâches)", [a.tasks]) : tr("{0} tâches ouvertes", [a.tasks])}{a.ai_share_pct ? tr(" · {0} % confiées à l'IA", [a.ai_share_pct]) : ''}</span>
      </div>}
      {a.corrections?.length > 0 && <div className="stack" style={{ gap: 4 }}>
        <span className="small muted">{tr("Pour le rendre faisable, au choix :")}</span>
        <ul style={{ margin: 0, paddingLeft: 20, display: 'grid', gap: 4 }}>
          {a.corrections.map((c: any, i: number) => <li key={i}>{tx(c.text)}{c.kind === 'retirer' && c.list?.length > 0 && <span className="small muted"> : {c.list.map((t: any) => `#${t.number} ${t.title}`).join(', ')}</span>}</li>)}
        </ul>
      </div>}
      {a.overloads?.length > 0 && <div className="stack" style={{ gap: 4, marginTop: 10 }}>
        {a.overloads.map((o: any, i: number) => <span key={i} className="small" style={{ color: 'var(--red)' }}><AlertTriangle size={14} strokeWidth={1.5} style={{ verticalAlign: -2 }} /> {tx(o.text)}</span>)}
      </div>}
      {onEdit && data.can_edit && <button className="btn more" onClick={onEdit}>{tr("Modifier l'équipe de la map")}</button>}
    </section>
  )
}

type Ligne = { on: boolean; hours: number; absences: { start: string; end: string }[] }

/** Choix de l'équipe : membres du studio, heures par semaine sur la map, indisponibilités. */
export function ChoixEquipe({ members, value, onChange, compact }: { members: { user_id: number; name: string; studio_hours: number }[]; value: Record<number, Ligne>; onChange: (v: Record<number, Ligne>) => void; compact?: boolean }) {
  const set = (id: number, l: Partial<Ligne>) => onChange({ ...value, [id]: { ...(value[id] ?? { on: false, hours: 10, absences: [] }), ...l } })
  return <div className="stack" style={{ gap: 8 }}>
    {members.map(m => { const l = value[m.user_id] ?? { on: false, hours: Math.min(m.studio_hours, 10), absences: [] }; return (
      <div key={m.user_id} className="stack" style={{ gap: 6 }}>
        <div className="row" style={{ gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
          <label className="row" style={{ gap: 8, flex: 1, minWidth: 140 }}><input type="checkbox" checked={l.on} onChange={e => set(m.user_id, { on: e.target.checked, hours: l.hours })} />{m.name}</label>
          <label className="row small" style={{ gap: 6, alignItems: 'center' }}><input className="field" type="number" min={1} max={80} style={{ width: 70, height: 32 }} disabled={!l.on} value={l.hours} onChange={e => set(m.user_id, { hours: Number(e.target.value) })} aria-label={tr("Heures par semaine de {0}", [m.name])} />{tr("h / semaine")}</label>
          {!compact && l.on && <button type="button" className="btn" style={{ height: 32 }} onClick={() => set(m.user_id, { absences: [...l.absences, { start: '', end: '' }] })}><Plus size={14} />{tr("Indisponibilité")}</button>}
        </div>
        {!compact && l.on && l.absences.map((a, i) => <div key={i} className="row small" style={{ gap: 6, paddingLeft: 26, alignItems: 'center' }}>
          {tr("Du")} <input className="field" type="date" style={{ height: 32 }} value={a.start} onChange={e => set(m.user_id, { absences: l.absences.map((x, j) => j === i ? { ...x, start: e.target.value } : x) })} />
          {tr("au")} <input className="field" type="date" style={{ height: 32 }} value={a.end} onChange={e => set(m.user_id, { absences: l.absences.map((x, j) => j === i ? { ...x, end: e.target.value } : x) })} />
          <button type="button" className="btn" style={{ height: 32 }} aria-label={tr("Retirer")} onClick={() => set(m.user_id, { absences: l.absences.filter((_, j) => j !== i) })}><Trash2 size={14} /></button>
        </div>)}
      </div>
    ) })}
  </div>
}

export const versEnvoi = (v: Record<number, Ligne>) => Object.entries(v).filter(([, l]) => l.on).map(([id, l]) => ({ user_id: Number(id), hours: l.hours, absences: l.absences.filter(a => a.start && a.end) }))

/** Équipe de la map (réglages) : début, marge, part de l'IA, personnes et indisponibilités. */
export function EquipeMap({ mapId, onSaved }: { mapId: number; onSaved?: () => void }) {
  const { toast } = useApp()
  const { data, reload } = useLoad<any>(`/api/maps/${mapId}/faisabilite`, [mapId])
  const [v, setV] = useState<Record<number, Ligne>>({})
  const [f, setF] = useState({ start_date: '', margin_pct: 20, ai_share_pct: 0 })
  useEffect(() => {
    if (!data) return
    const m: Record<number, Ligne> = {}
    for (const s of data.staff) m[s.user_id] = { on: true, hours: s.hours, absences: s.absences }
    setV(m); setF({ start_date: data.start_date ?? '', margin_pct: data.margin_pct, ai_share_pct: data.ai_share_pct })
  }, [data])
  if (!data) return null
  const save = async () => {
    try { await api.put(`/api/maps/${mapId}/faisabilite`, { ...f, staff: versEnvoi(v) }); toast(tr("Équipe de la map enregistrée")); reload(); onSaved?.() } catch (e: any) { toast(e.message, true) }
  }
  return (
    <section className="card">
      <CardHead title={tr("Équipe de la map")} sub={tr("Qui travaille dessus, combien d'heures par semaine, et ses indisponibilités")} />
      <div className="grid2" style={{ gridTemplateColumns: 'repeat(3, 1fr)', marginBottom: 12 }}>
        <label className="label">{tr("Date de début")}<input className="field" type="date" disabled={!data.can_edit} value={f.start_date} onChange={e => setF({ ...f, start_date: e.target.value })} /></label>
        <label className="label">{tr("Marge de sécurité (%)")}<input className="field" type="number" min={0} max={100} disabled={!data.can_edit} value={f.margin_pct} onChange={e => setF({ ...f, margin_pct: Number(e.target.value) })} /></label>
        <label className="label">{tr("Part des tâches pour l'IA (%)")}<input className="field" type="number" min={0} max={100} disabled={!data.can_edit} value={f.ai_share_pct} onChange={e => setF({ ...f, ai_share_pct: Number(e.target.value) })} /></label>
      </div>
      <ChoixEquipe members={data.members} value={v} onChange={setV} />
      {data.can_edit ? <div className="row" style={{ justifyContent: 'flex-end', marginTop: 12 }}><button className="btn pri" onClick={save}>{tr("Enregistrer")}</button></div>
        : <p className="small muted">{tr("Seuls les admins du studio modifient l'équipe de la map.")}</p>}
    </section>
  )
}
