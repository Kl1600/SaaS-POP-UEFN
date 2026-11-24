import { useEffect, useState } from 'react'
import { api } from './api'
import { useApp } from './state'
import { tr } from './i18n/t'

/** État du relais UEFN Link pour la map courante, tenu à jour par le WebSocket. */
export function useUefnStatus() {
  const { map, subscribe } = useApp()
  const [s, setS] = useState<{ status: string; pending: number; project: string; stale: boolean }>({ status: 'offline', pending: 0, project: '', stale: false })
  useEffect(() => {
    if (!map) return
    const load = () => api.get(`/api/maps/${map.id}/uefn`).then(d => setS({ status: d.status, pending: d.pending, project: d.project, stale: !!d.stale })).catch(() => {})
    load()
    return subscribe(m => { if (m.t === 'uefn' && (m.what === 'status' || m.what === 'journal')) load() })
  }, [map?.id])
  return s
}

export const STATUS_LABEL: Record<string, string> = { connected: tr("UEFN connecté"), closed: tr("UEFN fermé"), offline: tr("UEFN déconnecté"), unlinked: tr("Aucun projet UEFN relié") }
