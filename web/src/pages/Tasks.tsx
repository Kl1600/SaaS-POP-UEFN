import { useEffect, useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { ArrowRight, X } from 'lucide-react'
import { ago, api, frDate } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { Avatar, CardHead, Drawer, Modal, PageHead, STATUS, TAGS, Tabs } from '../components/ui'

type Task = {
  id: number; number: number; title: string; tag: string; status: string; assignee_id: number | null; assignee_name: string | null
  planned_day: string | null; deps: number[]; comments: number; subtasks: { total: number; done: number }; lot: string; ai: number
}
const COLS = ['todo', 'doing', 'blocked', 'done']

export function useMembers() {
  const { studio } = useApp()
  return useLoad<any>(studio ? `/api/studios/${studio.id}/members` : null, [studio?.id])
}

export default function Tasks() {
  const { map, canEdit, toast } = useApp()
  const [params] = useSearchParams()
  const [tab, setTab] = useState<'kanban' | 'people' | 'files'>('kanban')
  const [member, setMember] = useState<number | 'all'>('all')
  const [tag, setTag] = useState('all')
  const [q, setQ] = useState(params.get('q') ?? '')
  const [over, setOver] = useState<string | null>(null)
  const [open, setOpen] = useState<number | null>(null)
  const [msg, setMsg] = useState('')
  const { data: tasks, reload, setData } = useLoad<Task[]>(map ? `/api/maps/${map.id}/tasks` : null, [map?.id])
  const { data: mem } = useMembers()
  useLive(['tasks'], reload)
  useEffect(() => setQ(params.get('q') ?? ''), [params])
  useEffect(() => { const t = Number(params.get('t')); if (t) setOpen(t); if (params.get('vue') === 'personnes') setTab('people') }, [params])

  const vis = useMemo(() => (tasks ?? []).filter(t =>
    (member === 'all' || t.assignee_id === member) && (tag === 'all' || t.tag === tag) &&
    (!q || (t.title + ' #' + t.number).toLowerCase().includes(q.toLowerCase()))), [tasks, member, tag, q])

  const move = async (id: number, status: string) => {
    setOver(null)
    const t = tasks?.find(x => x.id === id)
    if (!t || t.status === status) return
    setData(tasks!.map(x => (x.id === id ? { ...x, status } : x)))
    try { await api.post(`/api/tasks/${id}/status`, { status }); setMsg('') } catch (e: any) { setMsg(e.message); reload() }
  }

  return (
    <main className="content">
      <PageHead title="Tâches" help="Des tâches d'une heure. Chaque changement de statut est tracé." />
      <Tabs value={tab} onChange={setTab} items={[['kanban', 'Kanban'], ['people', 'Par personne'], ['files', 'Fichiers verrouillés']]} />

      {tab === 'kanban' && <>
        <div className="row" style={{ flexWrap: 'wrap', gap: 8 }}>
          <input className="field" style={{ width: 220 }} placeholder="Filtrer…" value={q} onChange={e => setQ(e.target.value)} aria-label="Filtrer les tâches" />
          <span className="small muted" style={{ marginLeft: 8 }}>Membre</span>
          <button className={'chip' + (member === 'all' ? ' on' : '')} onClick={() => setMember('all')}>Tous</button>
          {mem?.members.map((m: any) => <button key={m.id} className={'chip' + (member === m.id ? ' on' : '')} onClick={() => setMember(m.id)}>{m.name.split(' ')[0]}</button>)}
          <span className="small muted" style={{ marginLeft: 8 }}>Étiquette</span>
          {['all', ...TAGS].map(t => <button key={t} className={'chip' + (tag === t ? ' on' : '')} onClick={() => setTag(t)}>{t === 'all' ? 'Toutes' : t}</button>)}
        </div>
        {msg && <div className="warn" role="alert"><span className="crit" style={{ marginTop: 8 }} /><span>{msg}</span><button className="iconbtn" style={{ marginLeft: 'auto', width: 28, height: 28 }} aria-label="Fermer" onClick={() => setMsg('')}><X size={16} /></button></div>}
        <div className="board">
          {COLS.map(c => {
            const cards = vis.filter(t => t.status === c)
            return (
              <section key={c} className={'col' + (over === c ? ' over' : '')} aria-label={STATUS[c]}
                onDragOver={e => { if (canEdit) { e.preventDefault(); setOver(c) } }} onDragLeave={() => setOver(null)}
                onDrop={e => { e.preventDefault(); move(Number(e.dataTransfer.getData('text/plain')), c) }}>
                <div className="row" style={{ justifyContent: 'space-between', padding: '4px 4px 6px' }}><span style={{ fontWeight: 500 }}>{STATUS[c]}</span><span className="small muted mono">{cards.length}</span></div>
                {cards.map(t => {
                  const blockedBy = t.deps.filter(n => tasks!.find(x => x.number === n)?.status !== 'done')
                  return (
                    <button key={t.id} className={'kcard' + (c === 'blocked' ? ' kblk' : c === 'done' ? ' kfin' : '')} draggable={canEdit}
                      onDragStart={e => e.dataTransfer.setData('text/plain', String(t.id))} onClick={() => setOpen(t.id)}>
                      <div className="row" style={{ justifyContent: 'space-between' }}><span className="small muted mono">#{t.number}</span><span className="small muted">1 h{t.ai ? ' · IA' : ''}</span></div>
                      <div style={{ fontWeight: 500, margin: '8px 0 10px' }}>{t.title}</div>
                      {blockedBy.length > 0 && c !== 'done' && <div className="small" style={{ marginBottom: 10 }}>{c === 'blocked' ? 'Bloquée par' : 'Dépend de'} <span className="mono">#{blockedBy.join(', #')}</span></div>}
                      <div className="row" style={{ justifyContent: 'space-between' }}>
                        <span className="row" style={{ gap: 6 }}><span className="badge">{t.tag}</span>{t.comments > 0 && <span className="small muted">{t.comments} comm.</span>}{t.subtasks.total > 0 && <span className="small muted">{t.subtasks.done}/{t.subtasks.total}</span>}</span>
                        {t.assignee_name && <Avatar name={t.assignee_name} small />}
                      </div>
                    </button>
                  )
                })}
              </section>
            )
          })}
        </div>
      </>}

      {tab === 'people' && <People />}
      {tab === 'files' && <Locks />}
      {open && <TaskDrawer id={open} onClose={() => setOpen(null)} members={mem?.members ?? []} tasks={tasks ?? []} />}
    </main>
  )
}

function People() {
  const { map } = useApp()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/people` : null, [map?.id])
  useLive(['tasks'], reload)
  if (!data) return null
  return (
    <div className="two">
      <section className="card">
        <CardHead title="Charge de la semaine" sub="Heures prévues et faites, retards" />
        <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Membre</th><th>En cours</th><th>Faites / prévues</th><th>Retards</th></tr></thead><tbody>
          {data.people.map((p: any) => (
            <tr key={p.id}>
              <td><span className="row" style={{ gap: 10 }}><Avatar name={p.name} small /><span><span style={{ display: 'block', fontWeight: 500 }}>{p.name}</span><span className="small muted">{p.specialty || p.role}</span></span></span></td>
              <td>{p.wip}</td>
              <td style={{ minWidth: 170 }}><div className="small" style={{ marginBottom: 6 }}>{p.done_h} h / {p.planned_h} h{p.overload && ' · surcharge'}</div><div className="bar"><span style={{ width: `${p.planned_h ? Math.min(100, (p.done_h / p.planned_h) * 100) : 0}%` }} /></div></td>
              <td>{p.late ? <span className="badge b-r">{p.late} en retard</span> : <span className="badge">Aucun</span>}</td>
            </tr>
          ))}
        </tbody></table></div>
      </section>
      <section className="card">
        <CardHead title="Qui bloque qui" sub="Tâches non finies dont d'autres dépendent" />
        {data.chains.length === 0 && <p className="muted">Personne n'est bloqué.</p>}
        <div className="list">
          {data.chains.map((c: any, i: number) => (
            <div key={i} className="li" style={{ flexWrap: 'wrap' }}>
              <span className="row" style={{ gap: 8 }}><Avatar name={c.from_who} small /><span><b style={{ fontWeight: 500 }}>{c.from_who ?? '—'}</b> <span className="muted mono">#{c.from}</span></span></span>
              <ArrowRight size={16} />
              <span className="row" style={{ gap: 8 }}><Avatar name={c.to_who} small /><span><b style={{ fontWeight: 500 }}>{c.to_who ?? '—'}</b> <span className="muted mono">#{c.to}</span></span></span>
              <span className="small muted" style={{ flex: '1 1 100%' }}>{c.from_title} → {c.to_title}</span>
            </div>
          ))}
        </div>
      </section>
    </div>
  )
}

function Locks() {
  const { map } = useApp()
  const { data } = useLoad<any[]>(map ? `/api/maps/${map.id}/locks` : null, [map?.id])
  return (
    <section className="card">
      <CardHead title="Qui travaille sur quel fichier" sub="Fichiers UEFN verrouillés dans Lore (alimenté par le webhook Lore, voir Intégrations)" />
      {data?.length === 0 && <p className="muted">Aucun fichier verrouillé.</p>}
      <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Fichier</th><th>Verrouillé par</th><th>Depuis</th><th>Tâche</th><th>État</th></tr></thead><tbody>
        {data?.map((l, i) => (
          <tr key={i}><td><code className="path">{l.path}</code></td><td><span className="row" style={{ gap: 8 }}><Avatar name={l.who} small />{l.who}</span></td><td className="muted">{ago(l.since)}</td><td className="mono muted">{l.task ? `#${l.task}` : '—'}</td><td>{l.stale ? <span className="badge b-r">Verrou de plus de 24 h</span> : <span className="badge">Actif</span>}</td></tr>
        ))}
      </tbody></table></div>
    </section>
  )
}

