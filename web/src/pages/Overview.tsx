import { useEffect, useRef, useState } from 'react'
import {
  Clock, Ban, CheckSquare, Flag, Coins, Users, Zap, Activity, ArrowRight, Upload, FileText, PenTool, AlertTriangle, MessageSquare, CheckCircle2, Rocket, Bot, User, Radio, ArrowLeft, HelpCircle, Pencil, Lightbulb, Map as MapIcon, RefreshCcw, EyeOff,
} from 'lucide-react'
import { ago, api, euros, frDate } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead, Drawer, Empty, PageHead, tagLabel } from '../components/ui'
import { tr, tx } from '../i18n/t'
import { useNav } from '../routes'
import { AvisFaisabilite } from '../components/Faisabilite'

const ICON: Record<string, [any, string]> = {
  checkin: [Upload, 'i-g'], board: [PenTool, 'i-n'], page: [FileText, 'i-n'], blocked: [Ban, 'i-r'], done: [CheckCircle2, 'i-g'],
  playtest: [Users, 'i-b'], comment: [MessageSquare, 'i-n'], money: [Coins, 'i-o'], alert: [AlertTriangle, 'i-r'], plan: [Rocket, 'i-b'], task: [CheckSquare, 'i-n'], map: [Rocket, 'i-b'],
}

