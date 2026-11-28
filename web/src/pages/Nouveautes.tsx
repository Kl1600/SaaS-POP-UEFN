// Onglet « Nouveautés » (#557) : ce qui a changé dans POP, groupé par jour. Ouvrir la page marque tout comme vu.
import { useEffect } from 'react'
import { PageHead } from '../components/ui'
import { ArrowRight } from '../icons'
import { api } from '../api'
import { useApp } from '../state'
import { useNav } from '../routes'
import { tr, LOCALE } from '../i18n/t'
import { NOUVEAUTES, DERNIERE, type Nouveaute } from '../nouveautes'

const TAG: Record<Nouveaute['tag'], [string, string]> = { nouveau: [tr("Nouveau"), 'nv-new'], ameliore: [tr("Amélioré"), 'nv-up'], corrige: [tr("Corrigé"), 'nv-fix'] }

export default function Nouveautes() {
  const { me } = useApp()
  const nav = useNav()
  useEffect(() => {
    if (!me || me.ui_prefs?.news_seen === DERNIERE()) return
    const all = { ...(me.ui_prefs ?? {}), news_seen: DERNIERE() }
    me.ui_prefs = all
    api.put('/api/me/ui-prefs', all).catch(() => {})
    window.dispatchEvent(new Event('pop-nouveautes-vues'))
  }, [me])
  const jours = new Map<string, Nouveaute[]>()
  for (const n of NOUVEAUTES()) jours.set(n.date, [...(jours.get(n.date) ?? []), n])
  return <main className="content nouveautes">
    <PageHead title={tr("Nouveautés")} help={tr("Ce qui a changé dans POP, du plus récent au plus ancien.")} />
    {[...jours].map(([d, list]) => <section key={d} className="nv-jour">
      <h3 className="nv-date">{new Date(d + 'T12:00:00').toLocaleDateString(LOCALE, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })}</h3>
      <div className="nv-liste">{list.map(n => <article key={n.titre} className="card nv-item">
        <span className={'nv-tag ' + TAG[n.tag][1]}>{TAG[n.tag][0]}</span>
        <div style={{ flex: 1 }}><b>{n.titre}</b><p className="small muted" style={{ margin: '4px 0 0' }}>{n.texte}</p></div>
        {n.lien && <button className="btn" onClick={() => nav(n.lien!)}>{tr("Voir")}<ArrowRight size={14} /></button>}
      </article>)}</div>
    </section>)}
  </main>
}
