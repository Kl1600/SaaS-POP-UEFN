// Panel administrateur de la plateforme POP : liste d'attente, création des comptes bêta, utilisateurs, vue générale.
import { useEffect, useState } from 'react'
import { Check, X, RotateCcw, Trash2, Copy, Mail, ShieldCheck, Ban, UserPlus, Lock, KeyRound, Link2, LockOpen } from 'lucide-react'
import { actionLabel } from '../components/Security'
import { ago, api, frDate } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead, Empty, Modal, PageHead } from '../components/ui'
import { tr, LOCALE } from '../i18n/t'

const n = (v?: number) => (v ?? 0).toLocaleString(LOCALE)
const STATUS: Record<string, [string, string]> = { pending: [tr("En attente"), 'b-o'], approved: [tr("Accepté, sans compte"), 'b-b'], refused: [tr("Refusée"), 'b-r'], registered: [tr("Compte créé"), 'b-g'] }

/** Identifiant proposé à partir du nom (ou de l'e-mail) : minuscules, sans accent, points à la place des espaces. */
export const suggestUsername = (name: string, email = '') => {
  const base = (name.trim() || email.split('@')[0] || '').normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
  return base.replace(/\s+/g, '.').replace(/[^a-z0-9._-]/g, '').replace(/\.{2,}/g, '.').replace(/^[._-]+|[._-]+$/g, '').slice(0, 32)
}

type Cred = { username: string; password: string; url: string; name?: string }
/** Identifiants à transmettre : affichés une seule fois. */
function CredBox({ c, onClose }: { c: Cred; onClose?: () => void }) {
  const { toast } = useApp()
  const msg = tr("Bonjour{0},\n\nTon accès à la bêta de POP est prêt.\n\nAdresse : {1}\nIdentifiant : {2}\nMot de passe provisoire : {3}\n\nTu choisiras ton propre mot de passe à la première connexion.\n\nÀ très vite,\nL’équipe POP", [c.name ? ' ' + c.name.split(' ')[0] : '', c.url, c.username, c.password])
  return <div className="b-callout" style={{ display: 'grid', gap: 10, marginTop: 10 }}>
    <b style={{ fontWeight: 500 }}>{tr("Identifiants à transmettre (affichés une seule fois)")}</b>
    <div className="mono" style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 14px', fontSize: 14 }}>
      <span className="muted">{tr("Identifiant")}</span><span>{c.username}</span>
      <span className="muted">{tr("Mot de passe")}</span><span>{c.password}</span>
      <span className="muted">{tr("Adresse")}</span><span style={{ wordBreak: 'break-all' }}>{c.url}</span>
    </div>
    <span className="small muted">{tr("La personne devra choisir son propre mot de passe à sa première connexion. Envoyez-lui ces informations en message privé (Discord, e-mail…).")}</span>
    <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
      <button className="btn pri" onClick={() => { navigator.clipboard?.writeText(msg); toast(tr("Message copié")) }}><Copy size={15} />{tr("Copier le message")}</button>
      <button className="btn" onClick={() => { navigator.clipboard?.writeText(c.password); toast(tr("Mot de passe copié")) }}><Copy size={15} />{tr("Mot de passe seul")}</button>
      {onClose && <button className="btn" onClick={onClose}>{tr("Fermer")}</button>}
    </div>
  </div>
}

type Sent = { username?: string; name?: string; email: string; mailed: boolean; link?: string }
/** Lien d'activation (#42) : envoyé par e-mail, ou à copier quand l'envoi d'e-mails n'est pas configuré. */
function ActivationBox({ s, onClose }: { s: Sent; onClose?: () => void }) {
  const { toast } = useApp()
  return <div className="b-callout" style={{ display: 'grid', gap: 10, marginTop: 10 }}>
    {s.mailed
      ? <span><Mail size={15} style={{ verticalAlign: -2 }} /> {tr("E-mail d’activation envoyé à {0}. La personne choisit son mot de passe avec le lien (valable 7 jours, une seule fois).", [s.email])}</span>
      : <>
        <b style={{ fontWeight: 500 }}>{tr("L’envoi d’e-mails n’est pas configuré : envoyez ce lien d’activation à {0} (valable 7 jours, une seule fois).", [s.email])}</b>
        <div className="row" style={{ gap: 8 }}><input className="field mono" readOnly value={s.link ?? ''} style={{ flex: 1, height: 34, fontSize: 12.5 }} onFocus={e => e.target.select()} />
          <button className="btn" style={{ height: 34 }} onClick={() => { navigator.clipboard?.writeText(s.link ?? ''); toast(tr("Lien copié")) }}><Copy size={14} />{tr("Copier")}</button></div>
        <span className="small muted">{tr("Avec ce lien, la personne choisit son mot de passe : ne le transmettez qu’à elle, en message privé.")}</span>
      </>}
    {s.username && <span className="small muted">{tr("Identifiant : {0}", [s.username])}</span>}
    {onClose && <div><button className="btn" onClick={onClose}>{tr("Fermer")}</button></div>}
  </div>
}

