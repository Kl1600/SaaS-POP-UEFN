// Adresses de l'application, sur trois niveaux :
//   compte  : /, /analyse/…, /aide, /parametres, /admin
//   studio  : /studio/:sid/equipe | integrations | maps | marche
//   map     : /map/:mid, /map/:mid/pages, /tableau, /planifier, /taches, /organiseur, /ia, /lancer, /remunerer
// Les anciennes adresses (/taches, /equipe…) restent valables : elles sont redirigées vers la map ou le studio en cours.
import { useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { useApp } from './state'

export const MAP_SECTIONS = ['pages', 'tableau', 'planifier', 'taches', 'organiseur', 'ia', 'lancer', 'remunerer']

/** Transforme une adresse « courte » (/taches?t=3, /equipe?tab=maps…) en adresse complète. */
export function resolvePath(to: string, mapId?: number | null, studioId?: number | null): string {
  if (!to.startsWith('/') || to.startsWith('/map/') || to.startsWith('/studio/')) return to
  const [pathname, search = ''] = to.split('?')
  const q = search ? '?' + search : ''
  const first = pathname.split('/')[1] ?? ''
  if (mapId && MAP_SECTIONS.includes(first)) return `/map/${mapId}${pathname}${q}`
  if (studioId && first === 'equipe') {
    const tab = new URLSearchParams(search).get('tab')
    const sub = tab === 'maps' ? 'maps' : tab === 'integ' ? 'integrations' : 'equipe'
    return `/studio/${studioId}/${sub}`
  }
  if (pathname === '/analyse/tiktok') return `/analyse/videos${q}`
  return to
}

/** Comme useNavigate, mais comprend les adresses courtes. `nav('/taches', { map: 12 })` pour une autre map. */
export function useNav() {
  const navigate = useNavigate()
  const { map, studio } = useApp()
  return useCallback((to: string | number, opts?: { map?: number; replace?: boolean }) => {
    if (typeof to === 'number') return navigate(to)
    navigate(resolvePath(to, opts?.map ?? map?.id, studio?.id), { replace: opts?.replace })
  }, [navigate, map?.id, studio?.id])
}

export const mapHome = (id: number) => `/map/${id}`
