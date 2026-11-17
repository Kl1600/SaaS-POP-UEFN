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

export function Market() {
  const { map, isAdmin, toast } = useApp()
  const { data } = useLoad<any>('/api/analytics/market')
  const { data: mine, reload: reloadMine } = useLoad<any>(map ? `/api/maps/${map.id}/island` : null, [map?.id])
  const { data: watched, reload: reloadW } = useLoad<any[]>('/api/analytics/watch')
  const [code, setCode] = useState('')
  const [sort, setSort] = useState<'players' | 'avg_players' | 'score' | 'd1'>('players')
  const isl = useIsland()
  const tags = useMemo(() => [...(data?.tags ?? [])].sort((a, b) => (b[sort] ?? 0) - (a[sort] ?? 0)), [data, sort])
  const maxP = Math.max(1, ...(data?.players_per_day ?? []).map((d: any) => d.players ?? 0))
  return <main className="content">
    <PageHead title="Analyse du marché" help="Quels genres attirent les joueurs, où la concurrence est forte, et comment votre map se situe." />
    <Status />
    <div className="tiles">
      <div className="card"><div className="lab">Îles mesurées</div><div className="num">{n(data?.islands)}</div></div>
      <div className="card"><div className="lab">Joueurs (dernier jour, îles créateurs)</div><div className="num">{n(data?.total_players)}</div></div>
      <div className="card"><div className="lab">Nouvelles sorties repérées (7 j)</div><div className="num">{n((data?.new_per_day ?? []).slice(-7).reduce((s: number, d: any) => s + d.count, 0))}</div></div>
    </div>
    <div className="grid2">
      <section className="card">
        <CardHead title="Votre map" sub={map ? `Code de l’île publiée de « ${map.name} »` : ''} />
        {mine?.code ? <div className="row" style={{ flexWrap: 'wrap' }}><span className="mono">{mine.code}</span><button className="btn" onClick={() => isl.open(mine.code)}>Voir ses statistiques</button>{isAdmin && <button className="btn" onClick={async () => { await api.put(`/api/maps/${map!.id}/island`, { code: null }); reloadMine() }}>Retirer</button>}</div>
          : isAdmin ? <form className="row" onSubmit={async e => { e.preventDefault(); try { await api.put(`/api/maps/${map!.id}/island`, { code }); reloadMine(); toast('Île reliée : ses chiffres arrivent à la prochaine collecte') } catch (er: any) { toast(er.message, true) } }}>
            <input className="field mono" placeholder="1234-5678-9012" value={code} onChange={e => setCode(e.target.value)} /><button className="btn pri">Relier</button></form> : <p className="small muted">Un admin peut relier le code de l’île publiée.</p>}
        <div className="dsec">Îles suivies</div>
        <form className="row" onSubmit={async e => { e.preventDefault(); const c = (e.currentTarget.elements.namedItem('w') as HTMLInputElement).value.trim(); try { await api.post('/api/analytics/watch', { code: c, on: true }); reloadW(); isl.open(c) } catch (er: any) { toast(er.message, true) } }}>
          <input name="w" className="field mono" placeholder="Code d’une map concurrente" /><button className="btn">Suivre</button></form>
        <div className="list" style={{ marginTop: 8 }}>{watched?.map(w => <button key={w.code} className="li rowlink" style={{ border: 0, background: 'none', font: 'inherit', color: 'inherit', textAlign: 'left' }} onClick={() => isl.open(w.code)}><Star size={14} /><span style={{ flex: 1 }}>{w.title || w.code}</span><span className="small muted">{n(w.players)} joueurs</span></button>)}</div>
      </section>
      <section className="card">
        <CardHead title="Joueurs par jour" sub="Toutes les îles de créateurs mesurées par POP" />
        <div className="minibars tall">{(data?.players_per_day ?? []).map((d: any) => <div key={d.day} title={`${day(d.day)} : ${n(d.players)}`}><span style={{ height: `${((d.players ?? 0) / maxP) * 100}%` }} /><em>{day(d.day).replace('.', '')}</em></div>)}</div>
      </section>
    </div>
    <section className="card">
      <CardHead title="Genres" sub="Opportunité = beaucoup de joueurs par île, bonne rétention, peu de nouvelles sorties concurrentes.">
        <select className="field" value={sort} onChange={e => setSort(e.target.value as any)} aria-label="Trier par"><option value="players">Joueurs au total</option><option value="avg_players">Joueurs par île</option><option value="score">Opportunité</option><option value="d1">Rétention D1</option></select>
      </CardHead>
      {tags.length === 0 ? <Empty text="Pas encore assez de données." /> :
        <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Genre</th><th>Part des joueurs</th><th>Joueurs</th><th>Îles</th><th>Joueurs / île</th><th>Meilleure île</th><th>Rétention D1</th><th>Sorties (7 j)</th><th>Opportunité</th></tr></thead>
          <tbody>{tags.map(t => <tr key={t.tag}><td><b style={{ fontWeight: 500 }}>{t.tag}</b></td><td><div className="bar" style={{ width: 120 }}><span style={{ width: `${Math.min(100, t.share)}%` }} /></div><span className="small muted">{t.share.toFixed(1).replace('.', ',')} %</span></td>
            <td>{n(t.players)}</td><td>{t.islands}</td><td>{n(t.avg_players)}</td><td>{n(t.top_players)}</td><td>{pct(t.d1)}</td><td>{t.new_7d}</td><td><b style={{ fontWeight: 500 }}>{t.score}</b></td></tr>)}</tbody></table></div>}
    </section>
    {isl.drawer}
  </main>
}

