// « Maps à faire » (#316) : le salon d'Analyse. D'abord, si on veut, on connecte des sources (YouTube, TikTok, X,
// Reddit : chacune seule, facultative, à tout moment) ; ensuite la liste des types de maps qui marchent avec peu de
// concurrence, calculée par l'algorithme des niches (ideas.rs : joueurs, fidélité, Discover, nouvelles maps qui
// percent, élan, et ce dont on parle sur les sources connectées).
import { useState } from 'react'
import { ArrowRight, Check, Youtube, Music2, AtSign, MessageCircle, X as Close } from 'lucide-react'
import { api } from '../api'
import { useApp, useLoad } from '../state'
import { useNav } from '../routes'
import { Empty, PageHead } from '../components/ui'
import { useIsland } from './Analysis'
import { tr, LANG, LOCALE } from '../i18n/t'

const num = (v?: number | null) => v === null || v === undefined ? '—' : v >= 1e6 ? (v / 1e6).toFixed(1).replace('.', ',') + ' M' : v >= 1e4 ? Math.round(v / 1e3) + ' k' : Math.round(v).toLocaleString(LOCALE)

type Kind = 'youtube' | 'tiktok' | 'x' | 'reddit'
const SOURCES: { kind: Kind; name: string; Icon: any }[] = [
  { kind: 'youtube', name: 'YouTube', Icon: Youtube },
  { kind: 'tiktok', name: 'TikTok', Icon: Music2 },
  { kind: 'x', name: 'X', Icon: AtSign },
  { kind: 'reddit', name: 'Reddit', Icon: MessageCircle },
]

/** Pourquoi cette map est à faire, en une phrase. */
function why(d: any): string {
  const b: string[] = d.badges ?? []
  if (b.includes('vague')) return tr("Les joueurs arrivent et il y a encore peu de maps.")
  if (b.includes('porte_ouverte')) return tr("Les nouvelles maps y percent : {0} sur {1} ce mois-ci.", [d.success100, d.newcomers])
  if (b.includes('joueurs_decus')) return tr("Beaucoup de joueurs, mais les maps en tête les retiennent mal.")
  if (b.includes('tendance_video')) return tr("On en parle en ce moment sur les réseaux.")
  if (b.includes('ca_monte')) return tr("Les joueurs arrivent cette semaine.")
  if (b.includes('peu_de_concurrence')) return tr("Peu de maps pour autant de joueurs.")
  return tr("Des joueurs fidèles et de la place pour une nouvelle map.")
}

/** Concurrence : d'après la part des nouvelles maps qui percent et le poids des 3 premières. */
function competition(d: any): [string, string] {
  const open = d.axes?.open ?? 0
  if (open >= 70 && (d.top3_share ?? 100) < 60) return [tr("Peu de concurrence"), 'b-g']
  if (open >= 50) return [tr("Concurrence moyenne"), 'b-o']
  return [tr("Forte concurrence"), 'b-r']
}

