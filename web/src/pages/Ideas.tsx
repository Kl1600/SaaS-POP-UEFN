// Idées de maps : un fil de fiches, comme des annonces, calculées à partir du marché et adaptées au créateur.
import { useEffect, useMemo, useState } from 'react'
import { useSearchParams, useNavigate } from 'react-router-dom'
import { Lightbulb, Heart, X, Rocket, Bell, BellOff, TrendingUp, Users, Clock, DoorOpen, Sparkles } from '../icons'
import { ago, api } from '../api'
import { useApp, useLoad, studioName } from '../state'
import { CardHead, Empty, PageHead, Tabs, useDialog, Layer } from '../components/ui'
import { IslandDrawer } from './Analysis'
import { tr, LANG, LOCALE } from '../i18n/t'

export const GENRE: Record<string, string> = {
  aventure: tr("Aventure et RPG"), battle_royale: tr("Battle royale"), deathrun: tr("Deathrun et plateformes"), horreur: tr("Horreur"),
  musique: tr("Musique et rythme"), fete: tr("Fête et mini-jeux"), social: tr("Jeu de rôle et social"), tir: tr("Tir"),
  simulation: tr("Simulation et tycoon"), sports: tr("Sports et courses"), strategie: tr("Stratégie"), survie: tr("Survie"),
}
const GENRE_HUE: Record<string, number> = { aventure: 150, battle_royale: 25, deathrun: 45, horreur: 280, musique: 320, fete: 340, social: 200, tir: 5, simulation: 95, sports: 175, strategie: 230, survie: 60 }
const BADGE: Record<string, [string, string]> = {
  vague: [tr("Vague naissante"), 'b-v'], porte_ouverte: [tr("Porte ouverte"), 'b-g'], ca_monte: [tr("Ça monte"), 'b-g'], joueurs_decus: [tr("Joueurs déçus"), 'b-o'],
  tendance_video: [tr("Tendance vidéo"), 'b-v'], peu_de_concurrence: [tr("Peu de concurrence"), 'b-g'], joueurs_fideles: [tr("Joueurs fidèles"), ''],
  nouveau_mot: [tr("Nouveau mot"), 'b-v'], saison: [tr("C’est la saison"), 'b-o'], domine: [tr("Dominée par 3 maps"), 'b-r'],
}
const AXES: [string, string, string][] = [
  ['demand', tr("Demande"), tr("Joueurs dans la niche et joueurs d’une map typique.")],
  ['fidelity', tr("Fidélité"), tr("Temps de jeu, retour à J+1 et J+7, recommandations : ce qu’Epic note par joueur pour Discover.")],
  ['open', tr("Porte ouverte"), tr("Part des maps sorties depuis 30 jours qui ont percé, et poids des 3 premières maps.")],
  ['gap', tr("Place à prendre"), tr("Beaucoup de joueurs mais des maps en tête qui les retiennent mal.")],
  ['momentum', tr("Élan"), tr("Progression des joueurs sur 7 et 14 jours, et mentions dans les vidéos récentes.")],
]

const num = (v?: number | null) => v === null || v === undefined ? '—' : v >= 1e6 ? (v / 1e6).toFixed(1).replace('.', ',') + ' M' : v >= 1e4 ? Math.round(v / 1e3) + ' k' : Math.round(v).toLocaleString(LOCALE)
const pc = (v?: number | null) => v === null || v === undefined ? '—' : `${Math.round(v * 100)} %`
const label = (x: any) => (LANG === 'fr' ? x.fr : x.en) || x.fr
const sign = (v?: number | null) => v === null || v === undefined ? '—' : `${v > 0 ? '+' : ''}${v} %`

/** La phrase d'accroche : le point fort le plus parlant de la niche. */
function hook(d: any): string {
  const b: string[] = d.badges ?? []
  if (b.includes('vague')) return tr("Une vague qui naît : les joueurs arrivent ({0} en 7 jours) et il y a encore peu de maps.", [sign(d.trend)])
  if (b.includes('porte_ouverte')) return tr("Les nouvelles maps y percent : {0} sur {1} sorties ce mois-ci ont dépassé 100 joueurs.", [d.success100, d.newcomers])
  if (b.includes('joueurs_decus')) return tr("Beaucoup de joueurs, mais les maps en tête les retiennent mal : il y a mieux à faire.")
  if (b.includes('tendance_video')) return tr("Le sujet monte en vidéo : {0} vidéos cette semaine contre {1} la semaine d’avant.", [d.videos, d.videos_prev])
  if (b.includes('ca_monte')) return tr("Les joueurs arrivent : {0} en 7 jours.", [sign(d.trend)])
  if (b.includes('peu_de_concurrence')) return tr("Peu de maps pour autant de joueurs.")
  if (b.includes('joueurs_fideles')) return tr("Des joueurs fidèles : {0} min par partie en moyenne.", [num(d.minutes)])
  return tr("Une niche solide, avec {0} joueurs hier.", [num(d.players)])
}

