import { useEffect, useState } from 'react'
import { api } from '../api'
import { useApp } from '../state'
import { WaitlistPage } from '../components/Waitlist'
import { BlockView } from './Pages'
import { tr } from '../i18n/t'
import { LegalLinks } from './Legal'
import LangPicker from '../components/LangPicker'

export function Login({ invite, initial, access }: { invite?: string; initial?: 'login' | 'register'; access?: { token: string; email: string; name: string } }) {
  const { reload } = useApp()
  const [mode, setMode] = useState<'login' | 'register' | 'forgot'>(invite ? 'register' : initial ?? 'login')
  const [sent, setSent] = useState('')
  const [f, setF] = useState({ email: access?.email ?? '', name: access?.name ?? '', password: '' })
  const [err, setErr] = useState('')
  const [terms, setTerms] = useState(false)
  const [busy, setBusy] = useState(false)
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); setBusy(true); setErr('')
    try {
      if (mode === 'forgot') {
        const r = await api.post('/api/auth/forgot', { email: f.email })
        setSent(r.mail ? tr("Si un compte existe avec cette adresse, un e-mail avec un lien vient de partir (valable 2 heures).") : tr("Pendant la bêta, écrivez à l’équipe POP : elle vous donnera un nouveau mot de passe provisoire."))
        return
      }
      if (mode === 'login') await api.post('/api/auth/login', { login: f.email.trim(), password: f.password })
      else await api.post('/api/auth/register', { ...f, invite, access: access?.token, accept_terms: terms })
      if (invite && mode === 'login') await api.post('/api/invitations/accept', { token: invite })
      await reload()
      if (invite || access) location.href = '/'
    } catch (e: any) { setErr(e.message) } finally { setBusy(false) }
  }
  return (
    <div className="auth app">
      <form className="card stack" onSubmit={submit} style={{ gap: 16 }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)' }} />
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>{mode === 'login' ? tr("Connexion") : mode === 'forgot' ? tr("Mot de passe oublié") : tr("Créer un compte")}</h1>
        <p className="muted" style={{ margin: 0 }}>{invite ? tr("Vous avez été invité dans un studio.") : access && mode === 'register' ? tr("Votre accès à la bêta est validé : créez votre compte.") : tr("L'espace de travail des créateurs de maps UEFN.")}</p>
        {mode === 'register' && <label className="label">{tr("Nom")}<input className="field" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} required maxLength={80} autoComplete="name" /></label>}
        <label className="label">{mode === 'login' ? tr("Identifiant") : 'E-mail'}<input className="field" type={mode === 'login' ? 'text' : 'email'} inputMode={mode === 'login' ? 'text' : 'email'} autoCapitalize="off" autoCorrect="off" spellCheck={false} value={f.email} onChange={e => setF({ ...f, email: e.target.value })} required autoComplete={mode === 'login' ? 'username' : 'email'} readOnly={!!access && mode === 'register'} />{mode === 'login' && <span className="small muted" style={{ fontWeight: 400 }}>{tr("Celui donné par l’équipe POP (ou votre e-mail).")}</span>}</label>
        {mode !== 'forgot' && <label className="label">{tr("Mot de passe")}{mode === 'register' && <span className="small muted">{' '}{tr("(10 caractères minimum, pas un mot courant)")}</span>}<input className="field" type="password" value={f.password} onChange={e => setF({ ...f, password: e.target.value })} required minLength={mode === 'register' ? 10 : 1} autoComplete={mode === 'login' ? 'current-password' : 'new-password'} /></label>}
        {mode === 'register' && <Terms checked={terms} onChange={setTerms} />}
        {sent && <div className="b-callout small">{sent}</div>}
        {err && <div className="err" role="alert">{err}</div>}
        <button className="btn pri" disabled={busy}>{mode === 'login' ? tr("Se connecter") : mode === 'forgot' ? tr("Recevoir un lien") : tr("Créer mon compte")}</button>
        {mode === 'login' && <button type="button" className="link" onClick={() => { setMode('forgot'); setErr('') }}>{tr("Mot de passe oublié ?")}</button>}
        {mode === 'login' && !invite && !access
          ? <a className="link" href="/inscription">{tr("Pas encore de compte ? Rejoindre la liste d’attente")}</a>
          : <button type="button" className="link" onClick={() => { setMode(mode === 'login' ? 'register' : 'login'); setSent('') }}>
            {mode === 'login' ? tr("Pas encore de compte ? Créer un compte") : tr("J'ai déjà un compte")}
          </button>}
      </form>
      <div className="row" style={{ marginTop: 16, gap: 14, justifyContent: 'center', alignItems: 'center' }}>
        {!invite && <a href="/" className="small" style={{ color: 'rgba(255,255,255,.6)' }}>{tr("← Retour au site")}</a>}
        <span className="authlang"><LangPicker app /></span>
      </div>
      <LegalLinks className="auth-legal" />
    </div>
  )
}