/** Formulaire de création d'un compte bêta (depuis une demande ou à la main). */
const PLANS: [string, string][] = [['starter', 'Starter (12,99 €)'], ['pro', 'Pro (24,99 €)']]
/** Plan d'un compte. Analyse est comprise dans le plan Pro. */
function PlanSelect({ value, onChange }: { value: string; onChange: (e: React.ChangeEvent<HTMLSelectElement>) => void }) {
  return <select className="field" value={value} onChange={onChange} title={tr("Analyse est comprise dans le plan Pro")}>{PLANS.map(([v, l]) => <option key={v} value={v}>{tr(l)}</option>)}</select>
}

function AccountForm({ init, waitlistId, onCreated, onCancel }: { init?: { name: string; email: string }; waitlistId?: number; onCreated: (s: Sent) => void; onCancel?: () => void }) {
  const { toast } = useApp()
  const [f, setF] = useState({ name: init?.name ?? '', email: init?.email ?? '', username: suggestUsername(init?.name ?? '', init?.email ?? ''), plan: 'pro' })
  const [touched, setTouched] = useState(false)
  const [busy, setBusy] = useState(false)
  const up = (k: keyof typeof f) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
    const v = { ...f, [k]: e.target.value }
    if (k === 'username') setTouched(true)
    else if (!touched && (k === 'name' || k === 'email')) v.username = suggestUsername(v.name, v.email)
    setF(v)
  }
  return <form className="stack" style={{ marginTop: 10 }} onSubmit={async e => {
    e.preventDefault(); setBusy(true)
    try { const r = await api.post('/api/admin/accounts', { ...f, waitlist_id: waitlistId }); onCreated(r); toast(r.mailed ? tr("Compte créé : e-mail d’activation envoyé") : tr("Compte créé : copiez le lien d’activation")) } catch (er: any) { toast(er.message, true) } finally { setBusy(false) }
  }}>
    <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
      <label className="label" style={{ flex: '1 1 180px' }}>{tr("Nom affiché")}<input className="field" required maxLength={80} value={f.name} onChange={up('name')} /></label>
      <label className="label" style={{ flex: '1 1 220px' }}>E-mail<input className="field" type="email" required maxLength={200} value={f.email} onChange={up('email')} /></label>
    </div>
    <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
      <label className="label" style={{ flex: '1 1 180px' }}>{tr("Identifiant")}<input className="field mono" required minLength={3} maxLength={32} pattern="[a-zA-Z0-9._\-]+" title={tr("Lettres sans accent, chiffres, points, tirets")} autoCapitalize="off" spellCheck={false} value={f.username} onChange={up('username')} /></label>
      <label className="label" style={{ flex: '1 1 160px' }}>{tr("Plan")}<PlanSelect value={f.plan} onChange={up('plan')} /></label>
    </div>
    <div className="row" style={{ gap: 8 }}>
      {onCancel && <button type="button" className="btn" onClick={onCancel}>{tr("Annuler")}</button>}
      <button className="btn pri" disabled={busy}><UserPlus size={16} />{tr("Créer le compte")}</button>
    </div>
  </form>
}

