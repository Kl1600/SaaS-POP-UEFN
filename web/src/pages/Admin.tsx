// Panel administrateur de la plateforme POP : liste d'attente, création des comptes bêta, utilisateurs, vue générale.
import { useEffect, useState } from 'react'
import { Check, X, RotateCcw, Trash2, Copy, Mail, Ban, UserPlus, Lock, KeyRound, Link2, LockOpen } from 'lucide-react'
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
/** Rôle d'un compte (#65) : choisi par un administrateur dans Utilisateurs. */
const ROLES: [string, string][] = [['user', "Utilisateur"], ['moderator', "Modérateur"], ['admin', "Admin"]]
const roleOf = (u: any) => u.is_admin ? 'admin' : u.is_moderator ? 'moderator' : 'user'
const ROLE_ASK: Record<string, string> = {
  user: "Remettre {0} en simple utilisateur ? Il perdra l’accès à l’administration.",
  moderator: "Rendre {0} modérateur ? Il pourra gérer la liste d’attente, débloquer les comptes et leur envoyer un lien de mot de passe.",
  admin: "Donner les droits admin à {0} ? Il pourra tout gérer : comptes, plans, paiements, liste d’attente.",
}

function PlanSelect({ value, onChange }: { value: string; onChange: (e: React.ChangeEvent<HTMLSelectElement>) => void }) {
  return <select className="field" value={value} onChange={onChange} title={tr("Analyse est comprise dans le plan Pro")}>{PLANS.map(([v, l]) => <option key={v} value={v}>{tr(l)}</option>)}</select>
}

function AccountForm({ init, waitlistId, onCreated, onCancel }: { init?: { name: string; email: string }; waitlistId?: number; onCreated: (s: Sent) => void; onCancel?: () => void }) {
  const { toast, me } = useApp()
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
      {me?.admin && <label className="label" style={{ flex: '1 1 160px' }}>{tr("Plan")}<PlanSelect value={f.plan} onChange={up('plan')} /></label>}
    </div>
    <div className="row" style={{ gap: 8 }}>
      {onCancel && <button type="button" className="btn" onClick={onCancel}>{tr("Annuler")}</button>}
      <button className="btn pri" disabled={busy}><UserPlus size={16} />{tr("Créer le compte")}</button>
    </div>
  </form>
}

/** Barres par jour (#68) : une barre par jour, le dernier jour en couleur ; une date sur quelques barres seulement. */
function Barres({ points, cle, titre, sous }: { points: any[]; cle: string; titre: string; sous: string }) {
  const max = Math.max(1, ...points.map(p => p[cle]))
  const total = points.reduce((s, p) => s + p[cle], 0)
  const pas = Math.ceil(points.length / 8)
  return <section className="card"><CardHead title={titre} sub={sous} />
    <div className="small muted" style={{ marginTop: -4 }}>{tr("Total sur la période : {0}", [n(total)])}</div>
    <div className="minibars" style={{ gap: points.length > 40 ? 1 : 4 }}>{points.map((d, i) => <div key={d.day} title={`${frDate(d.day)} : ${d[cle]}`}>
      <span style={{ height: `${(d[cle] / max) * 100}%` }} /><em style={{ visibility: (i % pas === 0 && points.length - 1 - i >= pas / 2) || i === points.length - 1 ? 'visible' : 'hidden' }}>{frDate(d.day).replace('.', '')}</em></div>)}</div>
  </section>
}

