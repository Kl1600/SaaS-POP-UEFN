// Paramètres → Sécurité : double authentification, codes de secours, appareils connectés, activité récente.
import { useEffect, useState } from 'react'
import QRCode from 'qrcode'
import { ShieldCheck, ShieldAlert, Monitor, Copy, Download } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead } from './ui'

const ACTIONS: Record<string, string> = {
  connexion: 'Connexion', connexion_échouée: 'Mot de passe incorrect', compte_bloqué: 'Compte bloqué 15 min', code_2fa_faux: 'Code de double authentification incorrect',
  '2fa_activée': 'Double authentification activée', '2fa_désactivée': 'Double authentification désactivée', mot_de_passe_changé: 'Mot de passe changé',
  session_fermée: 'Appareil déconnecté', sessions_fermées: 'Autres appareils déconnectés', compte_créé: 'Compte créé',
  admin_déverrouillé: 'Administration déverrouillée', admin_confirmation_échouée: 'Confirmation admin refusée',
}
export const actionLabel = (a: string) => ACTIONS[a] ?? a.replace(/_/g, ' ').replace(/^admin /, 'Admin : ')

function Recovery({ codes, onDone }: { codes: string[]; onDone: () => void }) {
  const { toast } = useApp()
  const text = 'Codes de secours POP (chacun ne sert qu’une fois)\n\n' + codes.join('\n')
  return <div className="b-callout" style={{ display: 'grid', gap: 10 }}>
    <b style={{ fontWeight: 500 }}>Gardez ces codes de secours en lieu sûr.</b>
    <span className="small">Si vous perdez votre téléphone, chacun permet de vous connecter une fois. Ils ne seront plus affichés.</span>
    <div className="mono" style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 6, fontSize: 14 }}>{codes.map(c => <span key={c}>{c}</span>)}</div>
    <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
      <button className="btn" onClick={() => { navigator.clipboard?.writeText(text); toast('Codes copiés') }}><Copy size={15} />Copier</button>
      <a className="btn" href={URL.createObjectURL(new Blob([text], { type: 'text/plain' }))} download="pop-codes-de-secours.txt"><Download size={15} />Télécharger</a>
      <button className="btn pri" onClick={onDone}>Je les ai rangés</button>
    </div>
  </div>
}

