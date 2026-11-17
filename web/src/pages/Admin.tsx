// Panel administrateur de la plateforme POP : liste d'attente, invitations, utilisateurs, vue générale.
import { useEffect, useState } from 'react'
import { Check, X, RotateCcw, Trash2, Copy, Mail, ShieldCheck, Ban, UserPlus, Lock } from 'lucide-react'
import { actionLabel } from '../components/Security'
import { ago, api, frDate } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead, Empty, PageHead } from '../components/ui'

const n = (v?: number) => (v ?? 0).toLocaleString('fr-FR')
const STATUS: Record<string, [string, string]> = { pending: ['En attente', 'b-o'], approved: ['Accès envoyé', 'b-b'], refused: ['Refusée', 'b-r'], registered: ['Compte créé', 'b-g'] }

function LinkBox({ r }: { r: { link: string; mailed: boolean; email: string } }) {
  const { toast } = useApp()
  return <div className="b-callout small" style={{ display: 'grid', gap: 8, marginTop: 10 }}>
    <span>{r.mailed ? <><Mail size={14} style={{ verticalAlign: -2 }} /> E-mail envoyé à {r.email}. Le lien est aussi ici :</> : <>L’envoi d’e-mails n’est pas configuré : envoyez ce lien à <b style={{ fontWeight: 500 }}>{r.email}</b> (valable 30 jours).</>}</span>
    <div className="row" style={{ gap: 8 }}><input className="field mono" readOnly value={r.link} style={{ flex: 1, height: 34, fontSize: 12.5 }} onFocus={e => e.target.select()} />
      <button className="btn" style={{ height: 34 }} onClick={() => { navigator.clipboard?.writeText(r.link); toast('Lien copié') }}><Copy size={14} />Copier</button></div>
  </div>
}

function Overview() {
  const { data } = useLoad<any>('/api/admin/overview')
  if (!data) return null
  const max = Math.max(1, ...data.requests_per_day.map((d: any) => d.count))
  return <>
    <div className="tiles">
      <div className="card"><div className="lab">Demandes en attente</div><div className="num">{n(data.waitlist.pending)}</div><div className="muted">{n(data.waitlist.approved)} accès envoyés, {n(data.waitlist.registered)} comptes créés</div></div>
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
          <div className="li"><span style={{ flex: 1 }}>Envoi d’e-mails</span>{data.mail ? <span className="badge b-g">Configuré</span> : <span className="badge b-o" title="Variables POP_SMTP_HOST, POP_SMTP_USER, POP_SMTP_PASS, POP_SMTP_FROM">Non configuré : liens à copier</span>}</div>
          <div className="li"><span style={{ flex: 1 }}>Inscriptions</span>{data.open_signup ? <span className="badge b-o">Ouvertes à tous (POP_OPEN_SIGNUP=1)</span> : <span className="badge">Sur liste d’attente</span>}</div>
        </div>
      </section>
    </div>
  </>
}

