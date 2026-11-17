// Accueil : vos maps perso et vos studios, chacun avec ses maps. On choisit une map pour ouvrir sa dashboard.
import { useNavigate } from 'react-router-dom'
import { Building2, Plus, Settings2, User, Users, ArrowRight } from 'lucide-react'
import { euros, frDate } from '../api'
import { useApp, useLoad, studioName, type Studio } from '../state'
import { PageHead } from '../components/ui'
import { openCreate } from '../components/MapSwitcher'
import { tr } from '../i18n/t'

function StudioBlock({ s }: { s: Studio }) {
  const nav = useNavigate()
  const { data } = useLoad<any[]>(`/api/studios/${s.id}/maps`, [s.id, s.maps.length])
  const admin = s.role === 'owner' || s.role === 'admin'
  const total = (data ?? []).reduce((a, m) => a + (m.last_gains_cents ?? 0), 0)
  const list = data ?? s.maps.map(m => ({ ...m, progress: 0, done: 0, tasks: 0, members: 1, last_gains_cents: 0 }))
  return <section className="home-block">
    <div className="home-head">
      <span className="home-ic">{s.personal ? <User size={18} /> : <Building2 size={18} />}</span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <h2>{studioName(s)}</h2>
        <span className="small muted">{s.personal ? tr("Les maps que vous faites seul.") : tr("{0} map(s) · gains du dernier mois : {1}", [s.maps.length, euros(total, 0)])}</span>
      </div>
      <button className="btn" onClick={() => nav(`/studio/${s.id}/${s.personal ? 'maps' : 'equipe'}`)}><Settings2 size={16} />{s.personal ? tr("Mes outils") : tr("Gérer le studio")}</button>
    </div>
    <div className="home-grid">
      {list.map((m: any) => <button key={m.id} className="home-map" onClick={() => nav(`/map/${m.id}`)}>
        <span className="home-map-top"><b>{m.name}</b><ArrowRight size={16} /></span>
        <span className="small muted">{[m.genre, m.release_date ? tr("sortie {0}", [frDate(m.release_date)]) : ''].filter(Boolean).join(' · ') || tr("Sans genre")}</span>
        <span className="bar" style={{ margin: '12px 0 6px' }}><span style={{ width: `${m.progress ?? 0}%` }} /></span>
        <span className="small muted">{tr("{0} / {1} tâches", [m.done ?? 0, m.tasks ?? 0])}{!s.personal ? ` · ${tr("{0} membre(s)", [m.members ?? 1])}` : ''}</span>
      </button>)}
      {admin && <button className="home-map add" onClick={() => openCreate({ kind: 'map', studioId: s.id })}><Plus size={20} /><b>{s.personal ? tr("Nouvelle map perso") : tr("Nouvelle map dans ce studio")}</b></button>}
    </div>
  </section>
}

export default function Home() {
  const { me } = useApp()
  if (!me) return null
  return <main className="content">
    <PageHead title={tr("Bonjour {0}", [me.user.name.split(' ')[0]])} help={tr("Choisissez une map pour ouvrir sa dashboard. Vos maps perso sont pour vous seul ; les studios regroupent les maps d’une équipe.")}>
      <button className="btn" onClick={() => openCreate({ kind: 'studio' })}><Users size={16} />{tr("Créer un studio (équipe)")}</button>
      <button className="btn pri" onClick={() => openCreate({ kind: 'map' })}><Plus size={16} />{tr("Nouvelle map")}</button>
    </PageHead>
    {me.studios.map(s => <StudioBlock key={s.id} s={s} />)}
  </main>
}
