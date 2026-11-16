import { useEffect, useState } from 'react'
import { api } from '../api'
import { useApp } from '../state'
import { BlockView } from './Pages'

export function Login({ invite }: { invite?: string }) {
  const { reload } = useApp()
  const [mode, setMode] = useState<'login' | 'register' | 'forgot'>(invite ? 'register' : 'login')
  const [sent, setSent] = useState('')
  const [f, setF] = useState({ email: '', name: '', password: '' })
  const [err, setErr] = useState('')
  const [busy, setBusy] = useState(false)
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); setBusy(true); setErr('')
    try {
      if (mode === 'forgot') {
        const r = await api.post('/api/auth/forgot', { email: f.email })
        setSent(r.mail ? 'Si un compte existe avec cette adresse, un e-mail avec un lien vient de partir (valable 2 heures).' : 'L’envoi d’e-mails n’est pas configuré sur ce POP : demandez à un admin de votre studio un lien de réinitialisation (Équipe et intégrations → Membres).')
        return
      }
      if (mode === 'login') await api.post('/api/auth/login', { email: f.email, password: f.password })
      else await api.post('/api/auth/register', { ...f, invite })
      if (invite && mode === 'login') await api.post('/api/invitations/accept', { token: invite })
      await reload()
      if (invite) location.href = '/'
    } catch (e: any) { setErr(e.message) } finally { setBusy(false) }
  }
  return (
    <div className="auth app">
      <form className="card stack" onSubmit={submit} style={{ gap: 16 }}>
        <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)' }} />
        <h1 style={{ margin: 0, fontSize: 24, fontWeight: 400 }}>{mode === 'login' ? 'Connexion' : mode === 'forgot' ? 'Mot de passe oublié' : 'Créer un compte'}</h1>
        <p className="muted" style={{ margin: 0 }}>{invite ? 'Vous avez été invité dans un studio.' : "L'espace de travail des créateurs de maps UEFN."}</p>
        {mode === 'register' && <label className="label">Nom<input className="field" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} required maxLength={80} autoComplete="name" /></label>}
        <label className="label">{mode === 'login' ? 'E-mail ou identifiant' : 'E-mail'}<input className="field" type={mode === 'login' ? 'text' : 'email'} inputMode="email" autoCapitalize="off" value={f.email} onChange={e => setF({ ...f, email: e.target.value })} required autoComplete="email" /></label>
        {mode !== 'forgot' && <label className="label">Mot de passe<input className="field" type="password" value={f.password} onChange={e => setF({ ...f, password: e.target.value })} required minLength={mode === 'register' ? 10 : 1} autoComplete={mode === 'login' ? 'current-password' : 'new-password'} /></label>}
        {sent && <div className="b-callout small">{sent}</div>}
        {err && <div className="err" role="alert">{err}</div>}
        <button className="btn pri" disabled={busy}>{mode === 'login' ? 'Se connecter' : mode === 'forgot' ? 'Recevoir un lien' : 'Créer mon compte'}</button>
        {mode === 'login' && <button type="button" className="link" onClick={() => { setMode('forgot'); setErr('') }}>Mot de passe oublié ?</button>}
        <button type="button" className="link" onClick={() => { setMode(mode === 'login' ? 'register' : 'login'); setSent('') }}>
          {mode === 'login' ? 'Pas encore de compte ? Créer un compte' : "J'ai déjà un compte"}
        </button>
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
