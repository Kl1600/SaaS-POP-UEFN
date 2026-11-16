import { useEffect, useState } from 'react'
import { api } from './api'
import { useApp } from './state'

/** État du relais UEFN Link pour la map courante, tenu à jour par le WebSocket. */
export function useUefnStatus() {
  const { map, subscribe } = useApp()
  const [s, setS] = useState<{ status: string; pending: number; project: string }>({ status: 'offline', pending: 0, project: '' })
  useEffect(() => {
    if (!map) return
    const load = () => api.get(`/api/maps/${map.id}/uefn`).then(d => setS({ status: d.status, pending: d.pending, project: d.project })).catch(() => {})
    load()
    return subscribe(m => { if (m.t === 'uefn' && (m.what === 'status' || m.what === 'journal')) load() })
  }, [map?.id])
  return s
}

export const STATUS_LABEL: Record<string, string> = { connected: 'UEFN connecté', closed: 'UEFN fermé', offline: 'Relais hors ligne' }
