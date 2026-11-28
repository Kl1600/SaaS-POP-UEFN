// Paramètres > Maps et studios (#269) : tous ses studios et leurs maps, avec ouvrir, supprimer ou quitter.
import { useState } from 'react'
import { Building2, LogOut, Trash2, User } from '../icons'
import { api } from '../api'
import { useApp } from '../state'
import { CardHead, ROLE } from './ui'
import { tr } from '../i18n/t'
import { useNav } from '../routes'
import { isAdmin } from './MoveMap'
import { DeleteDialog, type DeleteTarget } from './DeleteDialog'

export function MapsStudios() {
  const app = useApp()
  const nav = useNav()
  const [del, setDel] = useState<DeleteTarget | null>(null)
  const studios = [...(app.me?.studios ?? [])].sort((a, b) => Number(!a.personal) - Number(!b.personal))
  const leave = async (id: number, name: string) => {
    if (!confirm(tr("Quitter le studio {0} ? Vous n’aurez plus accès à ses maps.", [name]))) return
    try { await api.post(`/api/studios/${id}/leave`); await app.reload(); app.toast(tr("Vous avez quitté {0}", [name])); if (app.studio?.id === id) nav('/') } catch (er: any) { app.toast(er.message, true) }
  }
  return <>
    {studios.map(s => {
      const admin = isAdmin(s)
      return <section key={s.id} className="card">
        <CardHead icon={s.personal ? <User size={18} /> : <Building2 size={18} />} title={s.personal ? tr("Mes maps (seul)") : s.name}
          sub={s.personal ? tr("Votre espace perso : il ne se supprime pas.") : tr("{0} · {1} map(s)", [ROLE[s.role] ?? s.role, s.maps.length])}>
          {!s.personal && <div className="row" style={{ gap: 6 }}>
            <button className="btn sm" onClick={() => nav(`/studio/${s.id}/maps`)}>{tr("Ouvrir")}</button>
            {s.role === 'owner'
              ? <button className="btn sm" style={{ color: 'var(--tr)' }} onClick={() => setDel({ kind: 'studio', id: s.id, name: s.name, studioId: s.id, maps: s.maps.length })}><Trash2 size={14} />{tr("Supprimer le studio")}</button>
              : <button className="btn sm" onClick={() => leave(s.id, s.name)}><LogOut size={14} />{tr("Quitter")}</button>}
          </div>}
        </CardHead>
        {s.maps.length === 0 && <p className="small muted" style={{ margin: 0 }}>{tr("Aucune map.")}</p>}
        <div className="list">{s.maps.map(m => {
          const lastTeam = !s.personal && s.maps.length <= 1
          return <div key={m.id} className="li">
            <div style={{ flex: 1, minWidth: 0 }}><div style={{ fontWeight: 500 }}>{m.name}</div>{m.id === app.map?.id && <div className="small muted">{tr("Ouverte")}</div>}</div>
            <button className="btn sm" onClick={() => nav(`/map/${m.id}`)}>{tr("Ouvrir")}</button>
            {admin && <button className="iconbtn" aria-label={tr("Supprimer {0}", [m.name])} disabled={lastTeam}
              data-tip={lastTeam ? tr("Dernière map du studio : supprimez plutôt le studio, ou créez une autre map d’abord.") : tr("Supprimer la map")}
              onClick={() => setDel({ kind: 'map', id: m.id, name: m.name, studioId: s.id })}><Trash2 size={16} /></button>}
          </div>
        })}</div>
      </section>
    })}
    {del && <DeleteDialog t={del} onClose={() => setDel(null)} />}
  </>
}
