// Page « Autoriser » de la connexion OAuth des IA (#285) : l'IA a ouvert POP dans le navigateur, l'utilisateur
// connecté autorise (ou refuse) et choisit si l'IA peut modifier ou seulement lire.
import { useEffect, useState } from 'react'
import { Check, Lock, Unlock } from 'lucide-react'
import { api } from '../api'
import { useApp } from '../state'
import { tr } from '../i18n/t'
import { AiLogo } from '../components/AiLogo'

export default function Authorize() {
  const { me } = useApp()
  const qs = location.search
  const params = Object.fromEntries(new URLSearchParams(qs))
  const [info, setInfo] = useState<{ client: string; redirect_host: string; read_only: boolean } | null>(null)
  const [err, setErr] = useState('')
  const [ro, setRo] = useState(false)
  const [busy, setBusy] = useState(false)
  const [done, setDone] = useState(false)
  useEffect(() => {
    document.title = tr("Autoriser une IA") + ' · POP'
    api.get(`/api/oauth/authorize${qs}`).then((r: any) => { setInfo(r); setRo(!!r.read_only) }).catch(e => setErr(e.message))
  }, [qs])
  const decide = async (approve: boolean) => {
    setBusy(true); setErr('')
    try {
      const r = await api.post('/api/oauth/authorize', { ...params, approve, read_only: ro })
      setDone(approve)
      location.href = r.redirect
    } catch (e: any) { setErr(e.message); setBusy(false) }
  }
  const kind = info ? kindOf(info.client) : 'autre'
  return <div className="auth app">
    <div className="card stack oauth-card" style={{ gap: 16 }}>
      <img src="/logo.png" alt="POP" style={{ filter: 'invert(1)', height: 34, alignSelf: 'flex-start' }} />
      {err && !info && <><h1 className="oauth-h">{tr("Connexion de l’IA impossible")}</h1><div className="err" role="alert">{err}</div><a className="link" href="/">{tr("Retour à POP")}</a></>}
      {!info && !err && <p className="muted">{tr("Chargement…")}</p>}
      {info && done && <><h1 className="oauth-h"><Check size={22} /> {tr("C’est connecté")}</h1><p className="muted" style={{ margin: 0 }}>{tr("Vous pouvez fermer cette page et revenir à {0}.", [info.client])}</p></>}
      {info && !done && <>
        <div className="oauth-who"><span className="oauth-logo"><AiLogo kind={kind} size={26} /></span><div><h1 className="oauth-h">{tr("{0} veut se connecter à POP", [info.client])}</h1><p className="small muted" style={{ margin: '4px 0 0' }}>{tr("Compte : {0}", [me?.user.email ?? ''])}</p></div></div>
        <div className="oauth-list small">
          <p style={{ margin: '0 0 6px' }}>{tr("Cette IA pourra :")}</p>
          <ul>
            <li>{tr("voir vos maps perso et celles de vos studios, et choisir celle sur laquelle travailler ;")}</li>
            <li>{ro ? tr("lire vos tâches, pages et le niveau UEFN, sans rien modifier.") : tr("lire et modifier vos tâches et piloter UEFN par votre relais (chaque action est journalisée et annulable, une suppression attend votre validation).")}</li>
          </ul>
        </div>
        <div className="seg oauth-seg" role="radiogroup" aria-label={tr("Droits de l’IA")}>
          <button type="button" role="radio" aria-checked={!ro} className={!ro ? 'on' : ''} onClick={() => setRo(false)}><Unlock size={14} />{tr("Peut modifier")}</button>
          <button type="button" role="radio" aria-checked={ro} className={ro ? 'on' : ''} onClick={() => setRo(true)}><Lock size={14} />{tr("Lecture seule")}</button>
        </div>
        {err && <div className="err" role="alert">{err}</div>}
        <div className="row" style={{ gap: 10 }}>
          <button className="btn" style={{ flex: 1 }} disabled={busy} onClick={() => decide(false)}>{tr("Refuser")}</button>
          <button className="btn pri" style={{ flex: 1 }} disabled={busy} onClick={() => decide(true)}>{tr("Autoriser")}</button>
        </div>
        <p className="small muted" style={{ margin: 0 }}>{tr("Vous serez renvoyé vers {0}. Vous pourrez déconnecter cette IA à tout moment dans Paramètres → IA connectées.", [info.redirect_host])}</p>
      </>}
    </div>
  </div>
}

function kindOf(n: string) {
  const s = n.toLowerCase()
  if (s.includes('claude code') || s.includes('claude-code')) return 'claude_code'
  if (s.includes('claude')) return 'claude_desktop'
  if (s.includes('cursor')) return 'cursor'
  if (s.includes('windsurf')) return 'windsurf'
  if (s.includes('codex') || s.includes('openai') || s.includes('chatgpt')) return 'codex'
  if (s.includes('vs code') || s.includes('vscode') || s.includes('visual studio') || s.includes('copilot')) return 'vscode'
  if (s.includes('gemini')) return 'gemini'
  return 'autre'
}
