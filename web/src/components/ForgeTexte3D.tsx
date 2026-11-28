// Outil Texte 3D (#514) : une vraie 3D dans Forge, qu'on tourne à la souris ; épaisseur, biseau, hauteur, couleur et
// police changent en direct (three.js). « Importer dans UEFN » fait le FBX avec Blender aux mêmes réglages (#324) puis
// l'importe dans le projet (action journalisée et annulable, #90).
import { useEffect, useRef, useState } from 'react'
import { Upload } from '../icons'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { TTFLoader } from 'three/examples/jsm/loaders/TTFLoader.js'
import { Font } from 'three/examples/jsm/loaders/FontLoader.js'
import { TextGeometry } from 'three/examples/jsm/geometries/TextGeometry.js'
import { api } from '../api'
import { useApp } from '../state'
import { tr } from '../i18n/t'

type Invoke = <T = any>(cmd: string, args?: Record<string, unknown>) => Promise<T>
type Reglages = { texte: string; police: 'poppins' | 'anton'; taille: number; epaisseur: number; biseau: number; couleur: string }

/** Mêmes fichiers que ceux que Blender reçoit (link/src/texte3d.rs). */
const POLICES: Record<Reglages['police'], { nom: string; url: string }> = {
  poppins: { nom: 'Poppins', url: '/polices/Poppins-Bold.ttf' },
  anton: { nom: 'Anton', url: '/polices/Anton-Regular.ttf' },
}
const cache: Partial<Record<string, Promise<Font>>> = {}
const police = (url: string) => cache[url] ??= new Promise<Font>((ok, ko) => new TTFLoader().load(url, j => ok(new Font(j)), undefined, ko))

/** La scène 3D : texte extrudé et biseauté, éclairé, qu'on tourne à la souris. */
function Apercu3D({ r }: { r: Reglages }) {
  const boite = useRef<HTMLDivElement>(null)
  const scene = useRef<{ maj: (r: Reglages) => void } | null>(null)
  const [pret, setPret] = useState(false)
  const [erreur, setErreur] = useState('')
  useEffect(() => {
    const el = boite.current!
    let rendu: THREE.WebGLRenderer
    try { rendu = new THREE.WebGLRenderer({ antialias: true, alpha: true }) } catch { setErreur(tr("La 3D n’est pas disponible sur ce PC (WebGL).")); return }
    rendu.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    el.appendChild(rendu.domElement)
    const sc = new THREE.Scene()
    const cam = new THREE.PerspectiveCamera(35, 1, 0.01, 1000)
    const ctl = new OrbitControls(cam, rendu.domElement)
    ctl.enableDamping = true
    sc.add(new THREE.HemisphereLight(0xffffff, 0x444444, 1.6))
    const sol = new THREE.DirectionalLight(0xffffff, 2.2)
    sol.position.set(3, 5, 6)
    sc.add(sol)
    const mat = new THREE.MeshStandardMaterial({ roughness: 0.45, metalness: 0.05 })
    const maille = new THREE.Mesh(new THREE.BufferGeometry(), mat)
    sc.add(maille)
    let cadre = ''
    let n = 0
    const maj = async (x: Reglages) => {
      const id = ++n
      const f = await police(POLICES[x.police].url).catch(() => null)
      if (id !== n) return
      if (!f) { setErreur(tr("Police introuvable.")); return }
      const b = Math.min(x.biseau, x.epaisseur / 2)
      const g = new TextGeometry(x.texte.trim() || ' ', {
        font: f, size: x.taille, depth: Math.max(x.epaisseur - 2 * b, 0.001), curveSegments: 6,
        bevelEnabled: b > 0, bevelThickness: b, bevelSize: b, bevelSegments: 3,
      })
      g.center()
      maille.geometry.dispose()
      maille.geometry = g
      mat.color.set(x.couleur)
      // Recadrage quand le texte, la police ou la hauteur changent (même angle de vue) ; l'épaisseur et le biseau
      // ne bougent pas la caméra.
      const cle = `${x.texte}|${x.police}|${x.taille}`
      if (cadre !== cle && x.texte.trim()) {
        g.computeBoundingSphere()
        const rayon = g.boundingSphere?.radius ?? 1
        const dir = cadre ? cam.position.clone().sub(ctl.target).normalize() : new THREE.Vector3(0.22, 0.13, 1).normalize()
        ctl.target.set(0, 0, 0)
        cam.position.copy(dir.multiplyScalar(rayon * 2.9))
        cadre = cle
      }
      setPret(true)
    }
    const taille = () => { const w = el.clientWidth, h = el.clientHeight; rendu.setSize(w, h); cam.aspect = w / Math.max(h, 1); cam.updateProjectionMatrix() }
    const ro = new ResizeObserver(taille)
    ro.observe(el)
    taille()
    let boucle = 0
    const anime = () => { boucle = requestAnimationFrame(anime); ctl.update(); rendu.render(sc, cam) }
    anime()
    scene.current = { maj }
    return () => { cancelAnimationFrame(boucle); ro.disconnect(); ctl.dispose(); maille.geometry.dispose(); mat.dispose(); rendu.dispose(); rendu.domElement.remove(); scene.current = null }
  }, [])
  useEffect(() => { scene.current?.maj(r) }, [r.texte, r.police, r.taille, r.epaisseur, r.biseau, r.couleur])
  return <div style={{ position: 'relative' }}>
    <div ref={boite} role="img" aria-label={tr("Aperçu 3D de « {0} » : tournez-le à la souris", [r.texte])} data-pret={pret ? '1' : '0'}
      style={{ width: '100%', height: 380, borderRadius: 12, overflow: 'hidden', background: 'radial-gradient(circle at 50% 40%, #fafafa, #dcdcdc)', cursor: 'grab' }} />
    {erreur && <p className="small" role="alert" style={{ margin: '6px 0 0' }}>{erreur}</p>}
    <span className="small muted" style={{ position: 'absolute', left: 12, bottom: 8 }}>{tr("Clic gauche : tourner · molette : zoom · clic droit : déplacer")}</span>
  </div>
}

