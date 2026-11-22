// Paramètres du compte, en onglets (#205) : Mon compte, IA connectées (pour la map ouverte).
import { useSearchParams } from 'react-router-dom'
import { PageHead, Tabs } from '../components/ui'
import { BillingSettings } from '../components/Billing'
import { Account } from './Team'
import AI from './AI'
import { useApp } from '../state'
import { tr } from '../i18n/t'

export default function Settings() {
  const { map } = useApp()
  const [sp, setSp] = useSearchParams()
  const tab = sp.get('onglet') === 'ia' ? 'ia' : 'compte'
  return (
    <main className="content">
      <PageHead title={tr("Paramètres")} help={tr("Votre compte et vos connexions.")} />
      <Tabs value={tab} onChange={t => setSp(t === 'compte' ? {} : { onglet: t })} items={[['compte', tr("Mon compte")], ['ia', tr("IA connectées")]]} />
      {tab === 'compte' && <><BillingSettings /><Account /></>}
      {tab === 'ia' && (map ? <AI embedded /> : <p className="muted">{tr("Ouvrez une map pour y connecter une IA.")}</p>)}
    </main>
  )
}
