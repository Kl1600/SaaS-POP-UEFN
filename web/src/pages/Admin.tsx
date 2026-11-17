// Panel administrateur de la plateforme POP : liste d'attente, création des comptes bêta, utilisateurs, vue générale.
import { useEffect, useState } from 'react'
import { Check, X, RotateCcw, Trash2, Copy, ShieldCheck, Ban, UserPlus, Lock, KeyRound } from 'lucide-react'
import { actionLabel } from '../components/Security'
import { ago, api, frDate } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead, Empty, PageHead } from '../components/ui'

const n = (v?: number) => (v ?? 0).toLocaleString('fr-FR')
const STATUS: Record<string, [string, string]> = { pending: ['En attente', 'b-o'], approved: ['Accepté, sans compte', 'b-b'], refused: ['Refusée', 'b-r'], registered: ['Compte créé', 'b-g'] }

/** Identifiant proposé à partir du nom (ou de l'e-mail) : minuscules, sans accent, points à la place des espaces. */
export const suggestUsername = (name: string, email = '') => {
  const base = (name.trim() || email.split('@')[0] || '').normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
  return base.replace(/\s+/g, '.').replace(/[^a-z0-9._-]/g, '').replace(/\.{2,}/g, '.').replace(/^[._-]+|[._-]+$/g, '').slice(0, 32)
}

type Cred = { username: string; password: string; url: string; name?: string }
/** Identifiants à transmettre : affichés une seule fois. */
function CredBox({ c, onClose }: { c: Cred; onClose?: () => void }) {
  const { toast } = useApp()
  const msg = `Bonjour${c.name ? ' ' + c.name.split(' ')[0] : ''},\n\nTon accès à la bêta de POP est prêt.\n\nAdresse : ${c.url}\nIdentifiant : ${c.username}\nMot de passe provisoire : ${c.password}\n\nTu choisiras ton propre mot de passe à la première connexion.\n\nÀ très vite,\nL’équipe POP`
  return <div className="b-callout" style={{ display: 'grid', gap: 10, marginTop: 10 }}>
    <b style={{ fontWeight: 500 }}>Identifiants à transmettre (affichés une seule fois)</b>
    <div className="mono" style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 14px', fontSize: 14 }}>
      <span className="muted">Identifiant</span><span>{c.username}</span>
      <span className="muted">Mot de passe</span><span>{c.password}</span>
      <span className="muted">Adresse</span><span style={{ wordBreak: 'break-all' }}>{c.url}</span>
    </div>
    <span className="small muted">La personne devra choisir son propre mot de passe à sa première connexion. Envoyez-lui ces informations en message privé (Discord, e-mail…).</span>
    <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
      <button className="btn pri" onClick={() => { navigator.clipboard?.writeText(msg); toast('Message copié') }}><Copy size={15} />Copier le message</button>
      <button className="btn" onClick={() => { navigator.clipboard?.writeText(c.password); toast('Mot de passe copié') }}><Copy size={15} />Mot de passe seul</button>
      {onClose && <button className="btn" onClick={onClose}>Fermer</button>}
    </div>
  </div>
}

