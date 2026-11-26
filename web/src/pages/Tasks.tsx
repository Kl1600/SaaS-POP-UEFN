import { OuvrirForge } from '../components/OuvrirForge'
import { useEffect, useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { ArrowRight, X , AlertTriangle } from 'lucide-react'
import { ago, api, frDate } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { Avatar, CardHead, Drawer, Modal, PageHead, STATUS, TAGS, Tabs, tagLabel } from '../components/ui'
import { tr } from '../i18n/t'
import DepGraph from '../components/DepGraph'

/** Ce qu'est une dépendance (#160) : même définition que pour les IA (outil MCP create_task, plan IA). */
export const DEP_HELP = "Une dépendance = une tâche sans laquelle celle-ci ne peut être ni faite ni testée. Pas l’ordre dans lequel le joueur découvre le jeu : si seule une partie en a besoin, découpez la tâche."

type Task = {
  id: number; number: number; title: string; tag: string; status: string; assignee_id: number | null; assignee_name: string | null
  planned_day: string | null; deps: number[]; comments: number; subtasks: { total: number; done: number }; lot: string; ai: number; uefn: number
}
const COLS = ['todo', 'doing', 'blocked', 'done']

export function useMembers() {
  const { studio } = useApp()
  return useLoad<any>(studio ? `/api/studios/${studio.id}/members` : null, [studio?.id])
}

export default function Tasks() {
  const { map, canEdit, toast } = useApp()
  const [params] = useSearchParams()
  const [tab, setTab] = useState<'kanban' | 'graph' | 'people' | 'files'>('kanban')
  const [member, setMember] = useState<number | 'all'>('all')
  const [tag, setTag] = useState('all')
  const [q, setQ] = useState(params.get('q') ?? '')
  const [over, setOver] = useState<string | null>(null)
  const [open, setOpen] = useState<number | null>(null)
  const [msg, setMsg] = useState('')
  const [checkin, setCheckin] = useState<Task | null>(null)
  const { data: tasks, reload, setData } = useLoad<Task[]>(map ? `/api/maps/${map.id}/tasks` : null, [map?.id])
  const { data: mem } = useMembers()
  useLive(['tasks'], reload)
  useEffect(() => setQ(params.get('q') ?? ''), [params])
  useEffect(() => { const t = Number(params.get('t')); if (t) setOpen(t); if (params.get('vue') === 'dependances') setTab('graph') }, [params])

  const vis = useMemo(() => (tasks ?? []).filter(t =>
    (member === 'all' || t.assignee_id === member) && (tag === 'all' || t.tag === tag) &&
    (!q || (t.title + ' #' + t.number).toLowerCase().includes(q.toLowerCase()))), [tasks, member, tag, q])

  // Vue Dépendances (#182) : ajoute ou retire une dépendance ; le serveur refuse les boucles en français.
  const setDeps = async (number: number, f: (d: number[]) => number[]) => {
    const t = tasks?.find(x => x.number === number); if (!t) return
    const deps = [...new Set(f(t.deps))]
    try { await api.patch(`/api/tasks/${t.id}`, { deps }); toast(tr("#{0} : dépendances mises à jour", [number])); reload() } catch (e: any) { toast(e.message, true) }
  }
  const move = async (id: number, status: string) => {
    setOver(null)
    const t = tasks?.find(x => x.id === id)
    if (!t || t.status === status) return
    // Une tâche UEFN ne passe à « Faite » qu'avec le message de son check-in Lore (#127).
    if (status === 'done' && t.uefn) { setCheckin(t); return }
    setData(tasks!.map(x => (x.id === id ? { ...x, status } : x)))
    try { await api.post(`/api/tasks/${id}/status`, { status }); setMsg('') } catch (e: any) { setMsg(e.message); reload() }
  }

  return (
    <main className="content">
      <PageHead title={tr("Tâches")} help={tr("Des tâches d'une heure. Chaque changement de statut est tracé.")} />
      <Tabs value={tab} onChange={setTab} items={[['kanban', tr("Kanban")], ['graph', tr("Dépendances")]]} />

      {(tab === 'kanban' || tab === 'graph') && <div className="row" style={{ flexWrap: 'wrap', gap: 8 }}>
          <input className="field" style={{ width: 220 }} placeholder={tr("Filtrer…")} value={q} onChange={e => setQ(e.target.value)} aria-label={tr("Filtrer les tâches")} />
          <span className="small muted" style={{ marginLeft: 8 }}>{tr("Membre")}</span>
          <button className={'chip' + (member === 'all' ? ' on' : '')} onClick={() => setMember('all')}>{tr("Tous")}</button>
          {mem?.members.map((m: any) => <button key={m.id} className={'chip' + (member === m.id ? ' on' : '')} onClick={() => setMember(m.id)}>{m.name.split(' ')[0]}</button>)}
          <span className="small muted" style={{ marginLeft: 8 }}>{tr("Étiquette")}</span>
          {['all', ...TAGS].map(t => <button key={t} className={'chip' + (tag === t ? ' on' : '')} onClick={() => setTag(t)}>{t === 'all' ? tr("Toutes") : tagLabel(t)}</button>)}
        </div>}
      {tab === 'graph' && map && <DepGraph mapId={map.id} tasks={tasks ?? []} match={t => vis.some(v => v.id === t.id)} onOpen={setOpen}
        onLink={canEdit ? (from, to) => setDeps(to, d => [...d, from]) : undefined} onUnlink={canEdit ? (from, to) => setDeps(to, d => d.filter(x => x !== from)) : undefined} />}
      {tab === 'kanban' && <>
        {msg && <div className="warn" role="alert"><span className="crit" style={{ marginTop: 8 }} /><span>{msg}</span><button className="iconbtn" style={{ marginLeft: 'auto', width: 28, height: 28 }} aria-label={tr("Fermer")} onClick={() => setMsg('')}><X size={16} /></button></div>}
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
                    <button key={t.id} className={'kcard' + (c === 'blocked' ? ' kblk' : c === 'done' ? ' kfin' : '') + (blockedBy.length > 0 && c !== 'done' ? ' kwaits' : '')} draggable={canEdit}
                      onDragStart={e => e.dataTransfer.setData('text/plain', String(t.id))} onClick={() => setOpen(t.id)}>
                      <div className="row" style={{ justifyContent: 'space-between' }}><span className="small muted mono">#{t.number}</span><span className="row small muted" style={{ gap: 6 }}>1 h{t.uefn ? ' · UEFN' : ''}{t.ai ? ' · IA' : ''}{blockedBy.length > 0 && c !== 'done' && <span className="kdanger" data-tip={tr("Bloquée par #{0}", [blockedBy.join(', #')])} aria-label={tr("Bloquée par #{0}", [blockedBy.join(', #')])}><AlertTriangle size={16} strokeWidth={2} /></span>}</span></div>
                      <div style={{ fontWeight: 500, margin: '8px 0 10px' }}>{t.title}</div>
                      {blockedBy.length > 0 && c !== 'done' && <div className="small kwait" style={{ marginBottom: 10 }}>{tr("Attend")} <span className="mono">#{blockedBy.join(', #')}</span> · {tr("ne peut pas être prise")}</div>}
                      <div className="row" style={{ justifyContent: 'space-between' }}>
                        <span className="row" style={{ gap: 6 }}><span className="badge">{tagLabel(t.tag)}</span>{t.comments > 0 && <span className="small muted">{t.comments} comm.</span>}{t.subtasks.total > 0 && <span className="small muted">{t.subtasks.done}/{t.subtasks.total}</span>}</span>
                        {t.assignee_name && <span className="row small" style={{ gap: 6, minWidth: 0 }}><Avatar name={t.assignee_name} small /><span className="kwho">{t.assignee_name}</span></span>}
                      </div>
                    </button>
                  )
                })}
              </section>
            )
          })}
        </div>
      </>}

      {checkin && <CheckinModal task={checkin} onClose={() => setCheckin(null)} onDone={() => { setCheckin(null); setMsg(''); reload() }} />}
      {open && <TaskDrawer key={open} id={open} onClose={() => setOpen(null)} members={mem?.members ?? []} tasks={tasks ?? []} onOpen={setOpen} />}
    </main>
  )
}