function Overview() {
  const { data } = useLoad<any>('/api/admin/overview')
  if (!data) return null
  const max = Math.max(1, ...data.requests_per_day.map((d: any) => d.count))
  return <>
    <div className="tiles">
      <div className="card"><div className="lab">{tr("Demandes en attente")}</div><div className="num">{n(data.waitlist.pending)}</div><div className="muted">{tr("{0} comptes créés", [n(data.waitlist.registered)])}</div></div>
      <div className="card"><div className="lab">{tr("Utilisateurs")}</div><div className="num">{n(data.users)}</div><div className="muted">{tr("+{0} cette semaine · {1} actifs sur 7 j", [n(data.users_7d), n(data.active_7d)])}</div></div>
      <div className="card"><div className="lab">{tr("Studios et maps")}</div><div className="num">{n(data.studios)}</div><div className="muted">{n(data.maps)} maps</div></div>
      <div className="card"><div className="lab">{tr("Tâches")}</div><div className="num">{n(data.tasks)}</div><div className="muted">{tr("{0} finies sur 7 j", [n(data.tasks_done_7d)])}</div></div>
    </div>
    <div className="grid2">
      <section className="card"><CardHead title={tr("Demandes d’accès")} sub={tr("30 derniers jours")} />
        {data.requests_per_day.length === 0 ? <p className="small muted">{tr("Aucune demande pour l’instant.")}</p> :
          <div className="minibars">{data.requests_per_day.map((d: any) => <div key={d.day} title={`${frDate(d.day)} : ${d.count}`}><span style={{ height: `${(d.count / max) * 100}%` }} /><em>{frDate(d.day).replace('.', '')}</em></div>)}</div>}
      </section>
      <section className="card"><CardHead title={tr("Plateforme")} />
        <div className="list">
          <div className="li"><span style={{ flex: 1 }}>{tr("Relais UEFN en ligne")}</span><b style={{ fontWeight: 500 }}>{n(data.relays_online)}</b></div>
          <div className="li"><span style={{ flex: 1 }}>{tr("IA connectées actives (7 j)")}</span><b style={{ fontWeight: 500 }}>{n(data.ai_tokens_active)}</b></div>
          <div className="li"><span style={{ flex: 1 }}>{tr("Envoi d’e-mails")}</span>{data.mail ? <span className="badge b-g">{tr("Configuré")}</span> : <span className="badge" title={tr("Variables POP_SMTP_HOST, POP_SMTP_USER, POP_SMTP_PASS, POP_SMTP_FROM")}>{tr("Non configuré")}</span>}</div>
          <div className="li"><span style={{ flex: 1 }}>{tr("Comptes")}</span>{data.open_signup ? <span className="badge b-o">{tr("Inscriptions ouvertes à tous (POP_OPEN_SIGNUP=1)")}</span> : <span className="badge">{tr("Créés par l’administrateur")}</span>}</div>
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
  const [creds, setCreds] = useState<Record<number, Sent>>({})
  const act = async (fn: () => Promise<any>, msg: string) => { try { await fn(); toast(msg); reload(); reloadMe() } catch (e: any) { toast(e.message, true) } }
  return <section className="card">
    <div className="row" style={{ gap: 8, flexWrap: 'wrap', marginBottom: 14 }}>
      {[['pending', tr("En attente")], ['registered', tr("Compte créé")], ['refused', tr("Refusées")], ['all', tr("Toutes")]].map(([k, l]) => <button key={k} className={'chip' + (status === k ? ' on' : '')} style={{ height: 32 }} onClick={() => setStatus(k)}>{l}</button>)}
    </div>
    {!data ? null : data.length === 0 ? <Empty text={status === 'pending' ? tr("Aucune demande en attente.") : tr("Rien ici.")} /> :
      <div style={{ display: 'grid', gap: 10 }}>{data.map(w => <div key={w.id} className="adm-req">
        <div className="row" style={{ justifyContent: 'space-between', gap: 10, flexWrap: 'wrap' }}>
          <div style={{ minWidth: 0 }}>
            <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}><b style={{ fontWeight: 500, fontSize: 16 }}>{w.name}</b><span className="small muted">{w.email}</span>{w.activation_pending ? <span className="badge b-o">{tr("En attente d’activation")}</span> : <span className={'badge ' + STATUS[w.status][1]}>{STATUS[w.status][0]}</span>}</div>
            <div className="small muted" style={{ marginTop: 2 }}>{tr("{0} · demandé {1}{2}", [[w.studio && tr("Studio {0}", [w.studio]), w.team && tr("Équipe : {0}", [w.team]), w.experience].filter(Boolean).join(' · '), ago(w.created_at), w.decided_by ? tr(" · traité par {0}", [w.decided_by]) : ''])}</div>
          </div>
          <div className="row" style={{ gap: 6 }}>
            {w.status !== 'registered' && !w.activation_pending && !creds[w.id] && <button className="btn pri" style={{ height: 34 }} onClick={() => setOpen(open === w.id ? null : w.id)}><Check size={15} />{tr("Accepter et créer le compte")}</button>}
            {w.status === 'pending' && <button className="btn" style={{ height: 34 }} onClick={() => act(() => api.post(`/api/admin/waitlist/${w.id}/refuse`), tr("Demande refusée"))}><X size={15} />{tr("Refuser")}</button>}
            {(w.status === 'refused' || w.status === 'approved') && <button className="iconbtn" title={tr("Remettre en attente")} aria-label={tr("Remettre en attente")} onClick={() => act(() => api.post(`/api/admin/waitlist/${w.id}/reopen`), tr("Remise en attente"))}><RotateCcw size={16} /></button>}
            <button className="iconbtn" title={tr("Supprimer la demande")} aria-label={tr("Supprimer la demande")} onClick={() => { if (confirm(tr("Supprimer la demande de {0} ?", [w.name]))) act(() => api.del(`/api/admin/waitlist/${w.id}`), tr("Demande supprimée")) }}><Trash2 size={16} /></button>
          </div>
        </div>
        {w.links && <p className="small" style={{ margin: '8px 0 0' }}><span className="muted">{tr("Maps / réseaux :")}{' '}</span>{w.links}</p>}
        {w.message && <p style={{ margin: '8px 0 0', whiteSpace: 'pre-wrap' }}>{w.message}</p>}
        <input className="field small" style={{ marginTop: 10, height: 32, width: '100%' }} placeholder={tr("Note interne (visible seulement par les admins)")} defaultValue={w.note}
          onBlur={e => { if (e.target.value !== w.note) api.post(`/api/admin/waitlist/${w.id}/note`, { note: e.target.value }).then(() => toast(tr("Note enregistrée"))).catch(er => toast(er.message, true)) }} />
        {open === w.id && !creds[w.id] && <AccountForm init={{ name: w.name, email: w.email }} waitlistId={w.id} onCancel={() => setOpen(null)} onCreated={c => { w.activation_pending = true; setCreds(x => ({ ...x, [w.id]: c })); setOpen(null); reloadMe() }} />}
        {creds[w.id] && <ActivationBox s={creds[w.id]} onClose={() => { setCreds(x => { const y = { ...x }; delete y[w.id]; return y }); reload() }} />}
      </div>)}</div>}
  </section>
}