/** Formulaire de création d'un compte bêta (depuis une demande ou à la main). */
function AccountForm({ init, waitlistId, onCreated, onCancel }: { init?: { name: string; email: string }; waitlistId?: number; onCreated: (c: Cred) => void; onCancel?: () => void }) {
  const { toast } = useApp()
  const [f, setF] = useState({ name: init?.name ?? '', email: init?.email ?? '', username: suggestUsername(init?.name ?? '', init?.email ?? ''), password: '' })
  const [touched, setTouched] = useState(false)
  const [busy, setBusy] = useState(false)
  const up = (k: keyof typeof f) => (e: React.ChangeEvent<HTMLInputElement>) => {
    const v = { ...f, [k]: e.target.value }
    if (k === 'username') setTouched(true)
    else if (!touched && (k === 'name' || k === 'email')) v.username = suggestUsername(v.name, v.email)
    setF(v)
  }
  return <form className="stack" style={{ marginTop: 10 }} onSubmit={async e => {
    e.preventDefault(); setBusy(true)
    try { const r = await api.post('/api/admin/accounts', { ...f, waitlist_id: waitlistId }); onCreated({ ...r, name: r.name }); toast('Compte créé') } catch (er: any) { toast(er.message, true) } finally { setBusy(false) }
  }}>
    <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
      <label className="label" style={{ flex: '1 1 180px' }}>Nom affiché<input className="field" required maxLength={80} value={f.name} onChange={up('name')} /></label>
      <label className="label" style={{ flex: '1 1 220px' }}>E-mail<input className="field" type="email" required maxLength={200} value={f.email} onChange={up('email')} /></label>
    </div>
    <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
      <label className="label" style={{ flex: '1 1 180px' }}>Identifiant<input className="field mono" required minLength={3} maxLength={32} pattern="[a-zA-Z0-9._\-]+" title="Lettres sans accent, chiffres, points, tirets" autoCapitalize="off" spellCheck={false} value={f.username} onChange={up('username')} /></label>
      <label className="label" style={{ flex: '1 1 220px' }}>Mot de passe provisoire<input className="field mono" placeholder="Généré automatiquement si vide" value={f.password} onChange={up('password')} autoComplete="off" /></label>
    </div>
    <div className="row" style={{ gap: 8 }}>
      {onCancel && <button type="button" className="btn" onClick={onCancel}>Annuler</button>}
      <button className="btn pri" disabled={busy}><UserPlus size={16} />Créer le compte</button>
    </div>
  </form>
}

function Overview() {
  const { data } = useLoad<any>('/api/admin/overview')
  if (!data) return null
  const max = Math.max(1, ...data.requests_per_day.map((d: any) => d.count))
  return <>
    <div className="tiles">
      <div className="card"><div className="lab">Demandes en attente</div><div className="num">{n(data.waitlist.pending)}</div><div className="muted">{n(data.waitlist.registered)} comptes créés</div></div>
      <div className="card"><div className="lab">Utilisateurs</div><div className="num">{n(data.users)}</div><div className="muted">+{n(data.users_7d)} cette semaine · {n(data.active_7d)} actifs sur 7 j</div></div>
      <div className="card"><div className="lab">Studios et maps</div><div className="num">{n(data.studios)}</div><div className="muted">{n(data.maps)} maps</div></div>
      <div className="card"><div className="lab">Tâches</div><div className="num">{n(data.tasks)}</div><div className="muted">{n(data.tasks_done_7d)} finies sur 7 j</div></div>
    </div>
    <div className="grid2">
      <section className="card"><CardHead title="Demandes d’accès" sub="30 derniers jours" />
        {data.requests_per_day.length === 0 ? <p className="small muted">Aucune demande pour l’instant.</p> :
          <div className="minibars">{data.requests_per_day.map((d: any) => <div key={d.day} title={`${frDate(d.day)} : ${d.count}`}><span style={{ height: `${(d.count / max) * 100}%` }} /><em>{frDate(d.day).replace('.', '')}</em></div>)}</div>}
      </section>
      <section className="card"><CardHead title="Plateforme" />
        <div className="list">
          <div className="li"><span style={{ flex: 1 }}>Relais UEFN en ligne</span><b style={{ fontWeight: 500 }}>{n(data.relays_online)}</b></div>
          <div className="li"><span style={{ flex: 1 }}>IA connectées actives (7 j)</span><b style={{ fontWeight: 500 }}>{n(data.ai_tokens_active)}</b></div>
          <div className="li"><span style={{ flex: 1 }}>Envoi d’e-mails</span>{data.mail ? <span className="badge b-g">Configuré</span> : <span className="badge" title="Variables POP_SMTP_HOST, POP_SMTP_USER, POP_SMTP_PASS, POP_SMTP_FROM">Non configuré</span>}</div>
          <div className="li"><span style={{ flex: 1 }}>Comptes</span>{data.open_signup ? <span className="badge b-o">Inscriptions ouvertes à tous (POP_OPEN_SIGNUP=1)</span> : <span className="badge">Créés par l’administrateur</span>}</div>
        </div>
      </section>
    </div>
  </>
}

