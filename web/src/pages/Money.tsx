import { useEffect, useState } from 'react'
import { AlertTriangle, Upload, Trash2 } from 'lucide-react'
import { api, euros, frDate } from '../api'
import { useApp, useLoad } from '../state'
import { Avatar, CardHead, PageHead } from '../components/ui'
import { useMembers } from './Tasks'
import { tr, LOCALE } from '../i18n/t'

const KIND: Record<string, string> = { percent: tr("% des gains"), fixed: tr("Montant fixe"), per_task: tr("À la tâche finie") }
const monthLabel = (m: string) => frDate(m + '-01', { month: 'long', year: 'numeric' })

export default function Money() {
  const { map, isAdmin, toast } = useApp()
  const [month, setMonth] = useState<string | null>(null)
  const { data: st, reload } = useLoad<any>(map ? `/api/maps/${map.id}/statements${month ? `?month=${month}` : ''}` : null, [map?.id, month])
  const { data: rules, reload: reloadRules } = useLoad<any[]>(map ? `/api/maps/${map.id}/share-rules` : null, [map?.id])
  const { data: offers, reload: reloadOffers } = useLoad<any[]>(map ? `/api/maps/${map.id}/offers` : null, [map?.id])
  const { data: mem } = useMembers()
  const [sel, setSel] = useState<number | null>(null)
  if (!st) return null
  const cur = st.current
  const lines = cur.lines as any[]
  const me = lines.find(l => l.user_id === (sel ?? lines.find(x => x.user_id)?.user_id)) ?? lines[0]

  return (
    <main className="content">
      <PageHead title={tr("Rémunérer")} help={tr("Importez ce qu'Epic a versé : l'app calcule la part de chacun.")}>
        {st.months.length > 0 && <select className="field" value={st.month} onChange={e => setMonth(e.target.value)} aria-label={tr("Mois")}>{st.months.map((m: string) => <option key={m} value={m}>{monthLabel(m)}</option>)}</select>}
      </PageHead>
      <div className="warn" role="note"><AlertTriangle size={18} /><span><b>{tr("Calcul uniquement.")}</b>{' '}{tr("POP calcule les parts mais n'effectue aucun virement : les paiements se font en dehors de l'application.")}</span></div>

      {isAdmin && <section className="tiles">
        <div className="card"><div className="lab">{tr("Versé par Epic · {0}", [st.month ? monthLabel(st.month) : '—'])}</div><div className="num">{euros(cur.total_cents)}</div><div className="muted">{tr("{0} engagement · {1} ventes en jeu", [euros(cur.engagement_cents), euros(cur.sales_cents)])}</div></div>
        <div className="card"><div className="lab">{tr("Fixes et à la tâche")}</div><div className="num">{euros(cur.fixed_and_tasks_cents)}</div><div className="muted">{cur.underfunded ? tr("dépasse les gains du mois") : tr("payés en premier")}</div></div>
        <div className="card"><div className="lab">{tr("Partagé en pourcentage")}</div><div className="num">{euros(cur.shared_cents)}</div><div className="muted">{tr("le reste après fixes")}</div></div>
      </section>}

      <div className="two">
        <Rules rules={rules ?? []} members={mem?.members ?? []} reload={() => { reloadRules(); reload() }} lines={lines} />
        {isAdmin ? <Import reload={reload} /> : <section className="card"><CardHead title={tr("Import des gains")} sub={tr("Réservé aux admins du studio.")} /></section>}
      </div>

      <section className="card">
        <CardHead title={tr("Relevé par personne")} sub={tr("Sa part, d'où elle vient, et l'historique")}>
          {lines.filter(l => l.user_id).map(l => <button key={l.user_id} className={'chip' + (me?.user_id === l.user_id ? ' on' : '')} onClick={() => setSel(l.user_id)}>{l.name.split(' ')[0]}</button>)}
        </CardHead>
        {me ? <div className="grid2">
          <div>
            <div className="num" style={{ marginTop: 0 }}>{euros(me.amount_cents)}</div>
            <div className="muted">{tr("Revient à {0} · {1}{2}", [me.name, KIND[me.kind], me.kind === 'per_task' ? tr(" · {0} tâche(s) finie(s)", [me.tasks]) : ''])}</div>
            <div className="stack" style={{ marginTop: 20 }}>
              <div><div className="row" style={{ justifyContent: 'space-between', marginBottom: 6 }}><span>{tr("Gains d'engagement")}</span><b style={{ fontWeight: 500 }}>{euros(me.from_engagement_cents)}</b></div><div className="bar"><span style={{ width: `${me.amount_cents ? (me.from_engagement_cents / me.amount_cents) * 100 : 0}%` }} /></div></div>
              <div><div className="row" style={{ justifyContent: 'space-between', marginBottom: 6 }}><span>{tr("Ventes en jeu")}</span><b style={{ fontWeight: 500 }}>{euros(me.from_sales_cents)}</b></div><div className="bar"><span style={{ width: `${me.amount_cents ? (me.from_sales_cents / me.amount_cents) * 100 : 0}%` }} /></div></div>
            </div>
          </div>
          <div>
            <div className="lab" style={{ marginBottom: 12 }}>{tr("Historique")}</div>
            <div className="hist">{st.history.map((h: any) => { const l = h.lines.find((x: any) => x.user_id === me.user_id); const max = Math.max(1, ...st.history.map((x: any) => x.lines.find((y: any) => y.user_id === me.user_id)?.amount_cents ?? 0)); return (
              <div key={h.month} className="hcol"><span className="small mono">{euros(l?.amount_cents ?? 0, 0)}</span><div className="htrack"><div style={{ width: '100%', height: `${((l?.amount_cents ?? 0) / max) * 100}%`, background: h.month === st.month ? 'var(--text)' : 'var(--text3)', borderRadius: '6px 6px 0 0' }} /></div><span className="small muted">{frDate(h.month + '-01', { month: 'short' })}</span></div>
            ) })}</div>
          </div>
        </div> : <p className="muted">{tr("Aucune règle de partage ne vous concerne pour ce mois.")}</p>}
      </section>

      <Simulator cur={cur} offers={offers ?? []} month={st.month} />
      <Offers offers={offers ?? []} reload={reloadOffers} month={st.month} />
    </main>
  )
}

