// « Générer une miniature » (#308) : un clic crée une tâche POP pour l'IA du client, qui prend les photos de l'île
// (#306), s'inspire de la bibliothèque et fait dessiner la miniature par POP (#307), puis la rend (submit_thumbnail).
// Forge montre la miniature, « Conforme Discover », et « Utiliser », « Refaire », « Modifier ». Le site montre les
// versions choisies (`versions`).
import { Suspense, lazy, useEffect, useRef, useState } from 'react'
import { Box, Minimize2, Shapes, Type, Check, Download, ImagePlus, RefreshCw, Pencil, Paperclip, X } from '../icons'
import { ForgeBlender } from './ForgeBlender'
import { ForgeTexte } from './ForgeTexte'
import { ForgeLowMesh } from './ForgeLowMesh'
const ForgeTexte3D = lazy(() => import('./ForgeTexte3D').then(m => ({ default: m.ForgeTexte3D })))
import { ago, api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr } from '../i18n/t'

type Mini = { id: number; title: string; url?: string; logo?: string; bytes: number; submitted_at: string; used_at?: string | null; created_at: string }
type Liste = { ia: { id: number; name: string } | null; miniatures: Mini[]; en_cours: { task: number; status: string; idea: string }[] }

export function ForgeMiniature({ mapId, versions, retour, allerAgents }: { mapId: number; versions?: boolean; retour?: () => void; allerAgents?: () => void }) {
  const app = useApp()
  const { data, reload } = useLoad<Liste>(`/api/maps/${mapId}/miniatures`, [mapId])
  useLive(['miniatures', 'tasks'], reload)
  const [modif, setModif] = useState<string | null>(null)
  const [occupe, setOccupe] = useState(false)
  if (!data) return null
  const demander = async (base: number | undefined, idee: string, exemples: string[] = []): Promise<boolean> => {
    setOccupe(true)
    try {
      const r = await api.post(`/api/maps/${mapId}/miniatures/demandes`, { idee, base, exemples })
      app.toast(tr("{0} fait la miniature (tâche #{1}).", [r.agent, r.task]))
      setModif(null); reload(); setOccupe(false); return true
    } catch (e: any) { app.toast(e.message, true) }
    setOccupe(false); return false
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
  const ia = data.ia
  return <section className="stack" style={{ gap: 14 }} aria-label={tr("Miniature")}>
    <div className="row" style={{ gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
      {retour && <button className="btn" onClick={retour}>{tr("← Outils")}</button>}
      <h1 className="forge-h1" style={{ margin: 0 }}>{tr("Miniature")}</h1>
      <span className="small muted">{ia ? tr("Vous parlez à {0}. Il s'aide des compétences et de la bibliothèque miniature de POP.", [ia.name]) : ''}</span>
    </div>
    {!ia ? <div className="card stack" style={{ padding: 20, gap: 10 }} role="status">
      <b>{tr("Aucune IA connectée à cette map")}</b>
      <span className="muted">{tr("Cet outil parle à votre IA : lancez-en une depuis Agents → + Terminal, ou connectez la vôtre (POP → IA connectées), puis revenez ici.")}</span>
      {allerAgents && <div><button className="btn pri" onClick={allerAgents}>{tr("Aller aux Agents")}</button></div>}
    </div> : <>
      <Discussion mapId={mapId} aid={ia.id} nom={ia.name} />
      <Composer occupe={occupe} base={modif !== null ? derniere : undefined} annulerBase={() => setModif(null)}
        envoyer={(texte, exemples) => demander(modif !== null && derniere ? derniere.id : undefined, texte, exemples)} />
    </>}
    {data.en_cours.length > 0 && <p className="small" role="status" style={{ margin: 0 }}>{tr("En cours : tâche #{0}{1}. La miniature s'affichera ici dès que votre IA l'aura rendue.", [data.en_cours[0].task, data.en_cours[0].idea ? ` (« ${data.en_cours[0].idea} »)` : ''])}</p>}
    {derniere && <div className="stack" style={{ gap: 10 }}>
      {derniere.url && <img src={derniere.url} alt={derniere.title} style={{ width: '100%', maxWidth: 960, aspectRatio: '16 / 9', objectFit: 'cover', borderRadius: 12, border: '1px solid var(--border)' }} />}
      <div className="row" style={{ gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        <span className="chip" title={tr("1920 × 1080, moins de 5 Mo, aucun mot interdit par Epic, pas de commande de plateforme, pas de copie d'une inspiration")}><Check size={14} strokeWidth={2} />{tr("Conforme Discover")}</span>
        <span className="small muted">{derniere.title} · {ago(derniere.submitted_at)}{derniere.used_at ? ' · ' + tr("utilisée") : ''}</span>
        <div style={{ flex: 1 }} />
        <button className="btn pri" disabled={!!derniere.used_at || !app.canEdit} onClick={() => utiliser(derniere)}>{derniere.used_at ? tr("Utilisée") : tr("Utiliser cette miniature")}</button>
        <button className="btn" disabled={occupe || !app.canEdit || !ia} onClick={() => demander(undefined, tr("Refais une autre miniature."))}><RefreshCw size={15} strokeWidth={1.6} />{tr("Refaire")}</button>
        <button className="btn" disabled={!app.canEdit || !ia} onClick={() => setModif(m => m === null ? '' : null)}><Pencil size={15} strokeWidth={1.6} />{tr("Modifier")}</button>
      </div>
      {derniere.used_at && <div className="row" style={{ gap: 8 }}>
        {derniere.url && <a className="btn" href={derniere.url} download><Download size={15} strokeWidth={1.6} />{tr("Image 1920 × 1080")}</a>}
        {derniere.logo && <a className="btn" href={derniere.logo} download><Download size={15} strokeWidth={1.6} />{tr("Logo 1440 × 608")}</a>}
        <span className="small muted" style={{ alignSelf: 'center' }}>{tr("À envoyer dans le portail créateur d'Epic.")}</span>
      </div>}
    </div>}
  </section>
}

/** Discussion avec l'IA (#468) : les messages échangés dans POP Forge, les plus récents en bas. */
export function Discussion({ mapId, aid, nom, canal = 'miniature', vide }: { mapId: number; aid: number; nom: string; canal?: string; vide?: string }) {
  const { data, reload } = useLoad<any[]>(`/api/maps/${mapId}/agents/${aid}/messages?canal=${canal}`, [mapId, aid, canal])
  useLive(['messages'], reload)
  const fin = useRef<HTMLDivElement>(null)
  useEffect(() => { fin.current?.scrollIntoView({ block: 'nearest' }) }, [data?.length])
  const l = (data ?? []).slice(-12)
  return <div className="card mini-discussion" role="log" aria-label={tr("Discussion avec {0}", [nom])}>
    {!l.length && <p className="muted small" style={{ margin: 0 }}>{vide ?? tr("Expliquez la miniature que vous voulez : l'ambiance, le titre, ce qu'on doit voir. Joignez des exemples si vous en avez.")}</p>}
    {l.map(m => <div key={m.id} className={'mini-bulle' + (m.from === 'ia' ? ' ia' : '')}>
      <span className="small muted">{m.from === 'ia' ? nom : tr("Vous")} · {ago(m.at)}{m.from !== 'ia' && !m.read ? ' · ' + tr("pas encore lu") : ''}</span>
      <span style={{ whiteSpace: 'pre-wrap' }}>{String(m.body).replace(/\n\(Outil (Miniature|3D Studio) :[^)]*\)\s*$/, '')}</span>
    </div>)}
    <div ref={fin} />
  </div>
}

/** Zone d'envoi (#468) : texte et jusqu'à 4 images d'exemple ; « Modifier » ajoute la miniature de départ. */
function Composer({ occupe, base, annulerBase, envoyer }: { occupe: boolean; base?: Mini; annulerBase: () => void; envoyer: (texte: string, exemples: string[]) => Promise<boolean> }) {
  const app = useApp()
  const [texte, setTexte] = useState('')
  const [images, setImages] = useState<{ nom: string; data: string }[]>([])
  const fichier = useRef<HTMLInputElement>(null)
  const ajouter = async (l: FileList | null) => {
    const f = Array.from(l ?? []).filter(x => /^image\/(png|jpeg)$/.test(x.type))
    const lus = await Promise.all(f.map(x => new Promise<{ nom: string; data: string }>((ok, ko) => { const r = new FileReader(); r.onload = () => ok({ nom: x.name, data: String(r.result) }); r.onerror = ko; r.readAsDataURL(x) })))
    setImages(i => [...i, ...lus].slice(0, 4))
  }
  const go = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!texte.trim() && !images.length) return
    if (await envoyer(texte.trim(), images.map(i => i.data))) { setTexte(''); setImages([]) }
  }
  return <form className="card stack" style={{ padding: 12, gap: 8 }} onSubmit={go}>
    {base && <div className="row small" style={{ gap: 8, alignItems: 'center' }}>
      <span>{tr("Modifier « {0} » :", [base.title])}</span>
      <button type="button" className="linkbtn small" onClick={annulerBase}>{tr("Annuler")}</button>
    </div>}
    <textarea className="field" rows={3} value={texte} maxLength={300} disabled={!app.canEdit}
      onChange={e => setTexte(e.target.value)} onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); go(e as any) } }}
      placeholder={base ? tr("Ce qu'il faut changer (ex. titre plus gros)") : tr("Décrivez la miniature : ambiance, titre, ce qu'on doit voir… (ex. ambiance fun, gros chiffres)")}
      aria-label={tr("Message à votre IA")} />
    {images.length > 0 && <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>{images.map((i, k) => <div key={k} className="mini-pj">
      <img src={i.data} alt={i.nom} />
      <button type="button" className="iconbtn" aria-label={tr("Retirer {0}", [i.nom])} onClick={() => setImages(l => l.filter((_, j) => j !== k))}><X size={14} /></button>
    </div>)}</div>}
    <div className="row" style={{ gap: 8, alignItems: 'center' }}>
      <input ref={fichier} type="file" accept="image/png,image/jpeg" multiple hidden onChange={e => { ajouter(e.target.files); e.target.value = '' }} />
      <button type="button" className="btn" disabled={images.length >= 4} onClick={() => fichier.current?.click()}><Paperclip size={15} strokeWidth={1.6} />{tr("Exemples ({0}/4)", [images.length])}</button>
      <div style={{ flex: 1 }} />
      <button className="btn pri" disabled={occupe || (!texte.trim() && !images.length)}><ImagePlus size={16} strokeWidth={1.6} />{base ? tr("Envoyer la modification") : tr("Générer une miniature")}</button>
    </div>
  </form>
}

