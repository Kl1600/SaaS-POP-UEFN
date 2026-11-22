import { useState } from 'react'
import { CheckCircle2, AlertTriangle, Ban, Circle } from 'lucide-react'
import { ago, api, frDate } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead, Modal, PageHead, Tabs } from '../components/ui'
import { tr, tx } from '../i18n/t'
import LaunchDiscover from './LaunchDiscover'

const ST: Record<string, [string, string, any]> = { ok: [tr("Conforme"), 'b-g', CheckCircle2], warn: [tr("À corriger"), 'b-o', AlertTriangle], block: [tr("Bloquant"), 'b-r', Ban], todo: [tr("À vérifier"), '', Circle] }

export default function Launch() {
  const [tab, setTab] = useState<'discover' | 'checks' | 'swap' | 'dday'>('discover')
  return (
    <main className="content">
      <PageHead title={tr("Lancer et suivre")} help={tr("Préparez la sortie selon l’algorithme de Discover, suivez les 14 jours de test d’Epic, faites tester la map et préparez le jour J.")} />
      <Tabs value={tab} onChange={setTab} items={[['discover', 'Discover'], ['checks', tr("Vérification")], ['swap', tr("Test")], ['dday', tr("Jour J")]]} />
      {tab === 'discover' && <LaunchDiscover />}
      {tab === 'checks' && <Checks />}
      {tab === 'swap' && <Swap />}
      {tab === 'dday' && <DDay />}
    </main>
  )
}

function Checks() {
  const { map, canEdit, toast } = useApp()
  const { data, reload } = useLoad<any[]>(map ? `/api/maps/${map.id}/checks` : null, [map?.id])
  if (!data) return null
  const count = (s: string) => data.filter(c => c.status === s).length
  const set = async (key: string, status: string, note: string) => { try { await api.put(`/api/maps/${map!.id}/checks/${key}`, { status, note }); reload() } catch (e: any) { toast(e.message, true) } }
  return <>
    <div className="tiles">
      <div className="card"><div className="lab">{tr("Points conformes")}</div><div className="num">{count('ok')} / {data.length}</div><div className="bar" style={{ marginTop: 12 }}><span style={{ width: `${(count('ok') / data.length) * 100}%` }} /></div></div>
      <div className="card"><div className="lab">{tr("À corriger")}</div><div className="num">{count('warn')}</div><div className="muted">{tr("avant l'envoi en modération")}</div></div>
      <div className="card"><div className="lab">{tr("Bloquants")}</div><div className="num">{count('block')}</div><div className="muted">{tr("empêchent la publication")}</div></div>
    </div>
    <section className="card">
      <CardHead title={tr("Vérification avant publication")} sub={tr("Liste indicative à aligner sur les règles de modération Epic en vigueur.")} />
      <div className="list">
        {data.map(c => { const [l, cls, I] = ST[c.status]; return (
          <div key={c.key} className="li" style={{ flexWrap: 'wrap' }}>
            <span className={'ico ' + (cls === 'b-g' ? 'i-g' : cls === 'b-o' ? 'i-o' : cls === 'b-r' ? 'i-r' : 'i-n')}><I size={18} strokeWidth={1.5} /></span>
            <div style={{ flex: '1 1 280px', minWidth: 0 }}><div style={{ fontWeight: 500 }}>{tx(c.title)}</div><div className="muted">{tx(c.rule)}</div>
              <input className="field" style={{ marginTop: 6, width: '100%', height: 34 }} defaultValue={c.note} placeholder={tr("Explication ou correction à faire")} disabled={!canEdit} onBlur={e => e.target.value !== c.note && set(c.key, c.status, e.target.value)} /></div>
            <select className="field" value={c.status} disabled={!canEdit} onChange={e => set(c.key, e.target.value, c.note)} aria-label={tr("Statut de {0}", [tx(c.title)])}>{Object.entries(ST).map(([k, v]) => <option key={k} value={k}>{v[0]}</option>)}</select>
          </div>
        ) })}
      </div>
    </section>
  </>
}