function TaskDrawer({ id, onClose, members, tasks }: { id: number; onClose: () => void; members: any[]; tasks: Task[] }) {
  const { canEdit, isAdmin, toast } = useApp()
  const { data, reload } = useLoad<any>(`/api/tasks/${id}`, [id])
  const [comment, setComment] = useState('')
  const [sub, setSub] = useState('')
  const [deps, setDeps] = useState('')
  useLive(['tasks'], reload)
  useEffect(() => { if (data) setDeps(data.deps.map((d: any) => d.number).join(', ')) }, [data?.task.id])
  if (!data) return <Drawer onClose={onClose} label="Tâche"><p className="muted">Chargement…</p></Drawer>
  const t = data.task
  const patch = async (b: any) => { try { await api.patch(`/api/tasks/${id}`, b); reload() } catch (e: any) { toast(e.message, true) } }
  const status = async (s: string) => { try { await api.post(`/api/tasks/${id}/status`, { status: s }); reload() } catch (e: any) { toast(e.message, true) } }
  const origin = t.origin?.startsWith('board:') ? `/tableau/${t.origin.split(':')[1]}` : null
  return (
    <Drawer onClose={onClose} label={`Tâche #${t.number}`}>
      <div className="row" style={{ justifyContent: 'space-between' }}><span className="small muted mono">#{t.number} · {t.lot || 'sans lot'}</span><button className="iconbtn" aria-label="Fermer" onClick={onClose}><X size={18} /></button></div>
      <input className="ptitle" style={{ fontSize: 22, margin: '8px 0 16px' }} defaultValue={t.title} disabled={!canEdit} onBlur={e => e.target.value !== t.title && patch({ title: e.target.value })} aria-label="Titre" />
      <div className="props">
        <span className="muted">Statut</span>
        <select className="field" value={t.status} disabled={!canEdit} onChange={e => status(e.target.value)}>{COLS.map(c => <option key={c} value={c}>{STATUS[c]}</option>)}</select>
        <span className="muted">Assigné</span>
        <select className="field" value={t.assignee_id ?? ''} disabled={!canEdit} onChange={e => patch({ assignee_id: e.target.value ? Number(e.target.value) : null })}>
          <option value="">Personne</option>{members.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
        </select>
        <span className="muted">Étiquette</span>
        <select className="field" value={t.tag} disabled={!canEdit} onChange={e => patch({ tag: e.target.value })}>{TAGS.map(x => <option key={x}>{x}</option>)}</select>
        <span className="muted">Jour prévu</span>
        <input className="field" type="date" value={t.planned_day ?? ''} disabled={!canEdit} onChange={e => patch({ planned_day: e.target.value || null })} />
        <span className="muted">Dépend de</span>
        <input className="field mono" value={deps} disabled={!canEdit} placeholder="ex. 12, 18" onChange={e => setDeps(e.target.value)}
          onBlur={() => patch({ deps: deps.split(/[^0-9]+/).filter(Boolean).map(Number) })} />
        <span className="muted">Origine</span>
        <span>{origin ? <a href={origin}>Post-it du tableau blanc</a> : t.origin === 'plan' ? 'Générateur de projet' : 'Créée à la main'}</span>
      </div>
      {data.deps.length > 0 && <p className="small muted">{data.deps.map((d: any) => `#${d.number} : ${STATUS[d.status]}`).join(' · ')}</p>}

      <div className="dsec">Description</div>
      <textarea className="field" style={{ width: '100%', minHeight: 90, padding: 10 }} defaultValue={t.description} disabled={!canEdit} onBlur={e => e.target.value !== t.description && patch({ description: e.target.value })} aria-label="Description" />

      <div className="dsec">Ce qui est fait / ce qui reste</div>
      <div className="stack" style={{ gap: 8 }}>
        {data.subtasks.map((s: any) => (
          <label key={s.id} className="row" style={{ gap: 8 }}><input type="checkbox" checked={s.done} disabled={!canEdit} onChange={async e => { await api.patch(`/api/subtasks/${s.id}`, { done: e.target.checked }); reload() }} /><span className={s.done ? 'done' : ''}>{s.title}</span></label>
        ))}
        {canEdit && <form className="row" onSubmit={async e => { e.preventDefault(); if (!sub.trim()) return; await api.post(`/api/tasks/${id}/subtasks`, { title: sub }); setSub(''); reload() }}>
          <input className="field" style={{ flex: 1 }} value={sub} onChange={e => setSub(e.target.value)} placeholder="Ajouter une sous-tâche" /><button className="btn">Ajouter</button></form>}
      </div>

      <div className="dsec">Commentaires</div>
      {data.comments.map((c: any) => (
        <div key={c.id} className="row" style={{ alignItems: 'flex-start', padding: '6px 0' }}><Avatar name={c.who} small /><div><b style={{ fontWeight: 500 }}>{c.who}</b> <span className="small muted">{ago(c.at)}</span><div>{c.body}</div></div></div>
      ))}
      <form className="row" onSubmit={async e => { e.preventDefault(); if (!comment.trim()) return; try { await api.post(`/api/tasks/${id}/comments`, { body: comment }); setComment(''); reload() } catch (er: any) { toast(er.message, true) } }}>
        <input className="field" style={{ flex: 1 }} value={comment} onChange={e => setComment(e.target.value)} placeholder="Commenter, @ pour mentionner" aria-label="Commenter" /><button className="btn">Envoyer</button>
      </form>

      {data.files.length > 0 && <><div className="dsec">Fichiers</div><div className="row" style={{ flexWrap: 'wrap', gap: 8 }}>{data.files.map((f: string) => <code key={f} className="path">{f}</code>)}</div></>}

      <div className="dsec">Historique</div>
      <div className="list">
        {data.events.map((e: any, i: number) => (
          <div key={i} className="li" style={{ padding: '8px 0' }}><span className="small muted mono" style={{ width: 110, flex: 'none' }}>{frDate(e.at, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' })}</span><span style={{ flex: 1 }}>{e.what}</span><span className="small muted">{e.via}{e.who ? ` · ${e.who.split(' ')[0]}` : ''}</span></div>
        ))}
      </div>
      {isAdmin && <div style={{ marginTop: 20, paddingTop: 14, borderTop: '1px solid var(--border)' }}>
        <button className="btn" style={{ color: 'var(--tr)' }} onClick={async () => { if (!confirm(`Supprimer définitivement la tâche #${t.number} « ${t.title} » ?`)) return; try { await api.del(`/api/tasks/${t.id}`); toast(`Tâche #${t.number} supprimée`); onClose() } catch (e: any) { toast(e.message, true) } }}>Supprimer la tâche</button>
      </div>}
    </Drawer>
  )
}

export function NewTaskModal({ onClose }: { onClose: () => void }) {
  const { map, toast } = useApp()
  const { data: mem } = useMembers()
  const [f, setF] = useState({ title: '', tag: 'code', assignee_id: '', planned_day: '', deps: '' })
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (busy) return
    setBusy(true)
    try {
      const r = await api.post(`/api/maps/${map!.id}/tasks`, {
        title: f.title, tag: f.tag, assignee_id: f.assignee_id ? Number(f.assignee_id) : null, planned_day: f.planned_day || null,
        deps: f.deps.split(/[^0-9]+/).filter(Boolean).map(Number),
      })
      toast(`Tâche #${r.number} créée`); onClose()
    } catch (e: any) { setErr(e.message); setBusy(false) }
  }
  return (
    <Modal onClose={onClose} label="Nouvelle tâche">
      <form className="stack" onSubmit={submit}>
        <h2 style={{ margin: 0, fontWeight: 400 }}>Nouvelle tâche</h2>
        <p className="muted" style={{ margin: 0 }}>Une tâche doit tenir en une heure.</p>
        <label className="label">Titre<input className="field" autoFocus value={f.title} onChange={e => setF({ ...f, title: e.target.value })} required maxLength={140} /></label>
        <div className="grid2" style={{ gridTemplateColumns: '1fr 1fr' }}>
          <label className="label">Étiquette<select className="field" value={f.tag} onChange={e => setF({ ...f, tag: e.target.value })}>{TAGS.map(t => <option key={t}>{t}</option>)}</select></label>
          <label className="label">Assigné<select className="field" value={f.assignee_id} onChange={e => setF({ ...f, assignee_id: e.target.value })}><option value="">Personne</option>{mem?.members.map((m: any) => <option key={m.id} value={m.id}>{m.name}</option>)}</select></label>
          <label className="label">Jour prévu<input className="field" type="date" value={f.planned_day} onChange={e => setF({ ...f, planned_day: e.target.value })} /></label>
          <label className="label">Dépend de (n°)<input className="field mono" value={f.deps} onChange={e => setF({ ...f, deps: e.target.value })} placeholder="12, 18" /></label>
        </div>
        {err && <div className="err">{err}</div>}
        <div className="row" style={{ justifyContent: 'flex-end' }}><button type="button" className="btn" onClick={onClose}>Annuler</button><button className="btn pri" disabled={busy}>{busy ? 'Création…' : 'Créer la tâche'}</button></div>
      </form>
    </Modal>
  )
}
