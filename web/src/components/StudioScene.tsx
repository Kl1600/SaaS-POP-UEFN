// Scène du studio (#176) : en tête de la Vue d'ensemble, qui est connecté sur la map et ce que chacun fait, humains
// (présence du WebSocket : écran ouvert) comme IA (dernier outil MCP appelé). Chaque personnage est à son poste ;
// un clic ouvre une bulle (tâche, dernière action, « Faire signe »). Aucune donnée inventée : seulement les vraies
// présences et les vrais agents de la map.
import { useEffect, useState } from 'react'
import { ChevronDown, ChevronRight } from 'lucide-react'
import { api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr } from '../i18n/t'
import { useNav } from '../routes'

type Activite = 'code' | 'compile' | 'teste' | 'construit' | 'envoie' | 'planifie' | 'dessine' | 'pilote' | 'pause' | 'studio'

/** Écran ouvert par un humain → activité. */
export function activiteHumaine(page?: string): Activite {
  switch (page) {
    case 'verse': return 'code'
    case 'pages': return 'code'
    case 'taches': case 'planifier': return 'planifie'
    case 'tableau': return 'dessine'
    case 'uefn': case 'organiseur': return 'construit'
    case 'lancer': case 'remunerer': return 'pilote'
    default: return 'studio'
  }
}

const OUTILS: [Activite, string[]][] = [
  ['code', ['write_verse_file', 'read_verse_file', 'list_verse_files', 'create_widget']],
  ['compile', ['compile_verse']],
  ['teste', ['start_session', 'push_to_session', 'session_output', 'stop_session']],
  ['construit', ['place_device', 'place_mesh', 'update_actors', 'duplicate_actors', 'delete_actors', 'import_asset', 'check_level', 'save_level', 'list_actors', 'get_actor', 'set_island_settings', 'get_island_settings', 'uefn_status', 'undo_action']],
  ['envoie', ['lore_checkin']],
  ['planifie', ['list_tasks', 'next_task', 'get_task', 'claim_task', 'create_task', 'set_task_status', 'set_task_lot', 'comment_task', 'list_skills', 'get_skill', 'search_library', 'get_library_module', 'plan_tasks', 'assign_task', 'list_agents', 'save_progress', 'release_task', 'ask_user', 'get_answer', 'get_feasibility']],
  ['dessine', ['list_boards', 'get_board', 'create_board', 'upsert_board_objects', 'delete_board_objects', 'list_pages', 'read_page', 'write_page', 'list_milestones', 'add_milestone']],
]

/** Dernier outil MCP d'un agent et heure de son dernier appel → activité (au-delà de 10 min : en pause). */
export function activiteIA(outil: string | null | undefined, dernier: string | null | undefined, maintenant = Date.now()): Activite {
  if (dernier) {
    const t = Date.parse(dernier.replace(' ', 'T') + 'Z')
    if (!isNaN(t) && maintenant - t > 10 * 60_000) return 'pause'
  }
  for (const [a, l] of OUTILS) if (outil && l.includes(outil)) return a
  return 'planifie'
}

const LIBELLE: Record<Activite, string> = {
  code: tr("code"), compile: tr("compile"), teste: tr("teste en jeu"), construit: tr("construit le niveau"), envoie: tr("envoie dans Lore"),
  planifie: tr("planifie"), dessine: tr("au tableau"), pilote: tr("pilote le lancement"), pause: tr("en pause"), studio: tr("au studio"),
}

// Postes de la pièce (coordonnées du personnage dans la scène 960 × 300) ; plusieurs places par zone, puis on décale.
const ZONE: Record<Activite, { x: number; y: number }> = {
  code: { x: 120, y: 150 }, compile: { x: 120, y: 150 }, envoie: { x: 120, y: 150 },
  teste: { x: 340, y: 190 }, construit: { x: 520, y: 150 }, planifie: { x: 700, y: 120 }, dessine: { x: 700, y: 120 },
  pilote: { x: 330, y: 95 }, pause: { x: 860, y: 215 }, studio: { x: 860, y: 215 },
}
const POSTE: Record<Activite, string> = {
  code: 'bureaux', compile: 'bureaux', envoie: 'bureaux', teste: 'console', construit: 'maquette', planifie: 'tableau', dessine: 'tableau',
  pilote: 'pilotage', pause: 'cafe', studio: 'cafe',
}

