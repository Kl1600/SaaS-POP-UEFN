import { useState } from 'react'
import { Plus, Play, Pencil, Trash2, Check, History, X, ListPlus } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead, Modal } from './ui'
import { tr } from '../i18n/t'

// Scénarios de test rejouables (#348) : écrits par l'équipe (ou proposés par une IA, à valider), joués par des humains
// à chaque version ; une étape réussie puis échouée est une régression ; une étape échouée devient une tâche.
type Step = { id?: number; action: string; expected: string }
const V: Record<string, [string, string]> = { pass: [tr("Réussi"), 'b-g'], fail: [tr("Échoué"), 'b-r'], skip: [tr("Non testé"), ''] }

export function Scenarios() {
  const { map, canEdit, toast } = useApp()
  const { data, reload } = useLoad<any[]>(map ? `/api/maps/${map.id}/scenarios` : null, [map?.id])
  useLive(['scenarios'], reload)
  const [edit, setEdit] = useState<any>(null)
  const [play, setPlay] = useState<any>(null)
  const [hist, setHist] = useState<any>(null)
  if (!data) return null
  const act = async (f: () => Promise<any>, ok: string) => { try { await f(); toast(ok); reload() } catch (e: any) { toast(e.message, true) } }
  return (
    <section className="card">
      <CardHead title={tr("Scénarios de test")} sub={tr("Ce qu’un joueur doit pouvoir faire, rejoué à chaque version : on voit tout de suite ce qui a régressé.")}
>
        {canEdit && <button className="btn" onClick={() => setEdit({ title: '', steps: [{ action: '', expected: '' }] })}><Plus size={14} /> {tr("Nouveau scénario")}</button>}
      </CardHead>
      {!data.length && <p className="muted">{tr("Aucun scénario. Écrivez-en un, ou demandez à votre IA d’en proposer un à partir des tâches (outil propose_scenario).")}</p>}
      <div className="list">
        {data.map(s => (
          <div key={s.id} className="row" style={{ justifyContent: 'space-between', gap: 12, padding: '10px 0', borderBottom: '1px solid var(--line)' }}>
            <div>
              <b>{s.title}</b> {s.status === 'draft' && <span className="badge b-o">{tr("Proposé par une IA · à valider")}</span>}
              <div className="small muted">{tr("{0} étape(s)", [s.steps.length])}{s.last_run ? ' · ' + tr("dernier passage {0} {1}", [s.last_run.version ? `v${s.last_run.version}` : '', ago(s.last_run.at)]) : ' · ' + tr("jamais joué")}</div>
              {s.last_run && <div className="row" style={{ gap: 6, marginTop: 4 }}>
                <span className="badge b-g">{s.last_run.counts.pass} ✓</span><span className="badge b-r">{s.last_run.counts.fail} ✗</span><span className="badge">{s.last_run.counts.skip} –</span>
                {s.last_run.regressions > 0 && <span className="badge b-r">{tr("{0} régression(s)", [s.last_run.regressions])}</span>}
              </div>}
            </div>
            <div className="row" style={{ gap: 4 }}>
              {canEdit && s.status === 'draft' && <button className="btn" onClick={() => act(() => api.post(`/api/scenarios/${s.id}/validate`), tr("Scénario validé"))}><Check size={14} /> {tr("Valider")}</button>}
              {canEdit && s.status === 'ready' && <button className="btn pri" onClick={() => setPlay(s)}><Play size={14} /> {tr("Jouer")}</button>}
              <button className="iconbtn" title={tr("Historique")} aria-label={tr("Historique")} onClick={() => setHist(s)}><History size={16} /></button>
              {canEdit && <button className="iconbtn" title={tr("Modifier")} aria-label={tr("Modifier")} onClick={() => setEdit(s)}><Pencil size={16} /></button>}
              {canEdit && <button className="iconbtn" title={tr("Supprimer")} aria-label={tr("Supprimer")} onClick={() => confirm(tr("Supprimer ce scénario et tous ses passages ?")) && act(() => api.del(`/api/scenarios/${s.id}`), tr("Scénario supprimé"))}><Trash2 size={16} /></button>}
            </div>
          </div>
        ))}
      </div>
      {edit && <Editor s={edit} onClose={() => setEdit(null)} onDone={() => { setEdit(null); reload() }} />}
      {play && <Runner s={play} onClose={() => { setPlay(null); reload() }} />}
      {hist && <Historique s={hist} onClose={() => setHist(null)} />}
    </section>
  )
}

