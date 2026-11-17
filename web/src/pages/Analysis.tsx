import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { RefreshCw, Star, X, ExternalLink, TrendingUp, TrendingDown, Search } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead, Empty, PageHead } from '../components/ui'

const n = (v?: number | null) => v === null || v === undefined ? '—' : v >= 1e6 ? (v / 1e6).toFixed(1).replace('.', ',') + ' M' : v >= 1e4 ? Math.round(v / 1e3) + ' k' : v.toLocaleString('fr-FR')
const pct = (v?: number | null) => v === null || v === undefined ? '—' : `${Math.round(v * 100)} %`
const day = (d?: string) => d ? new Date(d + 'T12:00:00').toLocaleDateString('fr-FR', { day: 'numeric', month: 'short' }) : '—'

function Status() {
  const { data, reload } = useLoad<any>('/api/analytics/status')
  const [busy, setBusy] = useState(false)
  useEffect(() => { if (!busy) return; const t = setInterval(reload, 4000); const s = setTimeout(() => setBusy(false), 120000); return () => { clearInterval(t); clearTimeout(s) } }, [busy])
  if (!data) return null
  return <div className="row small muted" style={{ flexWrap: 'wrap' }}>
    <span>Source : API publique Fortnite d’Epic · {data.islands} îles suivies, {data.measured} mesurées{data.latest_day ? ` · données jusqu’au ${day(data.latest_day)}` : ''}{data.last_ok ? ` · mise à jour ${ago(data.last_ok)}` : ''}</span>
    {data.error && <span className="badge b-r" title={data.error}>Dernière collecte en erreur</span>}
    <button className="btn" style={{ height: 32 }} disabled={busy} onClick={async () => { await api.post('/api/analytics/refresh'); setBusy(true) }}><RefreshCw size={14} className={busy ? 'spin' : ''} />{busy ? 'Collecte…' : 'Actualiser'}</button>
  </div>
}

/** Fiche d'une île : 7 jours de joueurs, CCU, rétention, et suivi. */
export function IslandDrawer({ code, onClose }: { code: string; onClose: () => void }) {
  const { toast } = useApp()
  const { data, reload, error } = useLoad<any>(`/api/analytics/islands/${code}`, [code])
  const series = (data?.series ?? []).filter((s: any) => s.players !== null)
  const max = Math.max(1, ...series.map((s: any) => s.players ?? 0))
  return <>
    <div className="scrim" onClick={onClose} />
    <aside className="drawer" aria-label="Fiche de l’île">
      <div className="row" style={{ justifyContent: 'space-between' }}><h2 style={{ margin: 0, fontWeight: 500, fontSize: 20 }}>{data?.title || code}</h2><button className="iconbtn" aria-label="Fermer" onClick={onClose}><X size={18} /></button></div>
      {error && <p className="err">{String(error)}</p>}
      {data && <>
        <p className="small muted" style={{ margin: '6px 0 12px' }}><span className="mono">{code}</span>{data.creator && ` · ${data.creator}`}</p>
        <div className="row" style={{ flexWrap: 'wrap', gap: 6 }}>{(data.tags ?? []).map((t: string) => <span key={t} className="chip" style={{ height: 26 }}>{t}</span>)}</div>
        <div className="row" style={{ margin: '14px 0', flexWrap: 'wrap' }}>
          <button className={'btn' + (data.watched ? '' : ' pri')} onClick={async () => { await api.post('/api/analytics/watch', { code, on: !data.watched }); reload(); toast(data.watched ? 'Île retirée du suivi' : 'Île suivie : mesurée à chaque collecte') }}><Star size={16} />{data.watched ? 'Ne plus suivre' : 'Suivre cette île'}</button>
          <a className="btn" href={`https://www.fortnite.com/@${encodeURIComponent(data.creator || '')}/${code}`} target="_blank" rel="noreferrer"><ExternalLink size={16} />Voir sur fortnite.com</a>
          <a className="btn" href={`https://fortnite.gg/island?code=${code}`} target="_blank" rel="noreferrer"><ExternalLink size={16} />Joueurs en direct (fortnite.gg)</a>
        </div>
        <div className="dsec">Joueurs uniques par jour</div>
        {series.length === 0 ? <p className="small muted">Pas encore de données (moins de 5 joueurs par jour, ou île jamais mesurée).</p> :
          <div className="minibars">{series.map((s: any) => <div key={s.day} title={`${day(s.day)} : ${n(s.players)} joueurs`}><span style={{ height: `${(s.players / max) * 100}%` }} /><em>{day(s.day).replace('.', '')}</em></div>)}</div>}
        <table className="tbl" style={{ marginTop: 14 }}><thead><tr><th>Jour</th><th>Joueurs</th><th>Pic CCU</th><th>Parties</th><th>Min/joueur</th><th>D1</th></tr></thead>
          <tbody>{series.slice().reverse().map((s: any) => <tr key={s.day}><td>{day(s.day)}</td><td>{n(s.players)}</td><td>{n(s.peak_ccu)}</td><td>{n(s.plays)}</td><td>{s.avg_minutes ? Math.round(s.avg_minutes) : '—'}</td><td>{pct(s.d1)}</td></tr>)}</tbody></table>
      </>}
    </aside>
  </>
}