function Curseur({ nom, valeur, min, max, pas, unite, change }: { nom: string; valeur: number; min: number; max: number; pas: number; unite: string; change: (v: number) => void }) {
  return <label className="stack" style={{ gap: 2 }}>
    <span className="small row" style={{ justifyContent: 'space-between' }}><span>{nom}</span><span className="muted">{valeur.toLocaleString('fr-FR')} {unite}</span></span>
    <input type="range" style={{ accentColor: 'currentColor' }} min={min} max={max} step={pas} value={valeur} onChange={e => change(+e.target.value)} aria-label={nom} />
  </label>
}

export function ForgeTexte3D({ invoke, mapId, retour, allerBlender }: { invoke: Invoke | null; mapId: number; retour: () => void; allerBlender: () => void }) {
  const app = useApp()
  const [blender, setBlender] = useState<boolean | null>(null)
  const [r, setR] = useState<Reglages>({ texte: 'POP', police: 'poppins', taille: 2, epaisseur: 0.4, biseau: 0.03, couleur: '#ffcc00' })
  const [dossier, setDossier] = useState('')
  const [occupe, setOccupe] = useState(false)
  const [erreur, setErreur] = useState('')
  useEffect(() => { invoke?.<{ installe: boolean }>('blender_etat').then(e => setBlender(e.installe)).catch(() => setBlender(false)) }, [])
  const maj = <K extends keyof Reglages>(k: K, v: Reglages[K]) => setR(x => ({ ...x, [k]: v }))
  const importer = async () => {
    setErreur(''); setOccupe(true)
    try {
      const c = await invoke!<{ chemin: string; nom: string; licence: string }>('texte_creer', { reglages: { ...r, mode: '3d' } })
      await api.post(`/api/maps/${mapId}/uefn/action`, { action: 'asset_import', args: { local_path: c.chemin, filename: c.nom, folder: dossier.trim() } })
      app.toast(tr("{0} : import envoyé à UEFN (annulable dans le journal)", [c.nom]))
    } catch (er: any) { setErreur(er?.message ?? String(er)) }
    setOccupe(false)
  }
  return <div className="stack" style={{ gap: 16, maxWidth: 1100 }}>
    <div className="row" style={{ gap: 10, alignItems: 'center' }}>
      <button className="btn" onClick={retour}>{tr("← Outils")}</button>
      <h1 className="forge-h1" style={{ margin: 0 }}>{tr("Texte 3D")}</h1>
    </div>
    <div className="row" style={{ gap: 16, alignItems: 'flex-start', flexWrap: 'wrap' }}>
      <section className="card" style={{ flex: '2 1 460px', padding: 10 }} aria-label={tr("Aperçu 3D")}><Apercu3D r={r} /></section>
      <section className="card stack" style={{ flex: '1 1 280px', gap: 12 }} aria-label={tr("Réglages")}>
        <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Texte")}</span>
          <input className="field" value={r.texte} maxLength={60} onChange={e => maj('texte', e.target.value)} /></label>
        <div className="row" style={{ gap: 10 }}>
          <label className="stack" style={{ gap: 4, flex: 1 }}><span className="small">{tr("Police")}</span>
            <select className="field" value={r.police} onChange={e => maj('police', e.target.value as Reglages['police'])}>
              {Object.entries(POLICES).map(([k, p]) => <option key={k} value={k}>{p.nom}</option>)}
            </select></label>
          <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Couleur")}</span>
            <input type="color" className="field" style={{ width: 70, padding: 2 }} value={r.couleur} onChange={e => maj('couleur', e.target.value)} /></label>
        </div>
        <Curseur nom={tr("Épaisseur")} valeur={r.epaisseur} min={0.02} max={2} pas={0.02} unite="m" change={v => maj('epaisseur', v)} />
        <Curseur nom={tr("Biseau")} valeur={r.biseau} min={0} max={0.2} pas={0.01} unite="m" change={v => maj('biseau', v)} />
        <Curseur nom={tr("Hauteur des lettres")} valeur={r.taille} min={0.2} max={20} pas={0.1} unite="m" change={v => maj('taille', v)} />
        <label className="stack" style={{ gap: 4 }}><span className="small">{tr("Dossier du Content Browser (facultatif)")}</span>
          <input className="field" value={dossier} placeholder="/MonProjet/POP/Textes" onChange={e => setDossier(e.target.value)} /></label>
        {!invoke ? <p className="small muted" style={{ margin: 0 }}>{tr("L’import dans UEFN se fait depuis POP Forge.")}</p>
          : blender === false ? <div className="stack" style={{ gap: 6 }}>
            <span className="small muted">{tr("L’import utilise Blender (le FBX est fait aux mêmes réglages), à installer une seule fois.")}</span>
            <button className="btn" onClick={allerBlender}>{tr("Installer Blender")}</button>
          </div>
          : <button className="btn pri" disabled={occupe || !r.texte.trim() || blender == null} onClick={importer}><Upload size={16} strokeWidth={1.6} />{occupe ? tr("Blender fait le modèle…") : tr("Importer dans UEFN")}</button>}
        <span className="small muted">{tr("Police : {0} (licence libre OFL).", [POLICES[r.police].nom])}</span>
      </section>
    </div>
    {erreur && <p className="small" role="alert" style={{ color: 'var(--danger, #c0392b)', whiteSpace: 'pre-wrap' }}>{erreur}</p>}
  </div>
}
