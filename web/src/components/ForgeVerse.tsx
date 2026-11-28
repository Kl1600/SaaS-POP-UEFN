import { useState } from 'react'
import { ArrowLeft, FileDown, Plus, BadgeCheck, Lock } from '../icons'
import { api } from '../api'
import { useApp, useLoad } from '../state'
import { tr } from '../i18n/t'
import { Selecteur } from './Selecteur'

// Bibliothèque Verse de Forge (#86) : les modules du studio et ceux de POP. Recherche, catégories, code en lecture seule,
// « Insérer dans le projet » (écriture journalisée et annulable par le relais) et « Ajouter depuis un fichier du projet ».
const CATS: Record<string, string> = {
  economie: tr("Économie"), equipes: tr("Équipes"), minuteur: tr("Minuteur"), hud: tr("HUD"), sauvegarde: tr("Sauvegarde"),
  interface: tr("Interface"), jeu: tr("Jeu"), outils: tr("Outils"), autre: tr("Autre"),
}

/** Nom de fichier proposé pour un module : « Économie (POP) » → POP/economie_pop.verse. */
export function cheminPropose(nom: string) {
  const s = nom.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '')
  return `POP/${s || 'module'}.verse`
}

export function ForgeVerse({ mapId, studioId }: { mapId: number | null; studioId: number | null }) {
  const [q, setQ] = useState('')
  const [cat, setCat] = useState('')
  const [ouvert, setOuvert] = useState<number | null>(null)
  const [ajout, setAjout] = useState(false)
  const url = studioId ? `/api/studios/${studioId}/library?q=${encodeURIComponent(q)}&category=${cat}` : null
  const { data, reload } = useLoad<any[]>(url, [url])
  if (!studioId) return <p className="muted">{tr("Choisissez une map.")}</p>
  if (ouvert !== null) return <Module id={ouvert} mapId={mapId} retour={() => setOuvert(null)} />
  const liste = data ?? []
  return <div className="stack" style={{ gap: 14 }}>
    <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
      <input className="field" style={{ flex: 1, minWidth: 200, height: 36 }} placeholder={tr("Rechercher un module")} aria-label={tr("Rechercher un module")} value={q} onChange={e => setQ(e.target.value)} />
      <button className="btn" onClick={() => setAjout(!ajout)} disabled={!mapId}><Plus size={16} strokeWidth={1.5} />{tr("Ajouter depuis un fichier du projet")}</button>
    </div>
    <div className="chips" role="group" aria-label={tr("Catégories")} style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
      {[['', tr("Toutes")], ...Object.entries(CATS)].map(([k, l]) =>
        <button key={k} className={'chip' + (cat === k ? ' on' : '')} aria-pressed={cat === k} onClick={() => setCat(k)}>{l}</button>)}
    </div>
    {ajout && mapId && <Ajout mapId={mapId} studioId={studioId} fini={() => { setAjout(false); reload() }} />}
    {data && liste.length === 0 && <div className="card muted" style={{ padding: 24, textAlign: 'center' }}>{tr("Aucun module ne correspond.")}</div>}
    <div className="stack" style={{ gap: 8 }}>
      {liste.map(m => <button key={m.id} className="card verse-mod" onClick={() => setOuvert(m.id)}>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <b style={{ fontWeight: 500 }}>{m.name}</b>
          {m.official && <span className="badge" title={tr("Module de l'équipe POP")}><BadgeCheck size={13} />POP</span>}
          <span className="badge">{CATS[m.category] ?? m.category}</span>
          <div style={{ flex: 1 }} /><small className="muted">v{m.version}</small>
        </div>
        {m.description && <div className="muted verse-desc">{m.description}</div>}
      </button>)}
    </div>
  </div>
}