/** Connexion d'une source : quelques étapes et le champ de la clé. Les clés ne repartent jamais vers le navigateur. */
function SourceSetup({ kind, sid, cfg, onDone, onClose }: { kind: Kind; sid: number; cfg: any; onDone: () => void; onClose: () => void }) {
  const { toast } = useApp()
  const [busy, setBusy] = useState(false)
  const integ = kind === 'tiktok' ? 'apify' : kind
  const save = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const f = e.currentTarget.elements as any
    const c: any = { ...(cfg ?? {}) }
    if (kind === 'youtube') c.key = f.a.value.trim()
    if (kind === 'tiktok' || kind === 'x') c.token = f.a.value.trim()
    if (kind === 'reddit') { c.client_id = f.a.value.trim(); c.key = f.b.value.trim() }
    if (!f.a.value.trim() || (kind === 'reddit' && !f.b.value.trim())) return
    setBusy(true)
    try {
      await api.put(`/api/studios/${sid}/integrations/${integ}`, { enabled: true, config: c })
      toast(tr("Connecté : la collecte démarre."))
      onDone()
      // Première collecte tout de suite (en arrière-plan), puis automatique.
      api.post(`/api/studios/${sid}/tiktok/refresh`).then(onDone).catch((er: any) => toast(er.message, true))
    } catch (er: any) { toast(er.message, true) } finally { setBusy(false) }
  }
  const link = (href: string, label: string) => <a href={href} target="_blank" rel="noreferrer">{label}</a>
  return <div className="afaire-connect">
    <div className="row" style={{ justifyContent: 'space-between' }}>
      <b style={{ fontWeight: 500 }}>{tr("Connecter {0}", [SOURCES.find(s => s.kind === kind)!.name])}</b>
      <button className="iconbtn" aria-label={tr("Fermer")} onClick={onClose}><Close size={16} /></button>
    </div>
    <ol className="small">
      {kind === 'youtube' && <>
        <li>{tr("Ouvrez")}{' '}{link('https://console.cloud.google.com/apis/library/youtube.googleapis.com', tr("la page YouTube Data API v3"))}{' '}{tr("et cliquez sur « Activer ».")}</li>
        <li>{tr("Identifiants → Créer des identifiants → Clé API, puis copiez la clé. C’est gratuit.")}</li>
      </>}
      {kind === 'tiktok' && <>
        <li>{tr("Créez un compte sur")}{' '}{link('https://apify.com', 'apify.com')}{' '}{tr("(5 $ de crédit offerts chaque mois).")}</li>
        <li>{tr("Settings → API & Integrations → copiez le « Personal API token ».")}</li>
      </>}
      {kind === 'x' && <>
        <li>{tr("Ouvrez la")}{' '}{link('https://console.x.com', tr("console développeur de X"))}{tr(", créez une application et ajoutez du crédit : la lecture des messages est payante chez X (environ 0,005 $ par message lu, 150 messages au plus par collecte).")}</li>
        <li>{tr("Keys and tokens → copiez le « Bearer Token ».")}</li>
      </>}
      {kind === 'reddit' && <>
        <li>{tr("Il faut une application Reddit déjà approuvée : Reddit n’accepte plus de nouvelles demandes d’accès à son API depuis le 31 octobre 2026.")}</li>
        <li>{tr("Sur")}{' '}{link('https://www.reddit.com/prefs/apps', 'reddit.com/prefs/apps')}{tr(", copiez l’identifiant (sous le nom de l’application) et le secret.")}</li>
      </>}
    </ol>
    <form className="row" style={{ gap: 8, flexWrap: 'wrap' }} onSubmit={save}>
      <input name="a" type={kind === 'reddit' ? 'text' : 'password'} className="field mono" style={{ flex: '1 1 220px' }} autoComplete="off"
        aria-label={kind === 'reddit' ? tr("Identifiant de l’application") : tr("Clé")}
        placeholder={kind === 'youtube' ? 'AIza…' : kind === 'tiktok' ? 'apify_api_…' : kind === 'x' ? 'AAAA…' : tr("Identifiant de l’application")} />
      {kind === 'reddit' && <input name="b" type="password" className="field mono" style={{ flex: '1 1 220px' }} autoComplete="off" aria-label={tr("Secret")} placeholder={tr("Secret")} />}
      <button className="btn pri" disabled={busy}>{tr("Connecter")}</button>
    </form>
  </div>
}