export default function Overview() {
  const { map, toast } = useApp()
  const nav = useNav()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/overview` : null, [map?.id])
  // Tâches cochées ici : restent affichées (barrées) pour pouvoir annuler.
  const [checked, setChecked] = useState<Record<number, { item: any; prev: string }>>({})
  useEffect(() => setChecked({}), [map?.id])
  const undoRef = useRef<(id: number) => void>(() => {})
  const [journal, setJournal] = useState(false)
  useLive(['tasks', 'activity', 'plan'], reload)
  if (!data) return null
  const t = data.tiles
  const next = t.next_milestone
  const days = next ? Math.max(0, Math.ceil((new Date(next.date + 'T18:00:00').getTime() - Date.now()) / 864e5)) : null
  const diffWeek = t.done_week - t.done_prev_week
  const now = new Date()
  const ymd = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`

  const startTask = async (id: number, status: string) => {
    try { await api.post(`/api/tasks/${id}/status`, { status }); reload() } catch (e: any) { toast(e.message, true) }
  }
  const undo = async (id: number) => {
    const c = checked[id]; if (!c) return
    try { await api.post(`/api/tasks/${id}/status`, { status: c.prev }); setChecked(s => { const n = { ...s }; delete n[id]; return n }); reload(); toast(tr("#{0} n’est plus marquée comme faite", [c.item.number])) } catch (e: any) { toast(e.message, true) }
  }
  const check = async (x: any) => {
    try {
      await api.post(`/api/tasks/${x.id}/status`, { status: 'done' })
      setChecked(s => ({ ...s, [x.id]: { item: x, prev: x.status } })); reload()
      toast(tr("#{0} marquée comme faite", [x.number]), false, { label: tr("Annuler"), run: () => undoRef.current(x.id) })
    } catch (e: any) { toast(e.message, true) }
  }
  undoRef.current = undo
  const upcoming = data.milestones.filter((m: any) => m.date >= ymd(now))
  const todayList = [...data.today, ...Object.values(checked).map(c => c.item).filter(i => !data.today.some((x: any) => x.id === i.id))]

  return (
    <main className="content">
      <PageHead title={tr("Dashboard")} help={tr("Où en est {0}, en un coup d'œil.", [map?.name])}>
        <button className="btn pri" onClick={() => nav('/taches')}>{tr("Ouvrir les tâches")}</button>
      </PageHead>

      {map && <PremiersPas mapId={map.id} />}
      {map && <Parcours mapId={map.id} />}

      <section className="tiles six" aria-label={tr("Indicateurs")}>
        <div className="card"><div className="lab"><Clock size={18} strokeWidth={1.5} />{tr("Tâches en cours")}</div><div className="num">{t.doing}</div><div className="muted">{tr("{0} / {1} finies au total", [t.done, t.total])}</div></div>
        <div className="card"><div className="lab"><Ban size={18} strokeWidth={1.5} />{tr("Bloquées")}</div><div className="num">{t.blocked}</div><div className="muted">{t.blocked ? tr("voir « qui bloque qui »") : tr("rien ne bloque")}</div></div>
        <div className="card"><div className="lab"><CheckSquare size={18} strokeWidth={1.5} />{tr("Finies cette semaine")}</div><div className="num">{t.done_week}</div><div className="muted">{tr("{0}{1} vs semaine dernière", [diffWeek >= 0 ? '+' : '', diffWeek])}</div></div>
        <div className="card"><div className="lab"><Flag size={18} strokeWidth={1.5} />{tr("Prochain objectif")}</div><div className="num">{next ? tr('J-{0}', [days]) : '—'}</div><div className="muted">{next ? `${next.title} · ${frDate(next.date, { weekday: 'short', day: 'numeric', month: 'short' })}` : tr("aucun objectif à venir")}</div></div>
        <div className="card"><div className="lab"><Coins size={18} strokeWidth={1.5} />{tr("Gains du dernier mois")}</div><div className="num">{t.revenue ? euros(t.revenue.cents, 0) : '—'}</div><div className="muted">{t.revenue ? `${frDate(t.revenue.month + '-01', { month: 'long', year: 'numeric' })}${t.revenue_prev_cents ? ` · ${t.revenue.cents >= t.revenue_prev_cents ? '+' : ''}${Math.round((t.revenue.cents / t.revenue_prev_cents - 1) * 100)} %` : ''}` : tr("aucun import")}</div></div>
        <div className="card"><div className="lab"><Users size={18} strokeWidth={1.5} />{tr("Joueurs · dernier playtest")}</div><div className="num">{t.playtest ? t.playtest.players : '—'}</div><div className="muted">{t.playtest ? tr("décrochage moyen à {0} min", [Math.round(t.playtest.avg_minutes)]) : tr("aucun playtest")}</div></div>
      </section>

      {map && <AvisFaisabilite mapId={map.id} onEdit={() => nav('/planifier?vue=equipe')} />}

      <div className="two">
        <section className="card">
          <CardHead icon={<Zap size={22} strokeWidth={1.5} />} title={tr("À faire aujourd'hui")} sub={tr("Vos tâches d'une heure prévues pour aujourd'hui et demain")} />
          {todayList.length === 0 ? <Empty text={tr("Rien de prévu pour vous aujourd'hui.")} action={<button className="btn" onClick={() => nav('/taches')}>{tr("Prendre une tâche")}</button>} /> : (
            <div className="list">
              {todayList.map((x: any) => { const done = !!checked[x.id]; return (
                <div key={x.id} className={'li' + (done ? ' li-done' : '')}>
                  <input type="checkbox" className="chk" aria-label={done ? tr("Annuler : #{0} n’est pas faite", [x.number]) : tr("Marquer #{0} comme fait", [x.number])} checked={done} onChange={() => done ? undo(x.id) : check(x)} />
                  <span style={{ flex: 1, minWidth: 0 }}>
                    <span style={{ fontWeight: 500, textDecoration: done ? 'line-through' : undefined, opacity: done ? 0.55 : 1 }}><span className="mono muted">#{x.number}</span> {x.title}</span>
                    <span className="small muted" style={{ display: 'block' }}>{x.status === 'doing' ? tr("En cours") : tr("Prévu {0}", [frDate(x.planned_day)])}{x.waiting_on.length ? tr(" · attend #{0}", [x.waiting_on.join(', #')]) : ''}</span>
                  </span>
                  <span className="badge">{tagLabel(x.tag)}</span>
                  {done ? <button className="btn" onClick={() => undo(x.id)}>{tr("Annuler")}</button>
                    : x.status === 'todo' && !x.waiting_on.length && <button className="btn" onClick={() => startTask(x.id, 'doing')}>{tr("Commencer")}</button>}
                  <ArrowRight size={18} strokeWidth={1.5} className="muted" />
                </div>
              ) })}
            </div>
          )}
        </section>
        <section className="card">
          <CardHead title={tr("Prochains objectifs")} sub={tr("Recalculés quand une tâche prend du retard")} />
          {upcoming.length === 0 ? <Empty text={tr("Aucun objectif à venir.")} /> : <div className="list">
            {upcoming.slice(0, 3).map((m: any) => (
              <div key={m.id} className="li">
                <div style={{ width: 52, textAlign: 'center' }}><div style={{ fontSize: 22, fontWeight: 500 }}>{new Date(m.date + 'T12:00:00').getDate()}</div><div className="small muted">{frDate(m.date, { month: 'short' })}</div></div>
                <div style={{ flex: 1 }}>{m.title}</div>
              </div>
            ))}
          </div>}
          <button className="btn more" onClick={() => nav('/planifier')}>{upcoming.length > 3 ? tr("Voir plus ({0})", [upcoming.length - 3]) : tr("Voir le planning")}<ArrowRight size={16} strokeWidth={1.5} /></button>
        </section>
      </div>

      {map && <FilAvancement mapId={map.id} />}

      <section className="card">
        <CardHead icon={<Activity size={22} strokeWidth={1.5} />} title={tr("Journal d'activités")} sub={tr("Dernières actions et événements de l'équipe")} />
        <Feed items={data.activity.slice(0, 5)} />
        {data.activity.length > 5 && <button className="btn more" onClick={() => setJournal(true)}>{tr("Voir plus")}<ArrowRight size={16} strokeWidth={1.5} /></button>}
      </section>
      {journal && <Drawer label={tr("Journal d'activités")} onClose={() => setJournal(false)}>
        <CardHead icon={<Activity size={22} strokeWidth={1.5} />} title={tr("Journal d'activités")} sub={tr("Les {0} dernières actions de l'équipe", [data.activity.length])} />
        <Feed items={data.activity} />
      </Drawer>}
    </main>
  )
}