/** Onglet Outils de Forge (#468) : une carte par outil. */
export function ForgeOutils({ mapId, allerAgents, invoke }: { mapId: number; allerAgents: () => void; invoke: any }) {
  const [outil, setOutil] = useState<'' | 'miniature' | 'blender' | 'texte' | 'texte3d' | 'lowmesh'>('')
  if (outil === 'miniature') return <ForgeMiniature mapId={mapId} retour={() => setOutil('')} allerAgents={allerAgents} />
  if (outil === 'blender') return <ForgeBlender invoke={invoke} retour={() => setOutil('')} />
  if (outil === 'texte3d') return <Suspense fallback={null}><ForgeTexte3D invoke={invoke} mapId={mapId} retour={() => setOutil('')} allerBlender={() => setOutil('blender')} /></Suspense>
  if (outil === 'lowmesh') return <ForgeLowMesh invoke={invoke} mapId={mapId} retour={() => setOutil('')} allerBlender={() => setOutil('blender')} />
  if (outil === 'texte') return <ForgeTexte invoke={invoke} mapId={mapId} retour={() => setOutil('')} allerBlender={() => setOutil('blender')} />
  return <div className="stack" style={{ gap: 18 }}>
    <h1 className="forge-h1">{tr("Outils")}</h1>
    <div className="outils-grille">
      <button className="card outil-carte" onClick={() => setOutil('miniature')}>
        <ImagePlus size={22} strokeWidth={1.5} />
        <b>{tr("Miniature")}</b>
        <span className="small muted">{tr("Décrivez la miniature à votre IA : elle prend des photos de l'île et POP la dessine, conforme aux règles d'Epic.")}</span>
      </button>
      <button className="card outil-carte" onClick={() => setOutil('texte3d')}>
        <Shapes size={22} strokeWidth={1.5} />
        <b>{tr("Texte 3D")}</b>
        <span className="small muted">{tr("Un titre ou un panneau en vraie 3D : tournez-le, réglez l'épaisseur et le biseau en direct, puis importez-le dans le projet.")}</span>
      </button>
      <button className="card outil-carte" onClick={() => setOutil('texte')}>
        <Type size={22} strokeWidth={1.5} />
        <b>{tr("Texte 2D")}</b>
        <span className="small muted">{tr("Un titre ou un logo en image transparente (PNG), importé dans le projet en un clic.")}</span>
      </button>
      <button className="card outil-carte" onClick={() => setOutil('lowmesh')}>
        <Minimize2 size={22} strokeWidth={1.5} />
        <b>{tr("Low Mesh")}</b>
        <span className="small muted">{tr("Allégez un modèle 3D (FBX, OBJ, GLB) avant de l'importer : triangles avant et après, aperçu, import en un clic.")}</span>
      </button>
      <button className="card outil-carte" onClick={() => setOutil('blender')}>
        <Box size={22} strokeWidth={1.5} />
        <b>{tr("Blender")}</b>
        <span className="small muted">{tr("Le moteur des outils 3D (texte 3D, Low Mesh, 3D Studio) : à installer une fois, si vous le voulez.")}</span>
      </button>
    </div>
  </div>
}