/** Première connexion d'un compte créé par l'administrateur : remplacer le mot de passe provisoire. */
/** Acceptation des CGU et de la politique de confidentialité (obligatoire pour créer un compte). */
function Terms({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return <label className="terms small">
    <input type="checkbox" checked={checked} onChange={e => onChange(e.target.checked)} required />
    <span>{tr("J’accepte les")}{' '}<a href="/cgu" target="_blank" rel="noopener">{tr("conditions d’utilisation")}</a>{' '}{tr("et la")}{' '}<a href="/confidentialite" target="_blank" rel="noopener">{tr("politique de confidentialité")}</a>. {tr("J’ai au moins 15 ans, ou entre 13 et 15 ans avec l’accord d’un parent.")}</span>
  </label>
}

export function FirstPassword() {
  const { me, reload } = useApp()
  const [f, setF] = useState({ a: '', b: '' })
  const [terms, setTerms] = useState(false)
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); setErr('')
    if (f.a !== f.b) { setErr(tr("Les deux mots de passe ne sont pas identiques.")); return }
    setBusy(true)
    try { await api.post('/api/me/first-password', { new: f.a, accept_terms: terms }); await reload() } catch (er: any) { setErr(er.message) } finally { setBusy(false) }
  }
  return (
    <div className="auth app">
      <form className="card stack" onSubmit={submit} style={{ gap: 16 }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)' }} />
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>{tr("Bienvenue{0} !", [me ? `, ${me.user.name.split(' ')[0]}` : ''])}</h1>
        <p className="muted" style={{ margin: 0 }}>{tr("Votre compte a été créé par l’équipe POP avec un mot de passe provisoire. Choisissez le vôtre pour continuer.")}</p>
        {me?.username && <div className="small muted">{tr("Votre identifiant :")}{' '}<b className="mono" style={{ fontWeight: 500, color: 'var(--t1, inherit)' }}>{me.username}</b></div>}
        <input type="text" name="username" autoComplete="username" value={me?.username ?? me?.user.email ?? ''} readOnly hidden />
        <label className="label">{tr("Nouveau mot de passe")}<span className="small muted">{' '}{tr("(10 caractères minimum, pas un mot courant)")}</span><input className="field" type="password" autoFocus value={f.a} onChange={e => setF({ ...f, a: e.target.value })} required minLength={10} autoComplete="new-password" /></label>
        <label className="label">{tr("Encore une fois")}<input className="field" type="password" value={f.b} onChange={e => setF({ ...f, b: e.target.value })} required minLength={10} autoComplete="new-password" /></label>
        <Terms checked={terms} onChange={setTerms} />
        {err && <div className="err" role="alert">{err}</div>}
        <button className="btn pri" disabled={busy}>{tr("Enregistrer et continuer")}</button>
        <button type="button" className="link" onClick={async () => { await api.post('/api/auth/logout'); location.href = '/connexion' }}>{tr("Me déconnecter")}</button>
      </form>
      <LegalLinks className="auth-legal" />
    </div>
  )
}

export function Onboarding() {
  const { reload } = useApp()
  const [f, setF] = useState({ name: '', map_name: '', genre: '', release_date: '' })
  const [err, setErr] = useState('')
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); setErr('')
    try { await api.post('/api/studios', f); await reload() } catch (e: any) { setErr(e.message) }
  }
  return (
    <div className="auth app">
      <form className="card stack" onSubmit={submit} style={{ gap: 16 }}>
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>{tr("Créer votre studio")}</h1>
        <p className="muted" style={{ margin: 0 }}>{tr("Un studio regroupe votre équipe et vos maps.")}</p>
        <label className="label">{tr("Nom du studio")}<input className="field" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} required /></label>
        <label className="label">{tr("Première map")}<input className="field" value={f.map_name} onChange={e => setF({ ...f, map_name: e.target.value })} required /></label>
        <label className="label">{tr("Genre")}<input className="field" value={f.genre} onChange={e => setF({ ...f, genre: e.target.value })} placeholder={tr("Tycoon, box fight…")} /></label>
        <label className="label">{tr("Date de sortie visée")}<input className="field" type="date" value={f.release_date} onChange={e => setF({ ...f, release_date: e.target.value })} /></label>
        {err && <div className="err">{err}</div>}
        <button className="btn pri">{tr("Créer")}</button>
      </form>
    </div>
  )
}

export function InviteAccept({ token }: { token: string }) {
  const { me, loading, reload } = useApp()
  const [msg, setMsg] = useState('')
  useEffect(() => {
    if (me) api.post('/api/invitations/accept', { token }).then(() => reload().then(() => (location.href = '/'))).catch(e => setMsg(e.message))
  }, [me])
  if (loading) return <div className="auth" />
  if (!me) return <Login invite={token} />
  return <div className="auth app"><div className="card">{msg || tr("Invitation acceptée…")}</div></div>
}