function CreateAccount() {
  const [res, setRes] = useState<Sent | null>(null)
  const [k, setK] = useState(0)
  return <section className="card">
    <CardHead title={tr("Créer un compte bêta")} sub={tr("Pour donner un accès sans demande sur la liste d’attente. La personne reçoit un e-mail d’activation et choisit elle-même son mot de passe.")} />
    {res ? <ActivationBox s={res} onClose={() => { setRes(null); setK(k + 1) }} /> : <AccountForm key={k} onCreated={setRes} />}
  </section>
}

/** Heure locale d'une date SQLite en UTC (« 2026-11-19 14:05:00 » → « 15:05 »). */
const hhmm = (d: string) => new Date(d.replace(' ', 'T') + 'Z').toLocaleTimeString(LOCALE, { hour: '2-digit', minute: '2-digit' })

/** Lien de nouveau mot de passe (2 heures, une seule fois) à transmettre à la personne. */
function ResetLinkBox({ r, onClose }: { r: { link: string; mailed: boolean; email: string }; onClose: () => void }) {
  const { toast } = useApp()
  return <div className="b-callout small" style={{ display: 'grid', gap: 8, marginBottom: 12 }}>
    <div className="row" style={{ justifyContent: 'space-between', gap: 8 }}>
      <span>{r.mailed ? <><Mail size={14} style={{ verticalAlign: -2 }} /> {tr("E-mail envoyé à {0}. Le lien est aussi ici (valable 2 heures, une seule fois) :", [r.email])}</> : tr("L’envoi d’e-mails n’est pas configuré : envoyez ce lien à {0} (valable 2 heures, une seule fois).", [r.email])}</span>
      <button className="iconbtn" aria-label={tr("Fermer")} onClick={onClose}><X size={16} /></button>
    </div>
    <div className="row" style={{ gap: 8 }}><input className="field mono" readOnly value={r.link} style={{ flex: 1, height: 34, fontSize: 12.5 }} onFocus={e => e.target.select()} />
      <button className="btn" style={{ height: 34 }} onClick={() => { navigator.clipboard?.writeText(r.link); toast(tr("Lien copié")) }}><Copy size={14} />{tr("Copier")}</button></div>
  </div>
}

/** Suppression d'un compte : ce que deviennent ses studios, puis l'adresse e-mail à retaper. */
function DeleteUser({ u, onClose, onDone }: { u: any; onClose: () => void; onDone: () => void }) {
  const { toast } = useApp()
  const { data: plan } = useLoad<any>(`/api/admin/users/${u.id}/deletion`)
  const [confirm, setConfirm] = useState('')
  const ok = confirm.trim().toLowerCase() === u.email.toLowerCase()
  return <Modal onClose={onClose} label={tr("Supprimer le compte de {0}", [u.name])}>
    <div className="row" style={{ justifyContent: 'space-between' }}><h2 style={{ margin: 0, fontWeight: 500, fontSize: 20 }}>{tr("Supprimer le compte de {0}", [u.name])}</h2><button className="iconbtn" aria-label={tr("Fermer")} onClick={onClose}><X size={18} /></button></div>
    <p className="small" style={{ margin: '12px 0' }}>{tr("La suppression est définitive : la personne est déconnectée et ne pourra plus se connecter avec {0}.", [u.email])}</p>
    {plan && <div className="b-callout small" style={{ display: 'grid', gap: 6 }}>
      {plan.delete.length === 0 && plan.transfer.length === 0 && <span>{tr("Ce compte n’est propriétaire d’aucun studio.")}</span>}
      {plan.delete.length > 0 && <span>{tr("Studios supprimés avec le compte (aucun autre membre) : {0}", [plan.delete.join(', ')])}</span>}
      {plan.transfer.map((t: any) => <span key={t.studio}>{tr("Le studio {0} est transféré à {1}, qui en devient propriétaire.", [t.studio, t.to])}</span>)}
    </div>}
    <form className="stack" style={{ marginTop: 14 }} onSubmit={async e => { e.preventDefault(); if (!ok) return; try { await api.del(`/api/admin/users/${u.id}`, { confirm }); toast(tr("Compte supprimé")); onDone() } catch (er: any) { toast(er.message, true) } }}>
      <label className="label">{tr("Tapez l’adresse e-mail du compte pour confirmer")}<input className="field" autoFocus autoComplete="off" placeholder={u.email} value={confirm} onChange={e => setConfirm(e.target.value)} /></label>
      <button className="btn danger" disabled={!ok || !plan}><Trash2 size={15} />{tr("Supprimer définitivement")}</button>
    </form>
  </Modal>
}

