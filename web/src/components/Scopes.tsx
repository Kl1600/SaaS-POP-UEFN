// Portées de la dashboard : une map (/map/:mid/…) ou un studio (/studio/:sid/…).
import { useEffect } from 'react'
import { Navigate, Outlet, useLocation, useParams } from 'react-router-dom'
import { useApp } from '../state'
import { resolvePath } from '../routes'

/** /map/:mid/… : choisit la map de l'adresse, et n'affiche la page qu'une fois cette map chargée. */
export function MapScope() {
  const { mid } = useParams()
  const { me, map, setMapId, setStudioId } = useApp()
  const id = Number(mid)
  const exists = !!me?.studios.some(s => s.maps.some(m => m.id === id))
  useEffect(() => { if (exists) { setStudioId(null); if (map?.id !== id) setMapId(id) } }, [id, exists])
  if (!exists) return <Navigate to="/" replace />
  if (map?.id !== id) return null
  return <Outlet />
}

/** /studio/:sid/… : pages du studio (ou de l'espace perso), indépendantes de la map. */
export function StudioScope() {
  const { sid } = useParams()
  const { me, studio, map, setStudioId, setMapId } = useApp()
  const id = Number(sid)
  const target = me?.studios.find(s => s.id === id)
  const exists = !!target
  // La map affichée dans la barre latérale suit le studio ouvert.
  useEffect(() => {
    if (!target) return
    setStudioId(id)
    if (target.maps.length && !target.maps.some(m => m.id === map?.id)) setMapId(target.maps[0].id)
  }, [id, exists])
  useEffect(() => () => setStudioId(null), [])
  if (!exists) return <Navigate to="/" replace />
  if (studio?.id !== id) return null
  return <Outlet />
}

/** Anciennes adresses (/taches, /equipe?tab=…, /analyse/tiktok) : redirigées vers la map ou le studio en cours. */
export function Legacy() {
  const loc = useLocation()
  const { map, studio } = useApp()
  const to = resolvePath(loc.pathname + loc.search, map?.id, studio?.id)
  return <Navigate to={to === loc.pathname + loc.search ? '/' : to} replace />
}