type Perso = { key: string; nom: string; activite: Activite; ia?: { type: string; owner: string; task?: any; last_action?: string; last_used?: string }; uid?: number; place?: string; moi?: boolean }

const initiales = (n: string) => n.split(/\s+/).map(x => x[0] ?? '').join('').slice(0, 2).toUpperCase()

export default function StudioScene({ mapId }: { mapId: number }) {
  const { presence, me, toast } = useApp() as any
  const nav = useNav()
  const { data: agents, reload } = useLoad<any[]>(`/api/maps/${mapId}/agents`, [mapId])
  useLive(['agent', 'ai', 'tasks'], reload)
  useEffect(() => { const i = setInterval(reload, 10_000); return () => clearInterval(i) }, [reload])
  const cle = 'pop.scene.repliee'
  const [replie, setReplie] = useState(() => { try { return localStorage.getItem(cle) === '1' } catch { return false } })
  const [ouvert, setOuvert] = useState<string | null>(null)
  const basculer = () => { const v = !replie; setReplie(v); try { localStorage.setItem(cle, v ? '1' : '0') } catch {} }

  const humains: Perso[] = (presence ?? []).map((p: any) => ({ key: 'u' + p.uid, nom: p.name, uid: p.uid, place: p.place, activite: activiteHumaine(p.page), moi: p.uid === me?.user.id }))
  if (me && !humains.some(h => h.moi)) humains.unshift({ key: 'u' + me.user.id, nom: me.user.name, uid: me.user.id, activite: 'studio', moi: true })
  const jour = 24 * 3600_000
  const ias: Perso[] = (agents ?? [])
    .filter(a => a.last_used && Date.now() - Date.parse(a.last_used.replace(' ', 'T') + 'Z') < jour)
    .map(a => ({ key: 'a' + a.id, nom: a.name, activite: activiteIA(a.last_tool, a.last_used), ia: { type: a.kind, owner: a.owner, task: a.task, last_action: a.last_action, last_used: a.last_used } }))
  const tous = [...humains, ...ias]
  // Places : dans chaque poste, les personnages se rangent côte à côte, jamais l'un sur l'autre.
  const parPoste: Record<string, number> = {}
  const places = tous.map(p => {
    const poste = POSTE[p.activite]
    const i = parPoste[poste] = (parPoste[poste] ?? -1) + 1
    const z = ZONE[p.activite]
    return { ...p, x: z.x + (i % 4) * 46 - 46, y: z.y + Math.floor(i / 4) * 52 }
  })
  const nH = humains.length, nIA = ias.length
  const resume = tr("{0} {1} et {2} {3} connectées", [nH, nH > 1 ? tr("personnes") : tr("personne"), nIA, nIA > 1 ? tr("IA") : tr("IA")])
  const choisi = places.find(p => p.key === ouvert)

  const signe = async (uid: number) => {
    try { await api.post(`/api/maps/${mapId}/ping`, { uid }); toast(tr("Signe envoyé")) } catch (e: any) { toast(e.message, true) }
  }

  return (
    <section className="card scene">
      <button className="scene-tete" onClick={basculer} aria-expanded={!replie}>
        {replie ? <ChevronRight size={18} strokeWidth={1.5} /> : <ChevronDown size={18} strokeWidth={1.5} />}
        <span className="scene-titre">{tr("Le studio en ce moment")}</span>
        <span className="small muted">{resume}</span>
      </button>
      {!replie && <>
        <div className="scene-cadre">
          <svg viewBox="0 0 960 300" role="img" aria-label={resume}>
            {/* Sol et murs */}
            <path d="M20 270 L480 285 L940 270 L940 60 L480 45 L20 60 Z" className="scene-sol" />
            <line x1="20" y1="60" x2="940" y2="60" className="scene-mur" />
            {/* Bureaux avec ordinateurs */}
            {[0, 1, 2].map(i => <g key={'b' + i} transform={`translate(${60 + i * 46} 165)`}><rect x="0" y="0" width="40" height="8" rx="2" className="scene-meuble" /><rect x="8" y="-22" width="24" height="16" rx="2" className="scene-ecran" /></g>)}
            <text x="105" y="200" className="scene-zone">{tr("Bureaux")}</text>
            {/* Coin console */}
            <rect x="270" y="245" width="140" height="10" rx="3" className="scene-meuble" />
            <rect x="300" y="228" width="80" height="12" rx="2" className="scene-ecran" />
            <text x="340" y="272" className="scene-zone">{tr("Console")}</text>
            {/* Pilotage */}
            <rect x="285" y="108" width="110" height="8" rx="2" className="scene-meuble" />
            <text x="340" y="132" className="scene-zone">{tr("Pilotage")}</text>
            {/* Table de maquette */}
            <rect x="465" y="168" width="120" height="14" rx="3" className="scene-meuble" />
            <path d="M480 168 l10 -14 l14 8 l12 -18 l14 10 l12 -6 l10 20 Z" className="scene-maquette" />
            <text x="525" y="200" className="scene-zone">{tr("Maquette du niveau")}</text>
            {/* Tableau */}
            <rect x="640" y="66" width="150" height="40" rx="3" className="scene-tableau" />
            <text x="715" y="152" className="scene-zone">{tr("Tableau")}</text>
            {/* Café et canapé */}
            <rect x="790" y="236" width="140" height="18" rx="8" className="scene-meuble" />
            <rect x="900" y="200" width="22" height="30" rx="3" className="scene-meuble" />
            <text x="860" y="272" className="scene-zone">{tr("Café")}</text>
            {places.map(p => (
              <g key={p.key} className={`scene-perso act-${p.activite}`} transform={`translate(${p.x} ${p.y})`} tabIndex={0} role="button"
                aria-label={`${p.nom} · ${LIBELLE[p.activite]}`} onClick={() => setOuvert(ouvert === p.key ? null : p.key)}
                onKeyDown={e => { if (e.key === 'Enter') setOuvert(ouvert === p.key ? null : p.key) }}>
                <g className="scene-corps">
                  <rect x="-11" y="-6" width="22" height="22" rx="8" className="scene-buste" />
                  <circle cx="0" cy="-16" r="11" className={'scene-tete-perso' + (p.moi ? ' moi' : '')} />
                  <text x="0" y="-12" className="scene-init">{initiales(p.nom)}</text>
                  {p.activite === 'teste' && <rect x="-8" y="4" width="16" height="7" rx="3" className="scene-manette" />}
                  {p.activite === 'compile' && <rect x="-12" y="-36" width="24" height="3" rx="1.5" className="scene-progres" />}
                  {p.activite === 'pause' && <path d="M14 -24 q3 -4 0 -8 q-3 -4 0 -8" className="scene-vapeur" />}
                </g>
                <text x="0" y="30" className="scene-nom">{p.nom.length > 14 ? p.nom.slice(0, 13) + '…' : p.nom}</text>
                {p.ia && <g transform="translate(9 -30)"><rect x="0" y="0" width="16" height="10" rx="5" className="scene-pastille" /><text x="8" y="7.5" className="scene-pastille-t">IA</text></g>}
              </g>
            ))}
          </svg>
          {choisi && <div className="scene-bulle" role="dialog" aria-label={choisi.nom}>
            <b style={{ fontWeight: 500 }}>{choisi.nom}</b>{choisi.ia && <span className="small muted"> · {tr("IA")} {choisi.ia.type} · {tr("de {0}", [choisi.ia.owner])}</span>}
            <div className="small">{LIBELLE[choisi.activite]}{!choisi.ia && choisi.place ? ` · ${choisi.place}` : ''}</div>
            {choisi.ia ? <>
              {choisi.ia.task ? <button className="linkbtn small" onClick={() => nav(`/taches?q=%23${choisi.ia!.task.number}`)}>#{choisi.ia.task.number} {choisi.ia.task.title}</button> : <div className="small muted">{tr("Aucune tâche réservée")}</div>}
              {choisi.ia.last_action && <div className="small muted">{tr("Dernière action UEFN : {0}", [choisi.ia.last_action])}</div>}
              {choisi.ia.last_used && <div className="small muted">{tr("Dernier appel : {0}", [new Date(choisi.ia.last_used.replace(' ', 'T') + 'Z').toLocaleTimeString()])}</div>}
            </> : !choisi.moi && choisi.uid && <button className="btn" style={{ marginTop: 6 }} onClick={() => signe(choisi.uid!)}>{tr("Faire signe")}</button>}
          </div>}
        </div>
        {tous.length === 1 && <p className="small muted" style={{ margin: '8px 0 0' }}>{tr("Vous êtes seul sur la map pour l'instant. Connectez une IA pour la voir travailler ici.")} <button className="linkbtn small" onClick={() => nav('/parametres?onglet=ia')}>{tr("Connecter une IA")}</button></p>}
      </>}
    </section>
  )
}