function Ring({ v }: { v: number }) {
  const c = 2 * Math.PI * 16
  return <svg className="iring" width="44" height="44" viewBox="0 0 44 44" aria-label={tr("Note {0} sur 100", [v])}>
    <circle cx="22" cy="22" r="16" fill="none" stroke="currentColor" strokeOpacity=".15" strokeWidth="4" />
    <circle cx="22" cy="22" r="16" fill="none" stroke="currentColor" strokeWidth="4" strokeLinecap="round" strokeDasharray={`${(v / 100) * c} ${c}`} transform="rotate(-90 22 22)" />
    <text x="22" y="26.5" textAnchor="middle" fontSize="13" fontWeight="600" fill="currentColor">{v}</text>
  </svg>
}

/** Joueurs par jour de la niche, avec axes et unités (#202). */
function PlayersChart({ series }: { series: any[] }) {
  const v = series.map(s => s.players ?? 0)
  if (v.length < 2) return null
  const W = 520, H = 180, L = 48, B = 26, T = 10, R = 22
  const max = Math.max(1, ...v) * 1.1
  const x = (i: number) => L + (i / (v.length - 1)) * (W - L - R)
  const y = (p: number) => T + (1 - p / max) * (H - T - B)
  const ticks = [0, max / 2, max].map(t => Math.round(t))
  const pts = v.map((p, i) => `${x(i)},${y(p)}`).join(' ')
  const day = (d: string) => { const [, m, j] = String(d).split('-'); return j && m ? `${Number(j)}/${Number(m)}` : d }
  return <svg className="ichart" viewBox={`0 0 ${W} ${H}`} role="img" aria-label={tr("Joueurs par jour sur les {0} derniers jours", [v.length])}>
    {ticks.map(t => <g key={t}><line x1={L} x2={W - R} y1={y(t)} y2={y(t)} className="ichart-grid" /><text x={L - 6} y={y(t) + 4} textAnchor="end" className="ichart-lab">{num(t)}</text></g>)}
    <line x1={L} x2={L} y1={T} y2={H - B} className="ichart-axis" /><line x1={L} x2={W - R} y1={H - B} y2={H - B} className="ichart-axis" />
    <polyline points={pts} fill="none" stroke="currentColor" strokeWidth="2" />
    {v.map((p, i) => <circle key={i} cx={x(i)} cy={y(p)} r="3" fill="currentColor"><title>{tr("{0} : {1} joueurs", [day(series[i].day), num(p)])}</title></circle>)}
    {series.map((s, i) => <text key={i} x={x(i)} y={H - 6} textAnchor="middle" className="ichart-lab">{day(s.day)}</text>)}
  </svg>
}

