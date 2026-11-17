// Boussole : guide virtuel du jour (règles simples côté serveur, pas d'IA).
import { useEffect, useRef, useState } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import { Compass, AlertCircle, AlertTriangle, CheckCircle2, Info, Play, ArrowRight, X, Send } from 'lucide-react'
import { ask, suggested, type Fiche } from '../guideKB'
import { api } from '../api'
import { useApp, useLive, useLoad } from '../state'

const TONE: Record<string, [any, string]> = {
  bad: [AlertCircle, 'var(--tr)'], warn: [AlertTriangle, 'var(--to)'], good: [CheckCircle2, 'var(--tg)'], info: [Info, 'var(--text3)'],
}

export function GuidePanel({ compact, onNavigate }: { compact?: boolean; onNavigate?: () => void }) {
  const { map, me, toast } = useApp()
  const nav = useNavigate()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/guide` : null, [map?.id])
  useLive(['tasks', 'plan'], reload)
  if (!data) return null
  const today = new Date().toISOString().slice(0, 10)
  const take = async (s: any) => {
    try {
      if (!s.mine) await api.patch(`/api/tasks/${s.id}`, { assignee_id: me!.user.id, ...(s.planned_day ? {} : { planned_day: today }) })
      await api.post(`/api/tasks/${s.id}/status`, { status: 'doing' })
      toast(`#${s.number} est à vous, c’est parti`); reload()
    } catch (e: any) { toast(e.message, true) }
  }
  const open = (id: number) => { onNavigate?.(); nav(`/taches?t=${id}`) }
  return <div className="guide">
    <div className="guide-head">
      <span className="guide-ico"><Compass size={20} strokeWidth={1.75} /></span>
      <div style={{ minWidth: 0 }}>
        <div className="guide-name">Boussole <span className="small muted">· votre guide du jour</span></div>
        <div className="guide-hl">{data.headline}</div>
      </div>
    </div>
    {data.points.length > 0 && <ul className="guide-pts">{data.points.slice(0, compact ? 4 : 8).map((p: any, i: number) => { const [I, c] = TONE[p.tone] ?? TONE.info; return <li key={i}><I size={16} style={{ color: c, flex: 'none', marginTop: 2 }} /><span>{p.text}</span></li> })}</ul>}
    {data.pace && <div className="guide-pace" title={`${data.pace.remaining} tâche(s) restante(s), ${data.pace.per_day} finie(s) par jour en moyenne`}>
      <div className="row small" style={{ justifyContent: 'space-between' }}><span>Prochain jalon : <b style={{ fontWeight: 500 }}>{data.pace.title}</b></span><span className={'badge ' + (data.pace.on_track ? 'b-g' : 'b-r')}>{data.pace.on_track ? 'Dans les temps' : 'En retard'}</span></div>
    </div>}
    <div className="dsec" style={{ marginTop: 14 }}>À faire maintenant</div>
    {data.suggestions.length === 0 ? <p className="small muted" style={{ margin: 0 }}>Aucune tâche disponible pour vous. Créez-en une ou demandez à l’équipe.</p> :
      <div className="guide-sugs">{data.suggestions.map((s: any, i: number) => <div key={s.id} className={'guide-sug' + (i === 0 ? ' first' : '')}>
        <button className="guide-sug-main" onClick={() => open(s.id)}>
          <span className="guide-sug-t"><span className="mono muted">#{s.number}</span> {s.title}</span>
          <span className="small muted">{s.why.join(' · ')}</span>
        </button>
        {s.action === 'continue'
          ? <button className="btn" onClick={() => open(s.id)}><ArrowRight size={15} />Continuer</button>
          : <button className={'btn' + (i === 0 ? ' pri' : '')} onClick={() => take(s)}><Play size={14} />{s.mine ? 'Commencer' : 'Prendre'}</button>}
      </div>)}</div>}
    {compact && <AskBar />}
    <p className="small muted" style={{ margin: '12px 0 0' }}>Boussole n’est pas une IA : elle lit vos tâches, leurs dates, leurs dépendances et vos jalons, et répond à partir du guide de POP.</p>
  </div>
}

type Msg = { who: 'me' | 'bot'; text?: string; f?: Fiche; others?: Fiche[]; today?: boolean }