function Users() {
  const { toast } = useApp()
  const { data, reload } = useLoad<any[]>('/api/admin/users')
  const [q, setQ] = useState('')
  const [cred, setCred] = useState<Cred | null>(null)
  const [link, setLink] = useState<any>(null)
  const [sent, setSent] = useState<Sent | null>(null)
  const [del, setDel] = useState<any>(null)
  const run = async (fn: () => Promise<any>, msg: string) => { try { await fn(); toast(msg); reload() } catch (e: any) { toast(e.message, true) } }
  const set = async (id: number, patch: any, msg: string) => { try { await api.post(`/api/admin/users/${id}`, patch); toast(msg); reload() } catch (e: any) { toast(e.message, true) } }
  const list = (data ?? []).filter(u => !q || (u.name + ' ' + u.email + ' ' + (u.username ?? '')).toLowerCase().includes(q.toLowerCase()))
  return <section className="card">
    <CardHead title={tr("Utilisateurs")} sub={data ? `${data.length} comptes` : ''}><input className="field" style={{ height: 34, width: 220 }} placeholder={tr("Chercher")} value={q} onChange={e => setQ(e.target.value)} /></CardHead>
    {cred && <CredBox c={cred} onClose={() => setCred(null)} />}
    {link && <ResetLinkBox r={link} onClose={() => setLink(null)} />}
    {sent && <ActivationBox s={sent} onClose={() => setSent(null)} />}
    <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>{tr("Nom")}</th><th>{tr("Identifiant")}</th><th>E-mail</th><th>{tr("Inscrit")}</th><th>Vu</th><th>{tr("Studios")}</th><th>{tr("Tâches faites")}</th><th>{tr("Plan")}</th><th>{tr("Rôle")}</th><th /></tr></thead>
      <tbody>{list.map(u => <tr key={u.id} style={{ opacity: u.disabled ? 0.5 : 1 }}>
        <td><b style={{ fontWeight: 500 }}>{u.name}</b>{u.me && <span className="small muted"> {tr("(vous)")}</span>}{u.must_change && <span className="badge b-o" style={{ marginLeft: 6 }} title={tr("N’a pas encore choisi son mot de passe")}>{tr("Mot de passe provisoire")}</span>}{u.activation_pending && <span className="badge b-o" style={{ marginLeft: 6 }} title={u.activation_sent_at ? tr("Lien envoyé le {0}", [frDate(u.activation_sent_at)]) : undefined}>{tr("En attente d’activation")}</span>}{u.activation_pending && u.activation_sent_at && <div className="small muted">{tr("Lien envoyé {0}", [ago(u.activation_sent_at)])}</div>}</td><td className="small mono">{u.username ?? '—'}</td><td className="small">{u.email}</td>
        <td className="small muted">{frDate(u.created_at)}</td><td className="small muted">{u.last_seen ? ago(u.last_seen) : '—'}</td>
        <td>{u.studios}</td><td>{u.tasks_done}</td>
        <td><PlanSelect value={u.plan ?? 'starter'} onChange={async e => { try { await api.post(`/api/admin/users/${u.id}/plan`, { plan: e.target.value }); toast(tr("Plan mis à jour")); reload() } catch (er: any) { toast(er.message, true) } }} /></td>
        <td><div className="row" style={{ gap: 4, flexWrap: 'wrap' }}>{u.disabled ? <span className="badge b-r">{tr("Désactivé")}</span> : u.is_admin ? <span className="badge b-v">{tr("Admin")}</span> : <span className="badge">{tr("Utilisateur")}</span>}
          {u.locked_until && <span className="badge b-o" title={tr("Trop de mots de passe faux")}>{tr("Bloqué jusqu’à {0}", [hhmm(u.locked_until)])}</span>}</div></td>
        <td>{!u.me && <div className="row" style={{ gap: 4, justifyContent: 'flex-end' }}>
          {u.activation_pending && <button className="btn" style={{ height: 32 }} title={tr("Nouveau lien d’activation : l’ancien ne marchera plus")} onClick={async () => { try { const r = await api.post(`/api/admin/users/${u.id}/activation`); setSent({ ...r, username: u.username }); toast(r.mailed ? tr("Lien renvoyé par e-mail") : tr("Lien créé : copiez-le")); reload() } catch (e: any) { toast(e.message, true) } }}><Mail size={15} />{tr("Renvoyer le lien")}</button>}
          {u.locked_until && <button className="iconbtn" title={tr("Débloquer")} aria-label={tr("Débloquer")} onClick={() => run(() => api.post(`/api/admin/users/${u.id}/unlock`), tr("Compte débloqué"))}><LockOpen size={16} /></button>}
          <button className="iconbtn" title={tr("Lien de nouveau mot de passe")} aria-label={tr("Lien de nouveau mot de passe")} onClick={async () => { try { const r = await api.post(`/api/admin/users/${u.id}/reset-link`); setLink(r); toast(r.mailed ? tr("Lien envoyé par e-mail") : tr("Lien créé : copiez-le")) } catch (e: any) { toast(e.message, true) } }}><Link2 size={16} /></button>
          {!u.activation_pending && <button className="iconbtn" title={tr("Nouveau mot de passe provisoire")} aria-label={tr("Réinitialiser le mot de passe")} onClick={async () => { if (!confirm(tr("Donner un nouveau mot de passe provisoire à {0} ? Son mot de passe actuel ne marchera plus et il sera déconnecté.", [u.name]))) return; try { const r = await api.post(`/api/admin/users/${u.id}/password`); setCred({ ...r, name: u.name }); reload() } catch (e: any) { toast(e.message, true) } }}><KeyRound size={16} /></button>}
          <button className="iconbtn" title={u.is_admin ? tr("Retirer les droits admin") : tr("Rendre admin")} aria-label={tr("Droits admin")} onClick={() => { if (confirm(u.is_admin ? tr("Retirer les droits admin de {0} ?", [u.name]) : tr("Donner les droits admin à {0} ? Il pourra gérer la liste d’attente et les comptes.", [u.name]))) set(u.id, { is_admin: !u.is_admin }, tr("Droits mis à jour")) }}><ShieldCheck size={16} /></button>
          <button className="iconbtn" title={u.disabled ? tr("Réactiver") : tr("Désactiver le compte")} aria-label={tr("Désactiver")} onClick={() => { if (confirm(u.disabled ? tr("Réactiver le compte de {0} ?", [u.name]) : tr("Désactiver le compte de {0} ? Il sera déconnecté et ne pourra plus se connecter.", [u.name]))) set(u.id, { disabled: !u.disabled }, u.disabled ? tr("Compte réactivé") : tr("Compte désactivé")) }}><Ban size={16} /></button>
          <button className="iconbtn" title={tr("Supprimer le compte")} aria-label={tr("Supprimer le compte")} onClick={() => setDel(u)}><Trash2 size={16} /></button>
        </div>}</td>
      </tr>)}</tbody></table></div>
    {del && <DeleteUser u={del} onClose={() => setDel(null)} onDone={() => { setDel(null); reload() }} />}
  </section>
}

