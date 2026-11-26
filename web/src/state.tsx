import { tr } from './i18n/t'
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { api } from './api'

export type MapInfo = { id: number; name: string; genre: string; release_date: string | null; status: string }
export type Studio = { id: number; name: string; role: string; personal?: boolean; analyse?: boolean; maps: MapInfo[] }
/** Nom affiché d’un studio : « Mes maps » pour l’espace perso. */
export const studioName = (s?: { name: string; personal?: boolean } | null) => s ? (s.personal ? tr("Mes maps (seul)") : s.name) : ""
export type Me = { user: { id: number; name: string; email: string }; credits: number; studios: Studio[]; admin?: boolean; moderator?: boolean; admin_pending?: number; username?: string | null; must_change_password?: boolean; plan?: string; trial_until?: string | null; analyse?: boolean; billing_state?: string; email_verified?: boolean; announcement?: { id: number; text: string; level: string } | null; current_map?: number | null; forge?: boolean }
export type Presence = { uid: number; name: string; place: string; page?: string }

type Ctx = {
  me: Me | null
  loading: boolean
  reload: () => Promise<void>
  studio: Studio | null
  map: MapInfo | null
  setMapId: (id: number) => void
  /** Augmente quand une autre fenêtre du compte (Forge ou le site) change la map courante (#296). */
  mapSync: number
  /** Studio affiché par les pages /studio/:sid (sinon : celui de la map en cours). */
  setStudioId: (id: number | null) => void
  role: string
  canEdit: boolean
  isAdmin: boolean
  dark: boolean
  toggleDark: () => void
  toast: (msg: string, bad?: boolean, action?: { label: string; run: () => void }) => void
  presence: Presence[]
  subscribe: (fn: (m: any) => void) => () => void
  send: (m: any) => void
}

const C = createContext<Ctx>(null as any)
export const useApp = () => useContext(C)

export function AppProvider({ children }: { children: ReactNode }) {
  const [me, setMe] = useState<Me | null>(null)
  const [loading, setLoading] = useState(true)
  const [mapId, setMapIdState] = useState<number | null>(() => Number(localStorage.getItem('pop.map')) || null)
  const [studioId, setStudioId] = useState<number | null>(null)
  const [dark, setDark] = useState(() => localStorage.getItem('pop.dark') === '1')
  const [toastMsg, setToast] = useState<{ m: string; bad: boolean; action?: { label: string; run: () => void }; k: number } | null>(null)
  const [presence, setPresence] = useState<Presence[]>([])
  const subs = useRef(new Set<(m: any) => void>())
  const first = useRef(false)
  const [mapSync, setMapSync] = useState(0)
  const sock = useRef<WebSocket | null>(null)

  const reload = useCallback(async () => {
    try {
      const m = await api.get<Me>('/api/me')
      setMe(m)
      // Map courante du compte (#296), la même dans Forge et sur le site ; localStorage ne sert qu'en secours.
      if (m.current_map && !first.current) setMapIdState(m.current_map)
      first.current = true
    } catch { setMe(null) } finally { setLoading(false) }
  }, [])
  useEffect(() => { reload() }, [reload])

  const allMaps = me?.studios.flatMap(s => s.maps.map(m => ({ s, m }))) ?? []
  const cur = allMaps.find(x => x.m.id === mapId) ?? allMaps[0] ?? null
  const studio = (studioId ? me?.studios.find(s => s.id === studioId) : null) ?? cur?.s ?? null
  const map = cur?.m ?? null
  const role = studio?.role ?? 'viewer'

  const setMapId = (id: number) => {
    setMapIdState(id); try { localStorage.setItem('pop.map', String(id)) } catch {}
    if (id !== me?.current_map) api.put('/api/me/current-map', { map_id: id }).then(() => setMe(m => m && { ...m, current_map: id })).catch(() => {})
  }
  const toggleDark = () => setDark(d => { try { localStorage.setItem('pop.dark', d ? '0' : '1') } catch {}; return !d })
  const toastTimer = useRef<any>(null)
  const toast = useCallback((m: string, bad = false, action?: { label: string; run: () => void }) => {
    clearTimeout(toastTimer.current); setToast({ m, bad, action, k: Date.now() })
    toastTimer.current = setTimeout(() => setToast(null), action ? 6000 : 3200)
  }, [])

  // Un WebSocket par map : présence, curseurs, opérations du tableau blanc, rafraîchissements.
  useEffect(() => {
    if (!map) return
    let closed = false, retry: any
    const connect = () => {
      const ws = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/api/maps/${map.id}/ws`)
      sock.current = ws
      // Présence (#176) : l'écran ouvert est renvoyé dès que la connexion s'ouvre (sinon il était perdu au chargement).
      ws.onopen = () => { if (lastPlace.current) ws.send(JSON.stringify(lastPlace.current)) }
      ws.onmessage = e => {
        try {
          const m = JSON.parse(e.data)
          if (m.t === 'presence') setPresence(m.users)
          if (m.t === 'current_map' && typeof m.map_id === 'number') {
            setMapIdState(m.map_id); setMe(x => x && { ...x, current_map: m.map_id }); setMapSync(n => n + 1)
            try { localStorage.setItem('pop.map', String(m.map_id)) } catch {}
          }
          subs.current.forEach(fn => fn(m))
        } catch {}
      }
      ws.onclose = () => { if (!closed) retry = setTimeout(connect, 2000) }
    }
    connect()
    return () => { closed = true; clearTimeout(retry); sock.current?.close(); setPresence([]) }
  }, [map?.id])

  const subscribe = useCallback((fn: (m: any) => void) => { subs.current.add(fn); return () => { subs.current.delete(fn) } }, [])
  const lastPlace = useRef<any>(null)
  const send = useCallback((m: any) => {
    if (m?.t === 'place') lastPlace.current = m
    if (sock.current?.readyState === 1) sock.current.send(JSON.stringify(m))
  }, [])

  const value = useMemo<Ctx>(() => ({
    me, loading, reload, studio, map, setMapId, mapSync, setStudioId, role,
    canEdit: role !== 'viewer', isAdmin: role === 'owner' || role === 'admin',
    dark, toggleDark, toast, presence, subscribe, send,
  }), [me, loading, studio, map, role, dark, presence, mapSync])

  return (
    <C.Provider value={value}>
      {children}
      {toastMsg && <div key={toastMsg.k} className={'toast' + (toastMsg.bad ? ' bad' : '')} role="status">{toastMsg.m}{toastMsg.action && <button className="toast-act" onClick={() => { toastMsg.action!.run(); setToast(null) }}>{toastMsg.action.label}</button>}</div>}
    </C.Provider>
  )
}

/** Recharge des données quand le serveur signale un changement (« refresh »). */
export function useLive(what: string[], fn: () => void) {
  const { subscribe } = useApp()
  const ref = useRef(fn)
  ref.current = fn
  useEffect(() => subscribe(m => {
    if ((m.t === 'refresh' && what.includes(m.what)) || what.includes(m.t)) ref.current()
  }), [subscribe, what.join(',')])
}

export function useLoad<T>(url: string | null, deps: any[] = []) {
  const [data, setData] = useState<T | null>(null)
  const [error, setError] = useState<string | null>(null)
  const load = useCallback(async () => {
    if (!url) return
    try { setData(await api.get<T>(url)); setError(null) } catch (e: any) { setError(e.message) }
  }, [url])
  useEffect(() => { load() }, [load, ...deps])
  return { data, error, reload: load, setData }
}