function Overview() {
  const [jours, setJours] = useState<30 | 90>(30)
  const { data } = useLoad<any>(`/api/admin/overview?days=${jours}`, [jours])
  if (!data) return null
  const s = data.stats
  const hist = s.plans_history as any[]
  const evol = (k: string) => hist.length > 1 ? hist[hist.length - 1][k] - hist[0][k] : null
  const signe = (v: number | null) => v === null ? '' : ' · ' + tr("{0} sur la période", [(v > 0 ? '+' : '') + v])
  return <>
    <div className="tiles">
      <div className="card"><div className="lab">{tr("Demandes en attente")}</div><div className="num">{n(data.waitlist.pending)}</div><div className="muted">{tr("{0} comptes créés", [n(data.waitlist.registered)])}</div></div>
      <div className="card"><div className="lab">{tr("Utilisateurs")}</div><div className="num">{n(data.users)}</div><div className="muted">{tr("+{0} cette semaine · {1} actifs sur 7 j", [n(data.users_7d), n(data.active_7d)])}</div></div>
      <div className="card"><div className="lab">{tr("Studios et maps")}</div><div className="num">{n(data.studios)}</div><div className="muted">{n(data.maps)} maps</div></div>
      <div className="card"><div className="lab">{tr("Tâches")}</div><div className="num">{n(data.tasks)}</div><div className="muted">{tr("{0} finies sur 7 j", [n(data.tasks_done_7d)])}</div></div>
    </div>
    <div className="row" style={{ justifyContent: 'space-between', flexWrap: 'wrap', gap: 10 }}>
      <b style={{ fontWeight: 500 }}>{tr("Activité par jour")}</b>
      <div className="tabs" role="tablist">{([30, 90] as const).map(j => <button key={j} role="tab" aria-selected={jours === j} className={jours === j ? 'on' : ''} onClick={() => setJours(j)}>{tr("{0} jours", [j])}</button>)}</div>
    </div>
    <div className="grid2">
      <Barres points={s.daily} cle="signups" titre={tr("Inscriptions")} sous={tr("Comptes créés par jour")} />
      <Barres points={s.daily} cle="active" titre={tr("Comptes actifs")} sous={tr("Connectés ou actifs (tâches, pages, UEFN) ce jour-là")} />
      <Barres points={s.daily} cle="requests" titre={tr("Demandes d’accès")} sous={tr("Liste d’attente, par jour")} />
      <section className="card"><CardHead title={tr("Abonnements")} sub={tr("Comptes par plan aujourd’hui, avec l’évolution depuis le début de la période suivie")} />
        <div className="list">
          <div className="li"><span style={{ flex: 1 }}>Starter</span><b style={{ fontWeight: 500 }}>{n(s.plans.starter)}</b><span className="small muted">{signe(evol('starter'))}</span></div>
          <div className="li"><span style={{ flex: 1 }}>Pro</span><b style={{ fontWeight: 500 }}>{n(s.plans.pro)}</b><span className="small muted">{signe(evol('pro'))}</span></div>
          <div className="li"><span style={{ flex: 1 }}>{tr("Essais Pro en cours")}</span><b style={{ fontWeight: 500 }}>{n(s.plans.trials)}</b><span className="small muted">{signe(evol('trials'))}</span></div>
        </div>
        {hist.length <= 1 && <p className="small muted" style={{ marginBottom: 0 }}>{tr("L’évolution apparaît à partir du deuxième jour de suivi.")}</p>}
      </section>
    </div>
    <div className="grid2">
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
  const { toast, reload: reloadMe, me } = useApp()
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
            {(w.status === 'refused' || w.status === 'approved') && <button className="iconbtn" data-tip={tr("Remettre en attente")} aria-label={tr("Remettre en attente")} onClick={() => act(() => api.post(`/api/admin/waitlist/${w.id}/reopen`), tr("Remise en attente"))}><RotateCcw size={16} /></button>}
            {me?.admin && <button className="iconbtn" data-tip={tr("Supprimer la demande")} aria-label={tr("Supprimer la demande")} onClick={() => { if (confirm(tr("Supprimer la demande de {0} ?", [w.name]))) act(() => api.del(`/api/admin/waitlist/${w.id}`), tr("Demande supprimée")) }}><Trash2 size={16} /></button>}
          </div>
        </div>
        {w.links && <p className="small" style={{ margin: '8px 0 0' }}><span className="muted">{tr("Maps / réseaux :")}{' '}</span>{w.links}</p>}
        {w.message && <p style={{ margin: '8px 0 0', whiteSpace: 'pre-wrap' }}>{w.message}</p>}
        <input className="field small" style={{ marginTop: 10, height: 32, width: '100%' }} placeholder={tr("Note interne (visible seulement par les admins et modérateurs)")} defaultValue={w.note}
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
  const { toast, me } = useApp()
  // Un modérateur (#65) voit les comptes et dépanne les comptes ordinaires : débloquer, lien de mot de passe.
  const admin = !!me?.admin
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
        <td>{!admin ? <span className="small">{u.plan === 'pro' ? 'Pro' : 'Starter'}</span> : <PlanSelect value={u.plan ?? 'starter'} onChange={async e => { try { await api.post(`/api/admin/users/${u.id}/plan`, { plan: e.target.value }); toast(tr("Plan mis à jour")); reload() } catch (er: any) { toast(er.message, true) } }} />}</td>
        <td><div className="row" style={{ gap: 4, flexWrap: 'wrap' }}>{admin && !u.me
          ? <select className="field" style={{ width: 'auto' }} aria-label={tr("Rôle de {0}", [u.name])} value={roleOf(u)} onChange={e => { const r = e.target.value; if (confirm(tr(ROLE_ASK[r], [u.name]))) set(u.id, { is_admin: r === 'admin', is_moderator: r === 'moderator' }, tr("Rôle mis à jour")); else e.target.value = roleOf(u) }}>{ROLES.map(([v, l]) => <option key={v} value={v}>{tr(l)}</option>)}</select>
          : u.is_admin ? <span className="badge b-v">{tr("Admin")}</span> : u.is_moderator ? <span className="badge b-g">{tr("Modérateur")}</span> : <span className="badge">{tr("Utilisateur")}</span>}
          {u.disabled && <span className="badge b-r">{tr("Désactivé")}</span>}
          {u.locked_until && <span className="badge b-o" title={tr("Trop de mots de passe faux")}>{tr("Bloqué jusqu’à {0}", [hhmm(u.locked_until)])}</span>}</div></td>
        <td>{!u.me && (admin || !(u.is_admin || u.is_moderator)) && <div className="row" style={{ gap: 4, justifyContent: 'flex-end' }}>
          {admin && u.activation_pending && <button className="btn" style={{ height: 32 }} title={tr("Nouveau lien d’activation : l’ancien ne marchera plus")} onClick={async () => { try { const r = await api.post(`/api/admin/users/${u.id}/activation`); setSent({ ...r, username: u.username }); toast(r.mailed ? tr("Lien renvoyé par e-mail") : tr("Lien créé : copiez-le")); reload() } catch (e: any) { toast(e.message, true) } }}><Mail size={15} />{tr("Renvoyer le lien")}</button>}
          {u.locked_until && <button className="iconbtn" data-tip={tr("Débloquer")} aria-label={tr("Débloquer")} onClick={() => run(() => api.post(`/api/admin/users/${u.id}/unlock`), tr("Compte débloqué"))}><LockOpen size={16} /></button>}
          <button className="iconbtn" data-tip={tr("Lien de nouveau mot de passe")} aria-label={tr("Lien de nouveau mot de passe")} onClick={async () => { try { const r = await api.post(`/api/admin/users/${u.id}/reset-link`); setLink(r); toast(r.mailed ? tr("Lien envoyé par e-mail") : tr("Lien créé : copiez-le")) } catch (e: any) { toast(e.message, true) } }}><Link2 size={16} /></button>
          {admin && !u.activation_pending && <button className="iconbtn" data-tip={tr("Nouveau mot de passe provisoire")} aria-label={tr("Nouveau mot de passe provisoire")} onClick={async () => { if (!confirm(tr("Donner un nouveau mot de passe provisoire à {0} ? Son mot de passe actuel ne marchera plus et il sera déconnecté.", [u.name]))) return; try { const r = await api.post(`/api/admin/users/${u.id}/password`); setCred({ ...r, name: u.name }); reload() } catch (e: any) { toast(e.message, true) } }}><KeyRound size={16} /></button>}
          {admin && <>
          <button className="iconbtn" data-tip={u.disabled ? tr("Réactiver") : tr("Désactiver le compte")} aria-label={u.disabled ? tr("Réactiver") : tr("Désactiver le compte")} onClick={() => { if (confirm(u.disabled ? tr("Réactiver le compte de {0} ?", [u.name]) : tr("Désactiver le compte de {0} ? Il sera déconnecté et ne pourra plus se connecter.", [u.name]))) set(u.id, { disabled: !u.disabled }, u.disabled ? tr("Compte réactivé") : tr("Compte désactivé")) }}><Ban size={16} /></button>
          <button className="iconbtn" data-tip={tr("Supprimer le compte")} aria-label={tr("Supprimer le compte")} onClick={() => setDel(u)}><Trash2 size={16} /></button>
          </>}
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

/** Journal des actions des admins et modérateurs (#66) : qui a fait quoi, sur quel compte, quand ; filtres et pages. */
function Journal() {
  const [who, setWho] = useState('')
  const [type, setType] = useState('')
  const [page, setPage] = useState(1)
  const { data } = useLoad<any>(`/api/admin/journal?page=${page}${who ? `&who=${who}` : ''}${type ? `&type=${type}` : ''}`, [who, type, page])
  const heure = (at: string) => new Date(at.replace(' ', 'T') + 'Z').toLocaleString(LOCALE, { dateStyle: 'short', timeStyle: 'short' })
  return <section className="card"><CardHead title={tr("Journal des actions")} sub={tr("Ce que les administrateurs et modérateurs ont fait : liste d’attente, comptes, annonces. Le plus récent en haut.")} />
    <div className="row" style={{ flexWrap: 'wrap', gap: 10, marginBottom: 12 }}>
      <label className="label" style={{ minWidth: 200 }}>{tr("Auteur")}<select className="field" value={who} onChange={e => { setWho(e.target.value); setPage(1) }}>
        <option value="">{tr("Tout le monde")}</option>{data?.authors.map((a: any) => <option key={a.id} value={a.id}>{a.name}</option>)}</select></label>
      <label className="label" style={{ minWidth: 200 }}>{tr("Type d’action")}<select className="field" value={type} onChange={e => { setType(e.target.value); setPage(1) }}>
        <option value="">{tr("Toutes")}</option>{data?.types.map((t: any) => <option key={t.key} value={t.key}>{tr(t.label)}</option>)}</select></label>
    </div>
    {!data ? null : data.rows.length === 0 ? <Empty text={tr("Aucune action pour ce choix.")} /> :
      <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>{tr("Date et heure")}</th><th>{tr("Auteur")}</th><th>{tr("Action")}</th><th>{tr("Compte ou demande visé")}</th></tr></thead>
        <tbody>{data.rows.map((r: any) => <tr key={r.id}><td className="small" style={{ whiteSpace: 'nowrap' }}>{heure(r.at)}</td><td>{r.who ?? '—'}</td><td>{tr(r.label)}</td><td className="small">{r.target || '—'}</td></tr>)}</tbody></table></div>}
    {data && (page > 1 || data.more) && <div className="row" style={{ justifyContent: 'flex-end', marginTop: 10 }}>
      <button className="btn" disabled={page <= 1} onClick={() => setPage(p => p - 1)}>{tr("Plus récentes")}</button>
      <span className="small muted">{tr("Page {0}", [page])}</span>
      <button className="btn" disabled={!data.more} onClick={() => setPage(p => p + 1)}>{tr("Plus anciennes")}</button>
    </div>}
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
  // Modérateur (#65) : seulement la liste d'attente et les utilisateurs.
  const staff = !!(me?.admin || me?.moderator)
  const mod = staff && !me?.admin
  const [tab, setTab] = useState<'overview' | 'waitlist' | 'create' | 'users' | 'ia' | 'audit' | 'journal' | 'money' | 'legal' | 'announce'>(mod || me?.admin_pending ? 'waitlist' : 'overview')
  const [gate, setGate] = useState<{ code: string; msg: string } | null | 'ok'>(null)
  const check = () => api.get(mod ? '/api/admin/waitlist?status=pending' : '/api/admin/overview').then(() => setGate('ok')).catch((e: any) => setGate({ code: e.code ?? 'forbidden', msg: e.message }))
  useEffect(() => { if (staff) check() }, [staff])
  if (!me || !staff) return <main className="content"><PageHead title={tr("Administration")} help={tr("Réservé aux administrateurs de POP.")} /><section className="card"><Empty text={tr("Vous n’avez pas accès à cette page.")} /></section></main>
  return <main className="content">
    <PageHead title={tr("Administration")} help={tr("Gérer la plateforme POP : demandes d’accès à la bêta, comptes des bêta-testeurs et utilisateurs.")}>
      {gate === 'ok' && <button className="btn" onClick={async () => { await api.post('/api/admin/lock'); setGate(null); check() }}><Lock size={15} />{tr("Verrouiller")}</button>}
    </PageHead>
    {gate === null ? null : gate !== 'ok' ? <Gate code={gate.code} msg={gate.msg} onOk={() => { setGate(null); check() }} /> : <>
      <div className="tabs" role="tablist" style={{ alignSelf: 'flex-start' }}>
        {([['overview', tr("Vue générale")], ['waitlist', tr("Liste d’attente{0}", [me.admin_pending ? ` (${me.admin_pending})` : ''])], ['create', tr("Créer un compte")], ['users', tr("Utilisateurs")], ['ia', tr("Apprentissage de l’IA")], ['audit', tr("Sécurité")], ['journal', tr("Journal")], ['money', tr("Paiements et IA")], ['legal', tr("Infos légales")], ['announce', tr("Annonce")]] as const).filter(([k]) => !mod || k === 'waitlist' || k === 'users').map(([k, l]) => <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? 'on' : ''} onClick={() => setTab(k)}>{l}</button>)}
      </div>
      {tab === 'overview' && <Overview />}
      {tab === 'waitlist' && <Waitlist />}
      {tab === 'create' && <CreateAccount />}
      {tab === 'users' && <Users />}
      {tab === 'ia' && <Learning />}
      {tab === 'audit' && <Audit />}
      {tab === 'journal' && <Journal />}
      {tab === 'money' && <Money />}
      {tab === 'legal' && <LegalInfo />}
      {tab === 'announce' && <AnnounceAdmin />}
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

/** Annonce en bandeau pour tous les comptes connectés (#67). Une seule à la fois : publier remplace l'ancienne. */
function AnnounceAdmin() {
  const { toast } = useApp()
  const { data, reload } = useLoad<any>('/api/admin/announcement')
  const [text, setText] = useState(''); const [level, setLevel] = useState('info'); const [end, setEnd] = useState('')
  if (!data) return null
  const a = data.announcement
  return <section className="card">
    <CardHead title={tr("Annonce")} sub={tr("Un bandeau en haut de POP pour tous les comptes connectés, au prochain chargement de page. Chacun peut le fermer.")} />
    {a ? <div className="stack" style={{ marginBottom: 20 }}>
      <div className={'warn announce' + (a.level === 'important' ? ' important' : '')}><span style={{ flex: 1, overflowWrap: 'anywhere' }}>{a.text}</span></div>
      <span className="small muted">{tr("Publiée {0}", [ago(a.created_at)])}{a.ends_at ? ' · ' + tr("jusqu’au {0}", [new Date(a.ends_at.replace(' ', 'T') + 'Z').toLocaleString(LOCALE)]) : ''} · {tr("fermée par {0} compte(s)", [a.closed])}</span>
      <div><button className="btn" onClick={async () => { try { await api.del('/api/admin/announcement'); toast(tr("Annonce retirée")); reload() } catch (er: any) { toast(er.message, true) } }}>{tr("Retirer l’annonce")}</button></div>
    </div> : <p className="small muted" style={{ marginBottom: 20 }}>{tr("Aucune annonce en cours.")}</p>}
    <form className="stack" onSubmit={async e => {
      e.preventDefault()
      try { await api.post('/api/admin/announcement', { text, level, ends_at: end ? new Date(end).toISOString() : null }); toast(tr("Annonce publiée")); setText(''); setEnd(''); reload() } catch (er: any) { toast(er.message, true) }
    }}>
      <label className="label">{a ? tr("Nouvelle annonce (remplace celle en cours)") : tr("Nouvelle annonce")}<textarea className="field" rows={2} maxLength={300} required value={text} onChange={e => setText(e.target.value)} placeholder={tr("Ex. : maintenance ce soir de 22 h à 23 h, POP sera indisponible.")} /></label>
      <div className="grid2">
        <label className="label">{tr("Niveau")}<select className="field" value={level} onChange={e => setLevel(e.target.value)}><option value="info">{tr("Information")}</option><option value="important">{tr("Important")}</option></select></label>
        <label className="label">{tr("Fin (facultatif)")}<input className="field" type="datetime-local" value={end} onChange={e => setEnd(e.target.value)} /></label>
      </div>
      <div><button className="btn pri">{tr("Publier")}</button></div>
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

const pct = (v?: number | null) => v == null ? '—' : `${Math.round(v * 100)} %`

/** Apprentissage de l'IA (#70) : versions du modèle « chance de percer », précision mesurée sur des maps qu'il n'a
 * jamais vues, et prédictions vérifiées après les 14 jours de test d'Epic. */
function Learning() {
  const { data } = useLoad<any>('/api/admin/ia')
  if (!data) return null
  const models = [...data.models].reverse()
  const last = data.models[0]
  const W = 640, H = 160, P = 28
  const x = (i: number) => P + (models.length < 2 ? (W - 2 * P) / 2 : (i * (W - 2 * P)) / (models.length - 1))
  const yv = (v: number) => H - P - (Math.max(0.5, Math.min(1, v)) - 0.5) * 2 * (H - 2 * P)
  return <>
    <div className="tiles">
      <div className="card"><div className="lab">{tr("Précision sur l’avenir")}</div><div className="num">{last ? pct(last.auc) : '—'}</div><div className="muted">{tr("50 % = au hasard, 100 % = parfait")}</div></div>
      <div className="card"><div className="lab">{tr("Maps apprises")}</div><div className="num">{last ? n(last.n) : 0}</div><div className="muted">{last ? tr("dont {0} qui ont percé", [n(last.positives)]) : tr("pas encore assez")}</div></div>
      <div className="card"><div className="lab">{tr("Prédictions gardées")}</div><div className="num">{n(data.predictions)}</div><div className="muted">{tr("{0} déjà vérifiées", [n(data.graded)])}</div></div>
      <div className="card"><div className="lab">{tr("Dernier entraînement")}</div><div className="num" style={{ fontSize: 22 }}>{last ? ago(last.at) : '—'}</div><div className="muted">{tr("chaque jour, automatiquement")}</div></div>
    </div>
    <section className="card">
      <CardHead title={tr("Précision jour après jour")} sub={tr("Chaque jour, l’IA apprend sur les maps les plus anciennes et se teste sur les plus récentes, qu’elle n’a jamais vues. « Percer » = dépasser {0} joueurs par jour pendant les 14 jours de test d’Epic.", [n(data.threshold)])} />
      {models.length === 0 ? <Empty text={tr("Pas encore de version entraînée : il faut au moins 40 nouvelles maps sorties depuis plus de 15 jours, dont 5 qui ont percé.")} /> :
        <svg viewBox={`0 0 ${W} ${H}`} style={{ width: '100%', maxWidth: W, height: 'auto' }} role="img" aria-label={tr("Précision de l’IA à chaque entraînement")}>
          {[0.5, 0.75, 1].map(v => <g key={v}><line x1={P} x2={W - P} y1={yv(v)} y2={yv(v)} stroke="var(--border)" /><text x={2} y={yv(v) + 4} fontSize="11" fill="var(--text2)">{Math.round(v * 100)}</text></g>)}
          <polyline fill="none" stroke="var(--text)" strokeWidth="2" points={models.map((m: any, i: number) => `${x(i)},${yv(m.auc ?? 0.5)}`).join(' ')} />
          {models.map((m: any, i: number) => <circle key={m.id} cx={x(i)} cy={yv(m.auc ?? 0.5)} r="3.5" fill="var(--text)"><title>{`${frDate(m.at)} : ${pct(m.auc)} (${m.n} maps)`}</title></circle>)}
          {models.length > 0 && <><text x={P} y={H - 6} fontSize="11" fill="var(--text2)">{frDate(models[0].at)}</text><text x={W - P} y={H - 6} fontSize="11" fill="var(--text2)" textAnchor="end">{frDate(models[models.length - 1].at)}</text></>}
        </svg>}
    </section>
    <div className="grid2">
      <section className="card"><CardHead title={tr("Prédictions vérifiées, par semaine")} sub={tr("Chance annoncée au 3e jour de la map, comparée à ce qui s’est passé.")} />
        {data.weeks.length === 0 ? <p className="small muted">{tr("Les premières prédictions seront vérifiées 15 jours après leur enregistrement.")}</p> :
          <table className="tbl"><thead><tr><th>{tr("Semaine")}</th><th>{tr("Maps")}</th><th>{tr("Bonnes réponses")}</th><th>{tr("Précision")}</th></tr></thead>
            <tbody>{[...data.weeks].reverse().map((w: any) => <tr key={w.week}><td>{w.week.replace('-', ' · S')}</td><td>{w.n}</td><td>{pct(w.accuracy)}</td><td>{pct(w.auc)}</td></tr>)}</tbody></table>}
      </section>
      <section className="card"><CardHead title={tr("Dernières prédictions vérifiées")} />
        {data.recent.length === 0 ? <p className="small muted">{tr("Rien pour l’instant.")}</p> :
          <div className="list">{data.recent.map((r: any) => <div key={r.code} className="li">
            <span style={{ flex: 1, minWidth: 0 }}><b style={{ fontWeight: 500 }}>{r.title || r.code}</b><span className="small muted">{' '}{tr("annoncée {0}, max {1} joueurs", [pct(r.prob), n(r.best ?? 0)])}</span></span>
            {r.outcome == null ? <span className="badge">{tr("Non mesurée")}</span> : (r.prob >= 0.5) === (r.outcome === 1) ? <span className="badge b-g">{tr("Juste")}</span> : <span className="badge b-o">{tr("Fausse")}</span>}
          </div>)}</div>}
      </section>
    </div>
  </>
}