function Waitlist() {
  const { toast, reload: reloadMe } = useApp()
  const [status, setStatus] = useState('pending')
  const { data, reload } = useLoad<any[]>(`/api/admin/waitlist?status=${status}`, [status])
  const [links, setLinks] = useState<Record<number, any>>({})
  const act = async (fn: () => Promise<any>, msg: string) => { try { await fn(); toast(msg); reload(); reloadMe() } catch (e: any) { toast(e.message, true) } }
  return <section className="card">
    <div className="row" style={{ gap: 8, flexWrap: 'wrap', marginBottom: 14 }}>
      {[['pending', 'En attente'], ['approved', 'Accès envoyé'], ['registered', 'Compte créé'], ['refused', 'Refusées'], ['all', 'Toutes']].map(([k, l]) => <button key={k} className={'chip' + (status === k ? ' on' : '')} style={{ height: 32 }} onClick={() => setStatus(k)}>{l}</button>)}
    </div>
    {!data ? null : data.length === 0 ? <Empty text={status === 'pending' ? 'Aucune demande en attente.' : 'Rien ici.'} /> :
      <div style={{ display: 'grid', gap: 10 }}>{data.map(w => <div key={w.id} className="adm-req">
        <div className="row" style={{ justifyContent: 'space-between', gap: 10, flexWrap: 'wrap' }}>
          <div style={{ minWidth: 0 }}>
            <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}><b style={{ fontWeight: 500, fontSize: 16 }}>{w.name}</b><span className="small muted">{w.email}</span><span className={'badge ' + STATUS[w.status][1]}>{STATUS[w.status][0]}</span></div>
            <div className="small muted" style={{ marginTop: 2 }}>{[w.studio && `Studio ${w.studio}`, w.team && `Équipe : ${w.team}`, w.experience].filter(Boolean).join(' · ')} · demandé {ago(w.created_at)}{w.decided_by ? ` · traité par ${w.decided_by}` : ''}</div>
          </div>
          <div className="row" style={{ gap: 6 }}>
            {(w.status === 'pending' || w.status === 'refused') && <button className="btn pri" style={{ height: 34 }} onClick={async () => { try { const r = await api.post(`/api/admin/waitlist/${w.id}/approve`); setLinks(l => ({ ...l, [w.id]: r })); w.status = 'approved'; toast(r.mailed ? 'Accès accordé, e-mail envoyé' : 'Accès accordé : copiez le lien'); reloadMe() } catch (e: any) { toast(e.message, true) } }}><Check size={15} />Accepter</button>}
            {w.status === 'approved' && <button className="btn" style={{ height: 34 }} title="Génère un nouveau lien (l’ancien ne marche plus)" onClick={async () => { try { const r = await api.post(`/api/admin/waitlist/${w.id}/approve`); setLinks(l => ({ ...l, [w.id]: r })) } catch (e: any) { toast(e.message, true) } }}><Mail size={15} />Renvoyer un lien</button>}
            {w.status === 'pending' && <button className="btn" style={{ height: 34 }} onClick={() => act(() => api.post(`/api/admin/waitlist/${w.id}/refuse`), 'Demande refusée')}><X size={15} />Refuser</button>}
            {(w.status === 'refused' || w.status === 'approved') && <button className="iconbtn" title="Remettre en attente" aria-label="Remettre en attente" onClick={() => act(() => api.post(`/api/admin/waitlist/${w.id}/reopen`), 'Remise en attente')}><RotateCcw size={16} /></button>}
            <button className="iconbtn" title="Supprimer la demande" aria-label="Supprimer la demande" onClick={() => { if (confirm(`Supprimer la demande de ${w.name} ?`)) act(() => api.del(`/api/admin/waitlist/${w.id}`), 'Demande supprimée') }}><Trash2 size={16} /></button>
          </div>
        </div>
        {w.links && <p className="small" style={{ margin: '8px 0 0' }}><span className="muted">Maps / réseaux : </span>{w.links}</p>}
        {w.message && <p style={{ margin: '8px 0 0', whiteSpace: 'pre-wrap' }}>{w.message}</p>}
        <input className="field small" style={{ marginTop: 10, height: 32, width: '100%' }} placeholder="Note interne (visible seulement par les admins)" defaultValue={w.note}
          onBlur={e => { if (e.target.value !== w.note) api.post(`/api/admin/waitlist/${w.id}/note`, { note: e.target.value }).then(() => toast('Note enregistrée')).catch(er => toast(er.message, true)) }} />
        {links[w.id] && <LinkBox r={links[w.id]} />}
      </div>)}</div>}
  </section>
}

