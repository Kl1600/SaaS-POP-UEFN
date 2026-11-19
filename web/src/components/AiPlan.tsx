// « Plan de production par l'IA POP » : décrire la map, recevoir des tâches d'une heure, choisir celles à ajouter.
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { Sparkles } from 'lucide-react'
import { api } from '../api'
import { useApp } from '../state'
import { Modal, tagLabel } from './ui'
import { tr } from '../i18n/t'

export function AiPlanButton() {
  const [open, setOpen] = useState(false)
  return <>
    <button className="btn" onClick={() => setOpen(true)}><Sparkles size={16} />{tr("Plan par l’IA POP")}</button>
    {open && <AiPlanModal onClose={() => setOpen(false)} />}
  </>
}

function AiPlanModal({ onClose }: { onClose: () => void }) {
  const { map, me, toast, reload } = useApp()
  const [brief, setBrief] = useState('')
  const [size, setSize] = useState(40)
  const [busy, setBusy] = useState(false)
  const [res, setRes] = useState<any>(null)
  const [keep, setKeep] = useState<Set<number>>(new Set())
  const credits = me?.ai_credits ?? 0
  const gen = async () => {
    setBusy(true)
    try {
      const r = await api.post(`/api/maps/${map!.id}/ai/plan`, { brief, size })
      setRes(r); setKeep(new Set(r.tasks.map((_: any, i: number) => i))); reload()
    } catch (e: any) { toast(e.message, true) } finally { setBusy(false) }
  }
  const add = async () => {
    // Les dépendances sont renumérotées pour ne garder que les tâches choisies.
    const idx = [...keep].sort((a, b) => a - b)
    const pos = new Map(idx.map((i, j) => [i, j]))
    const tasks = idx.map(i => ({ ...res.tasks[i], after: (res.tasks[i].after ?? []).filter((a: number) => pos.has(a)).map((a: number) => pos.get(a)) }))
    try { const r = await api.post(`/api/maps/${map!.id}/ai/plan/apply`, { tasks }); toast(tr("{0} tâches ajoutées", [r.added])); onClose() } catch (e: any) { toast(e.message, true) }
  }
  const lots = res ? [...new Set(res.tasks.map((t: any) => t.lot))] as string[] : []
  return <Modal onClose={onClose} label={tr("Plan par l’IA POP")}>
    <h2 style={{ margin: 0, fontWeight: 500, fontSize: 20 }}>{tr("Plan de production par l’IA POP")}</h2>
    {!res ? <>
      <p className="small muted">{tr("Décrivez la map : le jeu, le nombre de joueurs, ce qui la rend unique. Laissez vide pour partir de la fiche idée de la map. POP propose des tâches d’une heure, que vous choisissez avant de les ajouter.")}</p>
      <textarea className="field" rows={6} maxLength={6000} value={brief} onChange={e => setBrief(e.target.value)} placeholder={tr("Ex. Prop hunt dans un manoir hanté, 2 équipes de 4, les chasseurs ont une lampe qui révèle les objets…")} aria-label={tr("Description de la map")} />
      <label className="label" style={{ marginTop: 10 }}>{tr("Nombre de tâches environ")}<input className="field" type="number" min={15} max={80} value={size} onChange={e => setSize(+e.target.value)} style={{ width: 120 }} /></label>
      <p className="small muted">{tr("Coût : quelques crédits selon la longueur (environ 5 à 30). Solde : {0} crédits.", [credits])} {credits < 30 && <Link to="/parametres" onClick={onClose}>{tr("Acheter des crédits")}</Link>}</p>
      <div className="row" style={{ gap: 8, justifyContent: 'flex-end' }}><button className="btn" onClick={onClose}>{tr("Annuler")}</button><button className="btn pri" disabled={busy} onClick={gen}><Sparkles size={16} />{busy ? tr("L’IA réfléchit…") : tr("Proposer un plan")}</button></div>
    </> : <>
      <p className="small muted">{tr("{0} tâches proposées · {1} crédits utilisés · solde {2}. Décochez celles que vous ne voulez pas.", [res.tasks.length, res.credits, res.balance])}</p>
      <div className="aiplan-list">{lots.map(l => <div key={l}>
        <div className="dsec" style={{ marginTop: 8 }}>{l || tr("Sans étape")}</div>
        {res.tasks.map((t: any, i: number) => t.lot !== l ? null : <label key={i} className="aiplan-item">
          <input type="checkbox" checked={keep.has(i)} onChange={e => { const k = new Set(keep); e.target.checked ? k.add(i) : k.delete(i); setKeep(k) }} />
          <span className="badge">{tagLabel(t.tag)}</span><span><b>{t.title}</b>{t.description && <small>{t.description}</small>}</span>
        </label>)}
      </div>)}</div>
      <div className="row" style={{ gap: 8, justifyContent: 'flex-end', marginTop: 12 }}><button className="btn" onClick={() => setRes(null)}>{tr("Recommencer")}</button><button className="btn pri" disabled={!keep.size} onClick={add}>{tr("Ajouter {0} tâches", [keep.size])}</button></div>
    </>}
  </Modal>
}
