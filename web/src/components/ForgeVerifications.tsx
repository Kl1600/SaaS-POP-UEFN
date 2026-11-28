import { useEffect, useState } from 'react'
import { CircleCheck, CircleAlert, OctagonX, RefreshCw, Crosshair, ExternalLink, Circle } from '../icons'
import { ago, api } from '../api'
import { useApp, useLoad } from '../state'
import { tr } from '../i18n/t'

// Écran Vérifications de Forge (#97) : problèmes du niveau (#96), checklist de publication complétée par les
// contrôles automatiques, et « feu vert de publication ». La publication reste sur le Creator Portal d'Epic.
const PORTAIL = 'https://create.fortnite.com/'

export function ForgeVerifications({ mapId, ouvrir }: { mapId: number; ouvrir: (url: string) => void }) {
  const app = useApp()
  const { data: rap, reload: relireRap } = useLoad<any>(`/api/maps/${mapId}/level-checks`, [mapId])
  const { data: build, reload: relireBuild } = useLoad<any>(`/api/maps/${mapId}/build`, [mapId])
  const { data: liste, reload: relireListe } = useLoad<any[]>(`/api/maps/${mapId}/checks`, [mapId])
  const [occupe, setOccupe] = useState<string | null>(null)
  const tout = () => { relireRap(); relireBuild(); relireListe() }
  useEffect(() => app.subscribe(m => { if (m?.t === 'uefn' || m?.t === 'verse' || (m?.t === 'refresh' && m?.what === 'launch')) tout() }), [mapId])

  const relancer = async () => {
    setOccupe('relancer')
    // Relit les acteurs dans UEFN (s'il est relié) puis refait les vérifications sur cet état.
    try { await api.post(`/api/maps/${mapId}/uefn/action`, { action: 'refresh_actors', args: {} }) } catch { /* UEFN absent : dernier état connu */ }
    tout(); setOccupe(null)
  }
  const voir = async (id: string) => {
    setOccupe(id)
    try { await api.post(`/api/maps/${mapId}/uefn/action`, { action: 'actor_select', args: { ids: [id] } }); app.toast(tr("Acteur sélectionné dans UEFN")) }
    catch (e: any) { app.toast(e.message, true) }
    setOccupe(null)
  }
  const cocher = async (key: string, ok: boolean) => {
    try { await api.put(`/api/maps/${mapId}/checks/${key}`, { status: ok ? 'ok' : 'todo' }); relireListe() } catch (e: any) { app.toast(e.message, true) }
  }
  if (!rap || !build || !liste) return null

  const diags: any[] = Array.isArray(build.compile?.diagnostics) ? build.compile.diagnostics : []
  // Erreurs de la dernière compilation terminée ; null : aucune compilation terminée connue.
  const c = build.compile
  const erreurs = diags.filter(d => d.severity === 'error').length
  const compile = !c || c.status === 'queued' || c.status === 'running' ? null : c.status === 'done' ? erreurs : Math.max(1, erreurs)
  const auto: [string, boolean, string][] = [
    [tr("Compilation sans erreur"), compile === 0,
      compile === null ? tr("Aucune compilation connue : demandez à votre IA de compiler.") : compile > 0 ? tr("{0} erreur(s) à la dernière compilation.", [compile]) : tr("Dernière compilation propre.")],
    [tr("Aucun problème bloquant"), rap.bloquants === 0,
      rap.bloquants > 0 ? tr("{0} problème(s) bloquant(s) ci-dessous.", [rap.bloquants]) : tr("Aucun problème bloquant dans le niveau.")],
  ]
  const manquants = [...auto.filter(a => !a[1]).map(a => a[0]), ...liste.filter(c => c.status !== 'ok').map(c => c.title)]
  const vert = manquants.length === 0

  return <div className="stack" style={{ gap: 18 }}>
    <div className={'card verif-feu ' + (vert ? 'ok' : 'non')} role="status">
      {vert ? <CircleCheck size={26} strokeWidth={1.5} /> : <OctagonX size={26} strokeWidth={1.5} />}
      <div style={{ flex: 1 }}>
        <b style={{ fontWeight: 500 }}>{vert ? tr("Feu vert : la map est prête à publier") : tr("Pas encore prête à publier")}</b>
        <div className="muted" style={{ fontSize: 13.5 }}>{vert ? tr("Publiez depuis le Creator Portal d'Epic.") : tr("Il manque : {0}.", [manquants.join(', ')])}</div>
      </div>
      <button className={'btn' + (vert ? ' pri' : '')} onClick={() => ouvrir(PORTAIL)}>{tr("Creator Portal")}<ExternalLink size={14} /></button>
    </div>

    <section className="stack" style={{ gap: 10 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <h3 style={{ margin: 0, fontWeight: 500, fontSize: 16 }}>{tr("Problèmes du niveau")}</h3>
        <small className="muted">{rap.etat_du ? tr("état du niveau : {0}", [ago(rap.etat_du)]) : tr("niveau jamais lu")}</small>
        <div style={{ flex: 1 }} />
        <button className="btn" onClick={relancer} disabled={occupe === 'relancer'}><RefreshCw size={16} strokeWidth={1.5} className={occupe === 'relancer' ? 'spin' : ''} />{tr("Relancer les vérifications")}</button>
      </div>
      {rap.problemes.length === 0 && <div className="card muted" style={{ padding: 18 }}>{tr("Aucun problème trouvé sur {0} acteur(s) et {1} fichier(s) Verse.", [rap.acteurs, rap.fichiers_verse])}</div>}
      {rap.problemes.map((p: any, i: number) => {
        const bloq = p.gravite === 'bloquant'
        return <div key={i} className="card" style={{ padding: '12px 16px', display: 'flex', gap: 12, alignItems: 'center' }}>
          {bloq ? <OctagonX size={18} className="verif-bloq" aria-hidden="true" /> : <CircleAlert size={18} className="muted" aria-hidden="true" />}
          <div style={{ flex: 1, minWidth: 0 }}>
            <span className={'badge' + (bloq ? ' verif-bloq' : '')}>{bloq ? tr("Bloquant") : tr("À vérifier")}</span>
            <div style={{ marginTop: 4 }}>{p.message}</div>
          </div>
          {p.acteur && <button className="btn" disabled={occupe === p.acteur} onClick={() => voir(p.acteur)} title={tr("Sélectionne l'acteur dans UEFN")}><Crosshair size={16} strokeWidth={1.5} />{tr("Voir dans l'Organiseur")}</button>}
        </div>
      })}
      <small className="muted">{tr("Les liens d'événements des appareils classiques ne sont pas lisibles par UEFN et ne sont pas vérifiés.")}</small>
    </section>

    <section className="stack" style={{ gap: 10 }}>
      <h3 style={{ margin: 0, fontWeight: 500, fontSize: 16 }}>{tr("Checklist de publication")}</h3>
      <div className="card" style={{ padding: 0 }}>
        {auto.map(([titre, ok, detail], i) => <div key={titre} className="verif-ligne">
          {ok ? <CircleCheck size={20} strokeWidth={1.5} className="verif-ok" aria-label={tr("Fait")} /> : <Circle size={20} strokeWidth={1.5} className="muted" aria-label={tr("À faire")} />}
          <div style={{ flex: 1, minWidth: 0 }}><div>{titre} <span className="badge">{tr("auto")}</span></div><small className="muted">{detail}</small>
            {/* Erreurs de la dernière compilation, qu'elle vienne d'une IA ou de vous (#395). */}
            {i === 0 && (compile ?? 0) > 0 && <ul className="verif-erreurs">
              {c?.error && !diags.length && <li>{c.error}</li>}
              {diags.filter(d => d.severity === 'error').slice(0, 30).map((d, k) => <li key={k}><code>{d.path}:{d.line}</code> {d.message}</li>)}
            </ul>}
            {i === 0 && c?.by && <small className="muted" style={{ display: 'block' }}>{tr("Compilé par {0}, {1}", [c.by, ago(c.at)])}</small>}
          </div>
        </div>)}
        {liste.map(c => <label key={c.key} className="verif-ligne" style={{ cursor: 'pointer' }}>
          <input type="checkbox" checked={c.status === 'ok'} onChange={e => cocher(c.key, e.target.checked)} />
          <div style={{ flex: 1 }}><div>{c.title}</div><small className="muted">{c.rule}</small></div>
        </label>)}
      </div>
    </section>
  </div>
}
