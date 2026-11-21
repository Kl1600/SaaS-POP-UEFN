// Déplacer une map d'un studio à l'autre (perso ↔ studio) : aperçu, puis confirmation.
// Utilisé par l'Accueil et par le sélecteur de map (glisser-déposer).
import { useState } from 'react'
import { api } from '../api'
import { useApp, studioName, type Studio } from '../state'
import { Modal } from './ui'
import { tr } from '../i18n/t'

export type Drag = { id: number; name: string; from: number }
export type Pending = Drag & { to: number; reassigned: number }

export const isAdmin = (s: Studio) => s.role === 'owner' || s.role === 'admin'

/** Demande l'aperçu au serveur (tâches réattribuées) avant d'ouvrir la confirmation. */
export async function previewMove(d: Drag, to: number): Promise<Pending> {
  const r = await api.post(`/api/maps/${d.id}/move`, { studio_id: to, preview: true })
  return { ...d, to, reassigned: r.reassigned ?? 0 }
}

export function ConfirmMove({ p, studios, onClose }: { p: Pending; studios: Studio[]; onClose: () => void }) {
  const { reload, toast } = useApp()
  const [busy, setBusy] = useState(false)
  const from = studios.find(x => x.id === p.from), to = studios.find(x => x.id === p.to)
  if (!from || !to) return null
  const solo = !!to.personal
  const go = async () => {
    if (busy) return; setBusy(true)
    try {
      const r = await api.post(`/api/maps/${p.id}/move`, { studio_id: p.to })
      await reload()
      toast(solo
        ? (r.reassigned > 0 ? tr("« {0} » est maintenant une map perso · {1} tâche(s) vous ont été réattribuées", [p.name, r.reassigned]) : tr("« {0} » est maintenant une map perso", [p.name]))
        : tr("« {0} » est maintenant dans {1}", [p.name, to.name]))
      onClose()
    } catch (er: any) { toast(er.message, true); setBusy(false) }
  }
  return <Modal onClose={busy ? () => {} : onClose} label={tr("Déplacer la map")}>
    <div className="stack">
      <h2 style={{ margin: 0 }}>{tr("Déplacer « {0} » ?", [p.name])}</h2>
      <p className="muted" style={{ margin: 0 }}>{tr("De {0} vers {1}. Tout le contenu suit : tâches, pages, tableaux, gains.", [studioName(from), studioName(to)])}</p>
      {solo
        ? <p style={{ margin: 0 }}>{p.reassigned > 0
            ? tr("{0} tâche(s) attribuée(s) à d’autres membres vous seront réattribuées. Les membres de {1} n’auront plus accès à cette map.", [p.reassigned, from.name])
            : tr("Les membres de {0} n’auront plus accès à cette map.", [from.name])}</p>
        : <p style={{ margin: 0 }}>{tr("Les membres de {0} auront accès à cette map.", [to.name])}</p>}
      <div className="row" style={{ justifyContent: 'flex-end' }}>
        <button type="button" className="btn" onClick={onClose} disabled={busy}>{tr("Annuler")}</button>
        <button type="button" className="btn pri" onClick={go} disabled={busy} autoFocus>{tr("Déplacer")}</button>
      </div>
    </div>
  </Modal>
}