function Feed({ items }: { items: any[] }) {
  if (!items.length) return <Empty text={tr("Rien pour l'instant.")} />
  return (
    <div className="list feed">
      {items.map((a: any, i: number) => {
        const [I, cls] = ICON[a.kind] ?? [Activity, 'i-n']
        return (
          <div key={i} className="li">
            <span className={'ico ' + cls}><I size={20} strokeWidth={1.5} /></span>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div><span>{tx(a.title)}</span> <span className="small muted">• {ago(a.at)}</span></div>
              <div className="muted">{tx(a.detail)}{a.who ? ` · ${a.who}` : ''}</div>
            </div>
          </div>
        )
      })}
    </div>
  )
}

/** Fil d'avancement (#137) : ce que l'IA et POP font sur la map, en direct, avec « Annuler » sur chaque action
 * annulable et « Valider » / « Refuser » sur les suppressions demandées par une IA. */
function FilAvancement({ mapId }: { mapId: number }) {
  const { toast } = useApp()
  const { data, reload } = useLoad<{ items: any[]; pending: number }>(`/api/maps/${mapId}/fil`, [mapId])
  useLive(['uefn', 'agent', 'tasks'], reload)
  const [tout, setTout] = useState(false)
  const [occupe, setOccupe] = useState<string | null>(null)
  const agir = async (cle: string, url: string, ok: string) => {
    setOccupe(cle)
    try { await api.post(url); toast(ok); reload() } catch (e: any) { toast(e.message, true) } finally { setOccupe(null) }
  }
  if (!data) return null
  const items = tout ? data.items : data.items.slice(0, 8)
  const etat = (x: any) => x.kind === 'validation'
    ? (x.status === 'pending' ? tr("en attente de validation") : x.status === 'approved' ? tr("validée") : tr("refusée"))
    : x.status === 'queued' ? tr("en attente d'envoi") : x.status === 'error' ? tr("échec") : x.status === 'undone' ? tr("annulée") : null
  return (
    <section className="card" aria-live="polite">
      <CardHead icon={<Radio size={22} strokeWidth={1.5} />} title={tr("Fil d'avancement")} sub={data.pending ? tr("{0} suppression(s) demandée(s) par une IA à valider", [data.pending]) : tr("Ce que l'IA et POP font sur la map, en direct")} />
      {items.length === 0 ? <Empty text={tr("Rien pour l'instant : les actions de l'IA apparaîtront ici dès qu'elle travaille.")} /> : (
        <div className="list feed">
          {items.map(x => {
            const e = etat(x)
            const I = x.ia ? Bot : User
            const cle = x.kind + x.id
            return (
              <div key={cle} className="li">
                <span className={'ico ' + (x.status === 'error' ? 'i-r' : x.kind === 'validation' && x.status === 'pending' ? 'i-o' : 'i-n')}><I size={20} strokeWidth={1.5} /></span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ textDecoration: x.status === 'undone' ? 'line-through' : undefined }}>
                    {x.kind === 'tache' && <span className="mono muted">#{x.task} </span>}{tx(x.text)}{x.kind === 'tache' && <span className="muted"> · {x.title}</span>}
                  </div>
                  <div className="small muted">{[x.who, ago(x.at), e, x.kind === 'uefn' && x.task ? tr("tâche #{0}", [x.task]) : null, x.error].filter(Boolean).join(' · ')}</div>
                </div>
                {x.kind === 'uefn' && x.undoable && <button className="btn" disabled={occupe !== null} onClick={() => agir(cle, `/api/uefn-actions/${x.id}/undo`, tr("Action annulée"))}>{tr("Annuler")}</button>}
                {x.kind === 'validation' && x.status === 'pending' && <>
                  <button className="btn" disabled={occupe !== null} onClick={() => agir(cle, `/api/approvals/${x.id}/refuse`, tr("Suppression refusée"))}>{tr("Refuser")}</button>
                  <button className="btn pri" disabled={occupe !== null} onClick={() => agir(cle, `/api/approvals/${x.id}/approve`, tr("Suppression validée"))}>{tr("Valider")}</button>
                </>}
              </div>
            )
          })}
        </div>
      )}
      {data.items.length > 8 && <button className="btn more" onClick={() => setTout(!tout)}>{tout ? tr("Voir moins") : tr("Voir plus ({0})", [data.items.length - 8])}<ArrowRight size={16} strokeWidth={1.5} /></button>}
    </section>
  )
}

