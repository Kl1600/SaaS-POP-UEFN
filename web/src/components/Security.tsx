// Paramètres → Sécurité : appareils connectés et activité récente.
import { Monitor } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead } from './ui'

const ACTIONS: Record<string, string> = {
  connexion: 'Connexion', connexion_échouée: 'Mot de passe incorrect', compte_bloqué: 'Compte bloqué 15 min', code_2fa_faux: 'Code de double authentification incorrect',
  '2fa_activée': 'Double authentification activée', '2fa_désactivée': 'Double authentification désactivée', mot_de_passe_changé: 'Mot de passe changé',
  session_fermée: 'Appareil déconnecté', sessions_fermées: 'Autres appareils déconnectés', compte_créé: 'Compte créé',
  admin_déverrouillé: 'Administration déverrouillée', admin_confirmation_échouée: 'Confirmation admin refusée',
  mot_de_passe_choisi: 'Mot de passe choisi', admin_compte_créé: 'Admin : compte créé', admin_mot_de_passe_réinitialisé: 'Admin : mot de passe réinitialisé',
}
export const actionLabel = (a: string) => ACTIONS[a] ?? a.replace(/_/g, ' ').replace(/^admin /, 'Admin : ')

export function SecuritySettings() {
  const { toast } = useApp()
  const { data, reload } = useLoad<any>('/api/me/security')
  if (!data) return null
  return <div id="securite">
    <section className="card">
      <CardHead title="Appareils connectés" sub="Déconnectez un appareil que vous ne reconnaissez pas, puis changez votre mot de passe.">
        {data.sessions.length > 1 && <button className="btn" style={{ height: 34 }} onClick={async () => { const r = await api.post('/api/me/sessions/close-others'); toast(`${r.closed} appareil(s) déconnecté(s)`); reload() }}>Déconnecter les autres</button>}
      </CardHead>
      <div className="list">{data.sessions.map((s: any) => <div key={s.id} className="li">
        <Monitor size={18} className="muted" />
        <span style={{ flex: 1, minWidth: 0 }}><span style={{ fontWeight: 500 }}>{s.device}</span>{s.current && <span className="badge b-g" style={{ marginLeft: 8 }}>Cet appareil</span>}
          <span className="small muted" style={{ display: 'block' }}>{s.ip || 'adresse inconnue'} · actif {s.last_seen ? ago(s.last_seen) : '—'}</span></span>
        {!s.current && <button className="btn" style={{ height: 32 }} onClick={async () => { await api.del(`/api/me/sessions/${s.id}`); toast('Appareil déconnecté'); reload() }}>Déconnecter</button>}
      </div>)}</div>
      <div className="dsec">Activité récente</div>
      <div className="list">{data.events.length === 0 ? <p className="small muted">Rien pour l’instant.</p> : data.events.slice(0, 8).map((e: any, i: number) => <div key={i} className="li" style={{ padding: '6px 0' }}>
        <span style={{ flex: 1 }} className={/échou|faux|bloqué/.test(e.action) ? 'err' : ''}>{actionLabel(e.action)}</span><span className="small muted">{e.ip} · {ago(e.at)}</span></div>)}</div>
    </section>
  </div>
}
