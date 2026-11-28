// Bibliothèque qui apprend (#120) : en tête de Forge → Bibliothèque, ce qui attend une décision. Les fichiers Verse
// écrits pendant les tâches finies (aperçu, « Ajouter à la bibliothèque » ou « Ignorer ») et les leçons proposées par
// les IA (« Valider » : elle devient une compétence lue par les IA suivantes). Seul un membre décide.
import { useState } from 'react'
import { Check, Code, Lightbulb, X } from '../icons'
import { api, ago } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr } from '../i18n/t'

type Verse = { path: string; tache: number; titre_tache: string; lignes: number; apercu: string }
type Lecon = { id: number; titre: string; contenu: string; agent: string | null; created_at: string }
type Donnees = { peut_valider: boolean; verse: Verse[]; lecons: Lecon[] }

export function ForgeApprendre({ mapId }: { mapId: number }) {
  const app = useApp()
  const { data, reload } = useLoad<Donnees>(`/api/maps/${mapId}/apprendre`, [mapId])
  useLive(['apprendre', 'tasks'], reload)
  const [ouvert, setOuvert] = useState<string | null>(null)
  const [occupe, setOccupe] = useState('')
  if (!data || (!data.verse.length && !data.lecons.length)) return null
  const faire = async (cle: string, chemin: string, corps: any, msg: string) => {
    setOccupe(cle)
    try { await api.post(`/api/maps/${mapId}/apprendre/${chemin}`, corps); app.toast(msg); reload() } catch (e: any) { app.toast(e.message, true) }
    setOccupe('')
  }
  const ok = data.peut_valider
  return <section className="card stack" style={{ gap: 12 }} aria-label={tr("À garder pour les prochaines maps")}>
    <div>
      <b>{tr("À garder pour les prochaines maps")}</b>
      <p className="small muted" style={{ margin: '2px 0 0' }}>{ok ? tr("Ce qui a marché sur cette map : ajoutez-le à la bibliothèque du studio. Rien n’est ajouté sans vous.") : tr("Un membre du studio décide de ce qui est gardé.")}</p>
    </div>
    {data.verse.map(v => <div key={v.path} className="stack" style={{ gap: 6, borderTop: '1px solid var(--line)', paddingTop: 10 }}>
      <div className="row" style={{ gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
        <Code size={16} strokeWidth={1.6} />
        <button className="linkbtn" aria-expanded={ouvert === v.path} onClick={() => setOuvert(o => o === v.path ? null : v.path)}>{v.path}</button>
        <span className="small muted">{tr("{0} lignes · tâche #{1} « {2} »", [v.lignes, v.tache, v.titre_tache])}</span>
        <div style={{ flex: 1 }} />
        {ok && <button className="btn pri" disabled={!!occupe} onClick={() => faire('v' + v.path, 'verse', { path: v.path }, tr("{0} ajouté à la bibliothèque du studio.", [v.path]))}><Check size={15} strokeWidth={1.8} />{tr("Ajouter à la bibliothèque")}</button>}
        {ok && <button className="btn" disabled={!!occupe} onClick={() => faire('i' + v.path, 'verse/ignorer', { path: v.path }, tr("{0} ne sera plus proposé.", [v.path]))}>{tr("Ignorer")}</button>}
      </div>
      {ouvert === v.path && <pre className="small" style={{ margin: 0, maxHeight: 260, overflow: 'auto', padding: 10, borderRadius: 10, background: 'var(--bg2, rgba(127,127,127,.08))' }}>{v.apercu}{v.lignes > 30 ? '\n…' : ''}</pre>}
    </div>)}
    {data.lecons.map(l => <div key={l.id} className="stack" style={{ gap: 6, borderTop: '1px solid var(--line)', paddingTop: 10 }}>
      <div className="row" style={{ gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
        <Lightbulb size={16} strokeWidth={1.6} />
        <b style={{ fontWeight: 500 }}>{tr("Leçon : {0}", [l.titre])}</b>
        <span className="small muted">{[l.agent, ago(l.created_at)].filter(Boolean).join(' · ')}</span>
        <div style={{ flex: 1 }} />
        {ok && <button className="btn pri" disabled={!!occupe} onClick={() => faire('l' + l.id, `lecons/${l.id}/valider`, {}, tr("Leçon validée : les IA la liront."))}><Check size={15} strokeWidth={1.8} />{tr("Valider")}</button>}
        {ok && <button className="btn" disabled={!!occupe} onClick={() => faire('r' + l.id, `lecons/${l.id}/refuser`, {}, tr("Leçon refusée."))}><X size={15} strokeWidth={1.8} />{tr("Refuser")}</button>}
      </div>
      <p className="small" style={{ margin: 0, whiteSpace: 'pre-wrap' }}>{l.contenu}</p>
    </div>)}
  </section>
}