function Waitlist() {
  const { toast, reload: reloadMe } = useApp()
  const [status, setStatus] = useState('pending')
  const { data, reload } = useLoad<any[]>(`/api/admin/waitlist?status=${status}`, [status])
  const [open, setOpen] = useState<number | null>(null)
  const [creds, setCreds] = useState<Record<number, Cred>>({})
  const act = async (fn: () => Promise<any>, msg: string) => { try { await fn(); toast(msg); reload(); reloadMe() } catch (e: any) { toast(e.message, true) } }
  return <section className="card">
    <div className="row" style={{ gap: 8, flexWrap: 'wrap', marginBottom: 14 }}>
      {[['pending', 'En attente'], ['registered', 'Compte créé'], ['refused', 'Refusées'], ['all', 'Toutes']].map(([k, l]) => <button key={k} className={'chip' + (status === k ? ' on' : '')} style={{ height: 32 }} onClick={() => setStatus(k)}>{l}</button>)}
    </div>
    {!data ? null : data.length === 0 ? <Empty text={status === 'pending' ? 'Aucune demande en attente.' : 'Rien ici.'} /> :
      <div style={{ display: 'grid', gap: 10 }}>{data.map(w => <div key={w.id} className="adm-req">
        <div className="row" style={{ justifyContent: 'space-between', gap: 10, flexWrap: 'wrap' }}>
          <div style={{ minWidth: 0 }}>
            <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}><b style={{ fontWeight: 500, fontSize: 16 }}>{w.name}</b><span className="small muted">{w.email}</span><span className={'badge ' + STATUS[w.status][1]}>{STATUS[w.status][0]}</span></div>
            <div className="small muted" style={{ marginTop: 2 }}>{[w.studio && `Studio ${w.studio}`, w.team && `Équipe : ${w.team}`, w.experience].filter(Boolean).join(' · ')} · demandé {ago(w.created_at)}{w.decided_by ? ` · traité par ${w.decided_by}` : ''}</div>
          </div>
          <div className="row" style={{ gap: 6 }}>
            {w.status !== 'registered' && !creds[w.id] && <button className="btn pri" style={{ height: 34 }} onClick={() => setOpen(open === w.id ? null : w.id)}><Check size={15} />Accepter et créer le compte</button>}
            {w.status === 'pending' && <button className="btn" style={{ height: 34 }} onClick={() => act(() => api.post(`/api/admin/waitlist/${w.id}/refuse`), 'Demande refusée')}><X size={15} />Refuser</button>}
            {(w.status === 'refused' || w.status === 'approved') && <button className="iconbtn" title="Remettre en attente" aria-label="Remettre en attente" onClick={() => act(() => api.post(`/api/admin/waitlist/${w.id}/reopen`), 'Remise en attente')}><RotateCcw size={16} /></button>}
            <button className="iconbtn" title="Supprimer la demande" aria-label="Supprimer la demande" onClick={() => { if (confirm(`Supprimer la demande de ${w.name} ?`)) act(() => api.del(`/api/admin/waitlist/${w.id}`), 'Demande supprimée') }}><Trash2 size={16} /></button>
          </div>
        </div>
        {w.links && <p className="small" style={{ margin: '8px 0 0' }}><span className="muted">Maps / réseaux : </span>{w.links}</p>}
        {w.message && <p style={{ margin: '8px 0 0', whiteSpace: 'pre-wrap' }}>{w.message}</p>}
        <input className="field small" style={{ marginTop: 10, height: 32, width: '100%' }} placeholder="Note interne (visible seulement par les admins)" defaultValue={w.note}
          onBlur={e => { if (e.target.value !== w.note) api.post(`/api/admin/waitlist/${w.id}/note`, { note: e.target.value }).then(() => toast('Note enregistrée')).catch(er => toast(er.message, true)) }} />
        {open === w.id && !creds[w.id] && <AccountForm init={{ name: w.name, email: w.email }} waitlistId={w.id} onCancel={() => setOpen(null)} onCreated={c => { w.status = 'registered'; setCreds(x => ({ ...x, [w.id]: c })); setOpen(null); reloadMe() }} />}
        {creds[w.id] && <CredBox c={creds[w.id]} onClose={() => { setCreds(x => { const y = { ...x }; delete y[w.id]; return y }); reload() }} />}
      </div>)}</div>}
  </section>
}

