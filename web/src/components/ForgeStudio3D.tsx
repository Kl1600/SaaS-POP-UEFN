// 3D Studio (#327) : l'humain décrit un objet à son IA connectée ; elle écrit un script Blender (blender_build),
// Forge l'exécute isolé sur le PC (#351) ; le modèle s'affiche ici avec « Accepter » (import dans UEFN, journalisé et
// annulable) ou « Refuser » (rien n'est importé). Sans IA connectée ou sans Blender, l'outil l'explique.
import { useEffect, useState } from 'react'
import { Check, Send, X } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr } from '../i18n/t'
import { Discussion } from './ForgeMiniature'

type Invoke = <T = any>(cmd: string, args?: Record<string, unknown>) => Promise<T>
type Modele = { id: number; nom: string; triangles: number; objets: number; fichier: string; statut: 'propose' | 'accepte' | 'refuse'; created_at: string; lourd: boolean; apercu: string }
type Liste = { ia: { id: number; name: string } | null; modeles: Modele[] }

export function ForgeStudio3D({ invoke, mapId, retour, allerAgents, allerBlender, allerLowMesh }: { invoke: Invoke | null; mapId: number; retour: () => void; allerAgents: () => void; allerBlender: () => void; allerLowMesh: () => void }) {
  const app = useApp()
  const { data, reload } = useLoad<Liste>(`/api/maps/${mapId}/studio3d`, [mapId])
  useLive(['studio3d', 'messages'], reload)
  const [blender, setBlender] = useState<boolean | null>(null)
  const [texte, setTexte] = useState('')
  const [dossier, setDossier] = useState('')
  const [occupe, setOccupe] = useState<string>('')
  useEffect(() => { invoke?.<{ installe: boolean }>('blender_etat').then(e => setBlender(e.installe)).catch(() => setBlender(false)) }, [])
  if (!data) return null
  const ia = data.ia
  const envoyer = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!texte.trim()) return
    setOccupe('envoi')
    try { const r = await api.post(`/api/maps/${mapId}/studio3d/demandes`, { texte: texte.trim() }); setTexte(''); app.toast(tr("Demande envoyée à {0}.", [r.agent])) } catch (er: any) { app.toast(er.message, true) }
    setOccupe('')
  }
  const decider = async (m: Modele, quoi: 'accepter' | 'refuser') => {
    setOccupe(quoi + m.id)
    try {
      await api.post(`/api/maps/${mapId}/studio3d/${m.id}/${quoi}`, quoi === 'accepter' ? { folder: dossier.trim() } : {})
      app.toast(quoi === 'accepter' ? tr("{0} : import envoyé à UEFN (annulable dans le journal)", [m.nom]) : tr("{0} refusé : rien n'est importé.", [m.nom]))
      reload()
    } catch (er: any) { app.toast(er.message, true) }
    setOccupe('')
  }
  return <div className="stack" style={{ gap: 14, maxWidth: 960 }}>
    <div className="row" style={{ gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
      <button className="btn" onClick={retour}>{tr("← Outils")}</button>
      <h1 className="forge-h1" style={{ margin: 0 }}>{tr("3D Studio")}</h1>
      <span className="small muted">{ia ? tr("Vous parlez à {0}. Il écrit un script Blender qui tourne sur votre PC.", [ia.name]) : ''}</span>
    </div>
    {blender === false && <div className="card stack" style={{ padding: 16, gap: 8 }} role="status">
      <b>{tr("Blender n’est pas installé")}</b>
      <span className="muted">{tr("3D Studio construit les modèles avec Blender, sur votre PC. Installez-le une seule fois, puis revenez ici.")}</span>
      {invoke && <div><button className="btn pri" onClick={allerBlender}>{tr("Installer Blender")}</button></div>}
    </div>}
    {!ia ? <div className="card stack" style={{ padding: 20, gap: 10 }} role="status">
      <b>{tr("Aucune IA connectée à cette map")}</b>
      <span className="muted">{tr("3D Studio parle à votre IA : lancez-en une depuis Agents → + Terminal, ou connectez la vôtre (POP → IA connectées), puis revenez ici.")}</span>
      <div><button className="btn pri" onClick={allerAgents}>{tr("Aller aux Agents")}</button></div>
    </div> : <>
      <Discussion mapId={mapId} aid={ia.id} nom={ia.name} canal="studio3d" vide={tr("Décrivez l’objet : sa forme, sa taille, ses couleurs (ex. un tonneau en bois de 1 m, cerclé de fer).")} />
      <form className="card stack" style={{ padding: 12, gap: 8 }} onSubmit={envoyer}>
        <textarea className="field" rows={3} value={texte} maxLength={600} disabled={!app.canEdit}
          onChange={e => setTexte(e.target.value)} onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); envoyer(e as any) } }}
          placeholder={tr("Décrivez le modèle 3D à faire…")} aria-label={tr("Message à votre IA")} />
        <div className="row" style={{ justifyContent: 'flex-end' }}>
          <button className="btn pri" disabled={!texte.trim() || !!occupe || !app.canEdit}><Send size={15} strokeWidth={1.6} />{tr("Envoyer")}</button>
        </div>
      </form>
    </>}
    {data.modeles.length > 0 && <section className="stack" style={{ gap: 10 }} aria-label={tr("Modèles proposés")}>
      <label className="stack" style={{ gap: 4, maxWidth: 420 }}><span className="small">{tr("Dossier du Content Browser pour l’import (facultatif)")}</span>
        <input className="field" value={dossier} placeholder="/MonProjet/POP/Modeles" onChange={e => setDossier(e.target.value)} /></label>
      <div className="outils-grille">{data.modeles.map(m => <div key={m.id} className="card stack" style={{ gap: 8 }}>
        <img src={m.apercu} alt={tr("Aperçu de {0}", [m.nom])} loading="lazy" style={{ width: '100%', aspectRatio: '1', objectFit: 'contain', borderRadius: 10, background: 'repeating-conic-gradient(#e8e8e8 0 25%, #fff 0 50%) 0 0 / 16px 16px' }} />
        <b>{m.nom}</b>
        <span className="small muted">{tr("{0} triangles · {1}", [m.triangles.toLocaleString('fr-FR'), ago(m.created_at)])}</span>
        {m.lourd && m.statut === 'propose' && <span className="small">{tr("Lourd pour UEFN : demandez une version plus simple, ou allégez-le avec")} <button className="linkbtn small" onClick={allerLowMesh}>{tr("Low Mesh")}</button>.</span>}
        {m.statut === 'propose' ? <div className="row" style={{ gap: 8 }}>
          <button className="btn pri" disabled={!!occupe || !app.canEdit} onClick={() => decider(m, 'accepter')}><Check size={15} strokeWidth={1.8} />{tr("Accepter")}</button>
          <button className="btn" disabled={!!occupe || !app.canEdit} onClick={() => decider(m, 'refuser')}><X size={15} strokeWidth={1.8} />{tr("Refuser")}</button>
        </div> : <span className="chip small">{m.statut === 'accepte' ? tr("Importé dans UEFN") : tr("Refusé")}</span>}
      </div>)}</div>
    </section>}
  </div>
}