/** Panneau de détail d'une tâche ; aussi ouvert depuis Planifier (#396). */
export function TaskDrawer({ id, onClose, members, tasks, onOpen }: { id: number; onClose: () => void; members: any[]; tasks: Task[]; onOpen?: (id: number) => void }) {
  const { canEdit, isAdmin, me, toast, map } = useApp()
  const { data, reload } = useLoad<any>(`/api/tasks/${id}`, [id])
  const [comment, setComment] = useState('')
  const [sub, setSub] = useState('')
  const [deps, setDeps] = useState('')
  const [checkin, setCheckin] = useState(false)
  useLive(['tasks'], reload)
  useEffect(() => { if (data) setDeps(data.deps.map((d: any) => d.number).join(', ')) }, [data?.task.id])
  if (!data) return <Drawer onClose={onClose} label={tr("Tâche")}><p className="muted">{tr("Chargement…")}</p></Drawer>
  const t = data.task
  const patch = async (b: any) => { try { await api.patch(`/api/tasks/${id}`, b); reload() } catch (e: any) { toast(e.message, true) } }
  const status = async (s: string) => { if (s === 'done' && t.uefn) { setCheckin(true); return } try { await api.post(`/api/tasks/${id}/status`, { status: s }); reload() } catch (e: any) { toast(e.message, true) } }
  const origin = t.origin?.startsWith('board:') ? `/tableau/${t.origin.split(':')[1]}` : null
  return (<>
    <Drawer wide onClose={onClose} label={tr("Tâche #{0}", [t.number])}>
      <div className="row" style={{ justifyContent: 'space-between' }}><span className="small muted mono">#{t.number} · {t.lot || tr("sans lot")}</span><span className="row" style={{ gap: 6 }}>{map && <OuvrirForge mapId={map.id} tache={t.number} />}<button className="iconbtn" aria-label={tr("Fermer")} onClick={onClose}><X size={18} /></button></span></div>
      <input className="ptitle" style={{ fontSize: 22, margin: '8px 0 16px' }} defaultValue={t.title} disabled={!canEdit} onBlur={e => e.target.value !== t.title && patch({ title: e.target.value })} aria-label={tr("Titre")} />
      <div className="tgrid">
      <div className="tmain">
      <div className="dsec">{tr("Description")}</div>
      <textarea className="field" style={{ width: '100%', minHeight: 90, padding: 10 }} defaultValue={t.description} disabled={!canEdit} onBlur={e => e.target.value !== t.description && patch({ description: e.target.value })} aria-label={tr("Description")} />

      <div className="dsec">{tr("Ce qui est fait / ce qui reste")}</div>
      <div className="stack" style={{ gap: 8 }}>
        {data.subtasks.map((s: any) => (
          <label key={s.id} className="row" style={{ gap: 8 }}><input type="checkbox" checked={s.done} disabled={!canEdit} onChange={async e => { await api.patch(`/api/subtasks/${s.id}`, { done: e.target.checked }); reload() }} /><span className={s.done ? 'done' : ''}>{s.title}</span></label>
        ))}
        {canEdit && <form className="row" onSubmit={async e => { e.preventDefault(); if (!sub.trim()) return; await api.post(`/api/tasks/${id}/subtasks`, { title: sub }); setSub(''); reload() }}>
          <input className="field" style={{ flex: 1 }} value={sub} onChange={e => setSub(e.target.value)} placeholder={tr("Ajouter une sous-tâche")} /><button className="btn">{tr("Ajouter")}</button></form>}
      </div>

      <div className="dsec">{tr("Commentaires")}</div>
      {data.comments.map((c: any) => (
        <div key={c.id} className="row" style={{ alignItems: 'flex-start', padding: '6px 0' }}><Avatar name={c.who} small /><div><b style={{ fontWeight: 500 }}>{c.who}</b> <span className="small muted">{ago(c.at)}</span><div>{c.body}</div></div></div>
      ))}
      <form className="row" onSubmit={async e => { e.preventDefault(); if (!comment.trim()) return; try { await api.post(`/api/tasks/${id}/comments`, { body: comment }); setComment(''); reload() } catch (er: any) { toast(er.message, true) } }}>
        <input className="field" style={{ flex: 1 }} value={comment} onChange={e => setComment(e.target.value)} placeholder={tr("Commenter, @ pour mentionner")} aria-label={tr("Commenter")} /><button className="btn">{tr("Envoyer")}</button>
      </form>

      {data.files.length > 0 && <><div className="dsec">{tr("Fichiers")}</div><div className="row" style={{ flexWrap: 'wrap', gap: 8 }}>{data.files.map((f: string) => <code key={f} className="path">{f}</code>)}</div></>}

      <div className="dsec">{tr("Historique")}</div>
      <div className="list">
        {data.events.map((e: any, i: number) => (
          <div key={i} className="li" style={{ padding: '8px 0' }}><span className="small muted mono" style={{ width: 110, flex: 'none' }}>{frDate(e.at, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' })}</span><span style={{ flex: 1 }}>{e.what}</span><span className="small muted">{e.via}{e.who ? ` · ${e.who.split(' ')[0]}` : ''}</span></div>
        ))}
      </div>
      </div>
      <aside className="tside">
      <div className="props">
        <span className="muted">{tr("Statut")}</span>
        <select className="field" value={t.status} disabled={!canEdit} onChange={e => status(e.target.value)}>{COLS.map(c => <option key={c} value={c}>{STATUS[c]}</option>)}</select>
        <span className="muted">{tr("Assigné")}</span>
        {/* Chaque membre prend ses tâches lui-même ; seuls le propriétaire et les admins assignent les autres (#157). */}
        <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
          {isAdmin
            ? <select className="field" value={t.assignee_id ?? ''} onChange={e => patch({ assignee_id: e.target.value ? Number(e.target.value) : null })}>
                <option value="">{tr("Personne")}</option>{members.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}
              </select>
            : <span>{t.assignee_name ?? tr("Personne")}</span>}
          {canEdit && t.status !== 'done' && t.assignee_id == null && <button className="btn" onClick={() => patch({ assignee_id: me!.user.id })}>{tr("Je la prends")}</button>}
          {canEdit && t.assignee_id === me?.user.id && <button className="btn" onClick={() => patch({ assignee_id: null })}>{tr("Je la rends")}</button>}
        </div>
        <span className="muted">{tr("Étiquette")}</span>
        <select className="field" value={t.tag} disabled={!canEdit} onChange={e => patch({ tag: e.target.value })}>{TAGS.map(x => <option key={x} value={x}>{tagLabel(x)}</option>)}</select>
        <span className="muted">{tr("Jour prévu")}</span>
        <input className="field" type="date" value={t.planned_day ?? ''} disabled={!canEdit} onChange={e => patch({ planned_day: e.target.value || null })} />
        <span className="muted">{tr("Dépend de")}</span>
        <div>
          <input className="field mono" style={{ width: '100%' }} value={deps} disabled={!canEdit} placeholder="ex. 12, 18" onChange={e => setDeps(e.target.value)}
            onBlur={() => patch({ deps: deps.split(/[^0-9]+/).filter(Boolean).map(Number) })} />
          <p className="small muted" style={{ margin: '4px 0 0' }}>{tr(DEP_HELP)}</p>
        </div>
        <span className="muted">{tr("Fonctionnalité")}</span>
        <div>
          <input className="field" style={{ width: '100%' }} list="lots-de-la-map" key={t.lot} defaultValue={t.lot} disabled={!canEdit} maxLength={60} placeholder={tr("Sans fonctionnalité")}
            onBlur={e => e.target.value.trim() !== t.lot && patch({ lot: e.target.value })} onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur() }} aria-label={tr("Fonctionnalité")} />
          <datalist id="lots-de-la-map">{[...new Set(tasks.map(x => x.lot).filter(Boolean))].sort().map(l => <option key={l} value={l} />)}</datalist>
        </div>
        <span className="muted">{tr("UEFN")}</span>
        <label className="row" style={{ gap: 8 }}><input type="checkbox" checked={!!t.uefn} disabled={!canEdit} onChange={e => patch({ uefn: e.target.checked })} /><span className="small">{tr("Check-in Lore requis pour la fermer")}</span></label>
        <span className="muted">{tr("Origine")}</span>
        <span>{origin ? <a href={origin}>{tr("Post-it du tableau blanc")}</a> : t.origin === 'plan' ? tr("Générateur de projet") : tr("Créée à la main")}</span>
      </div>
      {data.deps.some((d: any) => d.status !== 'done') && <div className="warn" role="status"><AlertTriangle size={18} /><span>{tr("Bloquée par")} {data.deps.filter((d: any) => d.status !== 'done').map((d: any, i: number) => { const x = tasks.find(t => t.number === d.number); return <span key={d.number}>{i > 0 && ', '}<button className="linkbtn" onClick={() => x && onOpen?.(x.id)}>#{d.number}{x ? ` ${x.title}` : ''}</button> ({STATUS[d.status]})</span> })}</span></div>}
      {data.deps.length > 0 && data.deps.every((d: any) => d.status === 'done') && <p className="small muted">{data.deps.map((d: any) => `#${d.number} : ${STATUS[d.status]}`).join(' · ')}</p>}

      <div className="props tdates">
        <span className="muted">{tr("Prise le")}</span><span>{data.started_at ? frDate(data.started_at, { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : '—'}</span>
        <span className="muted">{tr("Finie le")}</span><span>{t.status === 'done' && t.done_at ? frDate(t.done_at, { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : '—'}</span>
      </div>
      </aside>
      </div>
      {isAdmin && <div style={{ marginTop: 20, paddingTop: 14, borderTop: tr("1px solid var(--border)") }}>
        <button className="btn" style={{ color: 'var(--tr)' }} onClick={async () => { if (!confirm(tr("Supprimer définitivement la tâche #{0} « {1} » ?", [t.number, t.title]))) return; try { await api.del(`/api/tasks/${t.id}`); toast(tr("Tâche #{0} supprimée", [t.number])); onClose() } catch (e: any) { toast(e.message, true) } }}>{tr("Supprimer la tâche")}</button>
      </div>}
    </Drawer>
    {/* Hors du panneau : sinon la fenêtre se place dans le panneau et reste cachée. */}
    {checkin && <CheckinModal task={t} onClose={() => setCheckin(false)} onDone={() => { setCheckin(false); reload() }} />}
  </>)
}

/** Fermer une tâche UEFN : demande le message du check-in Lore, qui commence par #N (#127). */
function CheckinModal({ task, onClose, onDone }: { task: { id: number; number: number; title: string }; onClose: () => void; onDone: () => void }) {
  const [m, setM] = useState(`#${task.number} `)
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (busy) return
    setBusy(true)
    try { await api.post(`/api/tasks/${task.id}/status`, { status: 'done', checkin: m }); onDone() } catch (er: any) { setErr(er.message); setBusy(false) }
  }
  return (
    <Modal onClose={onClose} label={tr("Check-in Lore")}>
      <form className="stack" onSubmit={submit}>
        <h2 style={{ margin: 0, fontWeight: 400 }}>{tr("Fermer #{0}", [task.number])}</h2>
        <p className="muted" style={{ margin: 0 }}>{tr("C’est une tâche UEFN : elle ne passe à « Faite » qu’avec un check-in Lore archivé sur cette map. Collez ici son message exact, qui commence par #{0}. Si son travail est parti dans le check-in d’une autre tâche, collez le message de ce check-in : l’historique le notera.", [task.number])}</p>
        <label className="label">{tr("Message du check-in Lore")}<input className="field mono" autoFocus value={m} onChange={e => setM(e.target.value)} required maxLength={300} /></label>
        {err && <div className="err" role="alert">{err}</div>}
        <div className="row" style={{ justifyContent: 'flex-end' }}><button type="button" className="btn" onClick={onClose}>{tr("Annuler")}</button><button className="btn pri" disabled={busy}>{busy ? tr("Envoi…") : tr("Marquer comme faite")}</button></div>
      </form>
    </Modal>
  )
}

export function NewTaskModal({ onClose }: { onClose: () => void }) {
  const { map, isAdmin, me, toast } = useApp()
  const { data: mem } = useMembers()
  const [f, setF] = useState({ title: '', tag: 'code', assignee_id: '', planned_day: '', deps: '', uefn: false })
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (busy) return
    setBusy(true)
    try {
      const r = await api.post(`/api/maps/${map!.id}/tasks`, {
        title: f.title, tag: f.tag, assignee_id: f.assignee_id ? Number(f.assignee_id) : null, planned_day: f.planned_day || null,
        deps: f.deps.split(/[^0-9]+/).filter(Boolean).map(Number), uefn: f.uefn,
      })
      toast(tr("Tâche #{0} créée", [r.number])); onClose()
    } catch (e: any) { setErr(e.message); setBusy(false) }
  }
  return (
    <Modal onClose={onClose} label={tr("Nouvelle tâche")}>
      <form className="stack" onSubmit={submit}>
        <h2 style={{ margin: 0, fontWeight: 400 }}>{tr("Nouvelle tâche")}</h2>
        <p className="muted" style={{ margin: 0 }}>{tr("Une tâche doit tenir en une heure.")}</p>
        <label className="label">{tr("Titre")}<input className="field" autoFocus value={f.title} onChange={e => setF({ ...f, title: e.target.value })} required maxLength={140} /></label>
        <div className="grid2" style={{ gridTemplateColumns: '1fr 1fr' }}>
          <label className="label">{tr("Étiquette")}<select className="field" value={f.tag} onChange={e => setF({ ...f, tag: e.target.value })}>{TAGS.map(t => <option key={t} value={t}>{tagLabel(t)}</option>)}</select></label>
          <label className="label">{tr("Assigné")}<select className="field" value={f.assignee_id} onChange={e => setF({ ...f, assignee_id: e.target.value })}><option value="">{tr("Personne")}</option>{mem?.members.filter((m: any) => isAdmin || m.id === me?.user.id).map((m: any) => <option key={m.id} value={m.id}>{m.name}</option>)}</select></label>
          <label className="label">{tr("Jour prévu")}<input className="field" type="date" value={f.planned_day} onChange={e => setF({ ...f, planned_day: e.target.value })} /></label>
          <label className="label">{tr("Dépend de (n°)")}<input className="field mono" value={f.deps} onChange={e => setF({ ...f, deps: e.target.value })} placeholder="12, 18" /><span className="small muted" style={{ fontWeight: 400 }}>{tr(DEP_HELP)}</span></label>
        </div>
        <label className="row" style={{ gap: 8 }}><input type="checkbox" checked={f.uefn} onChange={e => setF({ ...f, uefn: e.target.checked })} />{tr("Tâche UEFN : check-in Lore requis pour la fermer")}</label>
        {err && <div className="err">{err}</div>}
        <div className="row" style={{ justifyContent: 'flex-end' }}><button type="button" className="btn" onClick={onClose}>{tr("Annuler")}</button><button className="btn pri" disabled={busy}>{busy ? tr("Création…") : tr("Créer la tâche")}</button></div>
      </form>
    </Modal>
  )
}
