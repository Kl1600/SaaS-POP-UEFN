import { useRef, useState } from 'react'
import { Download, Lock, Globe, PackagePlus, Upload, Box, Music, Image as ImageIcon } from 'lucide-react'
import { api } from '../api'
import { useApp, useLoad } from '../state'
import { tr } from '../i18n/t'

// Bibliothèque de Forge (#382, #462) : tout asset est public (décision du 27/11) ; « Public » montre ceux de tous,
// « Mes assets » les vôtres, même publics. On les ajoute au projet UEFN de n'importe laquelle de ses maps.
// #92 : aperçus (image affichée, son écoutable, 3D : nom et taille) et envoi avec déclaration de droits.
type Onglet = 'public' | 'mine'

function taille(o: number) {
  return o >= 1048576 ? `${(o / 1048576).toFixed(1)} Mo` : `${Math.max(1, Math.round(o / 1024))} Ko`
}

const IMAGES = ['png', 'jpg', 'jpeg']
const SONS = ['wav', 'ogg', 'mp3']

/** Aperçu d'un asset : l'image elle-même, un lecteur pour un son ; la 3D (et le TGA, que le navigateur n'affiche pas) a une icône. */
function Apercu({ a }: { a: any }) {
  const k = String(a.kind).toLowerCase()
  if (IMAGES.includes(k)) return <img className="asset-img" src={`/api/assets/${a.id}/download`} alt={a.name} loading="lazy" />
  const Icone = SONS.includes(k) ? Music : k === 'tga' ? ImageIcon : Box
  return <div className="asset-img asset-ico" aria-hidden="true"><Icone size={26} strokeWidth={1.25} /></div>
}