function Rules({ rules, members, reload, lines }: { rules: any[]; members: any[]; reload: () => void; lines: any[] }) {
  const { map, isAdmin, toast } = useApp()
  const [edit, setEdit] = useState<any[] | null>(null)
  const start = () => setEdit(rules.map(r => ({ user_id: r.user_id, kind: r.kind, value: r.kind === 'percent' ? r.value / 100 : r.value / 100 })))
  const save = async () => {
    try { await api.put(`/api/maps/${map!.id}/share-rules`, edit!.map(r => ({ user_id: r.user_id || null, kind: r.kind, value: Math.round(Number(r.value) * 100) }))); setEdit(null); reload(); toast(tr("Règles enregistrées")) } catch (e: any) { toast(e.message, true) }
  }
  return (
    <section className="card">
      <CardHead title={tr("Règles de partage")} sub={tr("Fixes, puis à la tâche finie, puis pourcentages sur le reste")}>{isAdmin && !edit && <button className="btn" onClick={start}>{tr("Modifier")}</button>}</CardHead>
      {!edit ? <table className="tbl"><thead><tr><th>{tr("Membre")}</th><th>{tr("Type")}</th><th>{tr("Règle")}</th><th style={{ textAlign: 'right' }}>{tr("Ce mois")}</th></tr></thead><tbody>
        {rules.map(r => <tr key={r.id}><td><span className="row" style={{ gap: 8 }}><Avatar name={r.name} small />{r.name}</span></td><td><span className="badge">{KIND[r.kind]}</span></td><td className="muted" style={{ whiteSpace: 'nowrap' }}>{r.kind === 'percent' ? `${r.value / 100} %` : r.kind === 'fixed' ? `${euros(r.value)} / mois` : `${euros(r.value)} / tâche`}</td><td style={{ textAlign: 'right', whiteSpace: 'nowrap' }}>{euros(lines.find(l => l.user_id === r.user_id && l.kind === r.kind)?.amount_cents ?? 0)}</td></tr>)}
      </tbody></table> : <div className="stack">
        {edit.map((r, i) => (
          <div key={i} className="row" style={{ flexWrap: 'wrap' }}>
            <select className="field" value={r.user_id ?? ''} onChange={e => setEdit(edit.map((x, j) => (j === i ? { ...x, user_id: e.target.value ? Number(e.target.value) : null } : x)))}><option value="">{tr("Réserve du studio")}</option>{members.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}</select>
            <select className="field" value={r.kind} onChange={e => setEdit(edit.map((x, j) => (j === i ? { ...x, kind: e.target.value } : x)))}>{Object.entries(KIND).map(([k, l]) => <option key={k} value={k}>{l}</option>)}</select>
            <input className="field" type="number" step="0.01" min={0} style={{ width: 110 }} value={r.value} onChange={e => setEdit(edit.map((x, j) => (j === i ? { ...x, value: e.target.value } : x)))} aria-label={tr("Valeur")} /><span className="small muted">{r.kind === 'percent' ? '%' : '€'}</span>
            <button className="iconbtn" aria-label={tr("Retirer")} onClick={() => setEdit(edit.filter((_, j) => j !== i))}><Trash2 size={16} /></button>
          </div>
        ))}
        <div className="row"><button className="btn" onClick={() => setEdit([...edit, { user_id: null, kind: 'percent', value: 0 }])}>{tr("Ajouter une règle")}</button><span className="spacer" /><button className="btn" onClick={() => setEdit(null)}>{tr("Annuler")}</button><button className="btn pri" onClick={save}>{tr("Enregistrer")}</button></div>
      </div>}
    </section>
  )
}

