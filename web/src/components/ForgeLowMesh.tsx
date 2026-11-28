// Outil Low Mesh (#325) : alléger un modèle 3D avant import. Blender (#351) applique son Decimate sur le PC ; aperçus
// et comptes avant et après ; Forge prévient au-delà de 30 000 sommets ; import dans UEFN (journalisé, annulable).
import { useEffect, useRef, useState } from 'react'
import { FileUp, Minimize2, Upload } from '../icons'
import { api } from '../api'
import { useApp } from '../state'
import { tr } from '../i18n/t'

type Invoke = <T = any>(cmd: string, args?: Record<string, unknown>) => Promise<T>
type Compte = { triangles: number; sommets: number }
type Reduit = { chemin: string; nom: string; avant: Compte; apres: Compte; apercu_avant: string; apercu_apres: string; trop_lourd: boolean }

const base64 = (f: File) => new Promise<string>((ok, ko) => { const r = new FileReader(); r.onload = () => ok(String(r.result).split(',')[1] ?? ''); r.onerror = () => ko(r.error); r.readAsDataURL(f) })
const n = (x: number) => x.toLocaleString('fr-FR')

export function ForgeLowMesh({ invoke, mapId, retour, allerBlender }: { invoke: Invoke | null; mapId: number; retour: () => void; allerBlender: () => void }) {
  const app = useApp()
  const [installe, setInstalle] = useState<boolean | null>(null)
  const [fichier, setFichier] = useState<File | null>(null)
  const [taux, setTaux] = useState(50)
  const [res, setRes] = useState<Reduit | null>(null)
  const [dossier, setDossier] = useState('')
  const [occupe, setOccupe] = useState<'' | 'reduire' | 'importer'>('')
  const [erreur, setErreur] = useState('')
  const choix = useRef<HTMLInputElement>(null)
  useEffect(() => { invoke?.<{ installe: boolean }>('blender_etat').then(e => setInstalle(e.installe)).catch(() => setInstalle(false)) }, [])
  const reduire = async () => {
    if (!fichier) return
    setErreur(''); setRes(null)
    if (fichier.size > 100 * 1024 * 1024) { setErreur(tr("Modèle trop gros (100 Mo au plus).")); return }
    setOccupe('reduire')
    try { setRes(await invoke!<Reduit>('lowmesh_reduire', { nom: fichier.name, donnees: await base64(fichier), taux: taux / 100 })) } catch (er: any) { setErreur(String(er)) }
    setOccupe('')
  }
  const importer = async () => {
    if (!res) return
    setErreur(''); setOccupe('importer')
    try {
      await api.post(`/api/maps/${mapId}/uefn/action`, { action: 'asset_import', args: { local_path: res.chemin, filename: res.nom, folder: dossier.trim() } })
      app.toast(tr("{0} : import envoyé à UEFN (annulable dans le journal)", [res.nom]))
    } catch (er: any) { setErreur(er.message ?? String(er)) }
    setOccupe('')
  }
  const image = (b: string, alt: string) => b ? <img src={`data:image/png;base64,${b}`} alt={alt} style={{ width: '100%', maxWidth: 260, aspectRatio: '1', objectFit: 'contain', borderRadius: 12, background: 'repeating-conic-gradient(#e8e8e8 0 25%, #fff 0 50%) 0 0 / 16px 16px' }} /> : null
  return <div className="stack" style={{ gap: 16, maxWidth: 900 }}>
    <div className="row" style={{ gap: 10, alignItems: 'center' }}>
      <button className="btn" onClick={retour}>{tr("← Outils")}</button>
      <h1 className="forge-h1" style={{ margin: 0 }}>{tr("Low Mesh")}</h1>
    </div>
    {!invoke ? <p className="muted">{tr("Cet outil tourne dans POP Forge, sur votre PC.")}</p>
      : installe === false ? <section className="card stack" style={{ gap: 10 }}>
        <p style={{ margin: 0 }}>{tr("Cet outil utilise Blender, à installer une seule fois.")}</p>
        <div className="row"><button className="btn pri" onClick={allerBlender}>{tr("Installer Blender")}</button></div>
      </section>
      : <>
        <section className="card stack" style={{ gap: 12 }}>
          <div className="row" style={{ gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
            <input ref={choix} type="file" accept=".fbx,.obj,.glb,.gltf" hidden onChange={e => { setFichier(e.target.files?.[0] ?? null); setRes(null); e.target.value = '' }} />
            <button className="btn" onClick={() => choix.current?.click()}><FileUp size={16} strokeWidth={1.6} />{tr("Choisir un modèle (FBX, OBJ, GLB)")}</button>
            {fichier && <span className="small">{fichier.name} · {tr("{0} Mo", [(fichier.size / 1048576).toFixed(1)])}</span>}
          </div>
          <label className="stack" style={{ gap: 4 }}>
            <span className="small">{tr("Garder {0} % des triangles", [taux])}</span>
            <input type="range" style={{ accentColor: 'currentColor' }} min={5} max={100} step={5} value={taux} onChange={e => { setTaux(+e.target.value); setRes(null) }} aria-label={tr("Part des triangles gardée")} />
          </label>
          <div className="row" style={{ justifyContent: 'flex-end' }}>
            <button className="btn pri" disabled={!fichier || !!occupe || installe == null} onClick={reduire}><Minimize2 size={16} strokeWidth={1.6} />{occupe === 'reduire' ? tr("Blender travaille…") : tr("Réduire")}</button>
          </div>
        </section>
        {res && <section className="card stack" style={{ gap: 12 }} aria-label={tr("Résultat")}>
          <div className="row" style={{ gap: 16, alignItems: 'flex-start' }}>
            <div className="stack" style={{ gap: 6, flex: 1 }}><b className="small">{tr("Avant")}</b>{image(res.apercu_avant, tr("Modèle avant réduction"))}
              <span className="small muted">{tr("{0} triangles · {1} sommets", [n(res.avant?.triangles ?? 0), n(res.avant?.sommets ?? 0)])}</span></div>
            <div className="stack" style={{ gap: 6, flex: 1 }}><b className="small">{tr("Après")}</b>{image(res.apercu_apres, tr("Modèle après réduction"))}
              <span className="small muted">{tr("{0} triangles · {1} sommets", [n(res.apres?.triangles ?? 0), n(res.apres?.sommets ?? 0)])}</span></div>
          </div>
          {res.trop_lourd && <p className="small" role="alert" style={{ margin: 0 }}>{tr("Encore plus de 30 000 sommets : le validateur de mesh d’Epic avertit au-delà. Réduisez davantage si possible.")}</p>}
          <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Dossier du Content Browser (facultatif)")}</span>
            <input className="field" value={dossier} placeholder="/MonProjet/POP/Modeles" onChange={e => setDossier(e.target.value)} /></label>
          <div className="row" style={{ justifyContent: 'flex-end' }}>
            <button className="btn pri" disabled={!!occupe} onClick={importer}><Upload size={16} strokeWidth={1.6} />{occupe === 'importer' ? tr("Import…") : tr("Importer dans UEFN")}</button>
          </div>
        </section>}
      </>}
    {erreur && <p className="small" role="alert" style={{ color: 'var(--danger, #c0392b)', whiteSpace: 'pre-wrap' }}>{erreur}</p>}
  </div>
}
