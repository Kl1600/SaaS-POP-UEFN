// Onglet « Discover » de Lancer et suivre : être prêt pour l'algorithme d'Epic, puis suivre les 14 jours de test.
import { useEffect, useRef, useState } from 'react'
import { Lock, CheckCircle2, AlertTriangle, XCircle, Upload, TrendingUp, Rocket, Film, Megaphone, Type, Shapes, Image as ImageIcon, RefreshCw } from 'lucide-react'
import { api } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead } from '../components/ui'
import { GENRE } from './Ideas'
import { tr, LANG, LOCALE } from '../i18n/t'

const num = (v?: number | null) => v === null || v === undefined ? '—' : v >= 1e4 ? Math.round(v / 1e3) + ' k' : Math.round(v).toLocaleString(LOCALE)
const pc = (v?: number | null) => v === null || v === undefined ? '—' : `${Math.round(v * 100)} %`
const label = (x: any) => (LANG === 'fr' ? x.fr : x.en) || x.fr

const TITLE_ISSUE: Record<string, [string, string]> = {
  copie: [tr("Ce titre existe déjà"), tr("Epic attribue un titre au premier développeur qui l’a utilisé : une copie est beaucoup moins montrée dans Discover.")],
  proche: [tr("Titres très proches dans le catalogue"), tr("Les îles trop semblables à une île déjà publiée sont moins montrées. Trouvez un nom qui vous appartient.")],
  bourrage: [tr("Ressemble à du bourrage de mots-clés"), tr("Trop de mots, de genres ou de mots « appâts » (code, XP, best, free). Epic déclasse ces titres depuis avril 2026.")],
  mots_repris: [tr("Mots déjà très repris dans votre niche"), tr("Ils noient votre map parmi les autres. Gardez un seul mot de genre, et ajoutez ce qui vous distingue.")],
  majuscules: [tr("Trop de majuscules"), tr("Un titre tout en majuscules fait « spam » et se lit mal sur une petite vignette.")],
  symboles: [tr("Trop de symboles"), tr("Emojis et symboles brouillent la recherche et le titre sur la vignette.")],
  trop_long: [tr("Titre long"), tr("Au-delà de 40 caractères, le titre est coupé sur la vignette. Visez court et clair.")],
}
const THUMB_ISSUE: Record<string, [string, string]> = {
  basse_resolution: [tr("Résolution trop basse"), tr("Epic montre moins les vignettes en basse résolution. Exportez en 1920 × 1080.")],
  format: [tr("Pas au format 16:9"), tr("La vignette est recadrée en 16:9 : le bord de votre image sera coupé.")],
  fond_uni: [tr("Grande zone d’une seule couleur"), tr("Les fonds unis sont refusés automatiquement par Epic. Montrez une vraie scène du jeu.")],
  degrade: [tr("Fond en dégradé"), tr("Les fonds en dégradé sont refusés automatiquement. Utilisez un décor de la map.")],
  vide: [tr("Image presque vide"), tr("Peu de détails : la vignette ne dit pas à quoi on joue. Montrez l’action, un personnage, l’objectif.")],
}
const SIGNAL: Record<string, [string, string]> = {
  minutes: [tr("Temps de jeu moyen"), tr("Les joueurs partent vite. Retravaillez les 5 premières minutes : objectif clair tout de suite, moins d’attente, une première récompense rapide.")],
  d1: [tr("Retour le lendemain (J+1)"), tr("Peu de joueurs reviennent le lendemain. Donnez une raison de revenir : progression sauvegardée, défi du jour, récompense de connexion, classement.")],
  d7: [tr("Retour à une semaine (J+7)"), tr("La map ne fidélise pas sur la semaine. Ajoutez des objectifs longs (niveaux, collection) et annoncez vos prochaines mises à jour.")],
  rec: [tr("Recommandations par joueur"), tr("Peu de joueurs recommandent la map. Demandez-le au bon moment (après une victoire) et corrigez ce qui frustre : bugs, attente, équilibre.")],
}
const FACTOR: Record<string, [string, string]> = {
  players: [tr("Beaucoup de joueurs dès les premiers jours"), tr("Peu de joueurs les premiers jours : amenez-en par vos propres canaux")],
  minutes: [tr("Les joueurs restent longtemps"), tr("Les parties sont courtes : retravaillez les premières minutes")],
  d1: [tr("Les joueurs reviennent le lendemain"), tr("Peu de joueurs reviennent le lendemain")],
  growth: [tr("Les joueurs augmentent de jour en jour"), tr("Les joueurs baissent depuis la sortie")],
}
const GRADE: Record<string, [string, string]> = { top: [tr("Au niveau des meilleures"), 'b-g'], bon: [tr("Au-dessus de la niche"), 'b-g'], moyen: [tr("Un peu en dessous"), 'b-o'], faible: [tr("Nettement en dessous"), 'b-r'], inconnu: [tr("Pas encore mesuré"), ''] }