function Import({ reload }: { reload: () => void }) {
  const { map, toast } = useApp()
  const now = new Date()
  const [f, setF] = useState({ month: `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`, e: '', s: '' })
  const save = async (ev: React.FormEvent) => {
    ev.preventDefault()
    const c = (v: string) => Math.round(Number(v.replace(/\s/g, '').replace(',', '.')) * 100)
    try { await api.put(`/api/maps/${map!.id}/revenues`, { month: f.month, engagement_cents: c(f.e), sales_cents: c(f.s) }); toast(tr("Mois enregistré")); reload() } catch (e: any) { toast(e.message, true) }
  }
  const csv = async (file: File) => {
    try { const r = await api.post(`/api/maps/${map!.id}/revenues/csv`, { csv: await file.text() }); toast(tr("{0} mois importés{1}", [r.imported, r.errors.length ? tr(" · {0} ligne(s) ignorée(s)", [r.errors.length]) : ''])); reload() } catch (e: any) { toast(e.message, true) }
  }
  return (
    <section className="card">
      <CardHead title={tr("Import mensuel")} sub={tr("Les 2 sources versées par Epic, en saisie ou par CSV")} />
      <form className="stack" onSubmit={save}>
        <div className="grid2" style={{ gridTemplateColumns: 'repeat(3, 1fr)' }}>
          <label className="label">{tr("Mois")}<input className="field" type="month" value={f.month} onChange={e => setF({ ...f, month: e.target.value })} required /></label>
          <label className="label">{tr("Gains d'engagement (€)")}<input className="field" inputMode="decimal" value={f.e} onChange={e => setF({ ...f, e: e.target.value })} required /></label>
          <label className="label">{tr("Ventes en jeu (€)")}<input className="field" inputMode="decimal" value={f.s} onChange={e => setF({ ...f, s: e.target.value })} required /></label>
        </div>
        <button className="btn pri" style={{ alignSelf: 'flex-start' }}>{tr("Enregistrer le mois")}</button>
      </form>
      <label className="drop" style={{ cursor: 'pointer' }}><Upload size={20} /><span><b style={{ fontWeight: 500 }}>{tr("Importer un CSV")}</b><span className="small" style={{ display: 'block' }}>{tr("Colonnes : mois (AAAA-MM) ; engagement ; ventes")}</span></span>
        <input type="file" accept=".csv,text/csv" style={{ display: 'none' }} onChange={e => e.target.files?.[0] && csv(e.target.files[0])} /></label>
    </section>
  )
}