function Invite() {
  const { toast } = useApp()
  const [f, setF] = useState({ email: '', name: '' })
  const [res, setRes] = useState<any>(null)
  return <section className="card">
    <CardHead title="Inviter quelqu’un directement" sub="Donne un accès sans passer par la liste d’attente : la personne reçoit un lien pour créer son compte." />
    <form className="row" style={{ gap: 8, flexWrap: 'wrap' }} onSubmit={async e => { e.preventDefault(); try { const r = await api.post('/api/admin/invite', f); setRes(r); setF({ email: '', name: '' }); toast('Invitation créée') } catch (er: any) { toast(er.message, true) } }}>
      <input className="field" style={{ flex: '1 1 220px' }} type="email" required placeholder="E-mail" value={f.email} onChange={e => setF({ ...f, email: e.target.value })} />
      <input className="field" style={{ flex: '1 1 160px' }} placeholder="Nom (facultatif)" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} />
      <button className="btn pri"><UserPlus size={16} />Inviter</button>
    </form>
    {res && <LinkBox r={res} />}
  </section>
}

function Users() {
  const { toast } = useApp()
  const { data, reload } = useLoad<any[]>('/api/admin/users')
  const [q, setQ] = useState('')
  const set = async (id: number, patch: any, msg: string) => { try { await api.post(`/api/admin/users/${id}`, patch); toast(msg); reload() } catch (e: any) { toast(e.message, true) } }
  const list = (data ?? []).filter(u => !q || (u.name + ' ' + u.email).toLowerCase().includes(q.toLowerCase()))
  return <section className="card">
    <CardHead title="Utilisateurs" sub={data ? `${data.length} comptes` : ''}><input className="field" style={{ height: 34, width: 220 }} placeholder="Chercher" value={q} onChange={e => setQ(e.target.value)} /></CardHead>
    <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Nom</th><th>E-mail</th><th>Inscrit</th><th>Vu</th><th>Studios</th><th>Tâches faites</th><th>Rôle</th><th /></tr></thead>
      <tbody>{list.map(u => <tr key={u.id} style={{ opacity: u.disabled ? 0.5 : 1 }}>
        <td><b style={{ fontWeight: 500 }}>{u.name}</b>{u.me && <span className="small muted"> (vous)</span>}</td><td className="small">{u.email}</td>
        <td className="small muted">{frDate(u.created_at)}</td><td className="small muted">{u.last_seen ? ago(u.last_seen) : '—'}</td>
        <td>{u.studios}</td><td>{u.tasks_done}</td>
        <td>{u.disabled ? <span className="badge b-r">Désactivé</span> : u.is_admin ? <span className="badge b-v">Admin</span> : <span className="badge">Utilisateur</span>}</td>
        <td>{!u.me && <div className="row" style={{ gap: 4, justifyContent: 'flex-end' }}>
          <button className="iconbtn" title={u.is_admin ? 'Retirer les droits admin' : 'Rendre admin'} aria-label="Droits admin" onClick={() => { if (confirm(u.is_admin ? `Retirer les droits admin de ${u.name} ?` : `Donner les droits admin à ${u.name} ? Il pourra gérer la liste d’attente et les comptes.`)) set(u.id, { is_admin: !u.is_admin }, 'Droits mis à jour') }}><ShieldCheck size={16} /></button>
          <button className="iconbtn" title={u.disabled ? 'Réactiver' : 'Désactiver le compte'} aria-label="Désactiver" onClick={() => { if (confirm(u.disabled ? `Réactiver le compte de ${u.name} ?` : `Désactiver le compte de ${u.name} ? Il sera déconnecté et ne pourra plus se connecter.`)) set(u.id, { disabled: !u.disabled }, u.disabled ? 'Compte réactivé' : 'Compte désactivé') }}><Ban size={16} /></button>
        </div>}</td>
      </tr>)}</tbody></table></div>
  </section>
}

function Audit() {
  const { data } = useLoad<any[]>('/api/admin/audit')
  return <section className="card"><CardHead title="Journal de sécurité" sub="Connexions, échecs, blocages, double authentification et actions d’administration (300 derniers événements)." />
    {!data ? null : data.length === 0 ? <Empty text="Rien pour l’instant." /> :
      <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Quand</th><th>Qui</th><th>Événement</th><th>Détail</th><th>Adresse IP</th></tr></thead>
        <tbody>{data.map((e, i) => <tr key={i}><td className="small muted">{ago(e.at)}</td><td>{e.who ?? '—'}</td><td className={/échou|faux|bloqué|refus/.test(e.action) ? 'err' : ''}>{actionLabel(e.action)}</td><td className="small">{e.detail}</td><td className="small mono">{e.ip || '—'}</td></tr>)}</tbody></table></div>}
  </section>
}

