// Outil Texte 3D et 2D (#324) : un titre de map, un panneau ou un logo, fait par Blender sur le PC (#351), aperçu,
// puis import dans le projet UEFN (action journalisée et annulable, #90).
import { useEffect, useState } from 'react'
import { Type, Upload } from '../icons'
import { api } from '../api'
import { useApp } from '../state'
import { tr } from '../i18n/t'

type Invoke = <T = any>(cmd: string, args?: Record<string, unknown>) => Promise<T>
type Cree = { chemin: string; nom: string; apercu: string; triangles: number; licence: string }

export function ForgeTexte({ invoke, mapId, retour, allerBlender }: { invoke: Invoke | null; mapId: number; retour: () => void; allerBlender: () => void }) {
  const app = useApp()
  const [installe, setInstalle] = useState<boolean | null>(null)
  const [r, setR] = useState({ texte: '', mode: '3d', police: 'poppins', taille: 2, epaisseur: 0.4, biseau: 0.03, couleur: '#ffcc00' })
  const [cree, setCree] = useState<Cree | null>(null)
  const [dossier, setDossier] = useState('')
  const [occupe, setOccupe] = useState<'' | 'creer' | 'importer'>('')
  const [erreur, setErreur] = useState('')
  useEffect(() => { invoke?.<{ installe: boolean }>('blender_etat').then(e => setInstalle(e.installe)).catch(() => setInstalle(false)) }, [])
  const maj = (k: string, v: any) => { setR(x => ({ ...x, [k]: v })); setCree(null) }
  const creer = async (e: React.FormEvent) => {
    e.preventDefault(); setErreur(''); setCree(null); setOccupe('creer')
    try { setCree(await invoke!<Cree>('texte_creer', { reglages: { ...r, taille: +r.taille, epaisseur: +r.epaisseur, biseau: +r.biseau } })) } catch (er: any) { setErreur(String(er)) }
    setOccupe('')
  }
  const importer = async () => {
    if (!cree) return
    setErreur(''); setOccupe('importer')
    try {
      await api.post(`/api/maps/${mapId}/uefn/action`, { action: 'asset_import', args: { local_path: cree.chemin, filename: cree.nom, folder: dossier.trim() } })
      app.toast(tr("{0} : import envoyé à UEFN (annulable dans le journal)", [cree.nom]))
    } catch (er: any) { setErreur(er.message ?? String(er)) }
    setOccupe('')
  }
  const d3 = r.mode === '3d'
  return <div className="stack" style={{ gap: 16, maxWidth: 900 }}>
    <div className="row" style={{ gap: 10, alignItems: 'center' }}>
      <button className="btn" onClick={retour}>{tr("← Outils")}</button>
      <h1 className="forge-h1" style={{ margin: 0 }}>{tr("Texte 3D et 2D")}</h1>
    </div>
    {!invoke ? <p className="muted">{tr("Cet outil tourne dans POP Forge, sur votre PC.")}</p>
      : installe === false ? <section className="card stack" style={{ gap: 10 }}>
        <p style={{ margin: 0 }}>{tr("Cet outil utilise Blender, à installer une seule fois.")}</p>
        <div className="row"><button className="btn pri" onClick={allerBlender}>{tr("Installer Blender")}</button></div>
      </section>
      : <div className="row" style={{ gap: 16, alignItems: 'flex-start', flexWrap: 'wrap' }}>
        <form className="card stack" style={{ gap: 10, flex: '1 1 340px' }} onSubmit={creer}>
          <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Texte")}</span>
            <input className="field" value={r.texte} maxLength={60} placeholder={tr("Nom de la map, panneau…")} onChange={e => maj('texte', e.target.value)} /></label>
          <div className="row" style={{ gap: 6 }} role="radiogroup" aria-label={tr("Type")}>
            {[['3d', tr("Mesh 3D (FBX)")], ['2d', tr("Image 2D (PNG transparent)")]].map(([v, l]) =>
              <button key={v} type="button" role="radio" aria-checked={r.mode === v} className={'btn' + (r.mode === v ? ' pri' : '')} onClick={() => maj('mode', v)}>{l}</button>)}
          </div>
          <div className="row" style={{ gap: 10, flexWrap: 'wrap' }}>
            <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Police")}</span>
              <select className="field" value={r.police} onChange={e => maj('police', e.target.value)}>
                <option value="poppins">Poppins</option><option value="inter">Inter</option>
              </select></label>
            <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Couleur")}</span>
              <input type="color" className="field" style={{ width: 70, padding: 2 }} value={r.couleur} onChange={e => maj('couleur', e.target.value)} /></label>
            <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Hauteur (m)")}</span>
              <input type="number" className="field" style={{ width: 90 }} min={0.2} max={20} step={0.1} value={r.taille} onChange={e => maj('taille', e.target.value)} /></label>
            {d3 && <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Épaisseur (m)")}</span>
              <input type="number" className="field" style={{ width: 90 }} min={0} max={2} step={0.05} value={r.epaisseur} onChange={e => maj('epaisseur', e.target.value)} /></label>}
            {d3 && <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Biseau (m)")}</span>
              <input type="number" className="field" style={{ width: 90 }} min={0} max={0.2} step={0.01} value={r.biseau} onChange={e => maj('biseau', e.target.value)} /></label>}
          </div>
          <div className="row" style={{ justifyContent: 'flex-end' }}>
            <button className="btn pri" disabled={!!occupe || !r.texte.trim() || installe == null}><Type size={16} strokeWidth={1.6} />{occupe === 'creer' ? tr("Blender travaille…") : tr("Créer l’aperçu")}</button>
          </div>
        </form>
        <section className="card stack" style={{ gap: 10, flex: '1 1 340px' }} aria-label={tr("Aperçu")}>
          {cree ? <>
            <img src={`data:image/png;base64,${cree.apercu}`} alt={tr("Aperçu de {0}", [cree.nom])} style={{ width: '100%', maxHeight: 320, objectFit: 'contain', borderRadius: 12, background: 'repeating-conic-gradient(#e8e8e8 0 25%, #fff 0 50%) 0 0 / 16px 16px' }} />
            <span className="small muted">{cree.nom}{d3 ? ' · ' + tr("{0} triangles", [cree.triangles]) : ''}</span>
            <span className="small muted">{tr("Police : {0}", [cree.licence])}</span>
            <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Dossier du Content Browser (facultatif)")}</span>
              <input className="field" value={dossier} placeholder="/MonProjet/POP/Textes" onChange={e => setDossier(e.target.value)} /></label>
            <div className="row" style={{ justifyContent: 'flex-end' }}>
              <button className="btn pri" disabled={!!occupe} onClick={importer}><Upload size={16} strokeWidth={1.6} />{occupe === 'importer' ? tr("Import…") : tr("Importer dans UEFN")}</button>
            </div>
          </> : <p className="muted small" style={{ margin: 0 }}>{tr("L’aperçu apparaît ici. Rien n’est importé tant que vous ne cliquez pas « Importer dans UEFN ».")}</p>}
        </section>
      </div>}
    {erreur && <p className="small" role="alert" style={{ color: 'var(--danger, #c0392b)', whiteSpace: 'pre-wrap' }}>{erreur}</p>}
  </div>
}