function Editor({ s, onClose, onDone }: { s: any; onClose: () => void; onDone: () => void }) {
  const { map, toast } = useApp()
  const [title, setTitle] = useState<string>(s.title)
  const [steps, setSteps] = useState<Step[]>(s.steps.map((x: Step) => ({ ...x, expected: x.expected ?? '' })))
  const set = (i: number, k: 'action' | 'expected', v: string) => setSteps(steps.map((x, j) => j === i ? { ...x, [k]: v } : x))
  const save = async (e: React.FormEvent) => {
    e.preventDefault()
    try {
      if (s.id) await api.put(`/api/scenarios/${s.id}`, { title, steps }); else await api.post(`/api/maps/${map!.id}/scenarios`, { title, steps })
      toast(tr("Scénario enregistré")); onDone()
    } catch (er: any) { toast(er.message, true) }
  }
  return (
    <Modal onClose={onClose} label={tr("Scénario de test")}>
      <form className="stack" onSubmit={save}>
        <label className="label">{tr("Titre")}<input className="field" value={title} maxLength={140} onChange={e => setTitle(e.target.value)} placeholder={tr("Acheter une arme à la boutique")} /></label>
        {steps.map((x, i) => (
          <div key={i} className="row" style={{ gap: 6, alignItems: 'flex-start' }}>
            <b style={{ width: 22, paddingTop: 10 }}>{i + 1}.</b>
            <input className="field" style={{ flex: 1 }} value={x.action} maxLength={300} onChange={e => set(i, 'action', e.target.value)} placeholder={tr("Action : va au spawn")} aria-label={tr("Action de l’étape {0}", [i + 1])} />
            <input className="field" style={{ flex: 1 }} value={x.expected} maxLength={300} onChange={e => set(i, 'expected', e.target.value)} placeholder={tr("Résultat attendu")} aria-label={tr("Résultat attendu de l’étape {0}", [i + 1])} />
            <button type="button" className="iconbtn" aria-label={tr("Retirer l’étape")} disabled={steps.length === 1} onClick={() => setSteps(steps.filter((_, j) => j !== i))}><X size={14} /></button>
          </div>
        ))}
        <button type="button" className="btn" disabled={steps.length >= 50} onClick={() => setSteps([...steps, { action: '', expected: '' }])}><ListPlus size={14} /> {tr("Ajouter une étape")}</button>
        <div className="row" style={{ justifyContent: 'flex-end' }}><button type="button" className="btn" onClick={onClose}>{tr("Annuler")}</button><button className="btn pri">{tr("Enregistrer")}</button></div>
      </form>
    </Modal>
  )
}

function Runner({ s, onClose }: { s: any; onClose: () => void }) {
  const { toast } = useApp()
  const [version, setVersion] = useState('')
  const [run, setRun] = useState<number | null>(null)
  const [res, setRes] = useState<Record<number, { verdict?: string; comment: string; capture?: string }>>({})
  const [fin, setFin] = useState<any>(null)
  const start = async (e: React.FormEvent) => { e.preventDefault(); try { setRun((await api.post(`/api/scenarios/${s.id}/runs`, { version })).id) } catch (er: any) { toast(er.message, true) } }
  const send = async (step: number, patch: { verdict?: string; comment?: string; capture?: string }) => {
    const cur = { ...(res[step] ?? { comment: '' }), ...patch } as { verdict?: string; comment: string; capture?: string }
    setRes({ ...res, [step]: cur })
    if (!cur.verdict) return
    try { await api.put(`/api/scenario-runs/${run}/results/${step}`, { verdict: cur.verdict, comment: cur.comment, capture_key: patch.capture }) } catch (er: any) { toast(er.message, true) }
  }
  const capture = async (step: number, f: File | undefined) => {
    if (!f) return
    if (!res[step]?.verdict) { toast(tr("Donnez d’abord le verdict de l’étape."), true); return }
    try {
      const { url, key } = await api.post(`/api/scenario-runs/${run}/capture`, { step })
      const r = await fetch(url, { method: 'PUT', body: f, headers: { 'content-type': f.type || 'image/png' } })
      if (!r.ok) throw new Error(tr("Envoi de la capture refusé par le stockage ({0})", [r.status]))
      await send(step, { capture: key }); toast(tr("Capture jointe"))
    } catch (er: any) { toast(er.message, true) }
  }
  const finish = async () => { try { setFin(await api.post(`/api/scenario-runs/${run}/finish`)) } catch (er: any) { toast(er.message, true) } }
  return (
    <Modal onClose={onClose} label={tr("Jouer un scénario")}>
      <div className="stack">
        <h2 style={{ margin: 0, fontWeight: 400 }}>{s.title}</h2>
        {!run && <form className="row" style={{ gap: 6 }} onSubmit={start}>
          <input className="field" style={{ flex: 1 }} value={version} maxLength={60} onChange={e => setVersion(e.target.value)} placeholder={tr("Version jouée (ex. 1.4)")} aria-label={tr("Version")} />
          <button className="btn pri"><Play size={14} /> {tr("Commencer")}</button>
        </form>}
        {run && !fin && <>
          {s.steps.map((x: Step, i: number) => (
            <div key={x.id} className="card" style={{ padding: 12 }}>
              <div><b>{i + 1}. {x.action}</b></div>
              {x.expected && <div className="small muted">{tr("Attendu : {0}", [x.expected])}</div>}
              <div className="row" style={{ gap: 6, marginTop: 8, flexWrap: 'wrap' }}>
                {Object.entries(V).map(([k, [l]]) => <button key={k} type="button" className={'chip' + (res[x.id!]?.verdict === k ? ' on' : '')} onClick={() => send(x.id!, { verdict: k })}>{l}</button>)}
                <input className="field" style={{ flex: 1, minWidth: 160 }} placeholder={tr("Commentaire")} maxLength={1000} value={res[x.id!]?.comment ?? ''}
                  onChange={e => setRes({ ...res, [x.id!]: { ...res[x.id!], comment: e.target.value } })} onBlur={e => send(x.id!, { comment: e.target.value })} aria-label={tr("Commentaire de l’étape {0}", [i + 1])} />
                <label className="btn">{res[x.id!]?.capture ? tr("Capture jointe") : tr("Capture")}<input type="file" accept="image/*" hidden onChange={e => capture(x.id!, e.target.files?.[0])} /></label>
              </div>
            </div>
          ))}
          <div className="row" style={{ justifyContent: 'flex-end' }}><button className="btn pri" onClick={finish}>{tr("Terminer le passage")}</button></div>
        </>}
        {fin && <>
          <p>{tr("{0} réussie(s), {1} échouée(s), {2} non testée(s).", [fin.counts.pass, fin.counts.fail, fin.counts.skip])}</p>
          {fin.regressions.length > 0 && <p className="badge b-r">{tr("{0} régression(s) par rapport au passage précédent", [fin.regressions.length])}</p>}
          <p className="muted small">{tr("Les étapes échouées peuvent devenir des tâches depuis l’historique.")}</p>
          <div className="row" style={{ justifyContent: 'flex-end' }}><button className="btn pri" onClick={onClose}>{tr("Fermer")}</button></div>
        </>}
      </div>
    </Modal>
  )
}