export function TikTok() {
  const { studio, isAdmin, role, toast } = useApp()
  const { data, reload } = useLoad<any>(studio ? `/api/studios/${studio.id}/tiktok` : null, [studio?.id])
  const { data: integ, reload: reloadI } = useLoad<any>(studio && isAdmin ? `/api/studios/${studio.id}/integrations` : null, [studio?.id])
  const [busy, setBusy] = useState(false)
  const isl = useIsland()
  const nav = useNavigate()
  const cfg = integ?.apify?.config ?? {}
  const save = async (c: any) => { try { await api.put(`/api/studios/${studio!.id}/integrations/apify`, { enabled: true, config: c }); reloadI(); reload() } catch (e: any) { toast(e.message, true) } }
  const refresh = async () => { setBusy(true); try { const r = await api.post(`/api/studios/${studio!.id}/tiktok/refresh`); toast(`${r.videos} vidéos TikTok collectées`); reload() } catch (e: any) { toast(e.message, true) } finally { setBusy(false) } }
  return <main className="content">
    <PageHead title="Tendances TikTok" help="Ce qui monte sur TikTok autour de Fortnite : hashtags, sons, vidéos phares et maps mises en avant.">
      {data?.configured && ['owner', 'admin', 'member'].includes(role) && <button className="btn pri" disabled={busy} onClick={refresh}><RefreshCw size={16} className={busy ? 'spin' : ''} />{busy ? 'Collecte (1 à 3 min)…' : 'Collecter maintenant'}</button>}
    </PageHead>
    {!data?.configured && <section className="card">
      <CardHead title="Connecter TikTok" sub="TikTok ne propose pas d’accès public à ses tendances. POP passe par Apify, qui lit les vidéos publiques des hashtags choisis." />
      <ol style={{ margin: 0, paddingLeft: 20, lineHeight: 1.8 }}>
        <li>Créez un compte gratuit sur apify.com (5 $ de crédit offerts chaque mois, soit environ 1 000 vidéos).</li>
        <li>Settings → API &amp; Integrations → copiez votre <b style={{ fontWeight: 500 }}>Personal API token</b>.</li>
        <li>Collez-le ici :</li>
      </ol>
      {isAdmin ? <form className="row" style={{ marginTop: 10 }} onSubmit={e => { e.preventDefault(); const t = (e.currentTarget.elements.namedItem('t') as HTMLInputElement).value.trim(); if (t) save({ ...cfg, token: t }) }}><input name="t" type="password" className="field mono" style={{ flex: 1 }} placeholder="apify_api_…" /><button className="btn pri">Enregistrer</button></form>
        : <p className="small muted">Demandez à un admin du studio d’ajouter le jeton Apify.</p>}
    </section>}
    {data?.configured && <>
      <div className="row small muted" style={{ flexWrap: 'wrap' }}>
        <span>Hashtags suivis : {data.followed.map((h: string) => `#${h}`).join(' ')} · {data.total} vidéos{data.last_fetch ? ` · collecte ${ago(data.last_fetch)}` : ''}</span>
        {isAdmin && <button className="link small" onClick={() => { const h = prompt('Hashtags à suivre (séparés par des virgules)', data.followed.join(', ')); if (h !== null) save({ ...cfg, hashtags: h }) }}>Modifier</button>}
      </div>
      {data.total === 0 ? <section className="card"><Empty text="Aucune vidéo pour l’instant : lancez une collecte." /></section> : <>
        <div className="grid2">
          <section className="card"><CardHead title="Hashtags qui montent" sub="Associés aux hashtags suivis · élan = vues des 3 derniers jours comparées aux précédentes" />
            <table className="tbl"><thead><tr><th>Hashtag</th><th>Vues</th><th>Vidéos</th><th>Élan</th></tr></thead><tbody>{data.hashtags.slice(0, 20).map((h: any) => <tr key={h.name}><td>#{h.name}</td><td>{n(h.plays)}</td><td>{h.videos}</td><td>{h.momentum === null ? <span className="badge b-g">Nouveau</span> : `${h.momentum} %`}</td></tr>)}</tbody></table></section>
          <section className="card"><CardHead title="Sons tendance" sub="Musiques les plus utilisées dans ces vidéos" />
            <table className="tbl"><thead><tr><th>Son</th><th>Vues</th><th>Vidéos</th></tr></thead><tbody>{data.sounds.slice(0, 15).map((h: any) => <tr key={h.name}><td>{h.name}</td><td>{n(h.plays)}</td><td>{h.videos}</td></tr>)}</tbody></table>
            {data.sounds.length === 0 && <p className="small muted">Pas de son récurrent.</p>}</section>
        </div>
        <section className="card"><CardHead title="Maps citées sur TikTok" sub="Codes d’îles trouvés dans les descriptions des vidéos, avec leurs statistiques Fortnite" />
          {data.maps.length === 0 ? <p className="small muted">Aucun code d’île dans les vidéos collectées.</p> :
            <table className="tbl"><thead><tr><th>Map</th><th>Vidéos</th><th>Vues TikTok</th><th /></tr></thead><tbody>{data.maps.slice(0, 20).map((m: any) => <tr key={m.code} className="rowlink" onClick={() => isl.open(m.code)}><td><b style={{ fontWeight: 500 }}>{m.title || 'Île'}</b> <span className="small muted mono">{m.code}</span></td><td>{m.videos}</td><td>{n(m.plays)}</td><td className="small">Statistiques →</td></tr>)}</tbody></table>}
        </section>
        <section className="card"><CardHead title="Vidéos phares (7 jours)" />
          <div className="ttgrid">{data.videos.map((v: any) => <a key={v.id} className="ttcard" href={v.url} target="_blank" rel="noreferrer">
            <div className="small muted">@{v.author} · {v.created ? ago(v.created.replace('T', ' ').slice(0, 19)) : ''}</div>
            <div className="tttext">{v.text || '(sans description)'}</div>
            <div className="row small" style={{ gap: 10 }}><b style={{ fontWeight: 500 }}>{n(v.plays)} vues</b><span className="muted">{n(v.likes)} j’aime · {n(v.shares)} partages</span></div>
          </a>)}</div>
        </section>
      </>}
    </>}
    <p className="small muted">Pour comparer avec les chiffres de jeu, ouvrez <button className="link small" onClick={() => nav('/analyse/tendances')}>Maps qui marchent</button>.</p>
    {isl.drawer}
  </main>
}
