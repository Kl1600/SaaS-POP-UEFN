// Boussole : assistant intégré (calculs côté serveur sur les données de la map + guide de POP, rien ne sort de POP).
import { useEffect, useRef, useState } from 'react'
import { useLocation } from 'react-router-dom'
import { Compass, AlertCircle, AlertTriangle, CheckCircle2, Info, Play, ArrowRight, X, Send } from 'lucide-react'
import { ask, suggested, type Fiche } from '../guideKB'
import { api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { tr, LANG } from '../i18n/t'
import { useNav } from '../routes'
import { useDialog } from './ui'

const TONE: Record<string, [any, string]> = {
  bad: [AlertCircle, 'var(--tr)'], warn: [AlertTriangle, 'var(--to)'], good: [CheckCircle2, 'var(--tg)'], info: [Info, 'var(--text3)'],
}

/** /map/12/taches → /taches ; /studio/3/equipe → /equipe (pages des fiches d’aide). */
const shortPath = (p: string) => p.replace(/^\/map\/\d+/, '').replace(/^\/studio\/\d+\/(integrations|maps)/, '/equipe').replace(/^\/studio\/\d+\/marche/, '/analyse/marche').replace(/^\/analyse\/videos/, '/analyse/tiktok').replace(/^\/studio\/\d+/, '') || '/'

export function GuidePanel({ compact, onNavigate }: { compact?: boolean; onNavigate?: () => void }) {
  const { map, me, toast } = useApp()
  const nav = useNav()
  const { data, reload } = useLoad<any>(map ? `/api/maps/${map.id}/guide?lang=${LANG}` : null, [map?.id])
  useLive(['tasks', 'plan'], reload)
  if (!data) return null
  const today = new Date().toISOString().slice(0, 10)
  const take = async (s: any) => {
    try {
      if (!s.mine) await api.patch(`/api/tasks/${s.id}`, { assignee_id: me!.user.id, ...(s.planned_day ? {} : { planned_day: today }) })
      await api.post(`/api/tasks/${s.id}/status`, { status: 'doing' })
      toast(tr("#{0} est à vous, c’est parti", [s.number])); reload()
    } catch (e: any) { toast(e.message, true) }
  }
  const open = (id: number) => { onNavigate?.(); nav(`/taches?t=${id}`) }
  return <div className="guide">
    <div className="guide-head">
      <span className="guide-ico"><Compass size={20} strokeWidth={1.75} /></span>
      <div style={{ minWidth: 0 }}>
        <div className="guide-name">{tr("Boussole")}{' '}<span className="small muted">{tr("· assistant intégré")}</span></div>
        <div className="guide-hl">{data.headline}</div>
      </div>
    </div>
    {data.points.length > 0 && <ul className="guide-pts">{data.points.slice(0, compact ? 4 : 8).map((p: any, i: number) => { const [I, c] = TONE[p.tone] ?? TONE.info; return <li key={i}><I size={16} style={{ color: c, flex: 'none', marginTop: 2 }} /><span>{p.text}</span></li> })}</ul>}
    {data.pace && <div className="guide-pace" title={tr("{0} tâche(s) restante(s), {1} finie(s) par jour en moyenne", [data.pace.remaining, data.pace.per_day])}>
      <div className="row small" style={{ justifyContent: 'space-between' }}><span>{tr("Prochain jalon :")}{' '}<b style={{ fontWeight: 500 }}>{data.pace.title}</b></span><span className={'badge ' + (data.pace.on_track ? 'b-g' : 'b-r')}>{data.pace.on_track ? tr("Dans les temps") : tr("En retard")}</span></div>
    </div>}
    <div className="dsec" style={{ marginTop: 14 }}>{tr("À faire maintenant")}</div>
    {data.suggestions.length === 0 ? <p className="small muted" style={{ margin: 0 }}>{tr("Aucune tâche disponible pour vous. Créez-en une ou demandez à l’équipe.")}</p> :
      <div className="guide-sugs">{data.suggestions.map((s: any, i: number) => <div key={s.id} className={'guide-sug' + (i === 0 ? ' first' : '')}>
        <button className="guide-sug-main" onClick={() => open(s.id)}>
          <span className="guide-sug-t"><span className="mono muted">#{s.number}</span> {s.title}</span>
          <span className="small muted">{s.why.join(' · ')}</span>
        </button>
        {s.action === 'continue'
          ? <button className="btn" onClick={() => open(s.id)}><ArrowRight size={15} />{tr("Continuer")}</button>
          : <button className={'btn' + (i === 0 ? ' pri' : '')} onClick={() => take(s)}><Play size={14} />{s.mine ? tr("Commencer") : tr("Prendre")}</button>}
      </div>)}</div>}
    {compact && <AskBar />}
    <p className="small muted" style={{ margin: '12px 0 0' }}>{tr("Boussole lit vos tâches, vos jalons et votre équipe, et répond aussi aux questions sur POP. Tout reste dans POP.")}</p>
  </div>
}

type Msg = { who: 'me' | 'bot'; text?: string; f?: Fiche; others?: Fiche[]; today?: boolean; items?: { label: string; link: string; kind?: string }[]; typing?: boolean }

/** Questions-réponses : Boussole guide l'utilisateur dans POP (base de fiches locale, pas d'IA). */
function GuideChat({ initial, onNavigate, onToday }: { initial?: string; onNavigate?: () => void; onToday: () => void }) {
  const nav = useNav()
  const loc = useLocation()
  const [msgs, setMsgs] = useState<Msg[]>([{ who: 'bot', text: tr("Posez-moi une question : sur votre map (« qu’est-ce qui est en retard ? », « sur quoi travaille Hugo ? », « où en est la tâche #12 ? ») ou sur POP (« comment relier UEFN ? »).") }])
  const [q, setQ] = useState('')
  const end = useRef<HTMLDivElement>(null)
  const { map } = useApp()
  const send = async (text: string) => {
    const t = text.trim(); if (!t) return
    setQ(''); setMsgs(m => [...m, { who: 'me', text: t }, { who: 'bot', typing: true }])
    const res = ask(t, shortPath(loc.pathname))
    let bot: Msg
    // 1. Question sur les données de la map (retards, une personne, une tâche, le jalon, les chiffres…).
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
        {m.typing && <span className="gtyping"><i /><i /><i /></span>}
        {m.text && <p>{m.text}</p>}
        {m.items && m.items.length > 0 && <div className="gitems">{m.items.map((it, j) => <button key={j} onClick={() => { onNavigate?.(); nav(it.link) }}>{it.kind && <span className="badge" style={{ marginRight: 6 }}>{it.kind}</span>}{it.label}<ArrowRight size={13} /></button>)}</div>}
        {m.f && <>
          <p style={{ fontWeight: 500 }}>{m.f.q}</p>
          <p>{m.f.a}</p>
          {m.f.steps && <ol>{m.f.steps.map((s, j) => <li key={j}>{s}</li>)}</ol>}
          <div className="row" style={{ gap: 8, flexWrap: 'wrap', marginTop: 6 }}>
            {m.f.action === 'today' && <button className="btn pri" style={{ height: 32 }} onClick={onToday}><Compass size={14} />{tr("Voir ma journée")}</button>}
            {m.f.go && <button className="btn pri" style={{ height: 32 }} onClick={() => { onNavigate?.(); nav(m.f!.go![0]) }}>{m.f.go[1]}<ArrowRight size={14} /></button>}
          </div>
        </>}
        {m.others && m.others.length > 0 && <div className="gothers"><span className="small muted">{m.f ? tr("Vous cherchiez peut-être :") : tr("Sujets possibles :")}</span>{m.others.map(o => <button key={o.id} className="chip" onClick={() => pick(o)}>{o.q}</button>)}</div>}
      </div>)}
      <div ref={end} />
    </div>
    {msgs.length <= 1 && <div className="gothers" style={{ padding: '0 0 8px' }}>{suggested(shortPath(loc.pathname)).map(f => <button key={f.id} className="chip" onClick={() => pick(f)}>{f.q}</button>)}</div>}
    <form className="gask" onSubmit={e => { e.preventDefault(); send(q) }}>
      <input className="field" autoFocus value={q} onChange={e => setQ(e.target.value)} placeholder={tr("Comment je… ?")} aria-label={tr("Votre question")} />
      <button className="btn pri" aria-label={tr("Envoyer")}><Send size={16} /></button>
    </form>
  </div>
}