/** Parcours de questions-boutons (#135) : une question à la fois, posée par POP (cadrage) ou par l'IA (ask_user),
 * réponse en un clic ; retour en arrière ; une fois au bout, les réponses restent visibles et modifiables. */
function Parcours({ mapId }: { mapId: number }) {
  const { toast } = useApp()
  const { data, reload } = useLoad<{ questions: any[]; can_answer: boolean }>(`/api/maps/${mapId}/questions`, [mapId])
  useLive(['questions'], reload)
  const [pos, setPos] = useState<number | null>(null)
  const [autre, setAutre] = useState('')
  const [voir, setVoir] = useState(false)
  useEffect(() => { setPos(null); setAutre('') }, [mapId])
  if (!data) return null
  const qs = data.questions
  const premiere = qs.findIndex(q => !q.answer)
  const i = pos ?? premiere
  const q = i >= 0 ? qs[i] : null
  const repondre = async (rep: string) => {
    if (!q) return
    try {
      await api.post(`/api/maps/${mapId}/questions/${q.id}`, { answer: rep })
      setAutre(''); await reload()
      // Après une réponse : la question suivante sans réponse (ou fin du parcours).
      setPos(null)
    } catch (e: any) { toast(e.message, true) }
  }
  const faites = qs.filter(x => x.answer).length
  if (!q) {
    return (<>
      <OnLance mapId={mapId} />
      <section className="card">
        <CardHead icon={<CheckCircle2 size={22} strokeWidth={1.5} />} title={tr("Cadrage de la map")} sub={tr("{0} réponses · modifiables à tout moment", [faites])} />
        {voir ? <div className="list">
          {qs.map((x, k) => <div key={x.id} className="li">
            <span className="ico i-n">{x.author === 'ia' ? <Bot size={20} strokeWidth={1.5} /> : <HelpCircle size={20} strokeWidth={1.5} />}</span>
            <div style={{ flex: 1, minWidth: 0 }}><div>{tx(x.question)}</div><div className="small muted">{tx(x.answer)}{x.answered_by ? ` · ${x.answered_by}` : ''}</div></div>
            {data.can_answer && <button className="btn" onClick={() => setPos(k)}><Pencil size={15} strokeWidth={1.5} />{tr("Modifier")}</button>}
          </div>)}
        </div> : null}
        <button className="btn more" onClick={() => setVoir(!voir)}>{voir ? tr("Masquer les réponses") : tr("Voir les réponses")}<ArrowRight size={16} strokeWidth={1.5} /></button>
      </section>
    </>)
  }
  const precedente = i > 0 ? i - 1 : null
  return (
    <section className="card" aria-live="polite">
      <CardHead icon={q.author === 'ia' ? <Bot size={22} strokeWidth={1.5} /> : <HelpCircle size={22} strokeWidth={1.5} />}
        title={tx(q.question)}
        sub={q.author === 'ia' ? tr("Question de votre IA « {0} »", [q.agent ?? 'IA']) : tr("Cadrage de la map · question {0} sur {1}", [Math.min(i + 1, qs.length), qs.length])} />
      {!data.can_answer && <p className="small muted" style={{ marginTop: 0 }}>{tr("Vous pouvez voir les questions, mais pas y répondre : il faut le droit de modifier cette map.")}</p>}
      <div className="row" style={{ flexWrap: 'wrap', gap: 8 }}>
        {q.choices.map((c: string) => <button key={c} className={'btn' + (q.answer === c ? ' pri' : '')} disabled={!data.can_answer} onClick={() => repondre(c)}>{tx(c)}</button>)}
      </div>
      {q.other && data.can_answer && <form className="row" style={{ gap: 8, marginTop: 10 }} onSubmit={e => { e.preventDefault(); if (autre.trim()) repondre(autre) }}>
        <input className="field" style={{ flex: 1, maxWidth: 360 }} placeholder={tr("Autre réponse (facultatif)")} value={autre} onChange={e => setAutre(e.target.value)} maxLength={200} />
        <button className="btn" disabled={!autre.trim()}>{tr("Répondre")}</button>
      </form>}
      <div className="row" style={{ gap: 8, marginTop: 12 }}>
        {precedente !== null && <button className="btn" onClick={() => setPos(precedente)}><ArrowLeft size={16} strokeWidth={1.5} />{tr("Retour")}</button>}
        {pos !== null && <button className="btn" onClick={() => setPos(null)}>{tr("Revenir à la suite")}</button>}
        {q.answer && <span className="small muted">{tr("Réponse actuelle : {0}", [q.answer])}</span>}
      </div>
    </section>
  )
}