export function SecuritySettings() {
  const { toast } = useApp()
  const { data, reload } = useLoad<any>('/api/me/security')
  const [setup, setSetup] = useState<{ secret: string; uri: string; qr?: string } | null>(null)
  const [code, setCode] = useState('')
  const [codes, setCodes] = useState<string[] | null>(null)
  const [off, setOff] = useState(false)
  const [offF, setOffF] = useState({ password: '', code: '' })
  useEffect(() => { if (setup && !setup.qr) QRCode.toDataURL(setup.uri, { margin: 1, width: 200 }).then(qr => setSetup(s => s && { ...s, qr })).catch(() => {}) }, [setup])
  if (!data) return null
  return <div className="grid2" id="securite">
    <section className="card">
      <CardHead title="Double authentification" sub="Un code à 6 chiffres de votre téléphone en plus du mot de passe. Obligatoire pour l’administration." />
      {codes ? <Recovery codes={codes} onDone={() => { setCodes(null); reload() }} /> :
        data.totp ? <>
          <div className="row" style={{ gap: 10 }}><ShieldCheck size={22} style={{ color: 'var(--tg)' }} /><span><b style={{ fontWeight: 500 }}>Activée</b><span className="small muted" style={{ display: 'block' }}>{data.recovery_left} code{data.recovery_left > 1 ? 's' : ''} de secours restant{data.recovery_left > 1 ? 's' : ''}</span></span></div>
          {!off ? <button className="btn" style={{ marginTop: 14 }} onClick={() => setOff(true)}>Désactiver</button> :
            <form className="stack" style={{ marginTop: 14 }} onSubmit={async e => { e.preventDefault(); try { await api.post('/api/me/2fa/disable', offF); toast('Double authentification désactivée'); setOff(false); reload() } catch (er: any) { toast(er.message, true) } }}>
              <label className="label">Mot de passe<input className="field" type="password" required autoComplete="current-password" value={offF.password} onChange={e => setOffF({ ...offF, password: e.target.value })} /></label>
              <label className="label">Code à 6 chiffres ou code de secours<input className="field mono" required inputMode="numeric" autoComplete="one-time-code" value={offF.code} onChange={e => setOffF({ ...offF, code: e.target.value })} /></label>
              <div className="row"><button type="button" className="btn" onClick={() => setOff(false)}>Annuler</button><button className="btn" style={{ color: 'var(--tr)' }}>Désactiver</button></div>
            </form>}
        </> : !setup ? <>
          <div className="row" style={{ gap: 10 }}><ShieldAlert size={22} style={{ color: 'var(--to)' }} /><span>Pas encore activée{data.admin ? ' : nécessaire pour l’administration.' : '.'}</span></div>
          <button className="btn pri" style={{ marginTop: 14 }} onClick={async () => { try { setSetup(await api.post('/api/me/2fa/setup')) } catch (er: any) { toast(er.message, true) } }}>Activer</button>
        </> : <form className="stack" onSubmit={async e => { e.preventDefault(); try { const r = await api.post('/api/me/2fa/enable', { code }); setCodes(r.recovery); setSetup(null); setCode(''); toast('Double authentification activée') } catch (er: any) { toast(er.message, true) } }}>
          <ol style={{ margin: 0, paddingLeft: 20, lineHeight: 1.7 }}>
            <li>Installez une application d’authentification (Google Authenticator, Microsoft Authenticator, 1Password, Authy…).</li>
            <li>Scannez ce code, ou saisissez la clé à la main.</li>
            <li>Entrez le code à 6 chiffres affiché.</li>
          </ol>
          <div className="row" style={{ gap: 16, alignItems: 'center', flexWrap: 'wrap' }}>
            {setup.qr ? <img src={setup.qr} alt="QR code à scanner" width={160} height={160} style={{ borderRadius: 10, background: '#fff', padding: 6 }} /> : <div style={{ width: 160, height: 160 }} />}
            <div className="small" style={{ minWidth: 0 }}><span className="muted">Clé :</span><div className="mono" style={{ wordBreak: 'break-all', marginTop: 4 }}>{setup.secret.match(/.{1,4}/g)?.join(' ')}</div></div>
          </div>
          <label className="label">Code à 6 chiffres<input className="field mono" autoFocus required inputMode="numeric" autoComplete="one-time-code" maxLength={7} value={code} onChange={e => setCode(e.target.value)} style={{ letterSpacing: 4, fontSize: 18 }} /></label>
          <div className="row"><button type="button" className="btn" onClick={() => setSetup(null)}>Annuler</button><button className="btn pri">Vérifier et activer</button></div>
        </form>}
    </section>
    <section className="card">
      <CardHead title="Appareils connectés" sub="Déconnectez un appareil que vous ne reconnaissez pas, puis changez votre mot de passe.">
        {data.sessions.length > 1 && <button className="btn" style={{ height: 34 }} onClick={async () => { const r = await api.post('/api/me/sessions/close-others'); toast(`${r.closed} appareil(s) déconnecté(s)`); reload() }}>Déconnecter les autres</button>}
      </CardHead>
      <div className="list">{data.sessions.map((s: any) => <div key={s.id} className="li">
        <Monitor size={18} className="muted" />
        <span style={{ flex: 1, minWidth: 0 }}><span style={{ fontWeight: 500 }}>{s.device}</span>{s.current && <span className="badge b-g" style={{ marginLeft: 8 }}>Cet appareil</span>}
          <span className="small muted" style={{ display: 'block' }}>{s.ip || 'adresse inconnue'} · actif {s.last_seen ? ago(s.last_seen) : '—'}{s.mfa ? ' · double authentification' : ''}</span></span>
        {!s.current && <button className="btn" style={{ height: 32 }} onClick={async () => { await api.del(`/api/me/sessions/${s.id}`); toast('Appareil déconnecté'); reload() }}>Déconnecter</button>}
      </div>)}</div>
      <div className="dsec">Activité récente</div>
      <div className="list">{data.events.length === 0 ? <p className="small muted">Rien pour l’instant.</p> : data.events.slice(0, 8).map((e: any, i: number) => <div key={i} className="li" style={{ padding: '6px 0' }}>
        <span style={{ flex: 1 }} className={/échou|faux|bloqué/.test(e.action) ? 'err' : ''}>{actionLabel(e.action)}</span><span className="small muted">{e.ip} · {ago(e.at)}</span></div>)}</div>
    </section>
  </div>
}
