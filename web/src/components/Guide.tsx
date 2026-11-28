// Popy : assistant intégré (calculs côté serveur sur les données de la map + guide de POP, rien ne sort de POP).
import { useEffect, useRef, useState } from 'react'
import { useLocation } from 'react-router-dom'
import { Compass, Play, ArrowRight, X, Send } from '../icons'
import { ask, suggested, type Fiche } from '../guideKB'
import { api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr, LANG } from '../i18n/t'
import { useNav } from '../routes'
import { useDialog } from './ui'

/** /map/12/taches → /taches ; /studio/3/equipe → /equipe (pages des fiches d’aide). */
const shortPath = (p: string) => p.replace(/^\/map\/\d+/, '').replace(/^\/studio\/\d+\/(integrations|maps)/, '/equipe').replace(/^\/studio\/\d+\/marche/, '/analyse/marche').replace(/^\/analyse\/videos/, '/analyse/tiktok').replace(/^\/studio\/\d+/, '') || '/'

/** Récap de la journée, premier message du chat (#284) : une phrase, 3 points, l'objectif, les tâches à faire. */
function Recap({ onNavigate }: { onNavigate?: () => void }) {
  const { map, me, toast } = useApp()
  const nav = useNav()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/guide?lang=${LANG}` : null, [map?.id])
  useLive(['tasks', 'plan'], reload)
  if (!data) return <span className="gtyping"><i /><i /><i /></span>
  const today = new Date().toISOString().slice(0, 10)
  const take = async (s: any) => {
    try {
      if (!s.mine) await api.patch(`/api/tasks/${s.id}`, { assignee_id: me!.user.id, ...(s.planned_day ? {} : { planned_day: today }) })
      await api.post(`/api/tasks/${s.id}/status`, { status: 'doing' })
      toast(tr("#{0} est à vous, c’est parti", [s.number])); reload()
    } catch (e: any) { toast(e.message, true) }
  }
  const open = (id: number) => { onNavigate?.(); nav(`/taches?t=${id}`) }
  return <div className="grecap">
    <p className="grecap-hl">{data.headline}</p>
    {data.pace && !data.pace.on_track && <div className="grecap-goal"><span>{tr("Objectif :")}{' '}<b style={{ fontWeight: 500 }}>{data.pace.title}</b></span><span className={'badge ' + (data.pace.on_track ? 'b-g' : 'b-r')}>{data.pace.on_track ? tr("Dans les temps") : tr("En retard")}</span></div>}
    {data.suggestions.length === 0 ? <p className="small muted" style={{ margin: 0 }}>{tr("Rien à prendre pour l’instant.")}</p> :
      <div className="grecap-tasks">{data.suggestions.slice(0, 2).map((s: any, i: number) => <div key={s.id} className="grecap-task">
        <button className="grecap-t" onClick={() => open(s.id)}><span><span className="mono muted">#{s.number}</span> {s.title}</span></button>
        {s.action === 'continue'
          ? <button className="btn" onClick={() => open(s.id)}><ArrowRight size={14} />{tr("Continuer")}</button>
          : <button className={'btn' + (i === 0 ? ' pri' : '')} onClick={() => take(s)}><Play size={13} />{s.mine ? tr("Commencer") : tr("Prendre")}</button>}
      </div>)}</div>}
  </div>
}

type Msg = { who: 'me' | 'bot'; recap?: boolean; text?: string; f?: Fiche; others?: Fiche[]; today?: boolean; items?: { label: string; link: string; kind?: string }[]; typing?: boolean }

/** Questions-réponses : Popy guide l'utilisateur dans POP (base de fiches locale, pas d'IA). */
function GuideChat({ initial, onNavigate }: { initial?: string; onNavigate?: () => void }) {
  const nav = useNav()
  const loc = useLocation()
  const { map, me } = useApp()
  const first = (me?.user.name ?? '').split(' ')[0]
  // Le chat s'ouvre sur le récap de la journée (si une map est ouverte), puis l'invitation à poser une question.
  const [msgs, setMsgs] = useState<Msg[]>([
    ...(map ? [{ who: 'bot' as const, recap: true }] : [{ who: 'bot' as const, text: first ? tr("Bonjour {0} !", [first]) : tr("Bonjour !") }]),
  ])
  const [q, setQ] = useState('')
  const end = useRef<HTMLDivElement>(null)
  const onToday = () => setMsgs(m => [...m, { who: 'bot', recap: true }])
  const send = async (text: string) => {
    const t = text.trim(); if (!t) return
    setQ(''); setMsgs(m => [...m, { who: 'me', text: t }, { who: 'bot', typing: true }])
    const res = ask(t, shortPath(loc.pathname))
    let bot: Msg
    // 1. Question sur les données de la map (retards, une personne, une tâche, l’objectif, les chiffres…).
    const data = map ? await api.get(`/api/maps/${map.id}/guide/ask?lang=${LANG}&q=${encodeURIComponent(t)}`).catch(() => null) : null
    if (data?.answer) bot = { who: 'bot', text: data.answer, items: data.items, others: res.slice(0, 2).map(x => x.f) }
    // 2. Question sur l'utilisation de POP.
    else if (res.length) bot = { who: 'bot', f: res[0].f, others: res.slice(1, 4).filter(x => x.score >= res[0].score * 0.6).map(x => x.f) }
    else {
      // 3. Sinon, on cherche dans la map (tâches, pages, tableaux, Verse, membres…).
      const found: any[] = map && t.length >= 2 ? await api.get(`/api/maps/${map.id}/search?q=${encodeURIComponent(t)}`).catch(() => []) : []
      bot = found.length
        ? { who: 'bot', text: tr("Je n’ai pas de fiche sur ce sujet, mais voici ce que j’ai trouvé dans votre map :"), items: found.slice(0, 6).map(r => ({ label: r.title + (r.detail ? ` · ${r.detail}` : ''), link: r.link, kind: r.kind })) }
        : { who: 'bot', text: tr("Je n’ai pas trouvé de réponse. Reformulez avec d’autres mots, ou choisissez un sujet ci-dessous."), others: suggested(shortPath(loc.pathname)) }
    }
    setMsgs(m => [...m.filter(x => !x.typing), bot])
  }
  const pick = (f: Fiche) => setMsgs(m => [...m, { who: 'me', text: f.q }, { who: 'bot', f }])
  useEffect(() => { if (initial) send(initial) }, [initial])
  useEffect(() => { end.current?.scrollIntoView({ block: 'end', behavior: 'smooth' }) }, [msgs])
  return <div className="gchat">
    <div className="gmsgs">
      {msgs.map((m, i) => m.who === 'me' ? <div key={i} className="gmsg me">{m.text}</div> : <div key={i} className="gmsg bot">
        {m.recap && <Recap onNavigate={onNavigate} />}
        {m.typing && <span className="gtyping"><i /><i /><i /></span>}
        {m.text && <p>{m.text}</p>}
        {m.items && m.items.length > 0 && <div className="gitems">{m.items.map((it, j) => <button key={j} onClick={() => { onNavigate?.(); nav(it.link) }}>{it.kind && <span className="badge" style={{ marginRight: 6 }}>{it.kind}</span>}{it.label}<ArrowRight size={13} /></button>)}</div>}
        {m.f && <>
          <p style={{ fontWeight: 500 }}>{m.f.q}</p>
          <p>{m.f.a}</p>
          {m.f.steps && <ol>{m.f.steps.map((s, j) => <li key={j}>{s}</li>)}</ol>}
          <div className="row" style={{ gap: 8, flexWrap: 'wrap', marginTop: 6 }}>
            {m.f.action === 'today' && map && <button className="btn pri" style={{ height: 32 }} onClick={onToday}><Compass size={14} />{tr("Voir ma journée")}</button>}
            {m.f.go && <button className="btn pri" style={{ height: 32 }} onClick={() => { onNavigate?.(); nav(m.f!.go![0]) }}>{m.f.go[1]}<ArrowRight size={14} /></button>}
          </div>
        </>}
        {m.others && m.others.length > 0 && <div className="gothers"><span className="small muted">{m.f ? tr("Vous cherchiez peut-être :") : tr("Sujets possibles :")}</span>{m.others.map(o => <button key={o.id} className="chip" onClick={() => pick(o)}>{o.q}</button>)}</div>}
      </div>)}
      <div ref={end} />
    </div>
    {msgs.length <= 1 && <div className="gothers gsugs">{suggested(shortPath(loc.pathname)).slice(0, 3).map(f => <button key={f.id} className="chip" onClick={() => pick(f)}>{f.q}</button>)}</div>}
    <form className="gask gask-main" onSubmit={e => { e.preventDefault(); send(q) }}>
      <input className="field" value={q} onChange={e => setQ(e.target.value)} placeholder={tr("Posez votre question à Popy…")} aria-label={tr("Votre question")} />
      <button className="btn pri" aria-label={tr("Envoyer")}><Send size={16} /></button>
    </form>
  </div>
}

export function GuideDrawer({ onClose, ask: initial }: { onClose: () => void; ask?: string }) {
  const dlg = useDialog(onClose)
  return <>
    <div className="scrim" onClick={onClose} />
    <aside ref={dlg as any} tabIndex={-1} aria-modal="true" className="drawer gdrawer" role="dialog" aria-label={tr("Popy")}>
      <div className="row" style={{ justifyContent: 'space-between', marginBottom: 12 }}>
        <div className="row" style={{ gap: 10 }}>
          <span className="guide-ico"><Compass size={18} strokeWidth={1.75} /></span>
          <div><div className="guide-name">{tr("Popy")}</div><div className="small muted">{tr("Assistant intégré · tout reste dans POP")}</div></div>
        </div>
        <button className="iconbtn" aria-label={tr("Fermer")} onClick={onClose}><X size={18} /></button>
      </div>
      <GuideChat initial={initial || undefined} onNavigate={onClose} />
    </aside>
  </>
}