function Audit() {
  const { data } = useLoad<any[]>('/api/admin/audit')
  return <section className="card"><CardHead title={tr("Journal de sécurité")} sub={tr("Connexions, échecs, blocages et actions d’administration (300 derniers événements).")} />
    {!data ? null : data.length === 0 ? <Empty text={tr("Rien pour l’instant.")} /> :
      <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>{tr("Quand")}</th><th>{tr("Qui")}</th><th>{tr("Événement")}</th><th>{tr("Détail")}</th><th>{tr("Adresse IP")}</th></tr></thead>
        <tbody>{data.map((e, i) => <tr key={i}><td className="small muted">{ago(e.at)}</td><td>{e.who ?? '—'}</td><td className={/échou|faux|(?<!dé)bloqué|refus/.test(e.action) ? 'err' : ''}>{actionLabel(e.action)}</td><td className="small">{e.detail}</td><td className="small mono">{e.ip || '—'}</td></tr>)}</tbody></table></div>}
  </section>
}

/** Accès à l'administration : confirmer son mot de passe (valable 30 minutes). */
function Gate({ code, msg, onOk }: { code: string; msg: string; onOk: () => void }) {
  const { toast } = useApp()
  const [password, setPassword] = useState('')
  return <section className="card" style={{ maxWidth: 520 }}>
    <div className="row" style={{ gap: 10, marginBottom: 8 }}><Lock size={22} style={{ color: 'var(--to)' }} /><b style={{ fontWeight: 500, fontSize: 17 }}>{tr("Accès protégé")}</b></div>
    <p style={{ marginTop: 0 }}>{msg}</p>
    {code === 'sudo' && <form className="stack" onSubmit={async e => { e.preventDefault(); try { await api.post('/api/admin/unlock', { password }); onOk() } catch (er: any) { toast(er.message, true) } }}>
      <label className="label">{tr("Mot de passe")}<input className="field" type="password" required autoFocus autoComplete="current-password" value={password} onChange={e => setPassword(e.target.value)} /></label>
      <button className="btn pri"><Lock size={15} />{tr("Déverrouiller pour 30 minutes")}</button>
    </form>}
  </section>
}

