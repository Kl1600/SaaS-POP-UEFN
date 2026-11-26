// Accueil : vos maps perso et vos studios, chacun avec ses maps. On choisit une map pour ouvrir sa dashboard.
// Glisser une map d'un bloc à l'autre la déplace (solo ↔ studio), toujours après confirmation.
// En dessous, « Mes IA » : les IA connectées au compte et la connexion d'une nouvelle IA (#134).
import { useState } from 'react'
import { api } from '../api'
import { useNavigate } from 'react-router-dom'
import { Building2, Plus, Settings2, User, Users, ArrowRight } from 'lucide-react'
import { euros, frDate } from '../api'
import { useApp, useLoad, studioName, type Studio } from '../state'
import { PageHead } from '../components/ui'
import { openCreate } from '../components/MapSwitcher'
import { ConfirmMove, isAdmin, previewMove, type Drag, type Pending } from '../components/MoveMap'
import { tr } from '../i18n/t'
import { AiSection } from '../components/ConnectAI'

type Groupe = { space: string; label: string; studio_id: number | null; maps: { id: number; name: string }[] }

/** Maps perso d'une autre équipe Creator, à passer dans un studio (#143) : jamais rangées en silence. */
function ARanger() {
  const { me, reload: reloadMe, toast } = useApp()
  const { data, reload } = useLoad<{ groups: Groupe[] }>('/api/me/a-ranger', [me?.studios.reduce((n, s) => n + s.maps.length, 0)])
  if (!data?.groups.length) return null
  return <div className="stack" style={{ gap: 10, marginBottom: 14 }}>{data.groups.map(g => <Groupe key={g.space} g={g} fini={async () => { await reloadMe(); reload() }} toast={toast} studios={(me?.studios ?? []).filter(s => !s.personal && (s.role === 'owner' || s.role === 'admin'))} />)}</div>
}

