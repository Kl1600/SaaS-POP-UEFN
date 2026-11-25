import { useState } from 'react'
import { Search, Plus, RefreshCw, Link2 } from 'lucide-react'
import { api } from '../api'
import { tr } from '../i18n/t'

// Catalogue des appareils d'UEFN et graphe des liens entre appareils du niveau (#119).
export function Appareils({ mapId, canEdit, connected, act }: { mapId: number; canEdit: boolean; connected: boolean; act: (a: string, args: any, ok?: string) => Promise<any> }) {
  const [q, setQ] = useState('')
  const [cat, setCat] = useState<any | null>(null)
  const [graphe, setGraphe] = useState<any | null>(null)
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState('')
  const chercher = async () => {
    setBusy(true); setErr('')
    try { setCat(await api.get(`/api/maps/${mapId}/devices/catalog?q=${encodeURIComponent(q)}`)) } catch (e: any) { setErr(e.message) }
    setBusy(false)
  }
  const lireGraphe = async () => {
    setBusy(true); setErr('')
    try { setGraphe(await api.get(`/api/maps/${mapId}/devices/graph`)) } catch (e: any) { setErr(e.message) }
    setBusy(false)
  }
  const nom = (id: string) => graphe?.nodes.find((n: any) => n.id === id)?.name ?? id.split('.').pop()
  return (
    <div className="stack" style={{ gap: 20 }}>
      <div>
        <h3 style={{ margin: '0 0 8px' }}>{tr("Catalogue des appareils")}</h3>
        <form className="row" style={{ gap: 6 }} onSubmit={e => { e.preventDefault(); chercher() }}>
          <input className="field" style={{ flex: 1 }} placeholder={tr("bouton, minuteur, distributeur…")} value={q} onChange={e => setQ(e.target.value)} aria-label={tr("Chercher un appareil")} />
          <button className="btn" disabled={busy}><Search size={14} /> {tr("Chercher")}</button>
        </form>
        {cat?.message && <p className="small muted">{tr(cat.message)}</p>}
        {cat?.version && <p className="small muted">{tr("UEFN {0} · {1} résultat(s)", [cat.version, cat.devices.length])}</p>}
        {cat?.devices.map((d: any) => (
          <div key={d.asset} className="row" style={{ justifyContent: 'space-between', borderBottom: '1px solid var(--line)', padding: '6px 0' }}>
            <div><b>{d.name}</b> <span className="small muted">{d.category}</span><div className="small muted" style={{ wordBreak: 'break-all' }}>{d.asset}</div></div>
            {canEdit && <button className="btn" disabled={!connected} title={connected ? '' : tr("UEFN n’est pas connecté")} onClick={() => act('actor_spawn', { asset: d.asset, name: d.name }, tr("Appareil posé"))}><Plus size={14} /> {tr("Poser")}</button>}
          </div>
        ))}
      </div>
      <div>
        <div className="row" style={{ justifyContent: 'space-between' }}>
          <h3 style={{ margin: 0 }}>{tr("Liens entre appareils")}</h3>
          <button className="btn" disabled={busy} onClick={lireGraphe}><RefreshCw size={14} /> {tr("Lire les liens")}</button>
        </div>
        {graphe && !graphe.verse_props_read && <p className="small muted">{tr("UEFN n’est pas connecté : les références @editable des appareils Verse ne sont pas lues.")}</p>}
        {graphe && <p className="small">{tr("{0} appareil(s) · {1} lien(s) · {2} cassé(s)", [graphe.nodes.length, graphe.links.length, graphe.broken])}</p>}
        {graphe?.links.map((l: any, i: number) => (
          <div key={i} className="row" style={{ gap: 6, padding: '4px 0', color: l.broken ? '#d92d20' : undefined }}>
            <Link2 size={14} /> <b>{nom(l.from)}</b> → <b>{l.broken ? l.to.split('.').pop() : nom(l.to)}</b>
            <span className="small muted">{l.kind} · {l.label}</span>{l.broken && <span className="small">{tr("(cible absente du niveau)")}</span>}
          </div>
        ))}
      </div>
      {err && <p className="small" style={{ color: '#d92d20' }}>{err}</p>}
    </div>
  )
}
