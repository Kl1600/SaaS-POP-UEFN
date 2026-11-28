// Site vitrine de POP (visiteurs non connectés). Sombre, accent orange, maquettes de l'app en HTML.
import { LEGAL_PAGES } from './Legal'
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  ArrowRight, Check, ChevronDown, Compass, ListChecks, Boxes, PenTool, LineChart, Coins, Bot, Minus, Sparkles, AlertTriangle, AlertCircle,
  FileText, Users, Link2, Hammer, Flame, Rocket, MessagesSquare, Monitor,
} from '../icons'
import '../landing.css'
import { WaitlistModal } from '../components/Waitlist'
import LangPicker from '../components/LangPicker'
import { useI18n, resetHtmlLang, type Dict } from '../i18n'

// Tarifs : à ajuster ici. Accès sur liste d'attente pendant la bêta privée (aucun paiement n'est branché).
// Les textes viennent du dictionnaire de langue (src/i18n) ; ici seulement les prix et les cases du tableau.
// Deux abonnements ; à l'ouverture, 7 jours d'essai gratuit de Pro pour chaque nouveau compte.
const PLANS = [
  { id: 'starter', price: '12,99 €', per: true },
  { id: 'pro', price: '24,99 €', per: true, best: true },
]
// U = illimité, S = bientôt ; [] = titre de groupe. Même ordre que pricing.rows.
const ROWS: (string | boolean)[][] = [
  [], [true, true], [true, true], [true, true], ['U', 'U'], ['U', 'U'],
  [], [true, true], [true, true], ['U', 'U'], [false, 'S'],
  [], [false, true], [false, true], [true, true], [true, true], [true, true],
]

// Ligne « Organiseur d'acteurs » du comparatif, retirée avec l'Organiseur (#371).
const ORGANISEUR_ROW = 8

function useReveal() {
  useEffect(() => {
    const els = document.querySelectorAll('.lp [data-reveal]')
    if (!('IntersectionObserver' in window)) { els.forEach(e => e.classList.add('in')); return }
    const io = new IntersectionObserver(es => es.forEach(e => { if (e.isIntersecting) { e.target.classList.add('in'); io.unobserve(e.target) } }), { threshold: 0.12, root: document.querySelector('.lp') })
    els.forEach(e => io.observe(e))
    return () => io.disconnect()
  }, [])
}