function CreateAccount() {
  const [res, setRes] = useState<Cred | null>(null)
  const [k, setK] = useState(0)
  return <section className="card">
    <CardHead title="Créer un compte bêta" sub="Pour donner un accès sans demande sur la liste d’attente. Vous transmettez l’identifiant et le mot de passe provisoire ; la personne choisit son mot de passe à la première connexion." />
    {res ? <CredBox c={res} onClose={() => { setRes(null); setK(k + 1) }} /> : <AccountForm key={k} onCreated={setRes} />}
  </section>
}

function Users() {
  const { toast } = useApp()
  const { data, reload } = useLoad<any[]>('/api/admin/users')
  const [q, setQ] = useState('')
  const [cred, setCred] = useState<Cred | null>(null)
  const set = async (id: number, patch: any, msg: string) => { try { await api.post(`/api/admin/users/${id}`, patch); toast(msg); reload() } catch (e: any) { toast(e.message, true) } }
  const list = (data ?? []).filter(u => !q || (u.name + ' ' + u.email + ' ' + (u.username ?? '')).toLowerCase().includes(q.toLowerCase()))
  return <section className="card">
    <CardHead title="Utilisateurs" sub={data ? `${data.length} comptes` : ''}><input className="field" style={{ height: 34, width: 220 }} placeholder="Chercher" value={q} onChange={e => setQ(e.target.value)} /></CardHead>
    {cred && <CredBox c={cred} onClose={() => setCred(null)} />}
    <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Nom</th><th>Identifiant</th><th>E-mail</th><th>Inscrit</th><th>Vu</th><th>Studios</th><th>Tâches faites</th><th>Rôle</th><th /></tr></thead>
      <tbody>{list.map(u => <tr key={u.id} style={{ opacity: u.disabled ? 0.5 : 1 }}>
        <td><b style={{ fontWeight: 500 }}>{u.name}</b>{u.me && <span className="small muted"> (vous)</span>}{u.must_change && <span className="badge b-o" style={{ marginLeft: 6 }} title="N’a pas encore choisi son mot de passe">Mot de passe provisoire</span>}</td><td className="small mono">{u.username ?? '—'}</td><td className="small">{u.email}</td>
        <td className="small muted">{frDate(u.created_at)}</td><td className="small muted">{u.last_seen ? ago(u.last_seen) : '—'}</td>
        <td>{u.studios}</td><td>{u.tasks_done}</td>
        <td>{u.disabled ? <span className="badge b-r">Désactivé</span> : u.is_admin ? <span className="badge b-v">Admin</span> : <span className="badge">Utilisateur</span>}</td>
        <td>{!u.me && <div className="row" style={{ gap: 4, justifyContent: 'flex-end' }}>
          <button className="iconbtn" title="Nouveau mot de passe provisoire" aria-label="Réinitialiser le mot de passe" onClick={async () => { if (!confirm(`Donner un nouveau mot de passe provisoire à ${u.name} ? Son mot de passe actuel ne marchera plus et il sera déconnecté.`)) return; try { const r = await api.post(`/api/admin/users/${u.id}/password`); setCred({ ...r, name: u.name }); reload() } catch (e: any) { toast(e.message, true) } }}><KeyRound size={16} /></button>
          <button className="iconbtn" title={u.is_admin ? 'Retirer les droits admin' : 'Rendre admin'} aria-label="Droits admin" onClick={() => { if (confirm(u.is_admin ? `Retirer les droits admin de ${u.name} ?` : `Donner les droits admin à ${u.name} ? Il pourra gérer la liste d’attente et les comptes.`)) set(u.id, { is_admin: !u.is_admin }, 'Droits mis à jour') }}><ShieldCheck size={16} /></button>
          <button className="iconbtn" title={u.disabled ? 'Réactiver' : 'Désactiver le compte'} aria-label="Désactiver" onClick={() => { if (confirm(u.disabled ? `Réactiver le compte de ${u.name} ?` : `Désactiver le compte de ${u.name} ? Il sera déconnecté et ne pourra plus se connecter.`)) set(u.id, { disabled: !u.disabled }, u.disabled ? 'Compte réactivé' : 'Compte désactivé') }}><Ban size={16} /></button>
        </div>}</td>
      </tr>)}</tbody></table></div>
  </section>
}

