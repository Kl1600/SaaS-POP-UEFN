// Rangement automatique du niveau (#84) : dossiers par type d'appareil et noms « type_n », avec un aperçu
// avant → après, puis une seule action annulable. Les acteurs verrouillés dans Lore par un autre sont laissés et listés.
import { useMemo, useState } from 'react'
import { Lock } from '../icons'
import { Modal } from './ui'
import { tr } from '../i18n/t'

export type RangeActor = { id: string; name: string; type: string; class: string; folder: string; kind_label?: string; locked_by?: string }
export type Patch = { id: string; changes: { name?: string; folder?: string } }

const GROUPE: Record<string, string> = { device: tr("Appareils"), verse_device: tr("Appareils Verse"), prop: tr("Accessoires"), mesh: tr("Maillages") }

/** Acteurs du moteur et ceux qu'UEFN crée lui-même (le point de départ d'un Spawn Pad) : on n'y touche pas. */
const MOTEUR = /^(WorldDataLayers|WorldSettings|LevelBounds|WorldPartitionMiniMap|WorldPartitionHLOD|FortPlayerStartCreative|PlayerStart|Brush|DefaultPhysicsVolume|AbstractNavData|RecastNavMesh|NavMeshBoundsVolume|GameplayDebuggerPlayerManager)(_C)?$/
export const estMoteur = (a: RangeActor) => MOTEUR.test(a.class || '')

/** Groupe de rangement. Un appareil connu du catalogue (libellé d'appareil) reste un appareil, même si sa classe
 * finit par « _Prop » (le Spawn Pad, BP_Creative_Player_Spawner_Prop_C). */
const groupe = (a: RangeActor) => (a.type === 'prop' && a.kind_label ? 'device' : a.type)

/** Python d'UEFN renvoie « None » pour un acteur sans dossier. */
const dossier = (a: RangeActor) => (a.folder === 'None' ? '' : a.folder || '')

/** Nom de type lisible et sans espaces, pour les dossiers et les noms (« Bouton » → « Bouton », « BP_Arbre_C » → « Arbre »). */
export const baseType = (a: RangeActor) => {
  const brut = a.kind_label || a.class || a.type || 'Acteur'
  const net = brut.replace(/^(BP|B|SM|PBWA|Device)_/i, '').replace(/_C$/i, '').replace(/["]/g, '').trim()
  return net.replace(/[\s/\\:’']+/g, '_').replace(/_+/g, '_').replace(/^_|_$/g, '') || 'Acteur'
}

/** Propose le rangement : dossier « Groupe/Type » et nom « Type_n » (n dans l'ordre des noms actuels). */
export function proposer(actors: RangeActor[], me: string, dossiers: boolean, noms: boolean): { patches: Patch[]; verrouilles: RangeActor[]; moteur: RangeActor[] } {
  const moteur = actors.filter(estMoteur)
  const verrouilles = actors.filter(a => !estMoteur(a) && a.locked_by && a.locked_by.toLowerCase() !== me.toLowerCase())
  const libres = actors.filter(a => !estMoteur(a) && !verrouilles.includes(a)).sort((a, b) => a.name.localeCompare(b.name, 'fr', { numeric: true }))
  const compte = new Map<string, number>()
  const patches: Patch[] = []
  for (const a of libres) {
    const t = baseType(a)
    const changes: Patch['changes'] = {}
    if (dossiers) {
      const f = `${GROUPE[groupe(a)] ?? tr("Autres")}/${t}`
      if (f !== dossier(a)) changes.folder = f
    }
    if (noms) {
      const n = (compte.get(t) ?? 0) + 1
      compte.set(t, n)
      const nom = `${t}_${n}`
      if (nom !== a.name) changes.name = nom
    }
    if (changes.folder !== undefined || changes.name !== undefined) patches.push({ id: a.id, changes })
  }
  return { patches, verrouilles, moteur }
}

export function RangementModal({ actors, me, onClose, onApply }: {
  actors: RangeActor[]; me: string; onClose: () => void; onApply: (patches: Patch[]) => Promise<unknown>
}) {
  const [dossiers, setDossiers] = useState(true)
  const [noms, setNoms] = useState(true)
  const [occupe, setOccupe] = useState(false)
  const { patches, verrouilles, moteur } = useMemo(() => proposer(actors, me, dossiers, noms), [actors, me, dossiers, noms])
  const parId = useMemo(() => new Map(actors.map(a => [a.id, a])), [actors])

  return <Modal onClose={onClose} label={tr("Ranger le niveau")}>
    <h2 style={{ margin: 0, fontWeight: 400 }}>{tr("Ranger le niveau")}</h2>
    <p className="muted small" style={{ margin: '6px 0 14px' }}>{tr("Une seule action, annulable depuis le journal ou par Ctrl+Z.")}</p>
    <div style={{ display: 'flex', gap: 18, marginBottom: 12 }}>
      <label className="small"><input type="checkbox" checked={dossiers} onChange={e => setDossiers(e.target.checked)} /> {tr("Dossiers par type")}</label>
      <label className="small"><input type="checkbox" checked={noms} onChange={e => setNoms(e.target.checked)} /> {tr("Renommer en type_n")}</label>
    </div>
    <div style={{ maxHeight: 360, overflow: 'auto', border: '1px solid var(--border)', borderRadius: 10 }}>
      <table className="table" style={{ width: '100%', fontSize: 13 }}>
        <thead><tr><th style={{ textAlign: 'left' }}>{tr("Avant")}</th><th style={{ textAlign: 'left' }}>{tr("Après")}</th></tr></thead>
        <tbody>
          {patches.slice(0, 500).map(p => { const a = parId.get(p.id)!; return <tr key={p.id}>
            <td>{a.name}<div className="muted">{dossier(a) || '—'}</div></td>
            <td>{p.changes.name ?? a.name}<div className="muted">{p.changes.folder ?? (dossier(a) || '—')}</div></td>
          </tr> })}
          {patches.length === 0 && <tr><td colSpan={2} className="muted">{tr("Le niveau est déjà rangé.")}</td></tr>}
        </tbody>
      </table>
    </div>
    {patches.length > 500 && <p className="small muted">{tr("… et {0} autres.", [patches.length - 500])}</p>}
    {verrouilles.length > 0 && <div className="warn" style={{ marginTop: 12 }}><Lock size={16} /><span>
      {tr("Laissés tels quels (verrouillés dans Lore par un autre) : {0}", [verrouilles.map(a => a.name).join(', ')])}</span></div>}
    {moteur.length > 0 && <p className="small muted" style={{ marginTop: 10 }}>{tr("Acteurs du moteur laissés tels quels : {0}", [moteur.map(a => a.name).join(', ')])}</p>}
    <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 16 }}>
      <button className="btn" onClick={onClose}>{tr("Annuler")}</button>
      <button className="btn pri" disabled={occupe || patches.length === 0}
        onClick={async () => { setOccupe(true); try { await onApply(patches); onClose() } finally { setOccupe(false) } }}>
        {tr("Ranger {0} acteur(s)", [patches.length])}
      </button>
    </div>
  </Modal>
}