function Card({ d, onOpen, onSave }: { d: any; onOpen: () => void; onSave: () => void }) {
  const hue = GENRE_HUE[d.genre] ?? 200
  return <article className={'icard' + (d.status === 'new' ? ' fresh' : '')} onClick={onOpen} onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onOpen() } }} tabIndex={0} aria-label={label(d)}>
    <div className="icard-top" style={{ ['--h' as any]: hue }}>
      <span className="icard-genre">{GENRE[d.genre] ?? d.genre}</span>
      {d.status === 'new' && <span className="icard-new">{tr("Nouveau")}</span>}
      <Ring v={Math.round(d.score)} />
    </div>
    <div className="icard-body">
      <div className="row" style={{ justifyContent: 'space-between', alignItems: 'flex-start', gap: 8 }}>
        <h3>{label(d)}</h3>
        <button className={'iconbtn icard-save' + (d.status === 'saved' ? ' on' : '')} aria-label={d.status === 'saved' ? tr("Retirer des sauvegardes") : tr("Sauvegarder")} onClick={e => { e.stopPropagation(); onSave() }}><Heart size={18} /></button>
      </div>
      <p className="icard-hook">{hook(d)}</p>
      <div className="icard-badges">{(d.badges ?? []).filter((b: string) => BADGE[b]).slice(0, 3).map((b: string) => <span key={b} className={'badge ' + BADGE[b][1]}>{BADGE[b][0]}</span>)}</div>
      <div className="icard-stats">
        <div><Users size={15} /><b>{num(d.players)}</b><span>{tr("joueurs par jour")}</span></div>
        <div><Clock size={15} /><b>{num(d.minutes)} min</b><span>{tr("temps de jeu moyen")}</span></div>
        <div><DoorOpen size={15} /><b>{d.newcomers ? `${d.success100}/${d.newcomers}` : '—'}</b><span>{tr("nouvelles maps qui percent")}</span></div>
        <div><TrendingUp size={15} /><b>{sign(d.trend)}</b><span>{tr("évolution sur 7 jours")}</span></div>
      </div>
    </div>
    <div className="icard-foot small muted">
      <span>{d.status === 'started' ? tr("Lancée") : ''}</span>
      <span>{d.created_at ? ago(d.created_at) : ''}</span>
    </div>
  </article>
}

function Axis({ k, name, help, v }: { k: string; name: string; help: string; v: number }) {
  return <div className="iaxis" title={help}>
    <span className="iaxis-n">{name}</span>
    <span className={'iaxis-bar a-' + k}><i style={{ width: `${v}%` }} /></span>
    <span className="iaxis-v">{v}</span>
  </div>
}