// ---------- Maquettes (vrais écrans de POP, simplifiés) ----------
type M = Dict['mock']
function AppShot({ m }: { m: M }) {
  return <div className="mock appshot" aria-hidden>
    <div className="mbar"><i /><i /><i /><span>pop · {m.overview}</span></div>
    <div className="mbody">
      <aside className="mside">
        <img src="/logo.png" alt="" />
        {m.nav.map((x, i) => <div key={x} className={'mnav' + (i === 0 ? ' on' : '')}>{x}</div>)}
      </aside>
      <div className="mmain">
        <div className="mcard guide">
          <div className="mrow"><span className="gico"><Compass size={14} /></span><b>{m.hello}</b></div>
          <p><AlertCircle size={13} className="r" />{m.late}</p>
          <p><AlertTriangle size={13} className="o" />{m.mile}</p>
          <div className="msug first"><span><b>#42</b> {m.tasks[0]}</span><em>{m.unlocks}</em><span className="mbtn">{m.take}</span></div>
          <div className="msug"><span><b>#38</b> {m.tasks[1]}</span><em>{m.today}</em><span className="mbtn ghost">{m.start}</span></div>
        </div>
        <div className="mkan">
          {([[m.todo, ['#44 ' + m.tasks[2], '#45 ' + m.tasks[3]]], [m.doing, ['#42 ' + m.tasks[0]]], [m.done, ['#40 ' + m.tasks[4], '#41 ' + m.tasks[5]]]] as [string, string[]][]).map(([t, cs]) => <div key={t} className="mcol"><span>{t}</span>{cs.map(c => <div key={c} className="mtask">{c}<i>1 h</i></div>)}</div>)}
        </div>
      </div>
    </div>
  </div>
}
function TasksShot({ m }: { m: M }) {
  const cols: [string, string[][]][] = [[m.todo, [['#44', m.tasks[2], m.waits], ['#45', m.tasks[3], '']]], [m.doing, [['#42', m.tasks[0], 'Léa']]], [m.done, [['#40', m.tasks[4], 'Hugo'], ['#41', m.tasks[5], 'Léa']]]]
  return <div className="mock" aria-hidden><div className="mpad">
    <div className="mkan three">
      {cols.map(([t, cs]) => <div key={t} className="mcol"><span>{t}</span>{cs.map(c => <div key={c[0]} className="mtask big"><small>{c[0]} · 1 h</small>{c[1]}{c[2] && <em>{c[2]}</em>}</div>)}</div>)}
    </div>
  </div></div>
}
function BoardShot({ m }: { m: M }) {
  return <div className="mock wbshot" aria-hidden>
    <div className="bfmt"><b>B</b><i>I</i><u>U</u><s>S</s><span className="bsw" style={{ background: '#ED753E' }} /><span className="bsw" style={{ background: '#0090FF' }} /></div>
    <div className="bst" style={{ left: '8%', top: '30%', background: '#FFF3A3' }}>{m.s1}</div>
    <div className="bst" style={{ left: '38%', top: '22%', background: '#FFD8D6' }}><b>{m.s2a}</b> {m.s2b}</div>
    <div className="bst" style={{ left: '66%', top: '40%', background: '#D3F2D4' }}>{m.s3}</div>
    <div className="bshape">Zone 2</div>
  </div>
}
function MarketShot({ m }: { m: M }) {
  const G = [['Tycoon', m.verdicts[0], 'g', '1 650', '+12 %'], ['Horror', m.verdicts[1], 'o', '1 480', '+4 %'], ['Box fight', m.verdicts[2], 'r', '220', '−8 %'], ['Parkour', m.verdicts[3], 'b', '640', '+21 %']]
  return <div className="mock" aria-hidden><div className="mpad mgen">
    {G.map(g => <div key={g[0]} className="mgcard"><div className="mrow"><b>{g[0]}</b><em className={g[4].startsWith('−') ? 'down' : 'up'} dir="ltr">{g[4]}</em></div><span className={'mbadge ' + g[2]}>{g[1]}</span><div className="mbig"><b>{g[3]}</b> {m.typical}</div><div className="spk">{[3, 4, 4, 5, 6, 7, 8].map((h, i) => <i key={i} style={{ height: h * 4 }} />)}</div></div>)}
  </div></div>
}
function MoneyShot({ m }: { m: M }) {
  return <div className="mock" aria-hidden><div className="mpad">
    <div className="mrow" style={{ justifyContent: 'space-between' }}><b>{m.earnings}</b><b>4 820 €</b></div>
    {[['Léa', m.roles[0], '1 928 €'], ['Hugo', m.roles[1], '1 687 €'], ['Sam', m.roles[2], '300 €'], ['Studio', m.roles[3], '905 €']].map(r => <div key={r[0]} className="mpay"><span className="mav">{r[0][0]}</span><span>{r[0]}<small>{r[1]}</small></span><b>{r[2]}</b></div>)}
    <p className="mnote">{m.noTransfer}</p>
  </div></div>
}

// L'Organiseur est retiré de l'app (décision du 22/11) : k = index du texte dans t.feat.items (#371).
const FEATURES = [
  { n: '01', k: 0, id: 'f-taches', icon: ListChecks, Shot: TasksShot },
  { n: '02', k: 2, id: 'f-tableau', icon: PenTool, Shot: BoardShot },
  { n: '03', k: 3, id: 'f-analyse', icon: LineChart, Shot: MarketShot },
  { n: '04', k: 4, id: 'f-gains', icon: Coins, Shot: MoneyShot },
]