function Historique({ s, onClose }: { s: any; onClose: () => void }) {
  const { canEdit, toast } = useApp()
  const { data, reload } = useLoad<any>(`/api/scenarios/${s.id}/runs`, [s.id])
  const tache = async (run: number, step: number) => {
    try { const r = await api.post(`/api/scenario-runs/${run}/results/${step}/task`); toast(r.existing ? tr("Déjà la tâche n° {0}", [r.number]) : tr("Tâche n° {0} créée", [r.number])); reload() } catch (e: any) { toast(e.message, true) }
  }
  return (
    <Modal onClose={onClose} label={tr("Historique des passages")}>
      <div className="stack">
        <h2 style={{ margin: 0, fontWeight: 400 }}>{tr("Historique · {0}", [s.title])}</h2>
        {data && !data.runs.length && <p className="muted">{tr("Pas encore de passage.")}</p>}
        {data?.runs.map((r: any) => (
          <div key={r.id} className="card" style={{ padding: 12 }}>
            <div className="row" style={{ justifyContent: 'space-between' }}>
              <b>{r.version ? `v${r.version}` : tr("Sans version")} · {r.tester ?? '—'} · {ago(r.created_at)}</b>
              {r.regressions.length > 0 && <span className="badge b-r">{tr("{0} régression(s)", [r.regressions.length])}</span>}
            </div>
            {s.steps.map((x: Step, i: number) => {
              const v = r.results[String(x.id)]; const reg = r.regressions.includes(x.id)
              return (
                <div key={x.id} className="row" style={{ gap: 8, padding: '4px 0', color: reg ? '#d92d20' : undefined }}>
                  <span className={'badge ' + (v ? V[v.verdict][1] : '')}>{v ? V[v.verdict][0] : tr("Non joué")}</span>
                  <span>{i + 1}. {x.action}{reg && ` · ${tr("régression")}`}</span>
                  {v?.comment && <span className="small muted">« {v.comment} »</span>}
                  {v?.capture && <a className="small" href={v.capture} target="_blank" rel="noreferrer">{tr("capture")}</a>}
                  {v?.verdict === 'fail' && (v.task ? <span className="small">{tr("tâche n° {0}", [v.task])}</span> : canEdit && <button className="btn" onClick={() => tache(r.id, x.id!)}>{tr("Créer une tâche")}</button>)}
                </div>
              )
            })}
          </div>
        ))}
      </div>
    </Modal>
  )
}