function Detail({ d, onClose, onState, onStart }: { d: any; onClose: () => void; onState: (s: string) => void; onStart: () => void }) {
  const dlg = useDialog(onClose)
  const [isl, setIsl] = useState<string | null>(null)
  if (isl) return <IslandDrawer code={isl} onClose={() => setIsl(null)} />
  const Map = ({ m }: { m: any }) => <button className="ilink" onClick={() => setIsl(m.code)}><b>{m.title}</b><span className="small muted">{tr("{0} joueurs · {1} min · J+1 {2}", [num(m.players), num(m.minutes), pc(m.d1)])}</span></button>
  return <Layer>
    <div className="scrim" onClick={onClose} />
    <aside ref={dlg as any} tabIndex={-1} aria-modal="true" role="dialog" className="drawer idrawer" aria-label={tr("Fiche idée")}>
      <div className="idrawer-head" style={{ ['--h' as any]: GENRE_HUE[d.genre] ?? 200 }}>
        <div className="row" style={{ justifyContent: 'space-between' }}>
          <span className="icard-genre">{GENRE[d.genre] ?? d.genre}</span>
          <button className="iconbtn" aria-label={tr("Fermer")} onClick={onClose}><X size={18} /></button>
        </div>
        <div className="row" style={{ gap: 14, marginTop: 10 }}>
          <Ring v={Math.round(d.score)} />
          <div><h2>{label(d)}</h2><p className="small" style={{ margin: 0, opacity: .85 }}>{tr("Note d’opportunité sur 100 · données {0}", [d.confidence === 'bonne' ? tr("solides") : d.confidence === 'moyenne' ? tr("moyennes") : tr("minces")])}</p></div>
        </div>
      </div>
      <div className="itop">
        <p className="ihook">{hook(d)}</p>
        <div className="icard-badges">{(d.badges ?? []).filter((b: string) => BADGE[b]).map((b: string) => <span key={b} className={'badge ' + BADGE[b][1]}>{BADGE[b][0]}</span>)}</div>
          <div className="row" style={{ gap: 8, margin: '14px 0 0', flexWrap: 'wrap' }}>
            {d.status === 'started' ? <span className="badge b-g">{tr("Vous vous êtes lancé")}</span> : <button className="btn pri" onClick={onStart}><Rocket size={16} />{tr("Je me lance")}</button>}
            {d.status !== 'started' && <button className="btn" onClick={() => onState(d.status === 'saved' ? 'seen' : 'saved')}><Heart size={16} />{d.status === 'saved' ? tr("Sauvegardée") : tr("Sauvegarder")}</button>}
            {d.status !== 'started' && <button className="btn" onClick={() => onState('dismissed')}><X size={16} />{tr("Pas pour moi")}</button>}
          </div>

      </div>
      <div className="icols">
        <div>
          <div className="dsec">{tr("Pourquoi cette niche")}</div>
          {AXES.map(([k, n, h]) => <Axis key={k} k={k} name={n} help={h} v={d.axes?.[k] ?? 0} />)}
          <ul className="ifacts">
            <li>{tr("{0} maps jouées hier, {1} joueurs ; une map typique en a {2}.", [num(d.islands), num(d.players), num(d.typical)])}</li>
            <li>{tr("Temps de jeu moyen : {0} min (map typique de Fortnite : {1} min). Retour à J+1 : {2}, à J+7 : {3}.", [num(d.minutes), num(d.eco?.minutes), pc(d.d1), pc(d.d7)])}</li>
            <li>{d.newcomers ? tr("{0} maps sorties depuis 30 jours : {1} ont dépassé 100 joueurs, {2} ont dépassé 1 000.", [d.newcomers, d.success100, d.success1000]) : tr("Pas assez de nouvelles maps ce mois-ci pour juger si la porte est ouverte.")}</li>
            <li>{tr("Les 3 premières maps prennent {0} % des joueurs.", [d.top3_share])}</li>
            <li>{tr("Évolution : {0} sur 7 jours{1}.", [sign(d.trend), d.trend14 !== null && d.trend14 !== undefined ? tr(", {0} sur 14 jours", [sign(d.trend14)]) : ''])}</li>
            {(d.videos > 0 || d.videos_prev > 0) && <li>{tr("Vidéos TikTok et YouTube qui en parlent : {0} cette semaine, {1} la semaine d’avant.", [d.videos, d.videos_prev])}</li>}
          </ul>

        </div>
        <div>
          <div className="dsec">{tr("Chiffres à battre")}</div>
          <p className="small muted" style={{ marginTop: 0 }}>{tr("Ce que font les meilleures maps de la niche : ce qu’Epic regarde pour Discover.")}</p>
          <div className="itargets">
            <div><b>{num(d.targets?.minutes)} min</b><span>{tr("de temps de jeu moyen")}</span></div>
            <div><b>{pc(d.targets?.d1)}</b><span>{tr("de joueurs qui reviennent à J+1")}</span></div>
          </div>

          {(d.series ?? []).length > 1 && <><div className="dsec">{tr("Joueurs par jour")}</div><PlayersChart series={d.series} /></>}
        </div>
      </div>
      <div className="icols">
        <div>
          <div className="dsec">{tr("Maps à étudier")}</div>
          <div className="stack" style={{ gap: 6 }}>{(d.study ?? []).map((m: any) => <Map key={m.code} m={m} />)}</div>
          {d.best_new && <><p className="small muted" style={{ margin: '10px 0 6px' }}>{tr("Meilleure nouvelle venue du mois :")}</p><Map m={d.best_new} /></>}

        </div>
        <div>
          {(d.twists ?? []).length > 0 && <>
            <div className="dsec">{tr("Idées de twist")}</div>
            {(d.twists ?? []).map((t: any) => <p key={t.key} className="itwist"><Sparkles size={16} /><span>{tr("Croiser avec « {0} » : niche notée {1}/100, et seulement {2} maps combinent les deux.", [label(t), t.score, t.maps_together])}</span></p>)}
          </>}

          <div className="dsec">{tr("Pièges à éviter")}</div>
          <ul className="ifacts">
            {(d.title_words ?? []).length > 0 && <li>{tr("Mots déjà très repris dans les titres : {0}. Un titre ou une vignette trop proche d’une map existante est moins montré par Epic.", [d.title_words.join(', ')])}</li>}
            <li>{tr("Vignette originale qui montre le vrai jeu : pas de fond uni ou en dégradé, pas d’armes collées, pas de badges d’Epic.")}</li>
            <li>{tr("Dans le Creator Portal, choisissez le genre « {0} » : il décide de la rangée où la map peut apparaître.", [GENRE[d.genre] ?? d.genre])}</li>
            {(d.badges ?? []).includes('domine') && <li>{tr("Trois maps prennent l’essentiel des joueurs : il faudra un vrai angle différent.")}</li>}
          </ul>

        </div>
      </div>
      <p className="small muted" style={{ marginTop: 16 }}>{tr("Calculé sur les chiffres publics d’Epic du {0}. Les idées sont recalculées après chaque collecte.", [d.generated_day ?? ''])}</p>
    </aside>
  </Layer>
}