function Simulator({ cur, offers, month }: { cur: any; offers: any[]; month: string }) {
  const sales = offers.reduce((s, o) => s + (o.sales.find((x: any) => x.month === month)?.sales ?? 0), 0)
  const [e, setE] = useState(cur.engagement_cents / 100)
  const [s, setS] = useState(cur.sales_cents / 100)
  const [n, setN] = useState(sales || 100)
  useEffect(() => { setE(cur.engagement_cents / 100); setS(cur.sales_cents / 100); setN(sales || 100) }, [cur.month, sales])
  const before = e + s, after = e + s / 2, loss = s / 2
  const days = Math.ceil((new Date('2027-02-01T00:00:00').getTime() - Date.now()) / 864e5)
  return (
    <section className="card">
      <CardHead title={tr("Simulateur « passage à 50 % »")} sub={tr("Le 01/02/2027, la part reversée sur les ventes en jeu passe de 100 % à 50 %.")}>{days > 0 && <span className="badge b-o">{tr("Dans {0} jours", [days])}</span>}</CardHead>
      <div className="grid2" style={{ gridTemplateColumns: 'minmax(220px, 300px) 1fr' }}>
        <div className="stack">
          <label className="label">{tr("Gains d'engagement par mois (€)")}<input className="field" type="number" min={0} value={e} onChange={x => setE(Number(x.target.value))} /></label>
          <label className="label">{tr("Ventes en jeu par mois, aujourd'hui (€)")}<input className="field" type="number" min={0} value={s} onChange={x => setS(Number(x.target.value))} /></label>
          <label className="label">{tr("Nombre de ventes par mois")}<input className="field" type="number" min={1} value={n} onChange={x => setN(Math.max(1, Number(x.target.value)))} /></label>
        </div>
        <div className="tiles">
          <div className="card"><div className="lab">{tr("Avant · 100 %")}</div><div className="num">{euros(before * 100, 0)}</div></div>
          <div className="card"><div className="lab">{tr("Après · 50 %")}</div><div className="num">{euros(after * 100, 0)}</div></div>
          <div className="card"><div className="lab">{tr("Perte par mois")}</div><div className="num">−{euros(loss * 100, 0)}</div><div className="muted">{tr("−{0} % du revenu total", [before ? ((loss / before) * 100).toFixed(1).replace('.', ',') : 0])}</div></div>
          <div className="card"><div className="lab">{tr("Pour garder le même revenu")}</div><div className="num">{(n * 2).toLocaleString(LOCALE)} ventes</div><div className="muted">{tr("soit {0} de plus par mois (×2), ou +{1} d'engagement", [n.toLocaleString(LOCALE), euros(loss * 100, 0)])}</div></div>
        </div>
      </div>
    </section>
  )
}

function Offers({ offers, reload, month }: { offers: any[]; reload: () => void; month: string }) {
  const { map, canEdit, isAdmin, toast } = useApp()
  const [f, setF] = useState({ name: '', price: '' })
  const total = offers.reduce((s, o) => s + (o.sales.find((x: any) => x.month === month)?.revenue_cents ?? 0), 0)
  return (
    <section className="card">
      <CardHead title={tr("Offres en jeu")} sub={tr("Prix, ventes et revenu par offre · {0}", [month ? monthLabel(month) : ''])} />
      <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>{tr("Offre")}</th><th>{tr("Prix")}</th><th>{tr("Ventes")}</th><th>{tr("Revenu")}</th><th>{tr("Après le 01/02/2027")}</th><th>{tr("Part")}</th></tr></thead><tbody>
        {offers.map(o => { const m = o.sales.find((x: any) => x.month === month) ?? { sales: 0, revenue_cents: 0 }; return (
          <tr key={o.id}><td style={{ fontWeight: 500 }}>{o.name}</td><td className="muted">{o.price_vbucks} V-Bucks</td>
            <td>{isAdmin ? <input className="field" type="number" min={0} style={{ width: 90 }} defaultValue={m.sales} onBlur={async e => { await api.put(`/api/offers/${o.id}/sales`, { month, sales: Number(e.target.value), revenue_cents: m.revenue_cents }); reload() }} aria-label={tr("Ventes")} /> : m.sales}</td>
            <td>{isAdmin ? <input className="field" type="number" min={0} step="0.01" style={{ width: 110 }} defaultValue={m.revenue_cents / 100} onBlur={async e => { await api.put(`/api/offers/${o.id}/sales`, { month, sales: m.sales, revenue_cents: Math.round(Number(e.target.value) * 100) }); reload() }} aria-label={tr("Revenu")} /> : euros(m.revenue_cents)}</td>
            <td className="muted">{euros(m.revenue_cents / 2)}</td>
            <td style={{ minWidth: 140 }}><div className="row" style={{ gap: 8 }}><div className="bar" style={{ flex: 1 }}><span style={{ width: `${total ? (m.revenue_cents / total) * 100 : 0}%` }} /></div><span className="small mono">{total ? Math.round((m.revenue_cents / total) * 100) : 0} %</span></div></td></tr>
        ) })}
      </tbody></table></div>
      {canEdit && <form className="row" style={{ marginTop: 12, flexWrap: 'wrap' }} onSubmit={async e => { e.preventDefault(); try { await api.post(`/api/maps/${map!.id}/offers`, { name: f.name, price_vbucks: Number(f.price) }); setF({ name: '', price: '' }); reload() } catch (er: any) { toast(er.message, true) } }}>
        <input className="field" placeholder={tr("Nouvelle offre")} value={f.name} onChange={e => setF({ ...f, name: e.target.value })} required /><input className="field" type="number" min={1} placeholder={tr("Prix en V-Bucks")} value={f.price} onChange={e => setF({ ...f, price: e.target.value })} required /><button className="btn">{tr("Ajouter")}</button>
      </form>}
    </section>
  )
}
