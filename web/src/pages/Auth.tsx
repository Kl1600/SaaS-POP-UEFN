import { useEffect, useState } from 'react'
import { api } from '../api'
import { useApp } from '../state'
import { WaitlistPage } from '../components/Waitlist'
import { BlockView } from './Pages'

export function Login({ invite, initial, access }: { invite?: string; initial?: 'login' | 'register'; access?: { token: string; email: string; name: string } }) {
  const { reload } = useApp()
  const [mode, setMode] = useState<'login' | 'register' | 'forgot'>(invite ? 'register' : initial ?? 'login')
  const [sent, setSent] = useState('')
  const [f, setF] = useState({ email: access?.email ?? '', name: access?.name ?? '', password: '' })
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); setBusy(true); setErr('')
    try {
      if (mode === 'forgot') {
        const r = await api.post('/api/auth/forgot', { email: f.email })
        setSent(r.mail ? 'Si un compte existe avec cette adresse, un e-mail avec un lien vient de partir (valable 2 heures).' : 'Pendant la bêta, écrivez à l’équipe POP : elle vous donnera un nouveau mot de passe provisoire.')
        return
      }
      if (mode === 'login') await api.post('/api/auth/login', { login: f.email.trim(), password: f.password })
      else await api.post('/api/auth/register', { ...f, invite, access: access?.token })
      if (invite && mode === 'login') await api.post('/api/invitations/accept', { token: invite })
      await reload()
      if (invite || access) location.href = '/'
    } catch (e: any) { setErr(e.message) } finally { setBusy(false) }
  }
  return (
    <div className="auth app">
      <form className="card stack" onSubmit={submit} style={{ gap: 16 }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)' }} />
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>{mode === 'login' ? 'Connexion' : mode === 'forgot' ? 'Mot de passe oublié' : 'Créer un compte'}</h1>
        <p className="muted" style={{ margin: 0 }}>{invite ? 'Vous avez été invité dans un studio.' : access && mode === 'register' ? 'Votre accès à la bêta est validé : créez votre compte.' : "L'espace de travail des créateurs de maps UEFN."}</p>
        {mode === 'register' && <label className="label">Nom<input className="field" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} required maxLength={80} autoComplete="name" /></label>}
        <label className="label">{mode === 'login' ? 'Identifiant' : 'E-mail'}<input className="field" type={mode === 'login' ? 'text' : 'email'} inputMode={mode === 'login' ? 'text' : 'email'} autoCapitalize="off" autoCorrect="off" spellCheck={false} value={f.email} onChange={e => setF({ ...f, email: e.target.value })} required autoComplete={mode === 'login' ? 'username' : 'email'} readOnly={!!access && mode === 'register'} />{mode === 'login' && <span className="small muted" style={{ fontWeight: 400 }}>Celui donné par l’équipe POP (ou votre e-mail).</span>}</label>
        {mode !== 'forgot' && <label className="label">Mot de passe{mode === 'register' && <span className="small muted"> (10 caractères minimum, pas un mot courant)</span>}<input className="field" type="password" value={f.password} onChange={e => setF({ ...f, password: e.target.value })} required minLength={mode === 'register' ? 10 : 1} autoComplete={mode === 'login' ? 'current-password' : 'new-password'} /></label>}
        {sent && <div className="b-callout small">{sent}</div>}
        {err && <div className="err" role="alert">{err}</div>}
        <button className="btn pri" disabled={busy}>{mode === 'login' ? 'Se connecter' : mode === 'forgot' ? 'Recevoir un lien' : 'Créer mon compte'}</button>
        {mode === 'login' && <button type="button" className="link" onClick={() => { setMode('forgot'); setErr('') }}>Mot de passe oublié ?</button>}
        {mode === 'login' && !invite && !access
          ? <a className="link" href="/inscription">Pas encore de compte ? Rejoindre la liste d’attente</a>
          : <button type="button" className="link" onClick={() => { setMode(mode === 'login' ? 'register' : 'login'); setSent('') }}>
            {mode === 'login' ? 'Pas encore de compte ? Créer un compte' : "J'ai déjà un compte"}
          </button>}
      </form>
      {!invite && <a href="/" className="small" style={{ color: 'rgba(255,255,255,.6)', marginTop: 16, display: 'inline-block' }}>← Retour au site</a>}
    </div>
  )
}

