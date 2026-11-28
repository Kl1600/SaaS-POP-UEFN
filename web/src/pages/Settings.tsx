// Paramètres du compte, en onglets : Mon compte, IA connectées (#205 ; jetons du compte, #145), UEFN Link (#210, pour la map ouverte).
import { useSearchParams } from 'react-router-dom'
import { CardHead, PageHead, Tabs } from '../components/ui'
import { PanelLeft } from 'lucide-react'
import { api } from '../api'
import { BillingSettings } from '../components/Billing'
import { Account } from './Team'
import { MapAgents } from './AI'
import { AccountAI } from '../components/AiTokens'
import { MapsStudios } from '../components/MapsStudios'
import { lazy, Suspense } from 'react'
const UefnLinkSettings = lazy(() => import('./Organizer').then(m => ({ default: m.UefnLinkSettings })))
import { useApp } from '../state'
import { tr } from '../i18n/t'

export default function Settings() {
  const { map } = useApp()
  const [sp, setSp] = useSearchParams()
  const o = sp.get('onglet')
  const tab = o === 'ia' || o === 'uefn' || o === 'maps' ? o : 'compte'
  return (
    <main className="content">
      <PageHead title={tr("Paramètres")} help={tr("Votre compte et vos connexions.")} />
      <Tabs value={tab} onChange={t => setSp(t === 'compte' ? {} : { onglet: t })} items={[['compte', tr("Mon compte")], ['maps', tr("Maps et studios")], ['ia', tr("IA connectées")], ['uefn', tr("UEFN Link")]]} />
      {tab === 'compte' && <><BillingSettings /><Account /><Affichage /></>}
      {tab === 'maps' && <MapsStudios />}
      {tab === 'ia' && <><AccountAI /><MapAgents /></>}
      {tab === 'uefn' && (map ? <Suspense fallback={null}><UefnLinkSettings /></Suspense> : <p className="muted">{tr("Ouvrez une map pour la relier à UEFN.")}</p>)}
    </main>
  )
}

/** Affichage (#493) : remettre la barre latérale comme à l'origine (ordre des onglets, catégories dépliées). */
function Affichage() {
  const { me, reload, toast } = useApp()
  const nav = me?.ui_prefs?.nav
  const perso = !!nav && ((nav.collapsed ?? []).length > 0 || Object.keys(nav.order ?? {}).length > 0)
  return <section className="card">
    <CardHead icon={<PanelLeft size={18} />} title={tr("Affichage")} sub={tr("Repliez une catégorie de la barre latérale en cliquant son titre ; glissez un onglet pour le déplacer.")} />
    <button className="btn" disabled={!perso} onClick={async () => {
      const all = { ...(me?.ui_prefs ?? {}) }; delete all.nav
      try { await api.put('/api/me/ui-prefs', all); await reload(); toast(tr("Barre latérale rétablie.")) } catch (e: any) { toast(e.message, true) }
    }}>{tr("Rétablir l’ordre d’origine")}</button>
  </section>
}