function useIsland() {
  const [code, setCode] = useState<string | null>(null)
  return { code, open: setCode, drawer: code ? <IslandDrawer code={code} onClose={() => setCode(null)} /> : null }
}

function Filters({ q, setQ, tag, setTag, tags }: { q?: string; setQ?: (v: string) => void; tag: string; setTag: (v: string) => void; tags: string[] }) {
  return <div className="row" style={{ flexWrap: 'wrap' }}>
    {setQ && <label className="search" style={{ flex: '0 1 280px' }}><Search size={16} /><input placeholder="Nom, créateur, code" value={q} onChange={e => setQ(e.target.value)} aria-label="Filtrer" /></label>}
    <select className="field" value={tag} onChange={e => setTag(e.target.value)} aria-label="Genre"><option value="">Tous les genres</option>{tags.map(t => <option key={t}>{t}</option>)}</select>
  </div>
}

export function Trending() {
  const [q, setQ] = useState('')
  const [tag, setTag] = useState('')
  const [epic, setEpic] = useState(false)
  const { data } = useLoad<any[]>(`/api/analytics/trending?tag=${encodeURIComponent(tag)}&epic=${epic}&q=${encodeURIComponent(q)}`, [tag, epic, q])
  const { data: market } = useLoad<any>('/api/analytics/market')
  const isl = useIsland()
  return <main className="content">
    <PageHead title="Maps qui marchent" help="Les îles Fortnite qui ont le plus de joueurs en ce moment, avec leur évolution sur la veille." />
    <Status />
    <section className="card">
      <div className="row" style={{ justifyContent: 'space-between', flexWrap: 'wrap', marginBottom: 12 }}>
        <Filters q={q} setQ={setQ} tag={tag} setTag={setTag} tags={(market?.tags ?? []).map((t: any) => t.tag)} />
        <label className="row small" style={{ gap: 6 }}><input type="checkbox" checked={epic} onChange={e => setEpic(e.target.checked)} />Inclure les modes d’Epic</label>
      </div>
      {!data ? null : data.length === 0 ? <Empty text="Pas encore de données : la première collecte prend quelques minutes." /> :
        <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>#</th><th>Map</th><th>Joueurs (jour)</th><th>Évolution</th><th>Pic CCU</th><th>Min/joueur</th><th>Rétention D1</th><th>Genres</th></tr></thead>
          <tbody>{data.map((r, i) => <tr key={r.code} className="rowlink" onClick={() => isl.open(r.code)}>
            <td className="muted">{i + 1}</td><td><b style={{ fontWeight: 500 }}>{r.title || r.code}</b><div className="small muted mono">{r.code} · {r.creator}</div></td>
            <td>{n(r.players)}</td><td>{r.growth === null ? '—' : <span className={'badge ' + (r.growth >= 0 ? 'b-g' : 'b-r')}>{r.growth >= 0 ? <TrendingUp size={12} /> : <TrendingDown size={12} />}{r.growth > 0 ? '+' : ''}{r.growth} %</span>}</td>
            <td>{n(r.peak_ccu)}</td><td>{r.avg_minutes ? Math.round(r.avg_minutes) : '—'}</td><td>{pct(r.d1)}</td><td className="small muted">{(r.tags ?? []).slice(0, 3).join(', ')}</td>
          </tr>)}</tbody></table></div>}
    </section>
    {isl.drawer}
  </main>
}

