// Suppression d'une map ou d'un studio (#269) : une fenêtre commune au sélecteur de map, aux Paramètres et à la page
// du studio. Elle dit ce qui sera perdu, propose d'exporter le studio d'abord, et demande de taper le nom exact.
import { useState } from 'react'
import { AlertTriangle, Download, X } from 'lucide-react'
import { api } from '../api'
import { useApp } from '../state'
import { Modal } from './ui'
import { tr } from '../i18n/t'
import { useNav } from '../routes'

export type DeleteTarget = { kind: 'map' | 'studio'; id: number; name: string; studioId: number; maps?: number }

export function DeleteDialog({ t, onClose }: { t: DeleteTarget; onClose: () => void }) {
  const app = useApp()
  const nav = useNav()
  const [typed, setTyped] = useState('')
  const [busy, setBusy] = useState(false)
  const ok = typed.trim() === t.name.trim()
  const go = async () => {
    if (!ok || busy) return
    setBusy(true)
    try {
      await api.del(t.kind === 'map' ? `/api/maps/${t.id}` : `/api/studios/${t.id}`, { confirm: typed.trim() })
      const here = t.kind === 'map' ? app.map?.id === t.id : app.studio?.id === t.id
      await app.reload()
      app.toast(t.kind === 'map' ? tr("Map « {0} » supprimée", [t.name]) : tr("Studio « {0} » supprimé", [t.name]))
      onClose()
      if (here) nav('/')
    } catch (er: any) { app.toast(er.message, true); setBusy(false) }
  }
  return <Modal onClose={busy ? () => {} : onClose} label={t.kind === 'map' ? tr("Supprimer la map") : tr("Supprimer le studio")}>
    <form className="stack" onSubmit={e => { e.preventDefault(); go() }}>
      <div className="row" style={{ justifyContent: 'space-between' }}>
        <h2 style={{ margin: 0, fontWeight: 500 }}>{t.kind === 'map' ? tr("Supprimer « {0} » ?", [t.name]) : tr("Supprimer le studio « {0} » ?", [t.name])}</h2>
        <button type="button" className="iconbtn" aria-label={tr("Fermer")} onClick={onClose} disabled={busy}><X size={18} /></button>
      </div>
      <div className="warn" role="alert"><AlertTriangle size={18} /><div>
        {t.kind === 'map'
          ? tr("La map et tout son contenu seront supprimés définitivement : tâches, pages, tableaux blancs, Verse, gains. Impossible de revenir en arrière.")
          : tr("Le studio, ses {0} map(s) et tout leur contenu seront supprimés définitivement, et ses membres en seront retirés. Impossible de revenir en arrière.", [t.maps ?? 0])}
      </div></div>
      <a className="btn" href={`/api/studios/${t.studioId}/export`} download style={{ alignSelf: 'flex-start' }}><Download size={16} />{tr("Exporter le studio d’abord")}</a>
      <label className="stack" style={{ gap: 4 }}>
        <span className="small muted">{tr("Pour confirmer, tapez « {0} »", [t.name])}</span>
        <input className="field" autoFocus value={typed} onChange={e => setTyped(e.target.value)} autoComplete="off" spellCheck={false} />
      </label>
      <div className="row" style={{ justifyContent: 'flex-end', gap: 8 }}>
        <button type="button" className="btn" onClick={onClose} disabled={busy}>{tr("Annuler")}</button>
        <button className="btn danger" disabled={!ok || busy}>{busy ? tr("Suppression…") : tr("Supprimer définitivement")}</button>
      </div>
    </form>
  </Modal>
}
