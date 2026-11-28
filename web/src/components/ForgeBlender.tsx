// Blender dans Forge (#351) : installé au premier usage, après accord (taille annoncée), puis un essai de script isolé
// (aperçu PNG et nombre de triangles). Base des outils Texte 3D/2D, Low Mesh et 3D Studio.
import { useEffect, useRef, useState } from 'react'
import { Box, Download, Play } from 'lucide-react'
import { useApp } from '../state'
import { tr } from '../i18n/t'

type Invoke = <T = any>(cmd: string, args?: Record<string, unknown>) => Promise<T>
type Etat = { installe: boolean; version: string; taille: number; recu: number | null; decompression: boolean; disponible: boolean }

const mo = (o: number) => Math.round(o / 1_000_000)
export const SCRIPT_ESSAI = `import bpy
# Un cube de 2 m, biseauté.
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.ops.object.modifier_add(type='BEVEL')
bpy.context.object.modifiers["Bevel"].width = 0.1
`

export function ForgeBlender({ invoke, retour }: { invoke: Invoke | null; retour: () => void }) {
  const app = useApp()
  const [etat, setEtat] = useState<Etat | null>(null)
  const [script, setScript] = useState(SCRIPT_ESSAI)
  const [essai, setEssai] = useState<{ apercu: string; triangles: number; objets: number; taille: number } | null>(null)
  const [erreur, setErreur] = useState('')
  const [occupe, setOccupe] = useState(false)
  const suivi = useRef<number | null>(null)
  const lire = async () => { try { const e = await invoke!<Etat>('blender_etat'); setEtat(e); return e } catch (er: any) { setErreur(/not allowed|not found|unknown/i.test(String(er)) ? tr("Mettez Forge à jour (pop-uefn.com/forge) pour utiliser Blender.") : String(er)); return null } }
  const suivre = () => {
    if (suivi.current) return
    suivi.current = window.setInterval(async () => { const e = await lire(); if (!e || e.recu == null) { clearInterval(suivi.current!); suivi.current = null } }, 700)
  }
  useEffect(() => { if (invoke) lire().then(e => { if (e?.recu != null) suivre() }); return () => { if (suivi.current) clearInterval(suivi.current) } }, [])
  const installer = async () => {
    setErreur(''); suivre()
    try { await invoke!('blender_installer'); app.toast(tr("Blender est installé.")) } catch (er: any) { setErreur(String(er)) }
    lire()
  }
  const tester = async () => {
    setErreur(''); setEssai(null); setOccupe(true)
    try { setEssai(await invoke!('blender_tester', { script })) } catch (er: any) { setErreur(String(er)) }
    setOccupe(false)
  }
  return <div className="stack" style={{ gap: 16, maxWidth: 820 }}>
    <div className="row" style={{ gap: 10, alignItems: 'center' }}>
      <button className="btn" onClick={retour}>{tr("← Outils")}</button>
      <h1 className="forge-h1" style={{ margin: 0 }}>{tr("Blender")}</h1>
    </div>
    {!invoke ? <p className="muted">{tr("Blender se lance depuis POP Forge, sur votre PC.")}</p>
      : !etat ? (erreur ? <p className="small" role="alert">{erreur}</p> : <p className="muted">{tr("Chargement…")}</p>)
      : !etat.disponible ? <p className="muted">{tr("Blender n’est proposé que sous Windows.")}</p>
      : !etat.installe ? <section className="card stack" style={{ gap: 10 }}>
        <p style={{ margin: 0 }}>{tr("Les outils 3D de Forge (texte 3D, Low Mesh, 3D Studio) utilisent Blender, logiciel libre de la Blender Foundation (licence GPL). Il n’est pas inclus dans Forge : il s’installe une seule fois, si vous le voulez.")}</p>
        <p className="small muted" style={{ margin: 0 }}>{tr("Blender {0} LTS · téléchargé depuis download.blender.org, vérifié par son empreinte officielle, installé dans le dossier de Forge. Les scripts tournent isolés : sans Internet, sans accès à vos fichiers, 2 minutes au plus.", [etat.version])}</p>
        {etat.recu != null ? <div role="status">
          <progress max={etat.taille || 1} value={etat.decompression ? etat.taille : etat.recu} style={{ width: '100%' }} />
          <p className="small" style={{ margin: '4px 0 0' }}>{etat.decompression ? tr("Décompression et préparation de l’isolation…") : tr("Téléchargement : {0} / {1} Mo", [mo(etat.recu), mo(etat.taille)])}</p>
        </div> : <div className="row" style={{ justifyContent: 'flex-end' }}>
          <button className="btn pri" onClick={installer}><Download size={16} strokeWidth={1.6} />{etat.taille ? tr("Installer Blender ({0} Mo)", [mo(etat.taille)]) : tr("Installer Blender")}</button>
        </div>}
      </section>
      : <section className="card stack" style={{ gap: 10 }}>
        <p style={{ margin: 0 }}>{tr("Blender {0} LTS est installé.", [etat.version])} <span className="small muted">{tr("Essayez un script : il tourne isolé et rend un modèle glTF et son aperçu.")}</span></p>
        <textarea className="field" rows={8} spellCheck={false} value={script} onChange={e => setScript(e.target.value)} aria-label={tr("Script Blender (Python)")} style={{ fontFamily: 'ui-monospace, Consolas, monospace', fontSize: 13 }} />
        <div className="row" style={{ justifyContent: 'flex-end' }}>
          <button className="btn pri" disabled={occupe || !script.trim()} onClick={tester}><Play size={16} strokeWidth={1.6} />{occupe ? tr("Blender travaille…") : tr("Lancer l’essai")}</button>
        </div>
        {essai && <div className="row" style={{ gap: 14, alignItems: 'center' }}>
          {essai.apercu && <img src={`data:image/png;base64,${essai.apercu}`} alt={tr("Aperçu du modèle")} style={{ width: 220, height: 220, borderRadius: 12, background: 'var(--bg2, #f2f2f2)' }} />}
          <div className="stack small" style={{ gap: 4 }}>
            <span><Box size={14} strokeWidth={1.6} /> {tr("{0} triangles · {1} objet(s)", [essai.triangles, essai.objets])}</span>
            <span className="muted">{tr("Fichier glTF : {0} Ko", [Math.ceil(essai.taille / 1024)])}</span>
          </div>
        </div>}
      </section>}
    {erreur && etat && <p className="small" role="alert" style={{ color: 'var(--danger, #c0392b)', whiteSpace: 'pre-wrap' }}>{erreur}</p>}
  </div>
}