function Swap() {
  const { map, canEdit, toast, reload: reloadMe } = useApp()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/playtests` : null, [map?.id])
  const [testing, setTesting] = useState<any>(null)
  const [req, setReq] = useState({ slots: 5, reward: 20, minutes: 15, island_code: '' })
  if (!data) return null
  const order = async (e: React.FormEvent) => {
    e.preventDefault()
    try { const r = await api.post(`/api/maps/${map!.id}/playtests`, req); toast(tr("Demande publiée ({0} crédits)", [r.cost])); reload(); reloadMe() } catch (er: any) { toast(er.message, true) }
  }
  return <>
    <div className="tiles">
      <div className="card"><div className="lab">{tr("Mes crédits")}</div><div className="num">{data.credits}</div><div className="muted">{tr("gagnés en testant les maps des autres")}</div></div>
      <div className="card"><div className="lab">{tr("Mes demandes")}</div><div className="num">{data.mine.length}</div><div className="muted">{tr("{0} places encore ouvertes", [data.mine.reduce((s: number, m: any) => s + m.slots, 0)])}</div></div>
      <div className="card"><div className="lab">{tr("Retours reçus")}</div><div className="num">{data.feedback.length}</div><div className="muted">{data.feedback.length ? tr("note moyenne {0} / 5", [(data.feedback.reduce((s: number, f: any) => s + f.score, 0) / data.feedback.length).toFixed(1).replace('.', ',')]) : '—'}</div></div>
    </div>
    <section className="card">
      <CardHead title={tr("Demandes de test")} sub={tr("Testez les maps des autres pour gagner des crédits, et dépensez-les pour faire tester la vôtre.")} />
      <div className="two" style={{ alignItems: 'start' }}>
        <div>
          <div className="dsec" style={{ marginTop: 0 }}>{tr("À tester · demandes des autres")}</div>
        {data.open.length === 0 && <p className="muted">{tr("Aucune map à tester pour le moment.")}</p>}
          <div className="list">{data.open.map((m: any) => (
            <div key={m.id} className="li" style={{ flexWrap: 'wrap' }}>
              <div style={{ flex: '1 1 180px' }}><div style={{ fontWeight: 500 }}>{m.map}</div><div className="small muted">{tr("{0} · {1} min · {2}{3}", [m.genre, m.minutes, m.studio, m.island_code ? tr(" · code {0}", [m.island_code]) : ''])}</div></div>
              <span className="badge">{tr("+{0} crédits", [m.reward])}</span>
              <button className="btn" disabled={m.done} onClick={() => setTesting(m)}>{m.done ? tr("Déjà testée") : tr("Tester")}</button>
            </div>
          ))}</div>
        </div>
        <div>
          <div className="dsec" style={{ marginTop: 0 }}>{tr("Nos demandes · {0}", [map?.name])}</div>
          {data.mine.length === 0 && <p className="small muted">{tr("Aucune demande publiée.")}</p>}
          <div className="list">{data.mine.map((m: any) => <div key={m.id} className="li">
            <div style={{ flex: 1 }}><div style={{ fontWeight: 500 }}>{tr("Demande du {0}", [frDate(m.created_at)])}</div><div className="small muted">{tr("{0} min · {1} crédits par test", [m.minutes, m.reward])}</div></div>
            <span className="small muted">{tr("{0} retour(s)", [m.received])}</span>
            <span className={'badge ' + (m.slots > 0 ? 'b-o' : 'b-g')}>{m.slots > 0 ? tr("En cours · {0} place(s)", [m.slots]) : tr("Terminée")}</span>
          </div>)}</div>
          <div className="dsec">{tr("Nouvelle demande")}</div>
        <form className="stack" onSubmit={order}>
            <div className="grid2" style={{ gridTemplateColumns: 'repeat(3, 1fr)' }}>
              <label className="label">{tr("Testeurs")}<input className="field" type="number" min={1} max={20} value={req.slots} onChange={e => setReq({ ...req, slots: Number(e.target.value) })} /></label>
              <label className="label">{tr("Crédits / test")}<input className="field" type="number" min={5} max={100} value={req.reward} onChange={e => setReq({ ...req, reward: Number(e.target.value) })} /></label>
              <label className="label">{tr("Minutes")}<input className="field" type="number" min={5} max={60} value={req.minutes} onChange={e => setReq({ ...req, minutes: Number(e.target.value) })} /></label>
            </div>
            <label className="label">{tr("Code de l'île")}<input className="field mono" value={req.island_code} onChange={e => setReq({ ...req, island_code: e.target.value })} placeholder="1234-5678-9012" /></label>
            <button className="btn pri" disabled={!canEdit || req.slots * req.reward > data.credits}>{tr("Publier · {0} crédits", [req.slots * req.reward])}</button>
          </form>
        </div>
      </div>
    </section>
    <section className="card">
      <CardHead title={tr("Retours")} sub={data.feedback.length ? tr("{0} retour(s) des testeurs, avec leurs réponses au questionnaire", [data.feedback.length]) : tr("Les retours des testeurs arrivent ici.")} />
      <Reasons />
      <div className="list">{data.feedback.map((f: any, i: number) => (
        <div key={i} className="li" style={{ alignItems: 'flex-start' }}><div style={{ flex: 1 }}><div><span style={{ fontWeight: 500 }}>{f.who}</span> <span className="small muted">{tr("· {0} · parti à {1} min · objectif compris : {2}", [ago(f.at), f.left_minute, tx(f.understood) || '—'])}</span></div><div className="muted">{tx(f.reason)}{f.comeback ? tr(" — reviendrait pour : {0}", [f.comeback]) : ''}</div></div><span className="badge">{f.score} / 5</span></div>
      ))}</div>
    </section>
    {testing && <Questionnaire m={testing} onClose={() => setTesting(null)} onDone={() => { setTesting(null); reload(); reloadMe() }} />}
  </>
}

function Questionnaire({ m, onClose, onDone }: { m: any; onClose: () => void; onDone: () => void }) {
  const { toast } = useApp()
  const [f, setF] = useState({ understood: 'Oui', left_minute: 10, reason: 'Fin de session', comeback: '', score: 4 })
  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    try { const r = await api.post(`/api/playtests/${m.id}/feedback`, f); toast(tr("+{0} crédits", [r.earned])); onDone() } catch (er: any) { toast(er.message, true) }
  }
  return (
    <Modal onClose={onClose} label={tr("Questionnaire guidé")}>
      <form className="stack" onSubmit={submit}>
        <h2 style={{ margin: 0, fontWeight: 400 }}>{tr("Questionnaire · {0}", [m.map])}</h2>
        <p className="muted" style={{ margin: 0 }}>{tr("Jouez {0} minutes{1}, puis répondez.", [m.minutes, m.island_code ? ` (code ${m.island_code})` : ''])}</p>
        <fieldset style={{ border: 0, padding: 0 }}><legend style={{ marginBottom: 8 }}>{tr("1. Avez-vous compris l'objectif en moins d'une minute ?")}</legend>
          <div className="row">{['Oui', 'En partie', 'Non'].map(x => <label key={x} className="chip"><input type="radio" name="u" checked={f.understood === x} onChange={() => setF({ ...f, understood: x })} />{tr(x)}</label>)}</div></fieldset>
        <label className="label">{tr("2. À quelle minute avez-vous eu envie d'arrêter ?")}<input className="field" type="number" min={0} max={180} value={f.left_minute} onChange={e => setF({ ...f, left_minute: Number(e.target.value) })} /></label>
        <label className="label">{tr("3. Pourquoi ?")}<select className="field" value={f.reason} onChange={e => setF({ ...f, reason: e.target.value })}>
          {['Fin de session', 'Je ne savais pas quoi faire ensuite', 'Trop long avant la première récompense', 'Trop difficile', 'Partie déséquilibrée', 'Bugs ou lag', 'Pas mon style de jeu'].map(x => <option key={x} value={x}>{tr(x)}</option>)}</select></label>
        <label className="label">{tr("4. Qu'est-ce qui vous ferait revenir demain ?")}<textarea className="field" style={{ minHeight: 70, padding: 10 }} value={f.comeback} onChange={e => setF({ ...f, comeback: e.target.value })} /></label>
        <label className="label">{tr("5. Note globale")}<input type="range" min={1} max={5} value={f.score} onChange={e => setF({ ...f, score: Number(e.target.value) })} /><span>{f.score} / 5</span></label>
        <div className="row" style={{ justifyContent: 'flex-end' }}><button type="button" className="btn" onClick={onClose}>{tr("Annuler")}</button><button className="btn pri">{tr("Envoyer · +{0} crédits", [m.reward])}</button></div>
      </form>
    </Modal>
  )
}

/** Réponses au questionnaire : pourquoi les testeurs ont arrêté (part de chaque raison). */
function Reasons() {
  const { map } = useApp()
  const { data } = useLoad<any>(map ? `/api/maps/${map.id}/churn` : null, [map?.id])
  if (!data || data.testers === 0) return null
  return <div className="stack" style={{ gap: 10, marginBottom: 16 }}>
    <div className="small muted">{tr("Pourquoi ils ont arrêté · {0} testeurs", [data.testers])}</div>
    {data.reasons.map((r: any) => <div key={r.reason}><div className="row" style={{ justifyContent: 'space-between', marginBottom: 4 }}><span className="small">{tx(r.reason)}</span><span className="small mono">{Math.round((r.count / data.testers) * 100)} %</span></div><div className="bar"><span style={{ width: `${(r.count / data.testers) * 100}%` }} /></div></div>)}
  </div>
}

function DDay() {
  const { map, canEdit, toast } = useApp()
  const { data, reload } = useLoad<any[]>(map ? `/api/maps/${map.id}/launch-steps` : null, [map?.id])
  const { data: sw } = useLoad<any>(map ? `/api/maps/${map.id}/playtests` : null, [map?.id])
  const [f, setF] = useState({ time: '', title: '', owner: '' })
  useLive(['launch'], reload)
  if (!data) return null
  const rel = map?.release_date
  const daysLeft = rel ? Math.ceil((new Date(rel + 'T18:00').getTime() - Date.now()) / 864e5) : null
  return <>
    <div className="tiles">
      <div className="card"><div className="lab">{tr("Créneau de sortie")}</div><div className="num" style={{ fontSize: 28 }}>{rel ? frDate(rel, { weekday: 'short', day: 'numeric', month: 'short' }) : '—'}</div><div className="muted">{daysLeft !== null ? (daysLeft > 0 ? tr("dans {0} jours", [daysLeft]) : tr("c'est le jour J")) : tr("à définir dans Équipe › Maps")}</div></div>
      <div className="card"><div className="lab">{tr("Testeurs inscrits")}</div><div className="num">{sw?.feedback.length ?? 0}</div><div className="muted">{tr("via l'échange de playtests")}</div></div>
      <div className="card"><div className="lab">{tr("Checklist")}</div><div className="num">{data.filter(s => s.done).length} / {data.length}</div><div className="muted">{tr("étapes faites")}</div></div>
    </div>
    <section className="card">
      <CardHead title={tr("Checklist heure par heure")} sub={tr("Ajoutez aussi les posts à publier (Discord, TikTok, X…) avec leur heure.")} />
      <div className="list">{data.map(s => (
        <label key={s.id} className="li" style={{ cursor: 'pointer' }}>
          <input type="checkbox" className="chk" checked={s.done} disabled={!canEdit} onChange={async e => { await api.patch(`/api/launch-steps/${s.id}`, { done: e.target.checked }); reload() }} />
          <span className="mono small muted" style={{ width: 50 }}>{s.time}</span><span style={{ flex: 1 }} className={s.done ? 'done' : ''}>{tx(s.title)}</span><span className="small muted">{tx(s.owner)}</span>
        </label>
      ))}</div>
      {canEdit && <form className="row" style={{ marginTop: 12, flexWrap: 'wrap' }} onSubmit={async e => { e.preventDefault(); try { await api.post(`/api/maps/${map!.id}/launch-steps`, f); setF({ time: '', title: '', owner: '' }); reload() } catch (er: any) { toast(er.message, true) } }}>
        <input className="field mono" type="time" value={f.time} onChange={e => setF({ ...f, time: e.target.value })} required aria-label={tr("Heure")} />
        <input className="field" style={{ flex: 1, minWidth: 200 }} placeholder={tr("Étape ou post à publier")} value={f.title} onChange={e => setF({ ...f, title: e.target.value })} required />
        <input className="field" placeholder={tr("Qui")} value={f.owner} onChange={e => setF({ ...f, owner: e.target.value })} />
        <button className="btn">{tr("Ajouter")}</button>
      </form>}
    </section>
  </>
}