function StartModal({ d, onClose }: { d: any; onClose: () => void }) {
  const dlg = useDialog(onClose)
  const { me, toast, reload: reloadMe } = useApp()
  const navigate = useNavigate()
  const studios = (me?.studios ?? []).filter((s: any) => ['owner', 'admin'].includes(s.role))
  const [sid, setSid] = useState<number>(studios[0]?.id)
  const [name, setName] = useState(label(d))
  const go = async () => {
    try {
      const r = await api.post(`/api/ideas/${d.id}/start`, { studio_id: sid, name })
      await reloadMe()
      toast(tr("Map créée avec la fiche de l’idée"))
      navigate(`/map/${r.map_id}/pages/${r.page_id}`)
    } catch (e: any) { toast(e.message, true) }
  }
  return <><div className="scrim" onClick={onClose} style={{ zIndex: 50 }} />
    <div ref={dlg as any} tabIndex={-1} aria-modal="true" className="modal" role="dialog" aria-label={tr("Je me lance")} style={{ zIndex: 51 }}>
      <h2 style={{ marginTop: 0, fontWeight: 500 }}>{tr("Je me lance")}</h2>
      <p className="small muted">{tr("POP crée la map et y range la fiche de l’idée (chiffres, maps à étudier, objectifs) dans Pages.")}</p>
      <label className="label">{tr("Nom de la map")}<input className="field" maxLength={60} value={name} onChange={e => setName(e.target.value)} /></label>
      <label className="label" style={{ marginTop: 10 }}>{tr("Où")}<select className="field" value={sid} onChange={e => setSid(+e.target.value)}>{studios.map((s: any) => <option key={s.id} value={s.id}>{studioName(s)}</option>)}</select></label>
      <div className="row" style={{ gap: 8, marginTop: 16, justifyContent: 'flex-end' }}><button className="btn" onClick={onClose}>{tr("Annuler")}</button><button className="btn pri" disabled={!sid || !name.trim()} onClick={go}><Rocket size={16} />{tr("Créer la map")}</button></div>
    </div></>
}

function AllNiches() {
  const { data } = useLoad<any>('/api/ideas/niches')
  const [open, setOpen] = useState<any>(null)
  if (!data) return null
  return <section className="card">
    <CardHead title={tr("Toutes les niches")} sub={tr("{0} niches notées sur {1} maps jouées hier.", [data.niches?.length ?? 0, num(data.islands)])} />
    <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>{tr("Niche")}</th><th>{tr("Genre")}</th><th>{tr("Note")}</th>{AXES.map(([k, n]) => <th key={k}>{n}</th>)}<th>{tr("Maps")}</th><th>{tr("Joueurs")}</th><th>7 j</th></tr></thead>
      <tbody>{(data.niches ?? []).map((n: any) => <tr key={n.key} style={{ cursor: 'pointer', opacity: n.risky ? .5 : 1 }} tabIndex={0} onClick={() => setOpen(n)} onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setOpen(n) } }}>
        <td><b style={{ fontWeight: 500 }}>{label(n)}</b>{n.risky && <span className="small muted"> · {tr("pénalisé par Epic")}</span>}</td><td className="small">{GENRE[n.genre]}</td><td><b>{n.score}</b></td>
        {AXES.map(([k]) => <td key={k} className="small">{n.axes?.[k]}</td>)}<td>{n.islands}</td><td>{num(n.players)}</td><td className="small">{sign(n.trend)}</td></tr>)}</tbody></table></div>
    {open && <Detail d={open} onClose={() => setOpen(null)} onState={() => {}} onStart={() => {}} />}
  </section>
}