/** Questions-réponses : Boussole guide l'utilisateur dans POP (base de fiches locale, pas d'IA). */
function GuideChat({ initial, onNavigate, onToday }: { initial?: string; onNavigate?: () => void; onToday: () => void }) {
  const nav = useNavigate()
  const loc = useLocation()
  const [msgs, setMsgs] = useState<Msg[]>([{ who: 'bot', text: 'Posez-moi une question sur POP, par exemple « comment relier UEFN ? » ou « comment inviter quelqu’un ? ». Je vous explique et je vous emmène au bon endroit.' }])
  const [q, setQ] = useState('')
  const end = useRef<HTMLDivElement>(null)
  const send = (text: string) => {
    const t = text.trim(); if (!t) return
    const res = ask(t, loc.pathname)
    const bot: Msg = res.length === 0
      ? { who: 'bot', text: 'Je n’ai pas trouvé de réponse à cette question. Essayez avec d’autres mots, ou choisissez un sujet ci-dessous.', others: suggested(loc.pathname) }
      : { who: 'bot', f: res[0].f, others: res.slice(1, 4).filter(x => x.score >= res[0].score * 0.6).map(x => x.f) }
    setMsgs(m => [...m, { who: 'me', text: t }, bot]); setQ('')
  }
  const pick = (f: Fiche) => setMsgs(m => [...m, { who: 'me', text: f.q }, { who: 'bot', f }])
  useEffect(() => { if (initial) send(initial) }, [initial])
  useEffect(() => { end.current?.scrollIntoView({ block: 'end', behavior: 'smooth' }) }, [msgs])
  return <div className="gchat">
    <div className="gmsgs">
      {msgs.map((m, i) => m.who === 'me' ? <div key={i} className="gmsg me">{m.text}</div> : <div key={i} className="gmsg bot">
        {m.text && <p>{m.text}</p>}
        {m.f && <>
          <p style={{ fontWeight: 500 }}>{m.f.q}</p>
          <p>{m.f.a}</p>
          {m.f.steps && <ol>{m.f.steps.map((s, j) => <li key={j}>{s}</li>)}</ol>}
          <div className="row" style={{ gap: 8, flexWrap: 'wrap', marginTop: 6 }}>
            {m.f.action === 'today' && <button className="btn pri" style={{ height: 32 }} onClick={onToday}><Compass size={14} />Voir ma journée</button>}
            {m.f.go && <button className="btn pri" style={{ height: 32 }} onClick={() => { onNavigate?.(); nav(m.f!.go![0]) }}>{m.f.go[1]}<ArrowRight size={14} /></button>}
          </div>
        </>}
        {m.others && m.others.length > 0 && <div className="gothers"><span className="small muted">{m.f ? 'Vous cherchiez peut-être :' : 'Sujets possibles :'}</span>{m.others.map(o => <button key={o.id} className="chip" onClick={() => pick(o)}>{o.q}</button>)}</div>}
      </div>)}
      <div ref={end} />
    </div>
    {msgs.length <= 1 && <div className="gothers" style={{ padding: '0 0 8px' }}>{suggested(loc.pathname).map(f => <button key={f.id} className="chip" onClick={() => pick(f)}>{f.q}</button>)}</div>}
    <form className="gask" onSubmit={e => { e.preventDefault(); send(q) }}>
      <input className="field" autoFocus value={q} onChange={e => setQ(e.target.value)} placeholder="Comment je… ?" aria-label="Votre question" />
      <button className="btn pri" aria-label="Envoyer"><Send size={16} /></button>
    </form>
  </div>
}

/** Petite barre « Demandez à Boussole » (ouvre le panneau sur l'onglet questions). */
export function AskBar() {
  const [q, setQ] = useState('')
  return <form className="gask" style={{ marginTop: 12 }} onSubmit={e => { e.preventDefault(); window.dispatchEvent(new CustomEvent('boussole', { detail: q.trim() || '' })); setQ('') }}>
    <input className="field" value={q} onChange={e => setQ(e.target.value)} placeholder="Une question sur POP ? « Comment relier UEFN ? »" aria-label="Demander à Boussole" />
    <button className="btn" aria-label="Demander"><Send size={16} /></button>
  </form>
}

export function GuideDrawer({ onClose, ask: initial }: { onClose: () => void; ask?: string }) {
  const [tab, setTab] = useState<'day' | 'ask'>(initial !== undefined ? 'ask' : 'day')
  return <>
    <div className="scrim" onClick={onClose} />
    <aside className="drawer gdrawer" role="dialog" aria-label="Boussole">
      <div className="row" style={{ justifyContent: 'space-between', marginBottom: 12 }}>
        <div className="seg" role="tablist">
          <button role="tab" aria-selected={tab === 'day'} className={tab === 'day' ? 'on' : ''} onClick={() => setTab('day')}>Mon jour</button>
          <button role="tab" aria-selected={tab === 'ask'} className={tab === 'ask' ? 'on' : ''} onClick={() => setTab('ask')}>Poser une question</button>
        </div>
        <button className="iconbtn" aria-label="Fermer" onClick={onClose}><X size={18} /></button>
      </div>
      {tab === 'day' ? <GuidePanel onNavigate={onClose} /> : <GuideChat initial={initial || undefined} onNavigate={onClose} onToday={() => setTab('day')} />}
    </aside>
  </>
}