export function Launches() {
  const [tag, setTag] = useState('')
  const { data } = useLoad<any[]>(`/api/analytics/launches?tag=${encodeURIComponent(tag)}`, [tag])
  const { data: market } = useLoad<any>('/api/analytics/market')
  const isl = useIsland()
  return <main className="content">
    <PageHead title="Meilleurs démarrages" help="Les îles sorties récemment qui ont fait les meilleures statistiques dès leur premier jour." />
    <Status />
    <section className="card">
      <div style={{ marginBottom: 12 }}><Filters tag={tag} setTag={setTag} tags={(market?.tags ?? []).map((t: any) => t.tag)} /></div>
      {!data ? null : data.length === 0 ? <Empty text="Pas encore de sortie mesurée : POP repère les nouvelles îles à chaque collecte (toutes les 30 minutes)." /> :
        <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>#</th><th>Map</th><th>Sortie</th><th>Joueurs jour 1</th><th>Pic CCU jour 1</th><th>Parties jour 1</th><th>Joueurs aujourd’hui</th><th>Rétention D1</th></tr></thead>
          <tbody>{data.map((r, i) => <tr key={r.code} className="rowlink" onClick={() => isl.open(r.code)}>
            <td className="muted">{i + 1}</td><td><b style={{ fontWeight: 500 }}>{r.title || r.code}</b><div className="small muted mono">{r.code} · {(r.tags ?? []).slice(0, 2).join(', ')}</div></td>
            <td>{day(r.launch_day)}</td><td><b style={{ fontWeight: 500 }}>{n(r.launch_players)}</b></td><td>{n(r.launch_ccu)}</td><td>{n(r.launch_plays)}</td><td>{n(r.players_now)}</td><td>{pct(r.d1)}</td>
          </tr>)}</tbody></table></div>}
      <p className="small muted">Le « jour 1 » est le premier jour où l’API Fortnite publie des chiffres pour l’île (au moins 5 joueurs). Les îles sorties avant la fenêtre de 7 jours de l’API ne sont pas comptées.</p>
    </section>
    {isl.drawer}
  </main>
}

const VERDICT: Record<string, { label: string; cls: string; why: string; tip: string }> = {
  opportunite: { label: 'Bonne opportunité', cls: 'b-g', why: 'Une map normale de ce genre attire bien du monde, et peu de nouvelles maps arrivent en face.', tip: 'Bon genre pour se lancer : soignez la première minute de jeu pour garder les joueurs.' },
  dispute: { label: 'Populaire mais disputé', cls: 'b-o', why: 'Beaucoup de joueurs, mais aussi beaucoup de maps et de nouvelles sorties.', tip: 'Il y a du public, mais il faudra une idée qui se démarque (mécanique, thème, miniature).' },
  domine: { label: 'Dominé par quelques maps', cls: 'b-r', why: 'Les 3 premières maps prennent presque tous les joueurs.', tip: 'Difficile d’y percer sans un angle nouveau : les joueurs restent sur les maps connues.' },
  sature: { label: 'Saturé', cls: 'b-r', why: 'Beaucoup de maps pour peu de joueurs chacune.', tip: 'À éviter, sauf si votre map apporte quelque chose de vraiment différent.' },
  niche: { label: 'Niche', cls: 'b-b', why: 'Peu de maps et un public plus petit.', tip: 'Peu de concurrence : bien pour une première map ou pour tester une idée.' },
}
const FILTERS: [string, string][] = [['', 'Tous'], ['opportunite', 'Opportunités'], ['dispute', 'Disputés'], ['domine', 'Dominés'], ['sature', 'Saturés'], ['niche', 'Niches']]
const cap = (t: string) => t.charAt(0).toUpperCase() + t.slice(1)
const signed = (v?: number | null) => v === null || v === undefined ? null : `${v > 0 ? '+' : ''}${Math.round(v)} %`

function Trend({ v }: { v?: number | null }) {
  if (v === null || v === undefined) return <span className="small muted">—</span>
  const up = v >= 3, down = v <= -3
  return <span className="small" style={{ color: up ? 'var(--tg)' : down ? 'var(--tr)' : 'var(--text2)', display: 'inline-flex', alignItems: 'center', gap: 3, whiteSpace: 'nowrap' }}>
    {up ? <TrendingUp size={14} /> : down ? <TrendingDown size={14} /> : null}{signed(v)}</span>
}
function Spark({ series }: { series: any[] }) {
  const pts = series.filter(s => s.players !== null && s.players !== undefined)
  const max = Math.max(1, ...pts.map(s => s.players))
  return <div className="spark" aria-hidden>{series.map(s => <span key={s.day} style={{ height: `${Math.max(4, ((s.players ?? 0) / max) * 100)}%`, opacity: s.players === null ? 0.2 : 1 }} />)}</div>
}

