import { useState } from 'react'
import { Download, Lock, Users, Globe, PackagePlus } from 'lucide-react'
import { api } from '../api'
import { useApp, useLoad } from '../state'
import { tr } from '../i18n/t'
import { Selecteur } from './Selecteur'

// Bibliothèque de Forge (#382) : vos assets (privés par défaut), ceux du studio et les publics ; vous choisissez
// qui voit les vôtres et vous les ajoutez au projet UEFN de n'importe laquelle de vos maps.
type Onglet = 'mine' | 'studio' | 'public'
const VIS: Record<string, [string, typeof Lock]> = {
  private: [tr("Privé"), Lock],
  studio: [tr("Studio"), Users],
  public: [tr("Public"), Globe],
}

function taille(o: number) {
  return o >= 1048576 ? `${(o / 1048576).toFixed(1)} Mo` : `${Math.max(1, Math.round(o / 1024))} Ko`
}

export function ForgeBibliotheque({ mapId, studioId }: { mapId: number | null; studioId: number | null }) {
  const app = useApp()
  const [onglet, setOnglet] = useState<Onglet>('mine')
  const [q, setQ] = useState('')
  const url = onglet === 'studio' ? (studioId ? `/api/studios/${studioId}/assets` : null) : `/api/me/assets?tab=${onglet}`
  const { data, reload } = useLoad<any>(url, [url])
  const [occupe, setOccupe] = useState<number | null>(null)

  const changer = async (id: number, visibility: string) => {
    try { await api.post(`/api/assets/${id}/visibility`, { visibility }); app.toast(tr("Visibilité : {0}", [VIS[visibility][0]])); reload() }
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
  const onglets: [Onglet, string][] = [['mine', tr("Mes assets")], ['studio', tr("Studio")], ['public', tr("Public")]]
  return <div className="stack" style={{ gap: 18 }}>
    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
      <div className="tabs" role="tablist">
        {onglets.map(([k, l]) => <button key={k} role="tab" aria-selected={onglet === k} className={onglet === k ? 'on' : ''} onClick={() => setOnglet(k)}>{l}</button>)}
      </div>
      <div style={{ flex: 1 }} />
      <input className="field" style={{ width: 240, height: 36 }} placeholder={tr("Rechercher")} value={q} onChange={e => setQ(e.target.value)} aria-label={tr("Rechercher")} />
    </div>
    {onglet === 'mine' && <p className="muted" style={{ margin: 0 }}>{tr("Ce que vous envoyez est privé : vous seul le voyez et le réutilisez dans toutes vos maps. Rendez-le visible au studio ou à tous quand vous voulez.")}</p>}
    {onglet === 'studio' && !studioId && <p className="muted">{tr("Choisissez une map.")}</p>}
    {data && liste.length === 0 && <div className="card muted" style={{ padding: 24, textAlign: 'center' }}>
      {onglet === 'mine' ? tr("Aucun asset pour l'instant. Ceux que vous envoyez, ou que POP crée pour vous, arrivent ici.") : tr("Aucun asset.")}
    </div>}
    <div className="stack" style={{ gap: 8 }}>
      {liste.map(a => {
        const v = a.visibility && VIS[a.visibility]
        const Icone = v ? v[1] : Users
        return <div key={a.id} className="card" style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 16px' }}>
          <Icone size={18} strokeWidth={1.5} aria-hidden="true" />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{a.name}</div>
            <small className="muted">{[a.kind, taille(a.size), a.pop ? 'POP' : a.author].filter(Boolean).join(' · ')}</small>
          </div>
          {a.mine && <div className="sel-compact">
            <Selecteur label={tr("Visibilité")} placeholder={tr("Visibilité")} value={a.visibility}
              options={Object.entries(VIS).map(([k, [l]]) => ({ value: k, label: l }))}
              onChange={x => changer(a.id, x)} />
          </div>}
          <a className="iconbtn" href={`/api/assets/${a.id}/download`} title={tr("Télécharger")} aria-label={tr("Télécharger")}><Download size={18} strokeWidth={1.5} /></a>
          <button className="btn" disabled={occupe === a.id || !mapId} onClick={() => ajouter(a)} title={mapId ? '' : tr("Choisissez une map.")}>
            <PackagePlus size={16} strokeWidth={1.5} />{occupe === a.id ? tr("Ajout…") : tr("Ajouter au projet")}
          </button>
        </div>
      })}
    </div>
  </div>
}