export default function Admin() {
  const { me } = useApp()
  const [tab, setTab] = useState<'overview' | 'waitlist' | 'create' | 'users' | 'audit' | 'money' | 'legal'>(me?.admin_pending ? 'waitlist' : 'overview')
  const [gate, setGate] = useState<{ code: string; msg: string } | null | 'ok'>(null)
  const check = () => api.get('/api/admin/overview').then(() => setGate('ok')).catch((e: any) => setGate({ code: e.code ?? 'forbidden', msg: e.message }))
  useEffect(() => { if (me?.admin) check() }, [me?.admin])
  if (!me?.admin) return <main className="content"><PageHead title={tr("Administration")} help={tr("Réservé aux administrateurs de POP.")} /><section className="card"><Empty text={tr("Vous n’avez pas accès à cette page.")} /></section></main>
  return <main className="content">
    <PageHead title={tr("Administration")} help={tr("Gérer la plateforme POP : demandes d’accès à la bêta, comptes des bêta-testeurs et utilisateurs.")}>
      {gate === 'ok' && <button className="btn" onClick={async () => { await api.post('/api/admin/lock'); setGate(null); check() }}><Lock size={15} />{tr("Verrouiller")}</button>}
    </PageHead>
    {gate === null ? null : gate !== 'ok' ? <Gate code={gate.code} msg={gate.msg} onOk={() => { setGate(null); check() }} /> : <>
      <div className="tabs" role="tablist" style={{ alignSelf: 'flex-start' }}>
        {([['overview', tr("Vue générale")], ['waitlist', tr("Liste d’attente{0}", [me.admin_pending ? ` (${me.admin_pending})` : ''])], ['create', tr("Créer un compte")], ['users', tr("Utilisateurs")], ['audit', tr("Sécurité")], ['money', tr("Paiements et IA")], ['legal', tr("Infos légales")]] as const).map(([k, l]) => <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? 'on' : ''} onClick={() => setTab(k)}>{l}</button>)}
      </div>
      {tab === 'overview' && <Overview />}
      {tab === 'waitlist' && <Waitlist />}
      {tab === 'create' && <CreateAccount />}
      {tab === 'users' && <Users />}
      {tab === 'audit' && <Audit />}
      {tab === 'money' && <Money />}
      {tab === 'legal' && <LegalInfo />}
    </>}
  </main>
}

const LEGAL_FIELDS: [string, string, string][] = [
  ['editor_name', tr("Nom ou raison sociale de l’éditeur"), tr("Ex. POP SAS, ou votre nom si micro-entreprise")],
  ['editor_form', tr("Forme juridique"), tr("Ex. SAS, SASU, micro-entreprise")],
  ['capital', tr("Capital social"), tr("Ex. 1 000 € (rien pour une micro-entreprise)")],
  ['address', tr("Adresse du siège"), ''],
  ['siren', tr("SIREN / RCS"), tr("Ex. 123 456 789 RCS Paris")],
  ['vat', tr("TVA intracommunautaire"), tr("Ou « non applicable, art. 293 B du CGI »")],
  ['publisher', tr("Directeur de la publication"), tr("Le représentant légal, en général")],
  ['email', tr("E-mail de contact"), ''],
  ['phone', tr("Téléphone"), ''],
  ['host_name', tr("Hébergeur"), tr("Ex. OVH SAS, Scaleway SAS, Hetzner Online GmbH")],
  ['host_address', tr("Adresse de l’hébergeur"), ''],
  ['host_phone', tr("Téléphone de l’hébergeur"), ''],
  ['mediator_name', tr("Médiateur de la consommation"), tr("Obligatoire avant d’ouvrir les paiements aux particuliers")],
  ['mediator_url', tr("Site du médiateur"), ''],
  ['updated', tr("Date de mise à jour affichée"), tr("Ex. 17 novembre 2026")],
]