/** Envoi d'un asset (#92) : refusé sans déclaration de droits ni accord de partage public (#462). */
function Envoi({ studioId, fini }: { studioId: number; fini: () => void }) {
  const app = useApp()
  const fichier = useRef<HTMLInputElement>(null)
  const [f, setF] = useState<File | null>(null)
  const [droits, setDroits] = useState('')
  const [partage, setPartage] = useState(false)
  const [occupe, setOccupe] = useState(false)
  const envoyer = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!f) return
    if (!droits.trim()) { app.toast(tr("Déclarez d'où vient ce fichier et vos droits dessus."), true); return }
    setOccupe(true)
    try {
      const r = await api.post(`/api/studios/${studioId}/assets`, { name: f.name, size: f.size, rights: droits.trim(), partage_public: partage })
      const put = await fetch(r.upload_url, { method: 'PUT', body: f })
      if (!put.ok) throw new Error(tr("Envoi refusé par le stockage ({0})", [put.status]))
      await api.post(`/api/assets/${r.id}/confirm`)
      app.toast(tr("{0} est publié dans la bibliothèque", [f.name])); setF(null); setDroits(''); setPartage(false); if (fichier.current) fichier.current.value = ''; fini()
    } catch (er: any) { app.toast(er.message, true) }
    setOccupe(false)
  }
  return <form className="card stack" style={{ padding: 16, gap: 10 }} onSubmit={envoyer}>
    <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
      <input ref={fichier} type="file" hidden accept=".fbx,.obj,.png,.jpg,.jpeg,.tga,.wav,.ogg,.mp3" onChange={e => setF(e.target.files?.[0] ?? null)} aria-label={tr("Fichier")} />
      <button type="button" className="btn" onClick={() => fichier.current?.click()}><Upload size={16} strokeWidth={1.5} />{tr("Choisir un fichier")}</button>
      <span className={f ? '' : 'muted'} style={{ flex: 1, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{f ? f.name : tr("FBX, OBJ, PNG, JPG, TGA, WAV, OGG ou MP3")}</span>
    </div>
    <input className="field" style={{ height: 36 }} value={droits} maxLength={300} onChange={e => setDroits(e.target.value)}
      placeholder={tr("Droits : ex. « Créé par moi », « Acheté sur Fab, licence standard »")} aria-label={tr("Déclaration de droits")} />
    <label className="row small" style={{ gap: 8, alignItems: 'flex-start' }}><input type="checkbox" checked={partage} onChange={e => setPartage(e.target.checked)} style={{ marginTop: 2 }} />
      <span>{tr("Tout asset est public dans POP : je confirme avoir le droit de partager ce fichier avec tous les utilisateurs (créé par moi, ou licence libre).")}</span></label>
    <div><button className="btn pri" disabled={!f || occupe || !partage}>{occupe ? tr("Envoi…") : tr("Envoyer")}</button></div>
  </form>
}

export function ForgeBibliotheque({ mapId, studioId }: { mapId: number | null; studioId: number | null }) {
  const app = useApp()
  const [onglet, setOnglet] = useState<Onglet>('public')
  const [q, setQ] = useState('')
  const url = `/api/me/assets?tab=${onglet}`
  const { data, reload } = useLoad<any>(url, [url])
  const [occupe, setOccupe] = useState<number | null>(null)

  // Un ancien asset privé ou « studio » (d'avant le 27/11) peut seulement devenir public.
  const publier = async (a: any) => {
    if (!confirm(tr("Publier « {0} » ? Tous les utilisateurs de POP pourront le voir et l'importer.", [a.name]))) return
    try { await api.post(`/api/assets/${a.id}/visibility`, { visibility: 'public' }); app.toast(tr("{0} est public", [a.name])); reload() }
    catch (e: any) { app.toast(e.message, true) }
  }
  const ajouter = async (a: any) => {
    if (!mapId) { app.toast(tr("Choisissez une map."), true); return }
    setOccupe(a.id)
    try { await api.post(`/api/maps/${mapId}/uefn/action`, { action: 'asset_import', args: { asset_id: a.id } }); app.toast(tr("{0} : import envoyé à UEFN", [a.name])) }
    catch (e: any) { app.toast(e.message, true) }
    setOccupe(null)
  }

  const liste: any[] = (data?.assets ?? []).filter((a: any) => !q || a.name.toLowerCase().includes(q.toLowerCase()))
  const onglets: [Onglet, string][] = [['public', tr("Public")], ['mine', tr("Mes assets")]]
  return <div className="stack" style={{ gap: 18 }}>
    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
      <div className="tabs" role="tablist">
        {onglets.map(([k, l]) => <button key={k} role="tab" aria-selected={onglet === k} className={onglet === k ? 'on' : ''} onClick={() => setOnglet(k)}>{l}</button>)}
      </div>
      <div style={{ flex: 1 }} />
      <input className="field" style={{ width: 240, height: 36 }} placeholder={tr("Rechercher")} value={q} onChange={e => setQ(e.target.value)} aria-label={tr("Rechercher")} />
    </div>
    {onglet === 'mine' && studioId && <Envoi studioId={studioId} fini={reload} />}
    <p className="muted" style={{ margin: 0 }}>{onglet === 'mine' ? tr("Tout ce que vous envoyez, ou que POP crée pour vous, est public : chacun peut le réutiliser. Ici, seulement les vôtres.") : tr("Les assets de tous les utilisateurs de POP, à ajouter à vos projets.")}</p>
    {data && liste.length === 0 && <div className="card muted" style={{ padding: 24, textAlign: 'center' }}>
      {onglet === 'mine' ? tr("Aucun asset pour l'instant. Ceux que vous envoyez, ou que POP crée pour vous, arrivent ici.") : tr("Aucun asset.")}
    </div>}
    <div className="stack" style={{ gap: 8 }}>
      {liste.map(a => {
        const prive = a.visibility && a.visibility !== 'public'
        const Icone = prive ? Lock : Globe
        return <div key={a.id} className="card" style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 16px 10px 10px' }}>
          <Apercu a={a} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'flex', gap: 6, alignItems: 'center' }}><Icone size={14} strokeWidth={1.5} aria-hidden="true" />{a.name}</div>
            <small className="muted">{[a.kind, taille(a.size), a.pop ? 'POP' : a.author].filter(Boolean).join(' · ')}</small>
            {SONS.includes(String(a.kind).toLowerCase()) && <audio controls preload="none" src={`/api/assets/${a.id}/download`} style={{ display: 'block', height: 32, marginTop: 6, maxWidth: '100%' }} />}
          </div>
          {a.mine && prive && <button className="btn" onClick={() => publier(a)}>{tr("Rendre public")}</button>}
          <a className="iconbtn" href={`/api/assets/${a.id}/download`} title={tr("Télécharger")} aria-label={tr("Télécharger")}><Download size={18} strokeWidth={1.5} /></a>
          <button className="btn" disabled={occupe === a.id || !mapId} onClick={() => ajouter(a)} title={mapId ? '' : tr("Choisissez une map.")}>
            <PackagePlus size={16} strokeWidth={1.5} />{occupe === a.id ? tr("Ajout…") : tr("Ajouter au projet")}
          </button>
        </div>
      })}
    </div>
  </div>
}
