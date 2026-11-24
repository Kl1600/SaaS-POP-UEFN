// Paramètres → Sécurité : appareils connectés et activité récente.
import { Monitor } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead } from './ui'
import { tr } from '../i18n/t'

const ACTIONS: Record<string, string> = {
  connexion: tr("Connexion"), connexion_échouée: tr("Mot de passe incorrect"), compte_bloqué: tr("Compte bloqué 15 min"), code_2fa_faux: tr("Code de double authentification incorrect"),
  '2fa_activée': tr("Double authentification activée"), '2fa_désactivée': tr("Double authentification désactivée"), mot_de_passe_changé: tr("Mot de passe changé"),
  session_fermée: tr("Appareil déconnecté"), sessions_fermées: tr("Autres appareils déconnectés"), compte_créé: tr("Compte créé"),
  admin_déverrouillé: tr("Administration déverrouillée"), admin_confirmation_échouée: tr("Confirmation admin refusée"),
  mot_de_passe_choisi: tr("Mot de passe choisi"), admin_compte_créé: tr("Admin : compte créé"), admin_lien_activation_renvoyé: tr("Admin : lien d’activation renvoyé"), compte_activé: tr("Compte activé"), admin_mot_de_passe_réinitialisé: tr("Admin : mot de passe réinitialisé"),
  admin_lien_mot_de_passe: tr("Admin : lien de nouveau mot de passe"), admin_compte_débloqué: tr("Admin : compte débloqué"), admin_deconnecte_partout: tr("Admin : déconnecté partout"), admin_compte_supprimé: tr("Admin : compte supprimé"),
}
export const actionLabel = (a: string) => ACTIONS[a] ?? a.replace(/_/g, ' ').replace(/^admin /, tr("Admin : "))

export function SecuritySettings() {
  const { toast } = useApp()
  const { data, reload } = useLoad<any>('/api/me/security')
  if (!data) return null
  return <div id="securite">
    <section className="card">
      <CardHead title={tr("Appareils connectés")} sub={tr("Déconnectez un appareil que vous ne reconnaissez pas, puis changez votre mot de passe.")}>
        {data.sessions.length > 1 && <button className="btn" style={{ height: 34 }} onClick={async () => { const r = await api.post('/api/me/sessions/close-others'); toast(tr("{0} appareil(s) déconnecté(s)", [r.closed])); reload() }}>{tr("Déconnecter les autres")}</button>}
      </CardHead>
      <div className="list">{data.sessions.map((s: any) => <div key={s.id} className="li">
        <Monitor size={18} className="muted" />
        <span style={{ flex: 1, minWidth: 0 }}><span style={{ fontWeight: 500 }}>{s.device}</span>{s.current && <span className="badge b-g" style={{ marginLeft: 8 }}>{tr("Cet appareil")}</span>}
          <span className="small muted" style={{ display: 'block' }}>{tr("{0} · actif {1}", [s.ip || tr("adresse inconnue"), s.last_seen ? ago(s.last_seen) : '—'])}</span></span>
        {!s.current && <button className="btn" style={{ height: 32 }} onClick={async () => { await api.del(`/api/me/sessions/${s.id}`); toast(tr("Appareil déconnecté")); reload() }}>{tr("Déconnecter")}</button>}
      </div>)}</div>
      <div className="dsec">{tr("Activité récente")}</div>
      <div className="list">{data.events.length === 0 ? <p className="small muted">{tr("Rien pour l’instant.")}</p> : data.events.slice(0, 8).map((e: any, i: number) => <div key={i} className="li" style={{ padding: '6px 0' }}>
        <span style={{ flex: 1 }} className={/échou|faux|bloqué/.test(e.action) ? 'err' : ''}>{actionLabel(e.action)}</span><span className="small muted">{e.ip} · {ago(e.at)}</span></div>)}</div>
    </section>
  </div>
}