function Groupe({ g, fini, toast, studios }: { g: Groupe; fini: () => Promise<void>; toast: (m: string, e?: boolean) => void; studios: Studio[] }) {
  const [mode, setMode] = useState<'' | 'creer' | 'existant'>('')
  const [nom, setNom] = useState(g.label)
  const [vers, setVers] = useState(String(g.studio_id ?? studios[0]?.id ?? ''))
  const [busy, setBusy] = useState(false)
  const passer = async (cible: { new_studio?: string; studio_id?: number }) => {
    setBusy(true)
    try {
      let sid = cible.studio_id
      for (const m of g.maps) {
        const r = await api.post(`/api/maps/${m.id}/move`, sid ? { studio_id: sid } : cible)
        sid = r.studio_id
      }
      await api.put(`/api/studios/${sid}/creator-space`, { space: g.space })
      toast(tr("{0} map(s) passée(s) dans le studio", [g.maps.length]))
      await fini()
    } catch (er: any) { toast(er.message, true) } finally { setBusy(false) }
  }
  return <div className="card" style={{ padding: '14px 16px' }} role="group" aria-label={tr("À ranger · équipe {0}", [g.label])}>
    <div className="row" style={{ alignItems: 'baseline', gap: 8, flexWrap: 'wrap' }}>
      <b>{tr("À ranger · équipe {0}", [g.label])}</b>
      <span className="small muted">{tr("{0} map(s) de l'espace Creator {1} : {2}", [g.maps.length, g.space, g.maps.map(m => m.name).join(', ')])}</span>
    </div>
    {mode === '' && <div className="row" style={{ marginTop: 10, flexWrap: 'wrap', gap: 8 }}>
      <button className="btn pri" onClick={() => setMode('creer')}>{tr("Créer le studio et y passer ces maps")}</button>
      {studios.length > 0 && <button className="btn" onClick={() => setMode('existant')}>{tr("Les passer dans un studio existant")}</button>}
      <button className="btn" onClick={async () => { try { await api.post('/api/me/a-ranger/solo', { space: g.space }); await fini() } catch (er: any) { toast(er.message, true) } }}>{tr("Laisser en solo")}</button>
    </div>}
    {mode === 'creer' && <form className="row" style={{ marginTop: 10, gap: 8 }} onSubmit={e => { e.preventDefault(); passer({ new_studio: nom.trim() }) }}>
      <label className="label" style={{ flex: 1 }}>{tr("Nom du studio")}<input className="field" value={nom} onChange={e => setNom(e.target.value)} required maxLength={60} autoFocus /></label>
      <button className="btn pri" style={{ alignSelf: 'flex-end' }} disabled={busy}>{busy ? tr("Déplacement…") : tr("Créer et passer")}</button>
      <button type="button" className="btn" style={{ alignSelf: 'flex-end' }} onClick={() => setMode('')}>{tr("Annuler")}</button>
    </form>}
    {mode === 'existant' && <form className="row" style={{ marginTop: 10, gap: 8 }} onSubmit={e => { e.preventDefault(); passer({ studio_id: Number(vers) }) }}>
      <label className="label" style={{ flex: 1 }}>{tr("Studio")}<select className="field" value={vers} onChange={e => setVers(e.target.value)}>{studios.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>
      <button className="btn pri" style={{ alignSelf: 'flex-end' }} disabled={busy || !vers}>{busy ? tr("Déplacement…") : tr("Passer les maps")}</button>
      <button type="button" className="btn" style={{ alignSelf: 'flex-end' }} onClick={() => setMode('')}>{tr("Annuler")}</button>
    </form>}
  </div>
}

type DnD = { drag: Drag | null; setDrag: (d: Drag | null) => void; over: number | null; setOver: (id: number | null) => void; drop: (to: number) => void }

function StudioBlock({ s, dnd }: { s: Studio; dnd: DnD }) {
  const nav = useNavigate()
  const { data } = useLoad<any[]>(`/api/studios/${s.id}/maps`, [s.id, s.maps.length])
  const admin = isAdmin(s)
  const canDrop = admin && !!dnd.drag && dnd.drag.from !== s.id
  const total = (data ?? []).reduce((a, m) => a + (m.last_gains_cents ?? 0), 0)
  const list = data ?? s.maps.map(m => ({ ...m, progress: 0, done: 0, tasks: 0, members: 1, last_gains_cents: 0 }))
  return <section className={`home-block${canDrop ? ' drop-ok' : ''}${canDrop && dnd.over === s.id ? ' drop-over' : ''}`}
    onDragOver={e => { if (!canDrop) return; e.preventDefault(); e.dataTransfer.dropEffect = 'move'; if (dnd.over !== s.id) dnd.setOver(s.id) }}
    onDragLeave={e => { if (!e.currentTarget.contains(e.relatedTarget as Node | null) && dnd.over === s.id) dnd.setOver(null) }}
    onDrop={e => { if (!canDrop) return; e.preventDefault(); dnd.drop(s.id) }}>
    <div className="home-head">
      <span className="home-ic">{s.personal ? <User size={18} /> : <Building2 size={18} />}</span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <h2>{studioName(s)}</h2>
        <span className="small muted">{s.personal ? tr("Les maps que vous faites seul.") : tr("{0} map(s) · gains du dernier mois : {1}", [s.maps.length, euros(total, 0)])}</span>
      </div>
      <button className="btn" onClick={() => nav(`/studio/${s.id}/${s.personal ? 'maps' : 'equipe'}`)}><Settings2 size={16} />{s.personal ? tr("Mes outils") : tr("Gérer le studio")}</button>
    </div>
    {s.personal && <ARanger />}
    <div className="home-grid">
      {list.map((m: any) => <div key={m.id} role="button" tabIndex={0}
        className={`home-map${admin ? ' draggable' : ''}${dnd.drag?.id === m.id ? ' dragging' : ''}`}
        title={admin ? tr("Glissez la map vers un autre bloc pour la déplacer") : undefined}
        draggable={admin}
        onDragStart={e => { e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', String(m.id)); dnd.setDrag({ id: m.id, name: m.name, from: s.id }) }}
        onDragEnd={() => { dnd.setDrag(null); dnd.setOver(null) }}
        onClick={() => nav(`/map/${m.id}`)}
        onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); nav(`/map/${m.id}`) } }}>
        <span className="home-map-top"><b>{m.name}</b><ArrowRight size={16} /></span>
        <span className="small muted">{[m.genre, m.release_date ? tr("sortie {0}", [frDate(m.release_date)]) : ''].filter(Boolean).join(' · ') || tr("Sans genre")}</span>
        <span className="bar" style={{ margin: '12px 0 6px' }}><span style={{ width: `${m.progress ?? 0}%` }} /></span>
        <span className="small muted">{tr("{0} / {1} tâches", [m.done ?? 0, m.tasks ?? 0])}{!s.personal ? ` · ${tr("{0} membre(s)", [m.members ?? 1])}` : ''}</span>
      </div>)}
      {admin && <button className="home-map add" onClick={() => openCreate({ kind: 'map', studioId: s.id })}><Plus size={20} /><b>{s.personal ? tr("Nouvelle map perso") : tr("Nouvelle map dans ce studio")}</b></button>}
    </div>
  </section>
}

export default function Home() {
  const { me, toast } = useApp()
  const [drag, setDrag] = useState<Drag | null>(null)
  const [over, setOver] = useState<number | null>(null)
  const [pending, setPending] = useState<Pending | null>(null)
  if (!me) return null
  const drop = async (to: number) => {
    const d = drag; setDrag(null); setOver(null)
    if (!d || d.from === to) return
    try { setPending(await previewMove(d, to)) } catch (er: any) { toast(er.message, true) }
  }
  const dnd: DnD = { drag, setDrag, over, setOver, drop }
  return <main className="content">
    <PageHead title={tr("Bonjour {0}", [me.user.name.split(' ')[0]])} help={tr("Choisissez une map pour ouvrir sa dashboard. Vos maps perso sont pour vous seul ; les studios regroupent les maps d’une équipe.")}>
      <button className="btn" onClick={() => openCreate({ kind: 'studio' })}><Users size={16} />{tr("Créer un studio (équipe)")}</button>
      <button className="btn pri" onClick={() => openCreate({ kind: 'map' })}><Plus size={16} />{tr("Nouvelle map")}</button>
    </PageHead>
    {me.studios.map(s => <StudioBlock key={s.id} s={s} dnd={dnd} />)}
    <AiSection />
    {pending && <ConfirmMove p={pending} studios={me.studios} onClose={() => setPending(null)} />}
  </main>
}