export default function Ideas() {
  const { toast } = useApp()
  const [sp, setSp] = useSearchParams()
  const { data, reload } = useLoad<any>('/api/ideas')
  const [tab, setTab] = useState<'feed' | 'saved' | 'all'>('feed')
  const [start, setStart] = useState<any>(null)
  const openId = Number(sp.get('i')) || null
  const ideas: any[] = data?.ideas ?? []
  const open = useMemo(() => ideas.find(x => x.id === openId) ?? null, [ideas, openId])
  const setState = async (id: number, status: string) => {
    try { await api.post(`/api/ideas/${id}/state`, { status }); if (status === 'dismissed') { setSp({}, { replace: true }); toast(tr("Idée retirée : on ne vous la proposera plus")) } reload() } catch (e: any) { toast(e.message, true) }
  }
  // Ouvrir une fiche la marque comme vue.
  useEffect(() => { if (open?.status === 'new') setState(open.id, 'seen') }, [open?.id])
  if (!data) return <main className="content"><PageHead title={tr("Idées de maps")} help={tr("Des idées tirées du marché.")} /></main>
  const shown = tab === 'saved' ? ideas.filter(x => x.status === 'saved' || x.status === 'started') : ideas.filter(x => x.active)
  const fresh = ideas.filter(x => x.status === 'new').length
  return <main className="content">
    <PageHead title={tr("Idées de maps")} help={tr("POP découpe le marché en niches, note chacune sur la demande, la fidélité des joueurs, la porte ouverte aux nouvelles maps, la place à prendre et l’élan, et vous montre les meilleures.")}>
      <button className="btn" onClick={async () => { await api.put('/api/ideas/profile', { notify: !data.notify }); reload(); toast(data.notify ? tr("Alertes coupées") : tr("Vous serez alerté des nouvelles idées")) }}>{data.notify ? <Bell size={16} /> : <BellOff size={16} />}{data.notify ? tr("Alertes activées") : tr("Alertes coupées")}</button>
    </PageHead>
    <Tabs value={tab} onChange={setTab} items={[['feed', fresh ? tr("Pour vous ({0} nouvelles)", [fresh]) : tr("Pour vous")], ['saved', tr("Sauvegardées")], ['all', tr("Toutes les niches")]]} />
    {tab === 'all' ? <AllNiches /> : !data.ready ? <section className="card"><Empty text={tr("Pas encore de données du marché : la première collecte prend quelques minutes.")} /></section>
      : shown.length === 0 ? <section className="card"><Empty text={tab === 'saved' ? tr("Aucune idée sauvegardée. Touchez le cœur d’une fiche pour la garder.") : tr("Pas encore d’idée : elles arrivent après la prochaine collecte.")} /></section>
      : <div className="igrid">{shown.map(d => <Card key={d.id} d={d} onOpen={() => setSp({ i: String(d.id) })} onSave={() => setState(d.id, d.status === 'saved' ? 'seen' : 'saved')} />)}</div>}
    {open && <Detail d={open} onClose={() => setSp({}, { replace: true })} onState={s => setState(open.id, s)} onStart={() => setStart(open)} />}
    {start && <StartModal d={start} onClose={() => setStart(null)} />}
    {data.model && <p className="small muted imodel">{data.model.calibration?.samples >= 30
      ? tr("Les poids des axes sont appris : POP a vérifié {0} fois après coup si une niche bien notée a vraiment fait percer ses nouvelles maps.", [data.model.calibration.samples])
      : tr("Les poids des axes sont ceux de départ : ils s’ajusteront tout seuls quand POP aura assez d’historique pour vérifier ses notes après coup (environ un mois de données).")}</p>}
  </main>
}

/** Alerte en bas de l'écran quand une nouvelle idée arrive, comme une nouvelle annonce. */
export function IdeaPop() {
  const { me } = useApp()
  const navigate = useNavigate()
  const [item, setItem] = useState<any>(null)
  useEffect(() => {
    if (!me?.analyse) return
    const seen = (): number[] => { try { return JSON.parse(localStorage.getItem('pop-idea-pop') ?? '[]') } catch { return [] } }
    const check = () => api.get('/api/notifications').then((r: any) => {
      const it = (r.items ?? []).find((n: any) => !n.read && String(n.link).startsWith('/analyse/idees') && !seen().includes(n.id))
      if (it) setItem(it)
    }).catch(() => {})
    check()
    const t = setInterval(check, 120000)
    return () => clearInterval(t)
  }, [me?.analyse])
  if (!item) return null
  const close = () => { try { localStorage.setItem('pop-idea-pop', JSON.stringify([...JSON.parse(localStorage.getItem('pop-idea-pop') ?? '[]'), item.id].slice(-100))) } catch { /* stockage indisponible */ } setItem(null) }
  return <div className="ipop" role="status">
    <span className="ipop-ic"><Lightbulb size={20} /></span>
    <div className="ipop-txt"><b>{tr("Nouvelle idée de map")}</b><span>{String(item.text).replace(/^Nouvelle idée de map : /, '')}</span></div>
    <button className="btn pri" onClick={() => { close(); navigate(item.link) }}>{tr("Voir")}</button>
    <button className="iconbtn" aria-label={tr("Fermer")} onClick={close}><X size={16} /></button>
  </div>
}