function Audit() {
  const { data } = useLoad<any[]>('/api/admin/audit')
  return <section className="card"><CardHead title="Journal de sécurité" sub="Connexions, échecs, blocages et actions d’administration (300 derniers événements)." />
    {!data ? null : data.length === 0 ? <Empty text="Rien pour l’instant." /> :
      <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Quand</th><th>Qui</th><th>Événement</th><th>Détail</th><th>Adresse IP</th></tr></thead>
        <tbody>{data.map((e, i) => <tr key={i}><td className="small muted">{ago(e.at)}</td><td>{e.who ?? '—'}</td><td className={/échou|faux|bloqué|refus/.test(e.action) ? 'err' : ''}>{actionLabel(e.action)}</td><td className="small">{e.detail}</td><td className="small mono">{e.ip || '—'}</td></tr>)}</tbody></table></div>}
  </section>
}

/** Accès à l'administration : confirmer son mot de passe (valable 30 minutes). */
function Gate({ code, msg, onOk }: { code: string; msg: string; onOk: () => void }) {
  const { toast } = useApp()
  const [password, setPassword] = useState('')
  return <section className="card" style={{ maxWidth: 520 }}>
    <div className="row" style={{ gap: 10, marginBottom: 8 }}><Lock size={22} style={{ color: 'var(--to)' }} /><b style={{ fontWeight: 500, fontSize: 17 }}>Accès protégé</b></div>
    <p style={{ marginTop: 0 }}>{msg}</p>
    {code === 'sudo' && <form className="stack" onSubmit={async e => { e.preventDefault(); try { await api.post('/api/admin/unlock', { password }); onOk() } catch (er: any) { toast(er.message, true) } }}>
      <label className="label">Mot de passe<input className="field" type="password" required autoFocus autoComplete="current-password" value={password} onChange={e => setPassword(e.target.value)} /></label>
      <button className="btn pri"><Lock size={15} />Déverrouiller pour 30 minutes</button>
    </form>}
  </section>
}

export default function Admin() {
  const { me } = useApp()
  const [tab, setTab] = useState<'overview' | 'waitlist' | 'create' | 'users' | 'audit'>(me?.admin_pending ? 'waitlist' : 'overview')
  const [gate, setGate] = useState<{ code: string; msg: string } | null | 'ok'>(null)
  const check = () => api.get('/api/admin/overview').then(() => setGate('ok')).catch((e: any) => setGate({ code: e.code ?? 'forbidden', msg: e.message }))
  useEffect(() => { if (me?.admin) check() }, [me?.admin])
  if (!me?.admin) return <main className="content"><PageHead title="Administration" help="Réservé aux administrateurs de POP." /><section className="card"><Empty text="Vous n’avez pas accès à cette page." /></section></main>
  return <main className="content">
    <PageHead title="Administration" help="Gérer la plateforme POP : demandes d’accès à la bêta, comptes des bêta-testeurs et utilisateurs.">
      {gate === 'ok' && <button className="btn" onClick={async () => { await api.post('/api/admin/lock'); setGate(null); check() }}><Lock size={15} />Verrouiller</button>}
    </PageHead>
    {gate === null ? null : gate !== 'ok' ? <Gate code={gate.code} msg={gate.msg} onOk={() => { setGate(null); check() }} /> : <>
      <div className="tabs" role="tablist" style={{ alignSelf: 'flex-start' }}>
        {([['overview', 'Vue générale'], ['waitlist', `Liste d’attente${me.admin_pending ? ` (${me.admin_pending})` : ''}`], ['create', 'Créer un compte'], ['users', 'Utilisateurs'], ['audit', 'Sécurité']] as const).map(([k, l]) => <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? 'on' : ''} onClick={() => setTab(k)}>{l}</button>)}
      </div>
      {tab === 'overview' && <Overview />}
      {tab === 'waitlist' && <Waitlist />}
      {tab === 'create' && <CreateAccount />}
      {tab === 'users' && <Users />}
      {tab === 'audit' && <Audit />}
    </>}
  </main>
}