/** Analyse de la vignette dans le navigateur : rien n'est envoyé. */
async function analyzeThumb(file: File) {
  const img = await createImageBitmap(file)
  const w = img.width, h = img.height
  const W = 160, H = 90
  const c = document.createElement('canvas'); c.width = W; c.height = H
  const ctx = c.getContext('2d')!
  ctx.drawImage(img, 0, 0, W, H)
  const d = ctx.getImageData(0, 0, W, H).data
  const lum = new Float32Array(W * H)
  const counts = new Map<number, number>()
  for (let i = 0; i < W * H; i++) {
    const r = d[i * 4], g = d[i * 4 + 1], b = d[i * 4 + 2]
    lum[i] = 0.299 * r + 0.587 * g + 0.114 * b
    const k = ((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4)
    counts.set(k, (counts.get(k) ?? 0) + 1)
  }
  const top = Math.max(...counts.values()) / (W * H)
  let edge = 0
  for (let y = 0; y < H - 1; y++) for (let x = 0; x < W - 1; x++) { const i = y * W + x; edge += Math.abs(lum[i] - lum[i + 1]) + Math.abs(lum[i] - lum[i + W]) }
  edge /= (W - 1) * (H - 1)
  let lo = 255, hi = 0; for (const v of lum) { lo = Math.min(lo, v); hi = Math.max(hi, v) }
  const issues: string[] = []
  if (w < 1920 || h < 1080) issues.push('basse_resolution')
  if (Math.abs(w / h - 16 / 9) > 0.05) issues.push('format')
  if (top > 0.4) issues.push('fond_uni')
  else if (edge < 4 && hi - lo > 40) issues.push('degrade')
  if (edge < 6 && !issues.includes('fond_uni') && !issues.includes('degrade')) issues.push('vide')
  const pen: Record<string, number> = { fond_uni: 45, degrade: 40, vide: 25, basse_resolution: 20, format: 10 }
  const score = Math.max(0, 100 - issues.reduce((s, i) => s + pen[i], 0))
  return { w, h, score, issues, edge: Math.round(edge * 10) / 10, top: Math.round(top * 100) }
}

function Mark({ ok, warn }: { ok: boolean; warn?: boolean }) {
  return ok ? <CheckCircle2 size={18} className="ok-ic" /> : warn ? <AlertTriangle size={18} className="warn-ic" /> : <XCircle size={18} className="bad-ic" />
}

function Chart({ t }: { t: any }) {
  const pts = (t.series ?? []).filter((s: any) => s.age !== null && s.age >= 0 && s.players !== null)
  const bench = t.bench ?? []
  const maxAge = Math.max(14, ...pts.map((p: any) => p.age), ...bench.map((b: any) => b.age))
  const maxY = Math.max(10, ...pts.map((p: any) => p.players), ...bench.map((b: any) => b.p75 ?? 0))
  const X = (a: number) => 40 + (a / maxAge) * 600, Y = (v: number) => 200 - (v / maxY) * 180
  const band = bench.length >= 2 ? bench.map((b: any) => `${X(b.age)},${Y(b.p75)}`).join(' ') + ' ' + [...bench].reverse().map((b: any) => `${X(b.age)},${Y(b.p25)}`).join(' ') : ''
  return <svg className="dchart" viewBox="0 0 660 230" role="img" aria-label={tr("Joueurs par jour depuis la sortie, face aux nouvelles maps de la niche")}>
    {[0, 0.5, 1].map(f => <g key={f}><line x1="40" x2="640" y1={Y(maxY * f)} y2={Y(maxY * f)} className="dgrid" /><text x="34" y={Y(maxY * f) + 4} textAnchor="end" className="dclab">{num(maxY * f)}</text></g>)}
    <rect x={X(0)} y="20" width={X(14) - X(0)} height="180" className="dtest" />
    <text x={X(14) - 4} y="32" textAnchor="end" className="dclab">{tr("14 jours de test d’Epic")}</text>
    {band && <polygon points={band} className="dband" />}
    {bench.length >= 2 && <polyline points={bench.map((b: any) => `${X(b.age)},${Y(b.p50)}`).join(' ')} className="dmed" />}
    {pts.length >= 1 && <polyline points={pts.map((p: any) => `${X(p.age)},${Y(p.players)}`).join(' ')} className="dme" />}
    {pts.map((p: any) => <circle key={p.day} cx={X(p.age)} cy={Y(p.players)} r="3.5" className="dmedot"><title>{tr("J+{0} : {1} joueurs", [p.age, num(p.players)])}</title></circle>)}
    {[0, 7, 14, maxAge].filter((v, i, a) => a.indexOf(v) === i).map(a => <text key={a} x={X(a)} y="222" textAnchor="middle" className="dclab">{tr("J+{0}", [a])}</text>)}
  </svg>
}

export default function LaunchDiscover() {
  const { me, studio, map, canEdit, isAdmin, toast, reload: reloadMe } = useApp()
  const allowed = !!(me?.analyse || studio?.analyse)
  const [title, setTitle] = useState<string | null>(null)
  const [q, setQ] = useState('')
  useEffect(() => { const t = setTimeout(() => setQ(title ?? ''), 500); return () => clearTimeout(t) }, [title])
  const { data, reload } = useLoad<any>(allowed && map ? `/api/maps/${map.id}/discover${q ? `?title=${encodeURIComponent(q)}` : ''}` : null, [map?.id, q, allowed])
  const [preview, setPreview] = useState<string | null>(null)
  const [code, setCode] = useState('')
  const [upd, setUpd] = useState(new Date().toISOString().slice(0, 10))
  const fileRef = useRef<HTMLInputElement>(null)
  useEffect(() => { if (data && title === null) setTitle(data.title) }, [data])

  if (!allowed) return <section className="card locked">
    <span className="locked-ic"><Lock size={22} strokeWidth={1.5} /></span>
    <h2>{tr("Disponible avec le plan Pro")}</h2>
    <p className="muted">{tr("L’onglet Discover prépare votre sortie selon les règles de l’algorithme d’Epic et suit votre map pendant ses 14 jours de test. Il est compris dans le plan Pro.")}</p>
  </section>
  if (!data) return null

  const save = async (b: any, msg?: string) => { try { await api.post(`/api/maps/${map!.id}/discover`, b); if (msg) toast(msg); reload() } catch (e: any) { toast(e.message, true) } }
  const tc = data.title_check
  const thumb = data.thumb ?? {}
  const genreOk = !!data.genre && [data.genre, GENRE[data.genre]].includes(data.map_genre)
  const parts = [
    { k: 'title', w: 30, v: tc.score / 100 },
    { k: 'genre', w: 10, v: genreOk ? 1 : 0 },
    { k: 'thumb', w: 30, v: thumb.score !== undefined ? thumb.score / 100 : 0 },
    { k: 'trailer', w: 10, v: data.trailer ? 1 : 0 },
    { k: 'channels', w: 20, v: data.channels ? 1 : 0 },
  ]
  const ready = Math.round(parts.reduce((s, p) => s + p.w * p.v, 0))
  const t = data.tracking
  const phase = t?.phase ?? 'avant'

  const onFile = async (f?: File | null) => {
    if (!f) return
    try { const r = await analyzeThumb(f); setPreview(URL.createObjectURL(f)); await save({ thumb: r }, tr("Vignette analysée")) } catch { toast(tr("Image illisible : utilisez un PNG ou un JPG."), true) }
  }
  const addChannels = async () => {
    const steps = [['18:00', tr("Publier la map")], ['18:05', tr("Post TikTok avec le code de l’île")], ['18:10', tr("Message Discord avec le code et le lien")], ['18:30', tr("Short YouTube ou story avec le code")], ['20:00', tr("Relancer : « venez battre mon score »")]]
    try { for (const [time, title] of steps) await api.post(`/api/maps/${map!.id}/launch-steps`, { time, title }); await save({ channels: true }, tr("Ajouté à la checklist du Jour J")) } catch (e: any) { toast(e.message, true) }
  }

  return <>
    <div className="dphases">
      {[['avant', tr("Avant la sortie")], ['test', t?.age !== null && t?.age !== undefined && phase === 'test' ? tr("Test d’Epic · jour {0} sur 14", [t.age]) : tr("14 jours de test d’Epic")], ['apres', tr("Après")]].map(([k, l], i) =>
        <div key={k} className={'dphase' + (phase === k ? ' on' : '') + (['avant', 'test', 'apres'].indexOf(phase) > i ? ' done' : '')}><span>{i + 1}</span>{l}</div>)}
    </div>

    <section className="card">
      <CardHead icon={<Rocket size={18} />} title={tr("Prêt pour Discover : {0}/100", [ready])} sub={tr("Ce que l’algorithme d’Epic regarde avant même le premier joueur.")} />
      <div className="bar" style={{ marginBottom: 18 }}><span style={{ width: `${ready}%` }} /></div>

      <div className="dsec2"><Type size={16} /><span className="dsec2-l">{tr("Titre")}</span><span className={'badge ' + (tc.score >= 80 ? 'b-g' : tc.score >= 50 ? 'b-o' : 'b-r')}>{tc.score}/100</span></div>
      <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
        <input className="field" style={{ flex: '1 1 320px' }} maxLength={80} value={title ?? ''} onChange={e => setTitle(e.target.value)} aria-label={tr("Titre prévu")} placeholder={tr("Titre prévu dans Fortnite")} />
        {canEdit && <button className="btn" onClick={() => save({ title: title ?? '' }, tr("Titre enregistré"))}>{tr("Garder ce titre")}</button>}
      </div>
      <p className="small muted" style={{ margin: '6px 0 0' }}>{tr("{0} caractères · {1} mots", [tc.chars, tc.words])}</p>
      {tc.issues.length === 0 ? <p className="dok"><CheckCircle2 size={16} />{tr("Titre original et propre : rien de proche dans le catalogue.")}</p> :
        <ul className="dissues">{tc.issues.map((i: string) => <li key={i}><Mark ok={false} warn={!['copie', 'bourrage'].includes(i)} /><div><b>{TITLE_ISSUE[i]?.[0]}{i === 'mots_repris' ? ` : ${tc.overused.join(', ')}` : ''}</b><span>{TITLE_ISSUE[i]?.[1]}</span></div></li>)}</ul>}
      {tc.similar.length > 0 && <div className="dsimilar"><p className="small muted" style={{ margin: '8px 0 4px' }}>{tr("Titres proches déjà publiés :")}</p>
        {tc.similar.map((s: any) => <div key={s.code} className="small"><b style={{ fontWeight: 500 }}>{s.title}</b> <span className="mono muted">{s.code}</span> · {tr("{0} % semblable", [s.similarity])}{s.players ? ' · ' + tr("{0} joueurs", [num(s.players)]) : ''}</div>)}</div>}

      <div className="dsec2"><Shapes size={16} /><span className="dsec2-l">{tr("Genre officiel")}</span><Mark ok={genreOk} warn /></div>
      {data.genre ? <>
        <p style={{ margin: '0 0 6px' }}>{tr("Choisissez « {0} » dans le Creator Portal : le genre décide de la seule rangée de genre où la map peut apparaître, et en changer oblige à republier.", [GENRE[data.genre] ?? data.genre])}</p>
        {data.niche && <p className="small muted" style={{ margin: 0 }}>{tr("Votre niche : {0} · {1} maps, {2} joueurs hier · note d’opportunité {3}/100.", [label(data.niche), num(data.niche.islands), num(data.niche.players), data.niche.score])}</p>}
        {!genreOk && isAdmin && <button className="btn" style={{ marginTop: 8 }} onClick={async () => { await api.patch(`/api/maps/${map!.id}`, { genre: GENRE[data.genre] }); await reloadMe(); reload(); toast(tr("Genre de la map mis à jour")) }}>{tr("Noter ce genre sur la map")}</button>}
      </> : <p className="small muted" style={{ margin: 0 }}>{tr("POP ne reconnaît pas encore le genre depuis le titre. Choisissez le genre qui décrit le mieux la boucle de jeu, pas le décor.")}</p>}

      <div className="dsec2"><ImageIcon size={16} /><span className="dsec2-l">{tr("Vignette")}</span>{thumb.score !== undefined && <span className={'badge ' + (thumb.score >= 80 ? 'b-g' : thumb.score >= 50 ? 'b-o' : 'b-r')}>{thumb.score}/100</span>}</div>
      <div className="dthumb" onDragOver={e => e.preventDefault()} onDrop={e => { e.preventDefault(); onFile(e.dataTransfer.files?.[0]) }}>
        {preview ? <img src={preview} alt={tr("Aperçu de la vignette")} /> : <div className="dthumb-ph"><Upload size={22} /><span>{tr("Déposez la vignette ici")}</span></div>}
        <div className="dthumb-txt">
          <p className="small muted" style={{ marginTop: 0 }}>{tr("L’image est analysée dans votre navigateur, elle n’est envoyée nulle part.")}</p>
          <button className="btn" onClick={() => fileRef.current?.click()} disabled={!canEdit}><Upload size={16} />{thumb.score !== undefined ? tr("Analyser une autre vignette") : tr("Choisir la vignette")}</button>
          <input ref={fileRef} type="file" accept="image/png,image/jpeg,image/webp" hidden onChange={e => onFile(e.target.files?.[0])} />
          {thumb.score !== undefined && <p className="small muted">{tr("Dernière analyse : {0} × {1} px", [thumb.w, thumb.h])}</p>}
        </div>
      </div>
      {thumb.issues && (thumb.issues.length === 0 ? <p className="dok"><CheckCircle2 size={16} />{tr("Rien à signaler. Vérifiez aussi : elle montre le vrai jeu, pas d’armes collées, pas de badges d’Epic, pas de texte trompeur.")}</p> :
        <ul className="dissues">{thumb.issues.map((i: string) => <li key={i}><Mark ok={false} warn={!['fond_uni', 'degrade'].includes(i)} /><div><b>{THUMB_ISSUE[i]?.[0]}</b><span>{THUMB_ISSUE[i]?.[1]}</span></div></li>)}</ul>)}

      <div className="dsec2"><Film size={16} /><span className="dsec2-l">{tr("Bande-annonce")}</span><Mark ok={data.trailer} warn /></div>
      <label className="row" style={{ gap: 8 }}><input type="checkbox" checked={data.trailer} disabled={!canEdit} onChange={e => save({ trailer: e.target.checked })} />{tr("J’ai une bande-annonce qui suit les règles vidéo d’Epic")}</label>
      <p className="small muted" style={{ margin: '4px 0 0' }}>{tr("Le grand carrousel en haut de Discover va proposer aussi des maps choisies par l’algorithme : seules celles qui ont une bande-annonce y ont droit.")}</p>

      <div className="dsec2"><Megaphone size={16} /><span className="dsec2-l">{tr("Vos propres joueurs")}</span><Mark ok={data.channels} warn /></div>
      <p style={{ margin: '0 0 6px' }}>{tr("La rangée Trending compte surtout les joueurs qui arrivent d’eux-mêmes : code de l’île, liens, recherche, page créateur. Les joueurs que vous amenez valent plus que ceux que Discover vous envoie.")}</p>
      {canEdit && (data.channels ? <p className="dok"><CheckCircle2 size={16} />{tr("Plan ajouté à la checklist du Jour J.")}</p> : <button className="btn" onClick={addChannels}><Megaphone size={16} />{tr("Ajouter le plan à la checklist du Jour J")}</button>)}
    </section>

    <section className="card">
      <CardHead icon={<TrendingUp size={18} />} title={tr("Suivi dans Discover")} sub={t ? (t.age !== null && t.age !== undefined ? (t.age <= 14 ? tr("Jour {0} sur 14 : Epic teste la map auprès des joueurs pour décider de sa place.", [t.age]) : tr("Sortie il y a {0} jours : la période de test d’Epic est passée.", [t.age])) : tr("En attente des premiers chiffres d’Epic.")) : tr("Reliez le code de l’île publiée pour suivre ses chiffres.")}>
        {t && <button className="iconbtn" aria-label={tr("Actualiser")} onClick={reload}><RefreshCw size={16} /></button>}
      </CardHead>
      {!data.island_code ? (isAdmin ? <form className="row" style={{ gap: 8 }} onSubmit={async e => { e.preventDefault(); try { await api.put(`/api/maps/${map!.id}/island`, { code: code.trim() }); toast(tr("Île reliée : ses chiffres arrivent à la prochaine collecte")); reload() } catch (er: any) { toast(er.message, true) } }}>
        <input className="field mono" placeholder="1234-5678-9012" value={code} onChange={e => setCode(e.target.value)} required pattern="\d{4}-\d{4}-\d{4}" /><button className="btn pri">{tr("Relier l’île")}</button></form>
        : <p className="small muted">{tr("Un admin du studio peut relier le code de l’île.")}</p>) : <>
        {t.surge?.rising && <div className="dalert"><TrendingUp size={18} /><div><b>{tr("Ça monte plus vite que d’habitude")}</b><span>{tr("{0} joueurs en même temps ces 3 dernières heures, contre {1} d’habitude à ces heures-là. C’est le moment de pousser sur vos réseaux : c’est ce qui fait entrer dans Trending.", [num(t.surge.now), num(t.surge.usual)])}</span></div></div>}
        {t.launch && (t.launch.ready ? <div className="dlaunch">
          <div className="dlaunch-p"><b>{t.launch.prob} %</b><span>{tr("de chances de dépasser {0} joueurs par jour avant la fin des 14 jours de test", [num(t.launch.threshold)])}</span></div>
          <div className="small">
            {(t.launch.factors ?? []).map((f: any) => <p key={f.key} className="dlaunch-f"><span className={f.effect === 'plus' ? 'ok-ic' : 'bad-ic'}>{f.effect === 'plus' ? '▲' : '▼'}</span>{FACTOR[f.key]?.[f.effect === 'plus' ? 0 : 1]}</p>)}
            <p className="muted" style={{ margin: '6px 0 0' }}>{tr("Appris sur {0} nouvelles maps suivies par POP, dont {1} ont percé. Fiabilité : {2} %.", [num(t.launch.n), num(t.launch.positives), Math.round((t.launch.auc ?? 0.5) * 100)])}</p>
          </div>
        </div> : <p className="small muted">{tr("La « chance de percer » s’affichera quand POP aura suivi assez de nouvelles maps ({0} pour l’instant, dont {1} ont dépassé {2} joueurs).", [num(t.launch.n ?? 0), num(t.launch.positives ?? 0), num(t.launch.threshold)])}</p>)}
        {(t.series ?? []).length > 0 ? <>
          <Chart t={t} />
          <div className="dlegend small muted"><span><i className="lg-me" />{tr("Votre map")}</span>{t.bench.length > 0 && <><span><i className="lg-med" />{tr("Nouvelle map typique de la niche")}</span><span><i className="lg-band" />{tr("La moitié des nouvelles maps de la niche")}</span></>}</div>
          {t.percentile !== null && t.percentile !== undefined && <p className="dverdict">{tr("Au même âge, votre map fait mieux que {0} % des nouvelles maps de sa niche.", [t.percentile])}</p>}
        </> : <p className="small muted">{tr("Pas encore de chiffres : Epic publie les données d’une île à partir de 5 joueurs par jour. Elles arrivent à la prochaine collecte.")}</p>}

        <div className="dsec">{tr("Les signaux qu’Epic note")}</div>
        <div className="dsignals">{t.signals.map((s: any) => {
          const fmt = s.key === 'minutes' ? (v: any) => v === null || v === undefined ? '—' : `${Math.round(v)} min` : pc
          const weak = s.grade === 'moyen' || s.grade === 'faible'
          return <div key={s.key} className={'dsignal g-' + s.grade}>
            <div className="dsignal-h"><b>{SIGNAL[s.key][0]}</b><span className={'badge ' + GRADE[s.grade][1]}>{GRADE[s.grade][0]}</span></div>
            <div className="dsig-v">{fmt(s.value)}<span className="small muted">{s.niche !== null && s.niche !== undefined ? tr("niche : {0}", [fmt(s.niche)]) : ''}{s.target ? ' · ' + tr("meilleures : {0}", [fmt(s.target)]) : ''}</span></div>
            {weak && <p className="small">{SIGNAL[s.key][1]}</p>}
          </div>
        })}</div>
        {t.trend3 !== null && t.trend3 !== undefined && t.trend3 <= -20 && <div className="dalert warn"><AlertTriangle size={18} /><div><b>{tr("Les joueurs baissent ({0} % sur 3 jours)", [t.trend3])}</b><span>{t.phase === 'test' ? tr("Pendant le test, regardez d’abord le signal le plus faible ci-dessus : c’est lui qui fait baisser la place de la map.") : tr("Une vraie mise à jour (nouveau mode, nouvelle zone, nouvelle mécanique) relance l’exposition : Epic affiche alors le badge « Mis à jour » sur la vignette.")}</span></div></div>}
      </>}
    </section>

    <PrivateStats mid={map!.id} canEdit={canEdit} />

    <section className="card">
      <CardHead icon={<RefreshCw size={18} />} title={tr("Mises à jour")} sub={tr("Notez chaque mise à jour publiée pour voir son effet sur les joueurs.")} />
      {canEdit && <div className="row" style={{ gap: 8 }}><input className="field" type="date" value={upd} onChange={e => setUpd(e.target.value)} aria-label={tr("Date de la mise à jour")} /><button className="btn" onClick={() => save({ update_day: upd }, tr("Mise à jour notée"))}>{tr("J’ai publié une mise à jour")}</button></div>}
      {(data.updates ?? []).length === 0 ? <p className="small muted">{tr("Aucune mise à jour notée.")}</p> : <div className="list" style={{ marginTop: 10 }}>{[...(t?.effects ?? (data.updates ?? []).map((d: string) => ({ day: d })))].reverse().map((e: any) =>
        <div key={e.day} className="li"><span className="mono small" style={{ width: 96 }}>{e.day}</span><span style={{ flex: 1 }}>{e.change !== null && e.change !== undefined ? tr("{0} joueurs par jour avant, {1} après ({2}{3} %)", [num(e.before), num(e.after), e.change > 0 ? '+' : '', e.change]) : tr("Effet visible 3 jours après la mise à jour.")}</span>
          {canEdit && <button className="link small" onClick={() => save({ remove_update: e.day })}>{tr("Retirer")}</button>}</div>)}</div>}
    </section>
  </>
}


const PRIV: Record<string, [string, 'pct' | 'num' | 'min']> = {
  bounce: [tr("Taux de rebond"), 'pct'], new_players: [tr("Nouveaux joueurs"), 'num'], returning: [tr("Joueurs revenus"), 'num'],
  unique_players: [tr("Joueurs uniques"), 'num'], plays: [tr("Parties"), 'num'], avg_minutes: [tr("Minutes par joueur"), 'min'],
  minutes: [tr("Minutes jouées"), 'num'], d1: [tr("Retour à J+1"), 'pct'], d7: [tr("Retour à J+7"), 'pct'], peak_ccu: [tr("Pic de joueurs simultanés"), 'num'],
  impressions: [tr("Impressions"), 'num'], clicks: [tr("Clics"), 'num'], ctr: [tr("Taux de clic"), 'pct'], favorites: [tr("Favoris"), 'num'],
  recommends: [tr("Recommandations"), 'num'], queue: [tr("Attente"), 'num'],
}

/** Chiffres privés importés du Creator Portal (CSV d'analytics) : rebond, nouveaux joueurs, impressions… */
export function PrivateStats({ mid, canEdit }: { mid: number; canEdit: boolean }) {
  const { toast } = useApp()
  const { data, reload } = useLoad<any>(`/api/maps/${mid}/private-stats`, [mid])
  const ref = useRef<HTMLInputElement>(null)
  const upload = async (f?: File | null) => {
    if (!f) return
    try {
      const r = await api.post(`/api/maps/${mid}/private-stats`, { csv: await f.text(), filename: f.name })
      toast(tr("{0} chiffres importés sur {1} jour(s)", [r.values, r.days]) + (r.ignored?.length ? ' · ' + tr("colonnes ignorées : {0}", [r.ignored.slice(0, 4).join(', ')]) : ''))
      reload()
    } catch (e: any) { toast(e.message, true) }
  }
  const series: Record<string, any[]> = data?.series ?? {}
  const keys = Object.keys(PRIV).filter(k => series[k]?.length)
  const fmt = (k: string, v: number) => PRIV[k][1] === 'pct' ? `${Math.round(v * 1000) / 10} %` : PRIV[k][1] === 'min' ? `${Math.round(v * 10) / 10} min` : num(v)
  const bounce = series.bounce?.[series.bounce.length - 1]?.value
  return <section className="card">
    <CardHead icon={<Upload size={18} />} title={tr("Chiffres privés du Creator Portal")} sub={data?.imported_at ? tr("Dernier import : {0}", [data.filename || data.imported_at]) : tr("Le rebond, les nouveaux joueurs ou les impressions ne sont pas publics. Exportez le CSV d’analytics de votre map dans le Creator Portal (Projects → votre projet → Analytics) et déposez-le ici.")}>
      {canEdit && <><button className="btn" onClick={() => ref.current?.click()}><Upload size={16} />{tr("Importer un CSV")}</button>
        <input ref={ref} type="file" accept=".csv,text/csv" hidden onChange={e => { upload(e.target.files?.[0]); e.target.value = '' }} /></>}
    </CardHead>
    {keys.length > 0 && <div className="privgrid">{keys.map(k => { const s = series[k]; const last = s[s.length - 1]; const prev = s.length > 1 ? s[s.length - 2].value : null
      return <div key={k} className="privcell"><span className="small muted">{PRIV[k][0]}</span><b>{fmt(k, last.value)}</b>
        {prev !== null && prev !== 0 && <span className="small muted">{tr("veille : {0}", [fmt(k, prev)])}</span>}</div> })}</div>}
    {bounce !== undefined && <div className={'dalert' + (bounce > 0.4 ? ' warn' : '')} style={{ marginTop: 12 }}>{bounce > 0.4 ? <AlertTriangle size={18} /> : <CheckCircle2 size={18} />}<div>
      <b>{tr("Rebond : {0}", [`${Math.round(bounce * 100)} %`])}</b>
      <span>{bounce > 0.4 ? tr("Beaucoup de joueurs repartent avant 5 minutes, et Epic compte ce signal pour Discover. Montrez l’objectif tout de suite, réduisez l’attente et donnez une première récompense rapide.") : tr("La plupart des joueurs restent au-delà des premières minutes : c’est un bon signal pour Discover.")}</span>
    </div></div>}
  </section>
}