// Menu « Fonctionnalités » : icône et section visée pour chaque entrée (textes dans t.menu.items, même ordre).
const MENU_ITEMS: { icon: typeof ListChecks; to: string; soon?: boolean }[] = [
  { icon: ListChecks, to: 'f-taches' }, { icon: FileText, to: 'f-taches' }, { icon: PenTool, to: 'f-tableau' }, { icon: Users, to: 'f-taches' },
  { icon: Boxes, to: 'ia' }, { icon: Link2, to: 'ia' }, { icon: Bot, to: 'ia' }, { icon: Hammer, to: 'tarifs', soon: true },
  { icon: LineChart, to: 'f-analyse' }, { icon: Flame, to: 'f-analyse' }, { icon: Rocket, to: 'fonctionnalites' }, { icon: Coins, to: 'f-gains' },
]

function FeaturesMenu({ t, go }: { t: Dict; go: (id: string) => (e: React.MouseEvent) => void }) {
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  const timer = useRef<number>()
  const show = () => { window.clearTimeout(timer.current); setOpen(true) }
  const hide = () => { window.clearTimeout(timer.current); timer.current = window.setTimeout(() => setOpen(false), 160) }
  useEffect(() => {
    if (!open) return
    const down = (e: MouseEvent) => { if (!box.current?.contains(e.target as Node)) setOpen(false) }
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false) }
    document.addEventListener('mousedown', down); document.addEventListener('keydown', key)
    return () => { document.removeEventListener('mousedown', down); document.removeEventListener('keydown', key) }
  }, [open])
  const pick = (id: string) => (e: React.MouseEvent) => { setOpen(false); go(id)(e) }
  return <div className="lmenu" ref={box} onMouseEnter={show} onMouseLeave={hide}>
    <button className={'lmenu-btn' + (open ? ' on' : '')} onClick={() => setOpen(o => !o)} aria-expanded={open} aria-haspopup="true">{t.nav.features}<ChevronDown size={15} /></button>
    {open && <div className="lmega" onMouseEnter={show}>
      <div className="lmega-cols">
        {t.menu.groups.map((g, gi) => <div key={g} className="lmega-col">
          <span className="lmega-h">{g}</span>
          {t.menu.items.map((it, k) => ({ it, k })).filter(({ k }) => k !== 4 && Math.floor(k / 4) === gi).map(({ it, k }) => { const m = MENU_ITEMS[k]; return <a key={it.t} href={'#' + m.to} onClick={pick(m.to)} className="lmega-item">
            <span className="lmega-ic"><m.icon size={19} strokeWidth={1.6} /></span>
            <span><b>{it.t}{m.soon && <em className="ltag soon">{t.pricing.soon}</em>}</b><small>{it.d}</small></span>
          </a> })}
        </div>)}
      </div>
      <div className="lmega-foot">
        <span><MessagesSquare size={17} />{t.menu.bottom[0]}</span>
        <span><Monitor size={17} />{t.menu.bottom[1]}</span>
        <a href="#fonctionnalites" onClick={pick('fonctionnalites')}>{t.menu.all} <ArrowRight size={15} /></a>
      </div>
    </div>}
  </div>
}

