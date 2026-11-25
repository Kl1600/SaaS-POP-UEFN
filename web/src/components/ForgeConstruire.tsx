import { useEffect, useState } from 'react'
import { Hammer, Play, Square, Upload, RefreshCw } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLoad } from '../state'
import { tr } from '../i18n/t'

// Écran Construire de Forge (#83) : compilation et session de test, en direct, que l'action vienne d'une IA ou de vous.
const NOMS: Record<string, string> = { compile: tr("Compilation"), start_session: tr("Démarrage de la session"), stop_session: tr("Arrêt de la session"), push_session: tr("Envoi des changements") }

export function ForgeConstruire({ mapId }: { mapId: number }) {
  const app = useApp()
  const { data, reload } = useLoad<any>(`/api/maps/${mapId}/build`, [mapId])
  const [occupe, setOccupe] = useState<string | null>(null)
  const [sortie, setSortie] = useState<any[] | null>(null)
  // Chaque action UEFN (d'une IA ou d'un humain) arrive sur le WebSocket de la map : l'écran suit en direct.
  useEffect(() => app.subscribe(m => { if (m?.t === 'uefn' || m?.t === 'agent') reload() }), [mapId])
  const agir = async (action: string) => {
    setOccupe(action)
    try { await api.post(`/api/maps/${mapId}/uefn/action`, { action, args: {} }); app.toast(tr("{0} : terminé", [NOMS[action]])) } catch (e: any) { app.toast(e.message, true) }
    setOccupe(null); reload()
  }
  const lireSortie = async () => {
    try { const r = await api.post(`/api/maps/${mapId}/uefn/action`, { action: 'session_output', args: { max_results: 50 } }); setSortie(r?.lines ?? r?.output ?? r?.results ?? []) } catch (e: any) { app.toast(e.message, true) }
  }
  if (!data) return null
  const relie = data.relay === 'connected'
  const c = data.compile
  const diags: any[] = Array.isArray(c?.diagnostics) ? c.diagnostics : []
  const erreurs = diags.filter(d => d.severity === 'error')
  return <div className="stack" style={{ gap: 18 }}>
    <h1 className="forge-h1">{tr("Construire")}</h1>
    {!relie && <p className="muted" style={{ margin: 0 }}>{tr("UEFN n’est pas relié : ouvrez le projet dans UEFN et reliez-le depuis l’Accueil pour compiler et lancer la session.")}</p>}
    <div className="forge-ligne">
      <button className="btn pri" disabled={!relie || !!occupe} onClick={() => agir('compile')}><Hammer size={15} /> {occupe === 'compile' ? tr("Compilation…") : tr("Compiler")}</button>
      {data.session_running
        ? <button className="btn" disabled={!relie || !!occupe} onClick={() => agir('stop_session')}><Square size={15} /> {tr("Arrêter la session")}</button>
        : <button className="btn" disabled={!relie || !!occupe} onClick={() => agir('start_session')}><Play size={15} /> {occupe === 'start_session' ? tr("Démarrage…") : tr("Démarrer la session")}</button>}
      <button className="btn" disabled={!relie || !!occupe || !data.session_running} onClick={() => agir('push_session')}><Upload size={15} /> {tr("Envoyer les changements")}</button>
    </div>
    <section>
      <h2 className="forge-h2">{tr("Dernière compilation")}</h2>
      {!c ? <p className="muted" style={{ margin: 0 }}>{tr("Pas encore de compilation sur cette map.")}</p> : <div className="card" style={{ padding: 14 }}>
        <div className="forge-ligne">
          <span className="forge-point" style={{ background: c.status !== 'done' || erreurs.length ? 'var(--red)' : 'var(--tg)' }} aria-hidden="true" />
          <b>{c.status !== 'done' ? tr("Échec") : erreurs.length ? tr("{0} erreur(s)", [erreurs.length]) : tr("Compilation réussie")}</b>
          <span className="small muted">{c.by ? (c.ai ? tr("par l’IA {0}", [c.by]) : tr("par {0}", [c.by])) + ' · ' : ''}{ago(c.at)}</span>
        </div>
        {c.error && <p className="small" style={{ color: 'var(--red)', marginBottom: 0 }}>{c.error}</p>}
        {diags.length > 0 && <div className="forge-maps" style={{ marginTop: 10 }} role="list" aria-label={tr("Erreurs et avertissements")}>
          {diags.slice(0, 50).map((d, i) => <div key={i} role="listitem" className="forge-map-ligne" style={{ flexDirection: 'row', gap: 10, padding: '8px 12px', alignItems: 'baseline' }}>
            <span className="badge" style={{ color: d.severity === 'error' ? 'var(--red)' : undefined }}>{d.severity === 'error' ? tr("erreur") : d.severity === 'warning' ? tr("avertissement") : tr("info")}</span>
            <code>{d.path}:{d.line}</code>
            <span style={{ flex: 1 }}>{d.message}</span>
          </div>)}
        </div>}
      </div>}
    </section>
    <section>
      <div className="forge-ligne" style={{ justifyContent: 'space-between' }}>
        <h2 className="forge-h2" style={{ margin: 0 }}>{tr("Session de test")}</h2>
        <button className="linkbtn" disabled={!relie} onClick={lireSortie}><RefreshCw size={13} /> {tr("Lire la sortie de la session")}</button>
      </div>
      <p className="small muted">{data.session_running ? tr("Session en cours.") : tr("Pas de session en cours.")}</p>
      {data.session.length > 0 && <div className="forge-maps" role="log" aria-label={tr("Actions de session")}>
        {data.session.map((s: any) => <div key={s.id} className="forge-map-ligne" style={{ flexDirection: 'row', gap: 10, padding: '8px 12px' }}>
          <span className="forge-point" style={{ background: s.status === 'done' ? 'var(--tg)' : s.status === 'failed' ? 'var(--red)' : 'var(--text3)' }} aria-hidden="true" />
          <span style={{ flex: 1 }}>{NOMS[s.action] ?? s.action}{s.error ? ' · ' + s.error : ''}</span>
          <span className="small muted">{s.by ? s.by + ' · ' : ''}{ago(s.at)}</span>
        </div>)}
      </div>}
      {sortie && <pre className="small" style={{ marginTop: 10, maxHeight: 280, overflow: 'auto', whiteSpace: 'pre-wrap', background: 'var(--hover)', padding: 12, borderRadius: 10 }}>{sortie.length ? sortie.map((l: any) => typeof l === 'string' ? l : `${l.time ?? ''} ${l.file ? l.file + ':' + (l.line ?? '') + ' ' : ''}${l.message ?? l.text ?? JSON.stringify(l)}`).join('\n') : tr("Aucune sortie pour l’instant.")}</pre>}
    </section>
  </div>
}