/** « On lance ? » (#136) : à la fin du parcours, résumé de ce que POP crée (tâches, jalons, premières tâches de
 * l'IA) et un seul bouton. Sur une map qui a déjà des tâches, POP demande quoi faire au lieu de créer des doublons. */
function OnLance({ mapId }: { mapId: number }) {
  const { toast } = useApp()
  const nav = useNav()
  const { data, reload } = useLoad<any>(`/api/maps/${mapId}/lancement`, [mapId])
  useLive(['tasks', 'questions'], reload)
  const [ouvert, setOuvert] = useState(false)
  const [occupe, setOccupe] = useState(false)
  const [fait, setFait] = useState<any>(null)
  const [cache, setCache] = useState(false)
  if (!data || !data.ready || data.can_launch === false) return null
  const lancer = async (mode: string) => {
    setOccupe(true)
    try { const r = await api.post<any>(`/api/maps/${mapId}/lancement`, { mode }); setFait(r); setOuvert(false); reload() } catch (e: any) { toast(e.message, true) } finally { setOccupe(false) }
  }
  if (fait) return (
    <section className="card">
      <CardHead icon={<Rocket size={22} strokeWidth={1.5} />} title={tr("C'est parti")} sub={fait.created ? tr("{0} tâches et {1} jalons créés", [fait.created, fait.milestones]) : tx(fait.message)} />
      {fait.for_ai?.length > 0 && <p className="muted" style={{ marginTop: 0 }}>{tr("Votre IA a reçu ses premières tâches : {0}", [fait.for_ai.map((x: any) => `#${x.number} ${x.title}`).join(' · ')])}</p>}
      <div className="row" style={{ gap: 8 }}><button className="btn" onClick={() => nav('/taches')}>{tr("Voir les tâches")}</button><button className="btn" onClick={() => nav('/planifier')}>{tr("Voir le planning")}</button><button className="btn" onClick={() => setFait(null)}>{tr("Fermer")}</button></div>
    </section>
  )
  if ((data.launched || cache) && !ouvert) return (
    <div className="row" style={{ justifyContent: 'flex-end', marginBottom: -8 }}>
      <button className="btn" onClick={() => setOuvert(true)}><Rocket size={16} strokeWidth={1.5} />{tr("Relancer la mise en route")}</button>
    </div>
  )
  const deja = data.existing > 0
  return (
    <section className="card" aria-live="polite">
      <CardHead icon={<Rocket size={22} strokeWidth={1.5} />} title={tr("On lance ?")} sub={tr("POP prépare la production à partir de vos réponses, sans IA externe")} />
      <ul style={{ margin: '0 0 12px', paddingLeft: 20, display: 'grid', gap: 4 }}>
        <li>{tr("{0} tâches d'environ une heure, rangées par lot, avec leurs dépendances", [data.tasks])}</li>
        <li>{tr("{0} jalons, jusqu'à la sortie le {1}", [data.milestones, frDate(data.release_date, { day: 'numeric', month: 'long', year: 'numeric' })])}</li>
        <li>{data.agent ? tr("Les {0} premières tâches pour votre IA « {1} »", [data.for_ai, data.agent]) : tr("Aucune IA ne travaille sur cette map : les tâches restent à prendre")}</li>
      </ul>
      {deja && <p className="small" style={{ marginTop: 0 }}>{tr("Cette map a déjà {0} tâches. {1} tâches du plan n'existent pas encore.", [data.existing, data.missing])}</p>}
      <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
        {deja ? <button className="btn pri" disabled={occupe || data.missing === 0} onClick={() => lancer('manquantes')}>{tr("Ajouter seulement ce qui manque")}</button>
          : <button className="btn pri" disabled={occupe} onClick={() => lancer('tout')}><Rocket size={16} strokeWidth={1.5} />{tr("Lancer")}</button>}
        {deja && <button className="btn" onClick={() => { setOuvert(false); setCache(true) }}>{tr("Ne rien créer")}</button>}
      </div>
    </section>
  )
}