export default function Landing() {
  const nav = useNavigate()
  const { t, lang } = useI18n()
  const root = useRef<HTMLDivElement>(null)
  const [faq, setFaq] = useState<number | null>(0)
  const [scrolled, setScrolled] = useState(false)
  useReveal()
  useEffect(() => { document.title = t.meta.title }, [t])
  useEffect(() => resetHtmlLang, [])
  const go = (id: string) => (e: React.MouseEvent) => { e.preventDefault(); document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' }) }
  const [wait, setWait] = useState(false)
  const signup = () => setWait(true)
  const P = t.pricing
  const cell = (v: string | boolean) => v === true ? <Check size={16} className="ok" /> : v === false ? <Minus size={16} className="no" />
    : v === 'S' ? <span className="ltag soon">{P.soon}</span> : <span>{v === 'U' ? P.unlimited : v}</span>
  return <div className="lp" ref={root} dir={lang.rtl ? 'rtl' : 'ltr'} lang={lang.html} onScroll={e => setScrolled((e.target as HTMLDivElement).scrollTop > 10)}>
    <nav className={'lnav' + (scrolled ? ' solid' : '')}>
      <a href="#top" onClick={go('top')} className="lbrand" aria-label={t.nav.home}><img src="/logo.png" alt="POP" /></a>
      <div className="llinks">
        <FeaturesMenu t={t} go={go} />
        <a href="#tarifs" onClick={go('tarifs')}>{t.nav.pricing}</a>
        <a href="#faq" onClick={go('faq')}>{t.nav.faq}</a>
      </div>
      <div className="lacts">
        <LangPicker />
        <button className="lghost" onClick={() => nav('/connexion')}>{t.nav.login}</button>
        <button className="lpill" onClick={signup}>{t.nav.join}</button>
      </div>
    </nav>

    <header className="lhero" id="top">
      <div className="lglow" />
      <a className="lannounce" href="#beta" onClick={e => { e.preventDefault(); signup() }}><Sparkles size={14} /> {t.hero.announce} <ArrowRight size={14} /></a>
      <h1 data-reveal>{t.hero.h1a}<br /><span className="grad">{t.hero.h1b}</span></h1>
      <p className="lsub" data-reveal>{t.hero.subA}<b>{t.hero.subB}</b>{t.hero.subC}</p>
      <div className="lctas" data-reveal>
        <button className="lbtn pri" onClick={signup}>{t.hero.cta} <ArrowRight size={18} /></button>
        <button className="lbtn" onClick={go('fonctionnalites')}>{t.hero.see}</button>
      </div>
      <p className="lmicro" data-reveal><span className="ldot" /> <b>{t.hero.micro1}</b> · {t.hero.micro2} · {t.hero.micro3}</p>
      <div className="lshot" data-reveal><AppShot m={t.mock} /></div>
    </header>

    <section className="lmarquee" aria-label={t.marquee.label}>
      <p>{t.marquee.label}</p>
      <div className="mq" dir="ltr"><div className="mqt">{[0, 1].map(k => <span key={k}>{['UEFN', 'Verse', 'Discord', 'GitHub', 'Lore', 'Claude', 'Cursor', 'Codex', 'TikTok', 'YouTube Shorts', t.marquee.epic].map(x => <em key={x}>{x}</em>)}</span>)}</div></div>
    </section>

    <section className="lsec lpain">
      <div className="lhead" data-reveal><span className="leyebrow">{t.pain.eyebrow}</span><h2>{t.pain.h2a}<br />{t.pain.h2b}</h2></div>
      <div className="lvs" data-reveal>
        <div className="lcol bad"><span className="lcolh">{t.pain.without}</span>
          <ul>{t.pain.bad.map(x => <li key={x}><Minus size={14} />{x}</li>)}</ul></div>
        <div className="lcol good"><span className="lcolh">{t.pain.with}</span>
          <ul>{t.pain.good.map(x => <li key={x}><Check size={14} />{x}</li>)}</ul></div>
      </div>
    </section>

    <section className="lsec" id="fonctionnalites">
      <div className="lhead" data-reveal><span className="leyebrow">{t.feat.eyebrow}</span><h2>{t.feat.h2}</h2><p>{t.feat.p}</p></div>
      <div className="lfeats">{FEATURES.map((f, i) => { const it = t.feat.items[f.k]; return <article key={f.n} id={f.id} className={'lfeat' + (i % 2 ? ' rev' : '')} data-reveal>
        <div className="ltxt">
          <span className="lnum"><b>{f.n}</b><f.icon size={15} />{it.tag}</span>
          <h3>{it.h}</h3><p>{it.p}</p>
          <ul>{it.pts.map(p => <li key={p}><Check size={15} />{p}</li>)}</ul>
        </div>
        <figure><f.Shot m={t.mock} /></figure>
      </article> })}</div>
      <div className="lai" id="ia" data-reveal>
        <Bot size={22} />
        <div><b>{t.feat.aiB}</b> {t.feat.aiText}</div>
      </div>
    </section>

    <section className="lstats" data-reveal>
      {t.stats.map(s => <div key={s[0]}><b>{s[0]}</b><span>{s[1]}</span></div>)}
    </section>

    <section className="lsec" id="tarifs">
      <div className="lhead" data-reveal><span className="leyebrow">{P.eyebrow}</span><h2>{P.h2}</h2><p>{P.p}</p></div>
      <div className="lplans" data-reveal>{PLANS.map((p, i) => { const tx = P.plans[i]; return <div key={p.id} className={'lplan' + (p.best ? ' best' : '')}>
        <div className="mrow" style={{ justifyContent: 'space-between' }}><h3>{tx.name}</h3><span className={'ltag' + ((p as any).soon ? " soon" : "")}>{tx.tag}</span></div>
        <div className="lprice"><b>{p.price}</b><span>{p.per ? P.perMonth : ''}</span></div>
        <div className="lperday">{P.perDay[i]}</div>
        <p>{tx.desc}</p>
        <button className={'lbtn' + (p.best ? ' pri' : '')} onClick={signup}>{tx.cta} <ArrowRight size={16} /></button>
      </div> })}</div>
      <div className="ltable" data-reveal><table>
        <thead><tr><th>{P.compare}</th>{P.plans.map(p => <th key={p.name}>{p.name}</th>)}</tr></thead>
        <tbody>{ROWS.map((vals, r) => r === ORGANISEUR_ROW ? null : vals.length === 0 ? <tr key={r} className="grp"><td colSpan={3}>{P.rows[r]}</td></tr> :
          <tr key={r}><td>{P.rows[r]}{P.hints[r] && <small className="lhint">{P.hints[r]}</small>}</td>{vals.map((v, i) => <td key={i}>{cell(v)}</td>)}</tr>)}</tbody>
      </table></div>
    </section>

    <section className="lsec lfaq" id="faq">
      <div className="lhead" data-reveal><span className="leyebrow">{t.faq.eyebrow}</span><h2>{t.faq.h2}</h2></div>
      <div className="lqs" data-reveal>{t.faq.items.map(([q, a], i) => <div key={i} className={'lq' + (faq === i ? ' open' : '')}>
        <button onClick={() => setFaq(faq === i ? null : i)} aria-expanded={faq === i}>{q}<ChevronDown size={18} /></button>
        <div className="la"><p>{a}</p></div>
      </div>)}</div>
    </section>

    <section className="lfinal" data-reveal>
      <div className="lglow" />
      <h2>{t.final.h2a}<br /><span className="grad">{t.final.h2b}</span></h2>
      <p>{t.final.p}</p>
      <div className="lctas"><button className="lbtn pri" onClick={signup}>{t.hero.cta} <ArrowRight size={18} /></button><button className="lbtn" onClick={() => nav('/connexion')}>{t.nav.login}</button></div>
    </section>

    {wait && <WaitlistModal onClose={() => setWait(false)} />}
    <footer className="lfoot">
      <div className="lfb"><img src="/logo.png" alt="POP" /><span>© {new Date().getFullYear()} POP · {t.foot.made}</span><small>{t.foot.legal}</small></div>
      <div className="lfcols">
        <div><h4>{t.foot.product}</h4><a href="#fonctionnalites" onClick={go('fonctionnalites')}>{t.nav.features}</a><a href="#tarifs" onClick={go('tarifs')}>{t.nav.pricing}</a></div>
        <div><h4>{t.foot.help}</h4><a href="#faq" onClick={go('faq')}>{t.nav.faq}</a></div>
        <div><h4>{t.foot.legalTitle}</h4>{LEGAL_PAGES.map(([p, l]) => <a key={p} href={p} onClick={e => { e.preventDefault(); nav(p) }}>{l}</a>)}</div>
        <div><h4>{t.foot.account}</h4><a href="/connexion" onClick={e => { e.preventDefault(); nav('/connexion') }}>{t.nav.login}</a><a href="#beta" onClick={e => { e.preventDefault(); signup() }}>{t.nav.join}</a></div>
      </div>
    </footer>
  </div>
}
