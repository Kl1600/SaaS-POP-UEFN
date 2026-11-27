// « Générer une miniature » (#308) : un clic crée une tâche POP pour l'IA du client, qui prend les photos de l'île
// (#306), s'inspire de la bibliothèque et fait dessiner la miniature par POP (#307), puis la rend (submit_thumbnail).
// Forge montre la miniature, « Conforme Discover », et « Utiliser », « Refaire », « Modifier ». Le site montre les
// versions choisies (`versions`).
import { useState } from 'react'
import { Check, Download, ImagePlus, RefreshCw, Pencil } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr } from '../i18n/t'

type Mini = { id: number; title: string; url?: string; logo?: string; bytes: number; submitted_at: string; used_at?: string | null; created_at: string }
type Liste = { miniatures: Mini[]; en_cours: { task: number; status: string; idea: string }[] }

export function ForgeMiniature({ mapId, versions }: { mapId: number; versions?: boolean }) {
  const app = useApp()
  const { data, reload } = useLoad<Liste>(`/api/maps/${mapId}/miniatures`, [mapId])
  useLive(['miniatures', 'tasks'], reload)
  const [idee, setIdee] = useState('')
  const [modif, setModif] = useState<string | null>(null)
  const [occupe, setOccupe] = useState(false)
  if (!data) return null
  const demander = async (base?: number, phrase?: string) => {
    setOccupe(true)
    try {
      const r = await api.post(`/api/maps/${mapId}/miniatures/demandes`, { idee: phrase ?? idee, base })
      app.toast(r.agent ? tr("{0} fait la miniature (tâche #{1}).", [r.agent, r.task]) : tr("Tâche #{0} créée : votre IA la prendra avec next_task.", [r.task]))
      setIdee(''); setModif(null); reload()
    } catch (e: any) { app.toast(e.message, true) }
    setOccupe(false)
  }
  const utiliser = async (m: Mini) => {
    try { await api.post(`/api/maps/${mapId}/miniatures/${m.id}/utiliser`); app.toast(tr("Miniature choisie pour la map")); reload() } catch (e: any) { app.toast(e.message, true) }
  }
  const derniere = data.miniatures[0]
  const choisies = data.miniatures.filter(m => m.used_at)
  if (versions) return <section className="card stack" style={{ gap: 10 }}>
    <b>{tr("Miniature")}</b>
    {!choisies.length ? <p className="muted small" style={{ margin: 0 }}>{tr("Aucune miniature choisie. Dans POP Forge : Outils → Générer une miniature.")}</p>
      : <div className="mini-grille">{choisies.map((m, i) => <div key={m.id} className="mini-carte">
        {m.url && <img src={m.url} alt={m.title} loading="lazy" />}
        <b className="small">{m.title}{i === 0 ? ' · ' + tr("utilisée") : ''}</b>
        <span className="small muted">{tr("choisie {0}", [ago(m.used_at!)])}</span>
      </div>)}</div>}
  </section>
  return <section className="card stack" style={{ padding: 20, gap: 14 }} aria-label={tr("Miniature")}>
    <div className="row" style={{ gap: 10, alignItems: 'baseline', flexWrap: 'wrap' }}>
      <h2 className="forge-h2" style={{ margin: 0 }}>{tr("Miniature")}</h2>
      <span className="small muted">{tr("Votre IA prend des photos de l'île et POP dessine une miniature conforme aux règles d'Epic.")}</span>
    </div>
    <form className="row" style={{ gap: 8, flexWrap: 'wrap' }} onSubmit={e => { e.preventDefault(); demander() }}>
      <input className="field" style={{ flex: 1, minWidth: 240 }} value={idee} maxLength={300} onChange={e => setIdee(e.target.value)} placeholder={tr("Une idée en plus ? (ex. ambiance fun, gros chiffres)")} aria-label={tr("Une idée en plus ?")} disabled={!app.canEdit} />
      <button className="btn pri" disabled={occupe || !app.canEdit}><ImagePlus size={16} strokeWidth={1.6} />{tr("Générer une miniature")}</button>
    </form>
    {data.en_cours.length > 0 && <p className="small" role="status" style={{ margin: 0 }}>{tr("En cours : tâche #{0}{1}. La miniature s'affichera ici dès que votre IA l'aura rendue.", [data.en_cours[0].task, data.en_cours[0].idea ? ` (« ${data.en_cours[0].idea} »)` : ''])}</p>}
    {derniere && <div className="stack" style={{ gap: 10 }}>
      {derniere.url && <img src={derniere.url} alt={derniere.title} style={{ width: '100%', maxWidth: 960, aspectRatio: '16 / 9', objectFit: 'cover', borderRadius: 12, border: '1px solid var(--border)' }} />}
      <div className="row" style={{ gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        <span className="chip" title={tr("1920 × 1080, moins de 5 Mo, aucun mot interdit par Epic, pas de commande de plateforme, pas de copie d'une inspiration")}><Check size={14} strokeWidth={2} />{tr("Conforme Discover")}</span>
        <span className="small muted">{derniere.title} · {ago(derniere.submitted_at)}{derniere.used_at ? ' · ' + tr("utilisée") : ''}</span>
        <div style={{ flex: 1 }} />
        <button className="btn pri" disabled={!!derniere.used_at || !app.canEdit} onClick={() => utiliser(derniere)}>{derniere.used_at ? tr("Utilisée") : tr("Utiliser cette miniature")}</button>
        <button className="btn" disabled={occupe || !app.canEdit} onClick={() => demander()}><RefreshCw size={15} strokeWidth={1.6} />{tr("Refaire")}</button>
        <button className="btn" disabled={!app.canEdit} onClick={() => setModif(m => m === null ? '' : null)}><Pencil size={15} strokeWidth={1.6} />{tr("Modifier")}</button>
      </div>
      {modif !== null && <form className="row" style={{ gap: 8 }} onSubmit={e => { e.preventDefault(); if (modif.trim()) demander(derniere.id, modif) }}>
        <input className="field" style={{ flex: 1 }} autoFocus value={modif} maxLength={300} onChange={e => setModif(e.target.value)} placeholder={tr("Ce qu'il faut changer (ex. titre plus gros)")} aria-label={tr("Ce qu'il faut changer")} />
        <button className="btn pri" disabled={occupe || !modif.trim()}>{tr("Envoyer à mon IA")}</button>
      </form>}
      {derniere.used_at && <div className="row" style={{ gap: 8 }}>
        {derniere.url && <a className="btn" href={derniere.url} download><Download size={15} strokeWidth={1.6} />{tr("Image 1920 × 1080")}</a>}
        {derniere.logo && <a className="btn" href={derniere.logo} download><Download size={15} strokeWidth={1.6} />{tr("Logo 1440 × 608")}</a>}
        <span className="small muted" style={{ alignSelf: 'center' }}>{tr("À envoyer dans le portail créateur d'Epic.")}</span>
      </div>}
    </div>}
  </section>
}