/** Première connexion d'un compte créé par l'administrateur : remplacer le mot de passe provisoire. */
export function FirstPassword() {
  const { me, reload } = useApp()
  const [f, setF] = useState({ a: '', b: '' })
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); setErr('')
    if (f.a !== f.b) { setErr('Les deux mots de passe ne sont pas identiques.'); return }
    setBusy(true)
    try { await api.post('/api/me/first-password', { new: f.a }); await reload() } catch (er: any) { setErr(er.message) } finally { setBusy(false) }
  }
  return (
    <div className="auth app">
      <form className="card stack" onSubmit={submit} style={{ gap: 16 }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)' }} />
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>Bienvenue{me ? `, ${me.user.name.split(' ')[0]}` : ''} !</h1>
        <p className="muted" style={{ margin: 0 }}>Votre compte a été créé par l’équipe POP avec un mot de passe provisoire. Choisissez le vôtre pour continuer.</p>
        {me?.username && <div className="small muted">Votre identifiant : <b className="mono" style={{ fontWeight: 500, color: 'var(--t1, inherit)' }}>{me.username}</b></div>}
        <input type="text" name="username" autoComplete="username" value={me?.username ?? me?.user.email ?? ''} readOnly hidden />
        <label className="label">Nouveau mot de passe<span className="small muted"> (10 caractères minimum, pas un mot courant)</span><input className="field" type="password" autoFocus value={f.a} onChange={e => setF({ ...f, a: e.target.value })} required minLength={10} autoComplete="new-password" /></label>
        <label className="label">Encore une fois<input className="field" type="password" value={f.b} onChange={e => setF({ ...f, b: e.target.value })} required minLength={10} autoComplete="new-password" /></label>
        {err && <div className="err" role="alert">{err}</div>}
        <button className="btn pri" disabled={busy}>Enregistrer et continuer</button>
        <button type="button" className="link" onClick={async () => { await api.post('/api/auth/logout'); location.href = '/connexion' }}>Me déconnecter</button>
      </form>
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
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>Créer votre studio</h1>
        <p className="muted" style={{ margin: 0 }}>Un studio regroupe votre équipe et vos maps.</p>
        <label className="label">Nom du studio<input className="field" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} required /></label>
        <label className="label">Première map<input className="field" value={f.map_name} onChange={e => setF({ ...f, map_name: e.target.value })} required /></label>
        <label className="label">Genre<input className="field" value={f.genre} onChange={e => setF({ ...f, genre: e.target.value })} placeholder="Tycoon, box fight…" /></label>
        <label className="label">Date de sortie visée<input className="field" type="date" value={f.release_date} onChange={e => setF({ ...f, release_date: e.target.value })} /></label>
        {err && <div className="err">{err}</div>}
        <button className="btn pri">Créer</button>
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
  return <div className="auth app"><div className="card">{msg || 'Invitation acceptée…'}</div></div>
}

export function PublicPage({ token }: { token: string }) {
  const [p, setP] = useState<any>(null)
  const [err, setErr] = useState('')
  useEffect(() => { api.get(`/api/public/pages/${token}`).then(setP).catch(e => setErr(e.message)) }, [token])
  return (
    <div className="app" style={{ display: 'block', background: 'var(--bg)', minHeight: '100vh' }}>
      <div style={{ maxWidth: 760, margin: '0 auto', padding: 48 }}>
        {err && <p className="err">{err}</p>}
        {p && <>
          <p className="small muted">Page partagée en lecture · POP</p>
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
  return (
    <div className="auth app">
      <form className="card stack" style={{ gap: 16 }} onSubmit={async e => { e.preventDefault(); try { await api.post('/api/auth/reset', { token, password: pw }); setOk(true) } catch (er: any) { setMsg(er.message) } }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)' }} />
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>Nouveau mot de passe</h1>
        {ok ? <><p>Mot de passe changé. Vous pouvez vous connecter.</p><a className="btn pri" href="/">Se connecter</a></> : <>
          <label className="label">Nouveau mot de passe (10 caractères minimum)<input className="field" type="password" value={pw} onChange={e => setPw(e.target.value)} minLength={10} required autoComplete="new-password" /></label>
          {msg && <div className="err" role="alert">{msg}</div>}
          <button className="btn pri">Enregistrer</button>
        </>}
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
        <h1 style={{ margin: 0, fontSize: 26, fontWeight: 400 }}>Bienvenue, {me!.user.name.split(' ')[0]}</h1>
        <p className="muted" style={{ margin: 0 }}>Reliez UEFN : POP importe vos projets (une map par projet), leurs fichiers Verse et leurs acteurs, et les garde synchronisés.</p>
        <ol className="welcome-steps">
          <li><b>Sur le PC où UEFN est installé</b>, ouvrez un projet puis : Projet → Paramètres du projet → <b>Enable Project Python Scripts</b>, et Édition → Préférences de l’éditeur → Model Context Protocol → <b>Start Server</b>.</li>
          <li><a className="btn pri" href={`/api/studios/${studio.id}/relay/download`} onClick={() => setDl(true)} download>Télécharger UEFN Link (Windows)</a><div className="small muted" style={{ marginTop: 6 }}>Ce fichier est fait pour votre compte : rien à configurer. Valable 24 h. Ouvrez cette page sur le PC pour le télécharger directement dessus.</div></li>
          <li><b>Double-cliquez sur pop-link.exe.</b> Si Windows affiche « Windows a protégé votre ordinateur » : Informations complémentaires → Exécuter quand même. Laissez la fenêtre ouverte.</li>
        </ol>
        <div className="b-callout small">{dl ? 'En attente du PC… cette page s’ouvrira toute seule dès que vos projets seront importés.' : 'Cette page se mettra à jour toute seule dès que vos projets seront importés.'}</div>
        <details>
          <summary className="small" style={{ cursor: 'pointer' }}>Commencer sans UEFN</summary>
          <form className="row" style={{ marginTop: 10 }} onSubmit={async e => { e.preventDefault(); try { await api.post(`/api/studios/${studio.id}/skip-uefn`, { name }); await reload() } catch (er: any) { setErr(er.message) } }}>
            <input className="field" style={{ flex: 1 }} placeholder="Nom de la map (ex. VaultRush)" value={name} onChange={e => setName(e.target.value)} />
            <button className="btn">Continuer</button>
          </form>
          {err && <div className="err">{err}</div>}
        </details>
        <button className="link small" style={{ alignSelf: 'flex-start' }} onClick={async () => { await api.post('/api/auth/logout'); location.href = '/' }}>Se déconnecter</button>
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