/** Informations de l'éditeur, reprises dans les mentions légales, la confidentialité, les CGU et les CGV. */
function LegalInfo() {
  const { toast } = useApp()
  const { data } = useLoad<any>('/api/legal')
  const [f, setF] = useState<Record<string, string> | null>(null)
  useEffect(() => { if (data && !f) setF(Object.fromEntries(LEGAL_FIELDS.map(([k]) => [k, data[k] ?? '']))) }, [data])
  if (!f) return null
  const missing = LEGAL_FIELDS.filter(([k]) => !['capital', 'phone', 'host_phone', 'updated', 'vat'].includes(k) && !f[k]).length
  return <section className="card">
    <CardHead title={tr("Informations légales")} sub={missing ? tr("{0} champ(s) importants encore vides : ils apparaissent « à compléter » sur les pages légales.", [missing]) : tr("Tout est rempli.")} />
    <form className="stack" onSubmit={async e => { e.preventDefault(); try { await api.put('/api/admin/legal', f); toast(tr("Informations légales enregistrées")) } catch (er: any) { toast(er.message, true) } }}>
      <div className="grid2">{LEGAL_FIELDS.map(([k, l, ph]) => <label key={k} className="label">{l}<input className="field" value={f[k]} placeholder={ph} maxLength={400} onChange={e => setF({ ...f, [k]: e.target.value })} /></label>)}</div>
      <div className="row" style={{ gap: 8 }}><button className="btn pri">{tr("Enregistrer")}</button><a className="btn" href="/mentions-legales" target="_blank" rel="noopener">{tr("Voir les pages légales")}</a></div>
    </form>
  </section>
}

/** Paiements et crédits IA : ce qui a été vendu, ce que Claude a coûté, la marge, et les crédits encore dus. */
function Money() {
  const { toast } = useApp()
  const { data } = useLoad<any>('/api/admin/ai')
  const { data: users } = useLoad<any[]>('/api/admin/users')
  const [uid, setUid] = useState(''); const [cr, setCr] = useState('')
  if (!data) return null
  const eur = (v: number) => v.toLocaleString(LOCALE, { style: 'currency', currency: 'EUR' })
  const Block = ({ t, r }: { t: string; r: any }) => <div className="privcell"><span className="small muted">{t}</span>
    <b>{eur(r.margin_eur)}</b><span className="small muted">{tr("Ventes {0} · coût Claude {1}", [eur(r.revenue_eur), eur(r.claude_cost_eur)])}</span>
    <span className="small muted">{tr("{0} crédits vendus · {1} utilisés", [n(r.credits_sold), n(r.credits_used)])}</span></div>
  return <>
    <section className="card">
      <CardHead title={tr("Crédits IA")} sub={data.configured ? tr("Modèle : {0}. Les crédits sont payés avant d’être utilisés : la marge ne peut pas être négative.", [data.model]) : tr("Clé Anthropic absente : l’IA de POP est désactivée.")} />
      <div className="privgrid">
        <Block t={tr("Marge ce mois-ci")} r={data.month} />
        <Block t={tr("Marge depuis le début")} r={data.all} />
        <div className="privcell"><span className="small muted">{tr("Crédits pas encore utilisés")}</span><b>{n(data.outstanding_credits)}</b><span className="small muted">{tr("Coût Claude maximum : {0}", [eur(data.outstanding_max_cost_eur)])}</span></div>
      </div>
      <p className="small muted" style={{ marginBottom: 0 }}>{tr("Réglages : {0} crédits par dollar, plafond {1} crédits par jour et par personne, {2} $ par jour pour tout POP.", [data.settings.credits_per_usd, n(data.settings.user_daily), data.settings.global_daily_usd])}</p>
    </section>
    <section className="card">
      <CardHead title={tr("Offrir ou retirer des crédits")} sub={tr("Geste commercial ou correction. C’est noté dans l’historique.")} />
      <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
        <select className="field" style={{ width: 220 }} aria-label={tr("Compte")} value={uid} onChange={e => setUid(e.target.value)}><option value="">{tr("Choisir un compte")}</option>{(users ?? []).map((u: any) => <option key={u.id} value={u.id}>{u.name}</option>)}</select>
        <input className="field" style={{ width: 210 }} inputMode="numeric" placeholder={tr("Crédits (ex. 100 ou -50)")} aria-label={tr("Crédits")} value={cr} onChange={e => setCr(e.target.value)} />
        <button className="btn pri" disabled={!+uid || !+cr} onClick={async () => { try { await api.post(`/api/admin/users/${+uid}/credits`, { credits: +cr }); toast(tr("Crédits mis à jour")); setCr('') } catch (e: any) { toast(e.message, true) } }}>{tr("Appliquer")}</button>
      </div>
    </section>
  </>
}