const PAS_ICONE: Record<string, any> = { design: FileText, layout: MapIcon, brainstorm: Lightbulb, boucle: RefreshCcw }

/** Premiers pas d'une map neuve avec son IA (#146) : un clic crée le document (page ou tableau) et une tâche pour
 * l'IA connectée, puis ouvre le document. Seulement quand une IA travaille sur la map ; masquable pour la map. */
function PremiersPas({ mapId }: { mapId: number }) {
  const { toast } = useApp()
  const nav = useNav()
  const { data, reload } = useLoad<any>(`/api/maps/${mapId}/suggestions`, [mapId])
  useLive(['ai', 'tasks'], reload)
  const cle = `pop.suggestions.masquees.${mapId}`
  const [masque, setMasque] = useState(() => { try { return localStorage.getItem(cle) === '1' } catch { return false } })
  useEffect(() => { try { setMasque(localStorage.getItem(cle) === '1') } catch { setMasque(false) } }, [cle])
  const [occupe, setOccupe] = useState<string | null>(null)
  if (!data || !data.ai || masque) return null
  const restants = data.items.filter((x: any) => !x.done)
  if (restants.length === 0) return null
  const demarrer = async (k: string) => {
    setOccupe(k)
    try {
      const r = await api.post<any>(`/api/maps/${mapId}/suggestions/${k}`)
      if (r.created) toast(tr("Tâche #{0} donnée à {1}", [r.task, r.ai ?? 'IA']))
      nav(r.kind === 'page' ? `/pages/${r.id}` : `/tableau/${r.id}`)
    } catch (e: any) { toast(e.message, true) } finally { setOccupe(null) }
  }
  const masquer = () => { try { localStorage.setItem(cle, '1') } catch {}; setMasque(true) }
  return (
    <section className="card">
      <CardHead icon={<Lightbulb size={22} strokeWidth={1.5} />} title={tr("Premiers pas avec {0}", [data.ai])} sub={tr("Un clic crée le document et donne à votre IA la tâche de le remplir avec vous")} />
      {!data.can_start && <p className="small muted" style={{ marginTop: 0 }}>{tr("Il faut être membre du studio pour lancer ces premiers pas.")}</p>}
      <div className="list">
        {restants.map((x: any) => { const I = PAS_ICONE[x.key] ?? Lightbulb; return (
          <div key={x.key} className="li">
            <span className="ico i-n"><I size={20} strokeWidth={1.5} /></span>
            <div style={{ flex: 1, minWidth: 0 }}>{tx(x.label)}</div>
            <button className="btn" disabled={!data.can_start || occupe !== null} onClick={() => demarrer(x.key)}>{occupe === x.key ? tr("Création…") : tr("Démarrer")}</button>
          </div>
        ) })}
      </div>
      <button className="btn more" onClick={masquer}><EyeOff size={16} strokeWidth={1.5} />{tr("Masquer les suggestions")}</button>
    </section>
  )
}
