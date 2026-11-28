// Accès à Analyse selon l'abonnement.
// - Catégorie Analyse : plan du compte (me.analyse).
// - Onglet « Marché » d'un studio : plan du créateur du studio (studio.analyse) ; les membres y ont accès dans ce studio.
import type { ReactNode } from 'react'
import { useSearchParams, Navigate } from 'react-router-dom'
import { Lock } from '../icons'
import { useApp } from '../state'
import { PageHead, Tabs } from './ui'
import { tr } from '../i18n/t'
import { Market, TikTok } from '../pages/Analysis'

export function Locked({ title, text }: { title: string; text: string }) {
  return <main className="content">
    <PageHead title={title} help={tr("Votre plan ne comprend pas encore cette partie.")} />
    <section className="card locked">
      <span className="locked-ic"><Lock size={22} strokeWidth={1.5} /></span>
      <h2>{tr("Disponible avec le plan Pro")}</h2>
      <p className="muted">{text}</p>
    </section>
  </main>
}

/** Pages de la catégorie Analyse : réservées aux comptes dont le plan comprend Analyse. */
export function AnalyseGate({ title, children }: { title: string; children: ReactNode }) {
  const { me } = useApp()
  if (!me) return null
  if (!me.analyse) return <Locked title={title} text={tr("Analyse (idées de maps, marché, maps qui marchent, meilleurs démarrages, tendances vidéo) est comprise dans le plan Pro. Pendant la bêta, demandez à l’équipe POP de changer votre plan.")} />
  return <>{children}</>
}

/** Tendances vidéo de la catégorie Analyse : clés et vidéos de l'espace perso (le compte). */
export function AnalyseVideos() {
  const { me } = useApp()
  const perso = me?.studios.find(s => s.personal)
  return <AnalyseGate title={tr("Tendances vidéo")}>{perso ? <TikTok sid={perso.id} /> : null}</AnalyseGate>
}

/** Onglet « Marché » du studio : genres du marché + tendances vidéo du studio, si son créateur a le plan. */
export function StudioMarket() {
  const { studio } = useApp()
  const [sp, setSp] = useSearchParams()
  if (!studio) return null
  if (studio.personal) return <Navigate to="/analyse/marche" replace />
  const title = tr("Marché · {0}", [studio.name])
  if (!studio.analyse) return <Locked title={title} text={studio.role === 'owner'
    ? tr("L’onglet Marché de votre studio est compris dans le plan Pro. Avec ce plan, tous les membres du studio y ont accès. Pendant la bêta, demandez à l’équipe POP de changer votre plan.")
    : tr("L’onglet Marché est ouvert quand le créateur du studio a le plan Pro.")} />
  const v = sp.get('v') === 'videos' ? 'videos' : 'genres'
  const bar = <Tabs value={v} onChange={x => setSp(x === 'videos' ? { v: 'videos' } : {}, { replace: true })} items={[['genres', tr("Genres du marché")], ['videos', tr("Tendances vidéo du studio")]]} />
  return v === 'videos' ? <TikTok sid={studio.id} title={title} bar={bar} /> : <Market title={title} bar={bar} />
}
