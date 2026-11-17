// Accueil : vos maps perso et vos studios, chacun avec ses maps. On choisit une map pour ouvrir sa dashboard.
// Glisser une map d'un bloc à l'autre la déplace (solo ↔ studio), toujours après confirmation.
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Building2, Plus, Settings2, User, Users, ArrowRight } from 'lucide-react'
import { api, euros, frDate } from '../api'
import { useApp, useLoad, studioName, type Studio } from '../state'
import { PageHead, Modal } from '../components/ui'
import { openCreate } from '../components/MapSwitcher'
import { tr } from '../i18n/t'

type Drag = { id: number; name: string; from: number }
type Pending = Drag & { to: number; reassigned: number }
type DnD = { drag: Drag | null; setDrag: (d: Drag | null) => void; over: number | null; setOver: (id: number | null) => void; drop: (to: number) => void }

const isAdmin = (s: Studio) => s.role === 'owner' || s.role === 'admin'

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

function ConfirmMove({ p, studios, onClose }: { p: Pending; studios: Studio[]; onClose: () => void }) {
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

export default function Home() {
  const { me, toast } = useApp()
  const [drag, setDrag] = useState<Drag | null>(null)
  const [over, setOver] = useState<number | null>(null)
  const [pending, setPending] = useState<Pending | null>(null)
  if (!me) return null
  const drop = async (to: number) => {
    const d = drag; setDrag(null); setOver(null)
    if (!d || d.from === to) return
    try {
      const r = await api.post(`/api/maps/${d.id}/move`, { studio_id: to, preview: true })
      setPending({ ...d, to, reassigned: r.reassigned ?? 0 })
    } catch (er: any) { toast(er.message, true) }
  }
  const dnd: DnD = { drag, setDrag, over, setOver, drop }
  return <main className="content">
    <PageHead title={tr("Bonjour {0}", [me.user.name.split(' ')[0]])} help={tr("Choisissez une map pour ouvrir sa dashboard. Vos maps perso sont pour vous seul ; les studios regroupent les maps d’une équipe.")}>
      <button className="btn" onClick={() => openCreate({ kind: 'studio' })}><Users size={16} />{tr("Créer un studio (équipe)")}</button>
      <button className="btn pri" onClick={() => openCreate({ kind: 'map' })}><Plus size={16} />{tr("Nouvelle map")}</button>
    </PageHead>
    {me.studios.map(s => <StudioBlock key={s.id} s={s} dnd={dnd} />)}
    {pending && <ConfirmMove p={pending} studios={me.studios} onClose={() => setPending(null)} />}
  </main>
}