/** Petite barre « Demandez à Boussole » (ouvre le panneau sur l'onglet questions). */
export function AskBar() {
  const [q, setQ] = useState('')
  return <form className="gask" style={{ marginTop: 12 }} onSubmit={e => { e.preventDefault(); window.dispatchEvent(new CustomEvent('boussole', { detail: q.trim() || '' })); setQ('') }}>
    <input className="field" value={q} onChange={e => setQ(e.target.value)} placeholder={tr("Une question sur POP ? « Comment relier UEFN ? »")} aria-label={tr("Demander à Boussole")} />
    <button className="btn" aria-label={tr("Demander")}><Send size={16} /></button>
  </form>
}

export function GuideDrawer({ onClose, ask: initial }: { onClose: () => void; ask?: string }) {
  const dlg = useDialog(onClose)
  const [tab, setTab] = useState<'day' | 'ask'>(initial !== undefined ? 'ask' : 'day')
  return <>
    <div className="scrim" onClick={onClose} />
    <aside ref={dlg as any} tabIndex={-1} aria-modal="true" className="drawer gdrawer" role="dialog" aria-label={tr("Boussole")}>
      <div className="row" style={{ justifyContent: 'space-between', marginBottom: 12 }}>
        <div className="seg" role="tablist">
          <button role="tab" aria-selected={tab === 'day'} className={tab === 'day' ? 'on' : ''} onClick={() => setTab('day')}>{tr("Mon jour")}</button>
          <button role="tab" aria-selected={tab === 'ask'} className={tab === 'ask' ? 'on' : ''} onClick={() => setTab('ask')}>{tr("Poser une question")}</button>
        </div>
        <button className="iconbtn" aria-label={tr("Fermer")} onClick={onClose}><X size={18} /></button>
      </div>
      {tab === 'day' ? <GuidePanel onNavigate={onClose} /> : <GuideChat initial={initial || undefined} onNavigate={onClose} onToday={() => setTab('day')} />}
    </aside>
  </>
}