function GenreDrawer({ g, onClose, openIsland }: { g: any; onClose: () => void; openIsland: (c: string) => void }) {
  const v = VERDICT[g.verdict]
  const max = Math.max(1, ...g.series.map((s: any) => s.players ?? 0))
  return <>
    <div className="scrim" onClick={onClose} />
    <aside className="drawer" aria-label={`Genre ${g.tag}`}>
      <div className="row" style={{ justifyContent: 'space-between' }}><h2 style={{ margin: 0, fontWeight: 500, fontSize: 22 }}>{cap(g.tag)}</h2><button className="iconbtn" aria-label="Fermer" onClick={onClose}><X size={18} /></button></div>
      <div style={{ margin: '10px 0 16px' }}><span className={'badge ' + v.cls}>{v.label}</span></div>
      <p style={{ margin: '0 0 6px', lineHeight: 1.5 }}>{v.why}</p>
      <p className="b-callout" style={{ margin: '0 0 18px', lineHeight: 1.5 }}><b style={{ fontWeight: 500 }}>Conseil :</b> {v.tip}</p>
      <div className="facts">
        <div><b>{n(g.typical_players)}</b><span>joueurs pour une map typique du genre (par jour)</span></div>
        <div><b>{g.islands}</b><span>maps de ce genre mesurées, dont {g.new_7d} sorties cette semaine</span></div>
        <div><b>{n(g.players)}</b><span>joueurs au total hier, soit {g.share.toFixed(1).replace('.', ',')} % de tous les joueurs des maps créateurs</span></div>
        <div><b>{g.d1 === null ? '—' : pct(g.d1)}</b><span>des joueurs reviennent le lendemain (rétention J+1)</span></div>
        {g.minutes && <div><b>{g.minutes} min</b><span>de jeu en moyenne par joueur</span></div>}
        <div><b>{g.top3_share} %</b><span>des joueurs sont sur les 3 plus grosses maps</span></div>
        <div><b><Trend v={g.trend} /></b><span>d’évolution des joueurs sur la période mesurée</span></div>
      </div>
      <div className="dsec">Joueurs par jour</div>
      <div className="minibars">{g.series.map((s: any) => <div key={s.day} title={`${day(s.day)} : ${n(s.players)} joueurs`}><span style={{ height: `${((s.players ?? 0) / max) * 100}%` }} /><em>{day(s.day).replace('.', '')}</em></div>)}</div>
      <div className="dsec">Maps en tête</div>
      <div className="list">{g.top.map((m: any, i: number) => <button key={m.code} className="li rowlink" style={{ border: 0, background: 'none', font: 'inherit', color: 'inherit', textAlign: 'left', width: '100%' }} onClick={() => openIsland(m.code)}>
        <span className="muted" style={{ width: 18 }}>{i + 1}</span><span style={{ flex: 1 }}>{m.title || m.code}</span><span className="small muted">{n(m.players)} joueurs</span></button>)}</div>
    </aside>
  </>
}