export function PublicPage({ token }: { token: string }) {
  const [p, setP] = useState<any>(null)
  const [err, setErr] = useState('')
  useEffect(() => { api.get(`/api/public/pages/${token}`).then(setP).catch(e => setErr(e.message)) }, [token])
  return (
    <div className="app" style={{ display: 'block', background: 'var(--bg)', minHeight: '100vh' }}>
      <div style={{ maxWidth: 760, margin: tr("0 auto"), padding: 48 }}>
        {err && <p className="err">{err}</p>}
        {p && <>
          <p className="small muted">{tr("Page partagée en lecture · POP")}</p>
          <h1 style={{ fontSize: 34, fontWeight: 600 }}>{p.title}</h1>
          {p.blocks.map((b: any) => <BlockView key={b.id} b={b} />)}
        </>}
      </div>
    </div>
  )
}

export function ResetPassword({ token }: { token: string }) {
  const [pw, setPw] = useState('')
  const [msg, setMsg] = useState('')
  const [ok, setOk] = useState(false)
  const [revoked, setRevoked] = useState(0)
  return (
    <div className="auth app">
      <form className="card stack" style={{ gap: 16 }} onSubmit={async e => { e.preventDefault(); try { const r: any = await api.post('/api/auth/reset', { token, password: pw }); setOk(true); setRevoked(r?.tokens_revoked ?? 0) } catch (er: any) { setMsg(er.message) } }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)' }} />
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>{tr("Nouveau mot de passe")}</h1>
        {ok ? <><p>{tr("Mot de passe changé. Vous pouvez vous connecter.")}{revoked > 0 && <> {tr("Vos {0} jeton(s) d’IA connectée ont été révoqués : recréez-les dans « IA connectées ».", [revoked])}</>}</p><a className="btn pri" href="/">{tr("Se connecter")}</a></> : <>
          <label className="label">{tr("Nouveau mot de passe (10 caractères minimum)")}<input className="field" type="password" value={pw} onChange={e => setPw(e.target.value)} minLength={10} required autoComplete="new-password" /></label>
          {msg && <div className="err" role="alert">{msg}</div>}
          <button className="btn pri">{tr("Enregistrer")}</button>
        </>}
      </form>
    </div>
  )
}

/** /verifier-email/:token : lien reçu par e-mail pour confirmer l'adresse (#11). */
export function VerifyEmail({ token }: { token: string }) {
  const [state, setState] = useState<{ ok?: boolean; email?: string; msg?: string }>({})
  useEffect(() => {
    let done = false
    api.post('/api/auth/verify-email', { token })
      .then((r: any) => { if (!done) setState({ ok: true, email: r.email }) })
      .catch((er: any) => { if (!done) setState({ ok: false, msg: er.message }) })
    return () => { done = true }
  }, [token])
  return (
    <div className="auth app">
      <div className="card stack" style={{ gap: 16 }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)' }} />
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>Adresse e-mail</h1>
        {state.ok === undefined && <p className="muted">Vérification en cours…</p>}
        {state.ok === true && <p>C’est confirmé : <b>{state.email}</b> est bien votre adresse.</p>}
        {state.ok === false && <div className="err" role="alert">{state.msg}</div>}
        {state.ok !== undefined && <a className="btn pri" href="/">Ouvrir POP</a>}
      </div>
    </div>
  )
}

/** /activer?token=… : compte créé par l'administrateur, la personne choisit son mot de passe (#42). */
export function Activate() {
  const token = new URLSearchParams(location.search).get('token') ?? ''
  const [acc, setAcc] = useState<{ username?: string; email: string; name: string } | null>(null)
  const [err, setErr] = useState('')
  const [pw, setPw] = useState('')
  const [terms, setTerms] = useState(false)
  const [busy, setBusy] = useState(false)
  useEffect(() => { api.get(`/api/activation?token=${encodeURIComponent(token)}`).then(setAcc).catch((e: any) => setErr(e.message)) }, [token])
  return (
    <div className="auth app">
      <form className="card stack" style={{ gap: 16 }} onSubmit={async e => {
        e.preventDefault(); setBusy(true)
        try { await api.post('/api/activation', { token, password: pw, accept_terms: terms }); location.href = '/' } catch (er: any) { setErr(er.message); setBusy(false) }
      }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)' }} />
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>{tr("Activer votre compte")}</h1>
        {!acc && !err && <p className="muted">{tr("Vérification du lien…")}</p>}
        {acc && <>
          <p className="muted" style={{ margin: 0 }}>{tr("Bonjour {0}, choisissez votre mot de passe pour ouvrir POP.", [acc.name.split(' ')[0]])}</p>
          <div className="b-callout small" style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 14px' }}>
            <span className="muted">{tr("Identifiant")}</span><span className="mono">{acc.username ?? acc.email}</span>
            <span className="muted">E-mail</span><span>{acc.email}</span>
          </div>
          <label className="label">{tr("Mot de passe (10 caractères minimum)")}<input className="field" type="password" value={pw} onChange={e => setPw(e.target.value)} minLength={10} required autoFocus autoComplete="new-password" /></label>
          <Terms checked={terms} onChange={setTerms} />
        </>}
        {err && <div className="err" role="alert">{err}</div>}
        {acc && <button className="btn pri" disabled={busy}>{tr("Activer et ouvrir POP")}</button>}
        {!acc && err && <a className="btn" href="/">{tr("Ouvrir POP")}</a>}
      </form>
    </div>
  )
}

