// Abonnement (Starter / Pro) et écran de fin d'essai. Les paiements passent par Stripe.
import { useEffect, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { CreditCard, Check, LogOut, Download, ShieldCheck, RotateCcw } from '../icons'
import { api } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead } from './ui'
import { tr, LOCALE } from '../i18n/t'

const date = (d?: string | null) => d ? new Date(d.replace(' ', 'T') + 'Z').toLocaleDateString(LOCALE, { day: 'numeric', month: 'long', year: 'numeric' }) : ''

const FEATURES: Record<string, string[]> = {
  starter: [tr("Tâches, planning, pages, tableau blanc"), tr("UEFN Link (vos IA connaissent la map)"), tr("IA connectées (Claude, Cursor, Codex…)"), tr("Lancement, playtests, partage des gains"), tr("Seul ou en équipe, maps illimitées")],
  pro: [tr("Tout Starter"), tr("Idées de maps et analyse du marché"), tr("Ça monte maintenant, tendances vidéo"), tr("Onglet Discover et chance de percer"), tr("Chiffres privés du Creator Portal")],
}

async function go(path: string, body: any, toast: (m: string, e?: boolean) => void) {
  try { const r = await api.post(path, body); if (r.url) location.href = r.url } catch (e: any) { toast(e.message, true) }
}

// Prix ramené au jour (inspiré de Vinteer, #516) : 12,99 € / 30 j ≈ 0,43 €, 24,99 € ≈ 0,83 €.
const PAR_JOUR: Record<string, string> = { starter: tr("Moins de 0,50 € par jour"), pro: tr("Moins d’1 € par jour") }

function Plans({ b }: { b: any }) {
  const { toast } = useApp()
  const [an, setAn] = useState(false)
  const annuel = an && b.annual
  const active = ['active', 'trialing', 'past_due'].includes(b.sub_status)
  return <>
    {b.annual && !active && <div className="bfreq" role="group" aria-label={tr("Fréquence de facturation")}>
      <button type="button" className={!an ? 'on' : ''} aria-pressed={!an} onClick={() => setAn(false)}>{tr("Mensuel")}</button>
      <button type="button" className={an ? 'on' : ''} aria-pressed={an} onClick={() => setAn(true)}>{tr("Annuel")}<em>{tr("2 mois offerts")}</em></button>
    </div>}
    <div className="bplans">{(['starter', 'pro'] as const).map(p => {
    const current = active && b.plan === p
    return <div key={p} className={'bplan' + (p === 'pro' ? ' best' : '') + (current ? ' current' : '')}>
      <div className="row" style={{ justifyContent: 'space-between' }}><b>{p === 'pro' ? 'Pro' : 'Starter'}</b>{current && <span className="badge b-g">{tr("Votre plan")}</span>}</div>
      <div className="bprice">{annuel ? b.prices_year[p] : b.prices[p]}<span>{annuel ? tr("/ an") : tr("/ mois")}</span></div>
      <div className="bjour">{annuel ? tr("Soit 2 mois offerts sur l’année") : PAR_JOUR[p]}</div>
      <ul>{FEATURES[p].map(f => <li key={f}><Check size={14} />{f}</li>)}</ul>
      {!current && <button className={'btn' + (p === 'pro' ? ' pri' : '')} disabled={!b.configured} onClick={() => go('/api/billing/checkout', { kind: p, annuel }, toast)}>{active ? tr("Changer pour {0}", [p === 'pro' ? 'Pro' : 'Starter']) : tr("Choisir {0}", [p === 'pro' ? 'Pro' : 'Starter'])}</button>}
    </div>
  })}</div>
    <div className="bsure"><span><Check size={14} />{tr("Annulable à tout moment, en un clic")}</span><span><ShieldCheck size={14} />{tr("Paiement sécurisé par Stripe")}</span></div>
  </>
}

/** Paramètres → Abonnement. */
export function BillingSettings() {
  const { toast, reload: reloadMe } = useApp()
  const { data: b, reload } = useLoad<any>('/api/billing')
  const [sp, setSp] = useSearchParams()
  useEffect(() => {
    const ok = sp.get('paiement') === 'ok'
    if (ok) { toast(tr("Paiement reçu : votre abonnement est actif.")); setTimeout(() => { reload(); reloadMe() }, 2500); setSp({}, { replace: true }) }
  }, [])
  if (!b) return null
  const active = ['active', 'trialing', 'past_due'].includes(b.sub_status)
  const status = b.source === 'manual' ? tr("Accès bêta : plan {0} offert par l’équipe POP.", [b.plan === 'pro' ? 'Pro' : 'Starter'])
    : active ? (b.sub_status === 'past_due' ? tr("Le dernier paiement a échoué : mettez à jour votre carte pour garder votre plan.") : tr("Abonnement {0} actif, renouvelé le {1}.", [b.plan === 'pro' ? 'Pro' : 'Starter', date(b.sub_end)]))
    : b.trial_until ? tr("Essai de Pro jusqu’au {0}. Choisissez un plan pour continuer après.", [date(b.trial_until)])
    : tr("Aucun abonnement actif.")
  return <>
    <section className="card">
      <CardHead icon={<CreditCard size={18} />} title={tr("Abonnement")} sub={status}>
        {b.has_customer && b.configured && <button className="btn" onClick={() => go('/api/billing/portal', {}, toast)}>{tr("Gérer, factures, résilier")}</button>}
      </CardHead>
      {!b.configured ? <p className="small muted" style={{ margin: 0 }}>{tr("Les paiements ne sont pas encore ouverts : pendant la bêta, POP est gratuit.")}</p> : b.source !== 'manual' && <Plans b={b} />}
    </section>
  </>
}

/** Écran quand l'essai est fini sans abonnement : choisir un plan, exporter ses données ou se déconnecter. */
export function Paywall() {
  const { data: b } = useLoad<any>('/api/billing')
  return <div className="app paywall">
    <img src="/logo.png" alt="POP" className="paywall-logo" />
    <div className="card paywall-box">
      <h1>{tr("Votre essai de 7 jours est terminé")}</h1>
      <p>{tr("Merci d’avoir essayé POP. Rien n’est supprimé : vos maps, vos tâches, vos pages et votre équipe vous attendent. Choisissez un plan et tout reprend là où vous vous étiez arrêté.")}</p>
      <div className="bsure" style={{ justifyContent: 'center', marginBottom: 14 }}><span><RotateCcw size={14} />{tr("Tout est gardé tel quel")}</span><span><Check size={14} />{tr("Sans engagement, annulable en un clic")}</span><span><Download size={14} />{tr("Vos données exportables à tout moment")}</span></div>
      {b && <Plans b={b} />}
      <div className="row" style={{ gap: 10, justifyContent: 'center', marginTop: 18, flexWrap: 'wrap' }}>
        <a className="btn" href="/api/me/export"><Download size={16} />{tr("Exporter mes données")}</a>
        <button className="btn" onClick={async () => { await api.post('/api/auth/logout'); location.href = '/' }}><LogOut size={16} />{tr("Me déconnecter")}</button>
      </div>
    </div>
  </div>
}