export function Market() {
  const { map, isAdmin, toast } = useApp()
  const { data } = useLoad<any>('/api/analytics/market')
  const { data: mine, reload: reloadMine } = useLoad<any>(map ? `/api/maps/${map.id}/island` : null, [map?.id])
  const { data: watched, reload: reloadW } = useLoad<any[]>('/api/analytics/watch')
  const [code, setCode] = useState('')
  const [filter, setFilter] = useState('')
  const [q, setQ] = useState('')
  const [sort, setSort] = useState<'players' | 'score' | 'trend'>('score')
  const [genre, setGenre] = useState<any>(null)
  const isl = useIsland()
  const all: any[] = data?.tags ?? []
  const tags = useMemo(() => all.filter(t => (!filter || t.verdict === filter) && (!q || t.tag.includes(q.toLowerCase()))).sort((a, b) => (b[sort] ?? -1e9) - (a[sort] ?? -1e9)), [data, filter, q, sort])
  const counts = useMemo(() => Object.fromEntries(FILTERS.map(([k]) => [k, k ? all.filter(t => t.verdict === k).length : all.length])), [data])
  const best = useMemo(() => [...all].filter(t => t.verdict === 'opportunite').sort((a, b) => b.score - a.score)[0], [data])
  const biggest = all[0]
  const rising = useMemo(() => [...all].filter(t => t.trend !== null && t.islands >= 5).sort((a, b) => b.trend - a.trend)[0], [data])
  const crowded = useMemo(() => [...all].filter(t => t.verdict === 'sature').sort((a, b) => b.islands - a.islands)[0], [data])
  const maxP = Math.max(1, ...(data?.players_per_day ?? []).map((d: any) => d.players ?? 0))
  const Pick = ({ t }: { t: any }) => <button className="link" style={{ fontWeight: 500 }} onClick={() => setGenre(t)}>{cap(t.tag)}</button>
  return <main className="content">
    <PageHead title="Analyse du marché" help="Quels genres de maps attirent les joueurs, où il y a de la place, et où la concurrence est trop forte." />
    <Status />
    {!data ? null : all.length === 0 ? <section className="card"><Empty text="Pas encore assez de données : la première collecte prend quelques minutes, revenez un peu plus tard." /></section> : <>
      <section className="card">
        <CardHead title="En bref" sub={`Maps de créateurs (hors modes d’Epic), chiffres du ${day(data.last_day)}`} />
        <ul className="brief">
          <li><b>{n(data.total_players)}</b> joueurs ont joué hier à <b>{n(data.islands)}</b> maps de créateurs{data.total_trend !== null && data.total_trend !== undefined ? <> (<Trend v={data.total_trend} /> depuis le {day(data.first_day)}, à maps égales)</> : null}. Une map typique attire environ <b>{n(data.typical_players)}</b> joueurs par jour.</li>
          {biggest && <li>Le genre le plus joué : <Pick t={biggest} />, avec {biggest.share.toFixed(0)} % des joueurs.</li>}
          {best && <li>Meilleure opportunité : <Pick t={best} />. Une map typique y attire {n(best.typical_players)} joueurs par jour, pour {best.islands} maps en concurrence.</li>}
          {rising && rising.trend > 0 && <li>Le genre qui monte le plus : <Pick t={rising} /> (<Trend v={rising.trend} />).</li>}
          {crowded && <li>Le plus encombré : <Pick t={crowded} />. {crowded.islands} maps se battent pour les joueurs, et une map typique n’en attire que {n(crowded.typical_players)}.</li>}
        </ul>
      </section>

      <section className="card">
        <CardHead title="Les genres" sub="Cliquez sur un genre pour voir le détail, ses meilleures maps et un conseil." />
        <div className="row" style={{ flexWrap: 'wrap', gap: 8, marginBottom: 14 }}>
          {FILTERS.map(([k, l]) => counts[k] > 0 || !k ? <button key={k} className={'chip' + (filter === k ? ' on' : '')} style={{ height: 32 }} onClick={() => setFilter(k)}>{l} <span className="muted" style={{ opacity: 0.7 }}>{counts[k]}</span></button> : null)}
          <span style={{ flex: 1 }} />
          <label className="search" style={{ flex: '0 1 200px', height: 34 }}><Search size={15} /><input placeholder="Chercher un genre" value={q} onChange={e => setQ(e.target.value)} aria-label="Chercher un genre" /></label>
          <select className="field" style={{ height: 34 }} value={sort} onChange={e => setSort(e.target.value as any)} aria-label="Trier"><option value="score">Meilleures opportunités</option><option value="players">Les plus joués</option><option value="trend">Qui montent le plus</option></select>
        </div>
        {tags.length === 0 ? <p className="small muted">Aucun genre dans cette catégorie.</p> :
          <div className="genres">{tags.map(t => { const v = VERDICT[t.verdict]; return <button key={t.tag} className="genre" onClick={() => setGenre(t)}>
            <div className="row" style={{ justifyContent: 'space-between', gap: 8 }}><b className="gname">{cap(t.tag)}</b><Trend v={t.trend} /></div>
            <span className={'badge ' + v.cls} style={{ alignSelf: 'flex-start' }}>{v.label}</span>
            <div className="gbig"><b>{n(t.typical_players)}</b> joueurs / map typique</div>
            <div className="small muted">{t.islands} maps · {t.new_7d} nouvelle{t.new_7d > 1 ? 's' : ''} cette semaine{t.d1 !== null ? ` · ${pct(t.d1)} reviennent` : ''}</div>
            <Spark series={t.series} />
          </button> })}</div>}
      </section>
    </>}

    <div className="grid2">
      <section className="card">
        <CardHead title="Votre map" sub={map ? `Reliez le code de « ${map.name} » pour comparer ses chiffres au marché` : ''} />
        {mine?.code ? <div className="row" style={{ flexWrap: 'wrap' }}><span className="mono">{mine.code}</span><button className="btn" onClick={() => isl.open(mine.code)}>Voir ses statistiques</button>{isAdmin && <button className="btn" onClick={async () => { await api.put(`/api/maps/${map!.id}/island`, { code: null }); reloadMine() }}>Retirer</button>}</div>
          : isAdmin ? <form className="row" onSubmit={async e => { e.preventDefault(); try { await api.put(`/api/maps/${map!.id}/island`, { code }); reloadMine(); toast('Île reliée : ses chiffres arrivent à la prochaine collecte') } catch (er: any) { toast(er.message, true) } }}>
            <input className="field mono" placeholder="1234-5678-9012" value={code} onChange={e => setCode(e.target.value)} /><button className="btn pri">Relier</button></form> : <p className="small muted">Un admin peut relier le code de l’île publiée.</p>}
        <div className="dsec">Maps concurrentes suivies</div>
        <form className="row" onSubmit={async e => { e.preventDefault(); const c = (e.currentTarget.elements.namedItem('w') as HTMLInputElement).value.trim(); try { await api.post('/api/analytics/watch', { code: c, on: true }); reloadW(); isl.open(c) } catch (er: any) { toast(er.message, true) } }}>
          <input name="w" className="field mono" placeholder="Code d’une map à surveiller" /><button className="btn">Suivre</button></form>
        <div className="list" style={{ marginTop: 8 }}>{watched?.map(w => <button key={w.code} className="li rowlink" style={{ border: 0, background: 'none', font: 'inherit', color: 'inherit', textAlign: 'left' }} onClick={() => isl.open(w.code)}><Star size={14} /><span style={{ flex: 1 }}>{w.title || w.code}</span><span className="small muted">{n(w.players)} joueurs</span></button>)}</div>
      </section>
      <section className="card">
        <CardHead title="Joueurs par jour" sub="Toutes les maps de créateurs mesurées par POP" />
        <div className="minibars tall">{(data?.players_per_day ?? []).map((d: any) => <div key={d.day} title={`${day(d.day)} : ${n(d.players)}`}><span style={{ height: `${((d.players ?? 0) / maxP) * 100}%` }} /><em>{day(d.day).replace('.', '')}</em></div>)}</div>
      </section>
    </div>
    <details className="card howto"><summary>Comment lire ces chiffres ?</summary>
      <ul>
        <li><b>Joueurs / map typique</b> : combien de joueurs par jour attire une map « moyenne » du genre (la médiane, pour ne pas être faussé par une map géante).</li>
        <li><b>Reviennent</b> : part des joueurs qui rejouent le lendemain (rétention J+1). Au-dessus de 20 %, c’est bon.</li>
        <li><b>Évolution</b> : joueurs du dernier jour comparés au premier jour mesuré (jusqu’à 7 jours), sur les mêmes maps, pour ne pas compter les nouvelles sorties comme une hausse.</li>
        <li><b>Verdict</b> : calculé à partir des joueurs par map, du nombre de maps, des nouvelles sorties et du poids des 3 plus grosses maps.</li>
        <li>Source : API publique Fortnite d’Epic, qui ne publie pas les chiffres des maps de moins de 5 joueurs.</li>
      </ul>
    </details>
    {genre && <GenreDrawer g={genre} onClose={() => setGenre(null)} openIsland={c => { setGenre(null); isl.open(c) }} />}
    {isl.drawer}
  </main>
}