/** Accès à l'administration : double authentification obligatoire + confirmation récente (mot de passe et code). */
function Gate({ code, msg, onOk }: { code: string; msg: string; onOk: () => void }) {
  const { toast } = useApp()
  const [f, setF] = useState({ password: '', code: '' })
  return <section className="card" style={{ maxWidth: 520 }}>
    <div className="row" style={{ gap: 10, marginBottom: 8 }}><Lock size={22} style={{ color: 'var(--to)' }} /><b style={{ fontWeight: 500, fontSize: 17 }}>Accès protégé</b></div>
    <p style={{ marginTop: 0 }}>{msg}</p>
    {code === 'mfa_setup' && <a className="btn pri" href="/parametres#securite">Activer la double authentification</a>}
    {code === 'mfa_session' && <button className="btn pri" onClick={async () => { await api.post('/api/auth/logout'); location.href = '/connexion' }}>Me reconnecter</button>}
    {code === 'sudo' && <form className="stack" onSubmit={async e => { e.preventDefault(); try { await api.post('/api/admin/unlock', f); onOk() } catch (er: any) { toast(er.message, true) } }}>
      <label className="label">Mot de passe<input className="field" type="password" required autoFocus autoComplete="current-password" value={f.password} onChange={e => setF({ ...f, password: e.target.value })} /></label>
      <label className="label">Code à 6 chiffres<input className="field mono" required inputMode="numeric" autoComplete="one-time-code" value={f.code} onChange={e => setF({ ...f, code: e.target.value })} style={{ letterSpacing: 3 }} /></label>
      <button className="btn pri"><Lock size={15} />Déverrouiller pour 30 minutes</button>
    </form>}
  </section>
}

export default function Admin() {
  const { me } = useApp()
  const [tab, setTab] = useState<'overview' | 'waitlist' | 'invite' | 'users' | 'audit'>(me?.admin_pending ? 'waitlist' : 'overview')
  const [gate, setGate] = useState<{ code: string; msg: string } | null | 'ok'>(null)
  const check = () => api.get('/api/admin/overview').then(() => setGate('ok')).catch((e: any) => setGate({ code: e.code ?? 'forbidden', msg: e.message }))
  useEffect(() => { if (me?.admin) check() }, [me?.admin])
  if (!me?.admin) return <main className="content"><PageHead title="Administration" help="Réservé aux administrateurs de POP." /><section className="card"><Empty text="Vous n’avez pas accès à cette page." /></section></main>
  return <main className="content">
    <PageHead title="Administration" help="Gérer la plateforme POP : demandes d’accès à la bêta, invitations et comptes.">
      {gate === 'ok' && <button className="btn" onClick={async () => { await api.post('/api/admin/lock'); setGate(null); check() }}><Lock size={15} />Verrouiller</button>}
    </PageHead>
    {gate === null ? null : gate !== 'ok' ? <Gate code={gate.code} msg={gate.msg} onOk={() => { setGate(null); check() }} /> : <>
      <div className="tabs" role="tablist" style={{ alignSelf: 'flex-start' }}>
        {([['overview', 'Vue générale'], ['waitlist', `Liste d’attente${me.admin_pending ? ` (${me.admin_pending})` : ''}`], ['invite', 'Inviter'], ['users', 'Utilisateurs'], ['audit', 'Sécurité']] as const).map(([k, l]) => <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? 'on' : ''} onClick={() => setTab(k)}>{l}</button>)}
      </div>
      {tab === 'overview' && <Overview />}
      {tab === 'waitlist' && <Waitlist />}
      {tab === 'invite' && <Invite />}
      {tab === 'users' && <Users />}
      {tab === 'audit' && <Audit />}
    </>}
  </main>
}