function Module({ id, mapId, retour }: { id: number; mapId: number | null; retour: () => void }) {
  const app = useApp()
  const { data: m } = useLoad<any>(`/api/library/${id}`, [id])
  const { data: fichiers } = useLoad<any[]>(mapId ? `/api/maps/${mapId}/verse/files` : null, [mapId])
  const [chemin, setChemin] = useState<string | null>(null)
  const [occupe, setOccupe] = useState(false)
  if (!m) return null
  const p = chemin ?? cheminPropose(m.name)
  const existant = (fichiers ?? []).find(f => f.path === p)
  const inserer = async () => {
    if (!mapId) return
    if (existant && !confirm(tr("{0} existe déjà dans le projet : le remplacer ? (annulable depuis le journal)", [p]))) return
    setOccupe(true)
    try { await api.post(`/api/maps/${mapId}/uefn/action`, { action: 'write_file', args: { path: p, content: m.content } }); app.toast(tr("{0} : écriture envoyée à UEFN (annulable dans le journal)", [p])) }
    catch (e: any) { app.toast(e.message, true) }
    setOccupe(false)
  }
  return <div className="stack" style={{ gap: 14 }}>
    <div><button className="linkbtn" onClick={retour} style={{ display: 'inline-flex', gap: 6, alignItems: 'center' }}><ArrowLeft size={16} />{tr("Tous les modules")}</button></div>
    <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
      <h2 style={{ margin: 0, fontWeight: 400 }}>{m.name}</h2>
      <span className="badge">{CATS[m.category] ?? m.category}</span>
      <small className="muted">v{m.version}{m.author ? ' · ' + m.author : ''}</small>
    </div>
    {m.description && <p style={{ margin: 0 }}>{m.description}</p>}
    {m.devices?.length > 0 && <div><b style={{ fontWeight: 500 }}>{tr("Appareils à placer")}</b><ul style={{ margin: '6px 0 0', paddingLeft: 20 }}>{m.devices.map((d: string) => <li key={d}><code>{d}</code></li>)}</ul></div>}
    <div className="card" style={{ padding: 14, display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
      <input className="field" style={{ flex: 1, minWidth: 220, height: 36, fontFamily: 'ui-monospace, monospace' }} value={p} onChange={e => setChemin(e.target.value)} aria-label={tr("Fichier du projet")} />
      <button className="btn pri" disabled={!mapId || occupe || !!existant?.locked_by} onClick={inserer}><FileDown size={16} strokeWidth={1.5} />{occupe ? tr("Envoi…") : tr("Insérer dans le projet")}</button>
      {existant?.locked_by && <small style={{ width: '100%', display: 'flex', gap: 6, alignItems: 'center' }}><Lock size={14} />{tr("Verrouillé dans Lore par {0} : insertion impossible.", [existant.locked_by])}</small>}
      {existant && !existant.locked_by && <small className="muted" style={{ width: '100%' }}>{tr("Ce fichier existe déjà : il sera remplacé.")}</small>}
    </div>
    <pre className="verse-code" aria-label={tr("Code du module")}>{m.content}</pre>
  </div>
}

function Ajout({ mapId, studioId, fini }: { mapId: number; studioId: number; fini: () => void }) {
  const app = useApp()
  const { data: fichiers } = useLoad<any[]>(`/api/maps/${mapId}/verse/files`, [mapId])
  const [chemin, setChemin] = useState('')
  const [nom, setNom] = useState('')
  const [cat, setCat] = useState('autre')
  const [desc, setDesc] = useState('')
  const [occupe, setOccupe] = useState(false)
  const envoyer = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!chemin || !nom.trim()) return
    setOccupe(true)
    try {
      const f = await api.get(`/api/maps/${mapId}/verse/file?path=${encodeURIComponent(chemin)}`)
      await api.post(`/api/studios/${studioId}/library`, { name: nom.trim(), category: cat, description: desc.trim(), content: f.content })
      app.toast(tr("Module « {0} » ajouté à la bibliothèque", [nom.trim()])); fini()
    } catch (er: any) { app.toast(er.message, true) }
    setOccupe(false)
  }
  const liste = fichiers ?? []
  return <form className="card stack" style={{ padding: 16, gap: 10 }} onSubmit={envoyer}>
    {fichiers && liste.length === 0 && <p className="muted" style={{ margin: 0 }}>{tr("Aucun fichier Verse dans ce projet pour l'instant.")}</p>}
    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
      <Selecteur label={tr("Fichier du projet")} placeholder={tr("Fichier du projet")} value={chemin}
        options={liste.map(f => ({ value: f.path, label: f.path }))}
        onChange={v => { setChemin(v); if (!nom) setNom(v.split('/').pop()!.replace(/\.verse$/, '')) }} />
      <div className="sel-compact"><Selecteur label={tr("Catégorie")} placeholder={tr("Catégorie")} value={cat}
        options={Object.entries(CATS).map(([k, l]) => ({ value: k, label: l }))} onChange={setCat} /></div>
    </div>
    <input className="field" style={{ height: 36 }} value={nom} maxLength={80} onChange={e => setNom(e.target.value)} placeholder={tr("Nom du module")} aria-label={tr("Nom du module")} />
    <textarea className="field" rows={3} value={desc} maxLength={1000} onChange={e => setDesc(e.target.value)} placeholder={tr("À quoi sert-il, comment le brancher ?")} aria-label={tr("Description")} />
    <div><button className="btn pri" disabled={!chemin || !nom.trim() || occupe}>{occupe ? tr("Envoi…") : tr("Ajouter à la bibliothèque")}</button></div>
  </form>
}