export default function MapsToMake() {
  const { me } = useApp()
  const nav = useNav()
  const isl = useIsland()
  const perso = me?.studios.find(s => s.personal)
  const skipKey = `pop.afaire.passe.${me?.user.id ?? ''}`
  const [skipped, setSkipped] = useState(() => { try { return localStorage.getItem(skipKey) === '1' } catch { return false } })
  const [open, setOpen] = useState<Kind | null>(null)
  const { data: vids, reload } = useLoad<any>(perso ? `/api/studios/${perso.id}/tiktok` : null, [perso?.id])
  const { data: integ, reload: reloadI } = useLoad<any>(perso ? `/api/studios/${perso.id}/integrations` : null, [perso?.id])
  const { data } = useLoad<any>('/api/ideas/niches', [])
  const src = vids?.sources ?? {}
  const anyOn = SOURCES.some(s => src[s.kind]?.configured)
  const skip = () => { try { localStorage.setItem(skipKey, '1') } catch { /* rien */ } setSkipped(true); setOpen(null) }
  const done = () => { reload(); reloadI() }
  const cfgOf = (k: Kind) => integ?.[k === 'tiktok' ? 'apify' : k]?.config ?? {}
  const setup = open && perso ? <SourceSetup kind={open} sid={perso.id} cfg={cfgOf(open)} onDone={done} onClose={() => setOpen(null)} /> : null

  const list = (data?.niches ?? [])
    .filter((d: any) => !d.risky && !(d.badges ?? []).includes('domine') && (d.axes?.open ?? 0) >= 50 && (d.players ?? 0) > 0)
    .slice(0, 10)

  // Premier passage : on propose les sources (facultatif), puis la liste.
  if (perso && vids && !anyOn && !skipped) return <main className="content">
    <section className="card afaire-start">
      <p className="small muted" style={{ margin: 0 }}>{tr("Étape 1 sur 2 · facultatif")}</p>
      <h1 className="afaire-h">{tr("Trouver les maps à faire")}</h1>
      <p className="muted" style={{ margin: 0 }}>{tr("Connectez une ou plusieurs sources pour voir aussi ce dont on parle en ce moment. Sinon, passez : POP analyse les joueurs, Discover et les nouvelles maps. Vous pourrez connecter une source à tout moment.")}</p>
      <div className="afaire-srcs">{SOURCES.map(({ kind, name, Icon }) => <div key={kind}>
        <div className="afaire-src"><Icon size={20} strokeWidth={1.75} /><span style={{ flex: 1, fontWeight: 500 }}>{name}</span>
          <button className="btn" onClick={() => setOpen(open === kind ? null : kind)}>{tr("Connecter")}</button></div>
        {open === kind && setup}
      </div>)}</div>
      <div className="row" style={{ gap: 8 }}>
        <button className="btn pri" style={{ flex: 1 }} onClick={skip}>{tr("Voir les maps à faire")}<ArrowRight size={15} /></button>
        <button className="btn ghost" onClick={skip}>{tr("Passer")}</button>
      </div>
    </section>
  </main>

  return <main className="content">
    <PageHead title={tr("Maps à faire")} help={tr("Les types de maps qui marchent en ce moment et où il reste de la place. Calculé chaque jour à partir des joueurs, de Discover, des nouvelles maps qui percent et, si vous les connectez, de YouTube, TikTok, X et Reddit.")} />

    {perso && vids && <div className="afaire-pills">
      <span className="small muted">{tr("Sources :")}</span>
      <span className="afaire-pill always">{tr("Joueurs, Discover, nouvelles maps · toujours")}</span>
      {SOURCES.map(({ kind, name, Icon }) => {
        const s = src[kind]
        const on = !!s?.configured
        return <button key={kind} className={'afaire-pill' + (on ? ' on' : '') + (s?.error && on ? ' err' : '')} aria-expanded={open === kind}
          title={on ? (s?.error ? s.error : tr("Connecté. Cliquez pour changer la clé.")) : tr("Connecter {0}", [name])}
          onClick={() => setOpen(open === kind ? null : kind)}>
          <Icon size={15} strokeWidth={1.75} />{name}{on ? <> · {s?.error ? tr("erreur") : <><Check size={13} />{tr("connecté")}</>}</> : ` · ${tr("Connecter")}`}
        </button>
      })}
    </div>}
    {setup}

    <section className="card">
      {!data ? <p className="muted">{tr("Chargement…")}</p> : !data.ready ? <Empty text={tr("Analyse en cours : POP récupère les chiffres des maps. Revenez dans quelques minutes.")} /> :
        list.length === 0 ? <Empty text={tr("Rien de sûr aujourd’hui : aucun type de map ne combine assez de joueurs et peu de concurrence. La liste est recalculée chaque jour.")} /> :
        <ol className="afaire">{list.map((d: any, i: number) => {
          const [c, cls] = competition(d)
          const ex = d.best_new ?? d.study?.[0]
          return <li key={d.key} className="afaire-row">
            <span className="afaire-n mono">{i + 1}</span>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div className="afaire-t">{(LANG === 'fr' ? d.fr : d.en) || d.fr}</div>
              <div className="small muted">{why(d)}</div>
            </div>
            <span className={'badge ' + cls}>{c}</span>
            {ex && <button className="btn" title={tr("Exemple qui marche : {0}", [ex.title]) + (ex.players ? ` · ${num(ex.players)} ${tr("joueurs")}` : '')} onClick={() => isl.open(ex.code)}>{tr("Voir")}</button>}
          </li>
        })}</ol>}
    </section>
    <p className="small muted afaire-more">{tr("Pour aller plus loin :")}{' '}
      {([['/analyse/idees', tr("Idées de maps")], ['/analyse/marche', tr("Marché")], ['/analyse/tendances', tr("Maps qui marchent")], ['/analyse/lancements', tr("Meilleurs démarrages")], ['/analyse/videos', tr("Tendances vidéo")]] as [string, string][])
        .map(([to, l], i) => <span key={to}>{i > 0 && ' · '}<button className="linkbtn small" onClick={() => nav(to)}>{l}</button></span>)}
    </p>
    {isl.drawer}
  </main>
}