/** Premier écran : relier UEFN pour importer ses projets (une map par projet), ou commencer sans. */
export function Welcome() {
  const { me, reload } = useApp()
  const studio = me!.studios[0]
  const [dl, setDl] = useState(false)
  const [name, setName] = useState('')
  const [err, setErr] = useState('')
  useEffect(() => {
    const t = setInterval(async () => {
      try { const r = await api.get(`/api/studios/${studio.id}/link-status`); if (r.maps.length) { clearInterval(t); await reload() } } catch {}
    }, 2500)
    return () => clearInterval(t)
  }, [studio.id])
  return (
    <div className="auth app">
      <div className="card stack" style={{ gap: 16, width: 'min(620px, 100%)' }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)', width: 90 }} />
        <h1 style={{ margin: 0, fontSize: 26, fontWeight: 400 }}>{tr("Bienvenue, {0}", [me!.user.name.split(' ')[0]])}</h1>
        <p className="muted" style={{ margin: 0 }}>{tr("Reliez UEFN : POP importe vos projets (une map par projet), leurs fichiers Verse et leurs acteurs, et les garde synchronisés.")}</p>
        <ol className="welcome-steps">
          <li><b>{tr("Sur le PC où UEFN est installé")}</b>{tr(", ouvrez un projet puis : Projet → Paramètres du projet →")}{' '}<b>{tr("Enable Project Python Scripts")}</b>{tr(", et Édition → Préférences de l’éditeur → Model Context Protocol →")}{' '}<b>{tr("Start Server")}</b>.</li>
          <li><a className="btn pri" href={`/api/studios/${studio.id}/relay/download`} onClick={() => setDl(true)} download>{tr("Télécharger UEFN Link (Windows)")}</a><div className="small muted" style={{ marginTop: 6 }}>{tr("Ce fichier est fait pour votre compte : rien à configurer. Valable 24 h. Ouvrez cette page sur le PC pour le télécharger directement dessus.")}</div></li>
          <li><b>{tr("Double-cliquez sur pop-link.exe.")}</b>{' '}{tr("Si Windows affiche « Windows a protégé votre ordinateur » : Informations complémentaires → Exécuter quand même. Laissez la fenêtre ouverte.")}</li>
        </ol>
        <div className="b-callout small">{dl ? tr("En attente du PC… cette page s’ouvrira toute seule dès que vos projets seront importés.") : tr("Cette page se mettra à jour toute seule dès que vos projets seront importés.")}</div>
        <details>
          <summary className="small" style={{ cursor: 'pointer' }}>{tr("Commencer sans UEFN")}</summary>
          <form className="row" style={{ marginTop: 10 }} onSubmit={async e => { e.preventDefault(); try { await api.post(`/api/studios/${studio.id}/skip-uefn`, { name }); await reload() } catch (er: any) { setErr(er.message) } }}>
            <input className="field" style={{ flex: 1 }} placeholder={tr("Nom de la map (ex. VaultRush)")} value={name} onChange={e => setName(e.target.value)} />
            <button className="btn">{tr("Continuer")}</button>
          </form>
          {err && <div className="err">{err}</div>}
        </details>
        <button className="link small" style={{ alignSelf: 'flex-start' }} onClick={async () => { await api.post('/api/auth/logout'); location.href = '/' }}>{tr("Se déconnecter")}</button>
      </div>
    </div>
  )
}

/** /inscription : avec un lien d'accès validé → création du compte ; sinon → demande de liste d'attente. */
export function Signup() {
  const token = new URLSearchParams(location.search).get('acces')
  const [state, setState] = useState<{ email: string; name: string } | null | 'bad'>(null)
  useEffect(() => { if (token) api.get(`/api/waitlist/access?token=${encodeURIComponent(token)}`).then(setState).catch(() => setState('bad')) }, [token])
  if (token && state && state !== 'bad') return <Login initial="register" access={{ token, ...state }} />
  if (token && state === null) return <div className="auth" />
  return <WaitlistPage invalid={state === 'bad'} />
}