function Source({ name, desc, src, children }: { name: string; desc: string; src: any; children?: React.ReactNode }) {
  return <div className="srcrow">
    <div style={{ flex: 1, minWidth: 220 }}>
      <div className="row" style={{ gap: 8 }}><b style={{ fontWeight: 500 }}>{name}</b>{src?.configured ? (src.error ? <span className="badge b-r" title={src.error}>Erreur</span> : <span className="badge b-g">Connecté</span>) : <span className="badge">Non connecté</span>}</div>
      <div className="small muted">{desc}</div>
      {src?.configured && <div className="small muted">{src.videos} vidéos{src.last_ok ? ` · collecte ${ago(src.last_ok)}` : ''}</div>}
      {src?.error && <div className="small" style={{ color: 'var(--tr)' }}>{src.error}</div>}
    </div>
    {children}
  </div>
}

export function TikTok() {
  const { studio, isAdmin, role, toast } = useApp()
  const { data, reload } = useLoad<any>(studio ? `/api/studios/${studio.id}/tiktok` : null, [studio?.id])
  const { data: integ, reload: reloadI } = useLoad<any>(studio && isAdmin ? `/api/studios/${studio.id}/integrations` : null, [studio?.id])
  const [busy, setBusy] = useState(false)
  const [setup, setSetup] = useState<string | null>(null)
  const [platform, setPlatform] = useState('')
  const isl = useIsland()
  const nav = useNavigate()
  const tcfg = integ?.apify?.config ?? {}
  const ycfg = integ?.youtube?.config ?? {}
  const save = async (kind: string, c: any) => { try { await api.put(`/api/studios/${studio!.id}/integrations/${kind}`, { enabled: true, config: c }); reloadI(); reload(); setSetup(null); toast('Enregistré') } catch (e: any) { toast(e.message, true) } }
  const refresh = async () => { setBusy(true); try { const r = await api.post(`/api/studios/${studio!.id}/tiktok/refresh`); toast(`${r.videos} vidéos collectées${r.errors?.length ? ' · ' + r.errors.join(' · ') : ''}`, !!r.errors?.length); reload() } catch (e: any) { toast(e.message, true) } finally { setBusy(false) } }
  const src = data?.sources ?? {}
  const vids = (data?.videos ?? []).filter((v: any) => !platform || v.platform === platform)
  const rising = (data?.hashtags ?? []).filter((h: any) => h.rising)
  const KeyForm = ({ kind, k, ph, cfg }: { kind: string; k: string; ph: string; cfg: any }) => <form className="row" style={{ marginTop: 10 }} onSubmit={e => { e.preventDefault(); const t = (e.currentTarget.elements.namedItem('t') as HTMLInputElement).value.trim(); if (t) save(kind, { ...cfg, [k]: t }) }}>
    <input name="t" type="password" className="field mono" style={{ flex: 1 }} placeholder={ph} /><button className="btn pri">Enregistrer</button></form>
  return <main className="content">
    <PageHead title="Tendances vidéo" help="Ce qui marche en vidéo autour de Fortnite, sur TikTok et YouTube Shorts : hashtags qui montent, sons, vidéos qui décollent et maps mises en avant.">
      {data?.configured && ['owner', 'admin', 'member'].includes(role) && <button className="btn pri" disabled={busy} onClick={refresh}><RefreshCw size={16} className={busy ? 'spin' : ''} />{busy ? 'Collecte (1 à 4 min)…' : 'Collecter maintenant'}</button>}
    </PageHead>

    <section className="card">
      <CardHead title="Sources" sub="Plus il y a de sources, plus il y a de vidéos analysées. YouTube Shorts est gratuit et collecté automatiquement toutes les 6 heures." />
      <Source name="YouTube Shorts" desc="API officielle de YouTube, gratuite (clé Google). Environ 300 à 800 Shorts Fortnite récents par collecte." src={src.youtube}>
        {isAdmin && <button className="btn" onClick={() => setSetup(setup === 'yt' ? null : 'yt')}>{src.youtube?.configured ? 'Changer la clé' : 'Connecter'}</button>}
      </Source>
      {setup === 'yt' && <div className="setup">
        <ol>
          <li>Ouvrez <a href="https://console.cloud.google.com/apis/library/youtube.googleapis.com" target="_blank" rel="noreferrer">la page YouTube Data API v3</a> (connectez-vous avec un compte Google, créez un projet si demandé).</li>
          <li>Cliquez sur <b style={{ fontWeight: 500 }}>Activer</b>.</li>
          <li>Allez dans <b style={{ fontWeight: 500 }}>Identifiants → Créer des identifiants → Clé API</b>, puis copiez la clé.</li>
        </ol>
        <KeyForm kind="youtube" k="key" ph="AIza…" cfg={ycfg} />
      </div>}
      <Source name="TikTok" desc="Via Apify (TikTok n’a pas d’accès public). Hashtags + recherches par mots-clés, jusqu’à 100 vidéos par recherche." src={src.tiktok}>
        {isAdmin && <button className="btn" onClick={() => setSetup(setup === 'tt' ? null : 'tt')}>{src.tiktok?.configured ? 'Changer le jeton' : 'Connecter'}</button>}
      </Source>
      {setup === 'tt' && <div className="setup">
        <ol>
          <li>Créez un compte gratuit sur <a href="https://apify.com" target="_blank" rel="noreferrer">apify.com</a> (5 $ de crédit offerts chaque mois).</li>
          <li>Settings → API &amp; Integrations → copiez le <b style={{ fontWeight: 500 }}>Personal API token</b>.</li>
        </ol>
        <KeyForm kind="apify" k="token" ph="apify_api_…" cfg={tcfg} />
      </div>}
      {data?.configured && isAdmin && <details className="howto" style={{ marginTop: 12 }}><summary>Que rechercher ?</summary>
        <form style={{ display: 'grid', gap: 10, marginTop: 10 }} onSubmit={e => { e.preventDefault(); const f = e.currentTarget.elements as any; save('apify', { ...tcfg, hashtags: f.h.value, keywords: f.k.value, auto: f.a.checked }) }}>
          <label className="small">Hashtags suivis (séparés par des virgules)<textarea name="h" className="field" rows={2} defaultValue={data.followed.join(', ')} style={{ width: '100%', height: 'auto', padding: 8 }} /></label>
          <label className="small">Recherches par mots-clés (une par ligne ou séparées par des virgules)<textarea name="k" className="field" rows={3} defaultValue={data.keywords.join('\n')} style={{ width: '100%', height: 'auto', padding: 8 }} /></label>
          <label className="row small" style={{ gap: 6 }}><input name="a" type="checkbox" defaultChecked={data.auto_tiktok} />Collecter TikTok automatiquement une fois par jour (consomme du crédit Apify)</label>
          <div><button className="btn pri">Enregistrer</button></div>
        </form>
      </details>}
    </section>

    {data?.configured && (data.total === 0 ? <section className="card"><Empty text="Aucune vidéo pour l’instant : cliquez sur « Collecter maintenant »." /></section> : <>
      <div className="tiles">
        <div className="card"><div className="lab">Vidéos analysées (30 j)</div><div className="num">{n(data.total)}</div></div>
        <div className="card"><div className="lab">Hashtags qui montent</div><div className="num">{rising.length}</div></div>
        <div className="card"><div className="lab">Maps citées avec leur code</div><div className="num">{data.maps.length}</div></div>
      </div>
      <div className="grid2">
        <section className="card"><CardHead title="Hashtags du moment" sub="Hors hashtags génériques. « En hausse » = une grosse part de ses vues date des 3 derniers jours." />
          <table className="tbl"><thead><tr><th>Hashtag</th><th>Vues</th><th>Vidéos</th><th /></tr></thead><tbody>{data.hashtags.slice(0, 20).map((h: any) => <tr key={h.name}><td>#{h.name}</td><td>{n(h.plays)}</td><td>{h.videos}</td><td>{h.rising ? <span className="badge b-g">En hausse</span> : null}</td></tr>)}</tbody></table>
          {data.hashtags.length === 0 && <p className="small muted">Pas encore assez de vidéos.</p>}</section>
        <section className="card"><CardHead title="Sons tendance (TikTok)" sub="Musiques les plus utilisées dans ces vidéos" />
          {data.sounds.length === 0 ? <p className="small muted">Pas de son récurrent (nécessite la source TikTok).</p> :
            <table className="tbl"><thead><tr><th>Son</th><th>Vues</th><th>Vidéos</th></tr></thead><tbody>{data.sounds.slice(0, 15).map((h: any) => <tr key={h.name}><td>{h.name}</td><td>{n(h.plays)}</td><td>{h.videos}</td></tr>)}</tbody></table>}</section>
      </div>
      <section className="card"><CardHead title="Maps citées dans les vidéos" sub="Codes d’îles trouvés dans les descriptions, avec leurs statistiques Fortnite" />
        {data.maps.length === 0 ? <p className="small muted">Aucun code d’île dans les vidéos collectées.</p> :
          <table className="tbl"><thead><tr><th>Map</th><th>Vidéos</th><th>Vues</th><th /></tr></thead><tbody>{data.maps.slice(0, 20).map((m: any) => <tr key={m.code} className="rowlink" onClick={() => isl.open(m.code)}><td><b style={{ fontWeight: 500 }}>{m.title || 'Île'}</b> <span className="small muted mono">{m.code}</span></td><td>{m.videos}</td><td>{n(m.plays)}</td><td className="small">Statistiques →</td></tr>)}</tbody></table>}
      </section>
      <section className="card"><CardHead title="Vidéos qui décollent (7 jours)" sub="Classées par vues par jour depuis leur publication">
        <div className="row" style={{ gap: 6 }}>{[['', 'Toutes'], ['tiktok', 'TikTok'], ['youtube', 'YouTube']].map(([k, l]) => <button key={k} className={'chip' + (platform === k ? ' on' : '')} style={{ height: 30 }} onClick={() => setPlatform(k)}>{l}</button>)}</div>
      </CardHead>
        {vids.length === 0 ? <p className="small muted">Aucune vidéo récente.</p> :
          <div className="ttgrid">{vids.map((v: any) => <a key={v.id} className="ttcard" href={v.url} target="_blank" rel="noreferrer">
            {v.thumb && <img src={v.thumb} alt="" loading="lazy" referrerPolicy="no-referrer" className="ttthumb" onError={e => (e.currentTarget.style.display = 'none')} />}
            <div className="row small" style={{ gap: 6 }}><span className={'badge ' + (v.platform === 'youtube' ? 'b-r' : 'b-v')}>{v.platform === 'youtube' ? 'YouTube' : 'TikTok'}</span><span className="muted">{v.author}{v.created ? ` · ${ago(v.created.replace('T', ' ').slice(0, 19))}` : ''}</span></div>
            <div className="tttext">{v.text || '(sans description)'}</div>
            <div className="row small" style={{ gap: 10 }}><b style={{ fontWeight: 500 }}>{n(v.plays)} vues</b><span className="muted">≈ {n(v.per_day)}/jour · {n(v.likes)} j’aime</span></div>
          </a>)}</div>}
      </section>
    </>)}
    <p className="small muted">Pour comparer avec les chiffres de jeu, ouvrez <button className="link small" onClick={() => nav('/analyse/tendances')}>Maps qui marchent</button>.</p>
    {isl.drawer}
  </main>
}
