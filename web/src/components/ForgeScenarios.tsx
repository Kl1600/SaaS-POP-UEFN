import { useState } from 'react'
import { ClipboardCheck, Play } from '../icons'
import { useLoad } from '../state'
import { tr } from '../i18n/t'
import { Passage } from './Scenarios'

// Scénarios de test dans Forge (#350) : jouer un scénario validé à côté de la session de test, sans quitter Forge.
// Les verdicts vont au même passage que sur le site ; « Capture » photographie l'écran du jeu (commande capture_jeu).
export function ForgeScenarios({ mapId, capturer }: { mapId: number; capturer?: () => Promise<Blob> }) {
  const { data, reload } = useLoad<any[]>(`/api/maps/${mapId}/scenarios`, [mapId])
  const [joue, setJoue] = useState<any>(null)
  const prets = (data ?? []).filter(s => s.status === 'ready')
  return <section className="card stack" style={{ padding: 18, gap: 12 }} aria-label={tr("Scénarios de test")}>
    <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
      <ClipboardCheck size={18} strokeWidth={1.5} />
      <h3 style={{ margin: 0, fontWeight: 500, fontSize: 16 }}>{tr("Scénarios de test")}</h3>
    </div>
    {joue ? <Passage s={joue} capturer={capturer} onClose={() => { setJoue(null); reload() }} />
      : prets.length === 0 ? <p className="muted" style={{ margin: 0 }}>{tr("Aucun scénario validé pour cette map. Écrivez-en un sur le site : Lancer et suivre, onglet Test.")}</p>
      : prets.map(s => <div key={s.id} style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
        <div style={{ flex: 1, minWidth: 0 }}>{s.title} <small className="muted">· {tr("{0} étape(s)", [s.steps?.length ?? 0])}</small></div>
        <button className="btn" onClick={() => setJoue(s)}><Play size={14} strokeWidth={1.5} />{tr("Jouer")}</button>
      </div>)}
  </section>
}
