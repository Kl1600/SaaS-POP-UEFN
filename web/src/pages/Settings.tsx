// Paramètres du compte, en onglets : Mon compte, IA connectées (#205), UEFN Link (#210), pour la map ouverte.
import { useSearchParams } from 'react-router-dom'
import { PageHead, Tabs } from '../components/ui'
import { BillingSettings } from '../components/Billing'
import { Account } from './Team'
import AI from './AI'
import { lazy, Suspense } from 'react'
const UefnLinkSettings = lazy(() => import('./Organizer').then(m => ({ default: m.UefnLinkSettings })))
import { useApp } from '../state'
import { tr } from '../i18n/t'

export default function Settings() {
  const { map } = useApp()
  const [sp, setSp] = useSearchParams()
  const o = sp.get('onglet')
  const tab = o === 'ia' || o === 'uefn' ? o : 'compte'
  return (
    <main className="content">
      <PageHead title={tr("Paramètres")} help={tr("Votre compte et vos connexions.")} />
      <Tabs value={tab} onChange={t => setSp(t === 'compte' ? {} : { onglet: t })} items={[['compte', tr("Mon compte")], ['ia', tr("IA connectées")], ['uefn', tr("UEFN Link")]]} />
      {tab === 'compte' && <><BillingSettings /><Account /></>}
      {tab === 'ia' && (map ? <AI embedded /> : <p className="muted">{tr("Ouvrez une map pour y connecter une IA.")}</p>)}
      {tab === 'uefn' && (map ? <Suspense fallback={null}><UefnLinkSettings /></Suspense> : <p className="muted">{tr("Ouvrez une map pour la relier à UEFN.")}</p>)}
    </main>
  )
}
