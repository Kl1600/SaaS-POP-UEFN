// Site vitrine de POP (visiteurs non connectés). Sombre, accent orange, maquettes de l'app en HTML.
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  ArrowRight, Check, ChevronDown, Compass, ListChecks, Code2, Boxes, PenTool, LineChart, Coins, Bot, Minus, Sparkles, Send, AlertTriangle, AlertCircle, CheckCircle2,
} from 'lucide-react'
import '../landing.css'
import { WaitlistModal } from '../components/Waitlist'
import LangPicker from '../components/LangPicker'
import { useI18n, resetHtmlLang, type Dict } from '../i18n'

// Tarifs : à ajuster ici. Accès sur liste d'attente pendant la bêta privée (aucun paiement n'est branché).
// Les textes viennent du dictionnaire de langue (src/i18n) ; ici seulement les prix et les cases du tableau.
const PLANS = [
  { id: 'createur', price: '0 €', per: false },
  { id: 'studio', price: '19 €', per: true, best: true },
  { id: 'forge', price: '49 €', per: true, soon: true },
]
// U = illimité, S = bientôt ; [] = titre de groupe. Même ordre que pricing.rows.
const ROWS: (string | boolean)[][] = [
  [], [true, true, true], [true, true, true], [true, true, true], ['1', 'U', 'U'], ['3', 'U', 'U'],
  [], [true, true, true], [true, true, true], [false, true, true], ['1', 'U', 'U'], [false, false, 'S'],
  [], [true, true, true], [false, true, true], [true, true, true], [false, true, true], [false, true, true],
]

/** Texte avec **gras**. */
function Rich({ s }: { s: string }) {
  return <>{s.split(/(\*\*[^*]+\*\*)/g).map((x, i) => x.startsWith('**') && x.endsWith('**') ? <b key={i}>{x.slice(2, -2)}</b> : x)}</>
}
/** Réponse de démo : [r] [o] [g] = icône, « 1. » = étape numérotée. */
function Answer({ lines }: { lines: string[] }) {
  const out: React.ReactNode[] = []
  let steps: string[] = []
  const flush = () => { if (steps.length) { out.push(<ol key={'o' + out.length}>{steps.map((x, i) => <li key={i}><Rich s={x} /></li>)}</ol>); steps = [] } }
  lines.forEach(l => {
    const m = l.match(/^\d+\.\s+(.*)$/)
    if (m) { steps.push(m[1]); return }
    flush()
    const ic = l.match(/^\[(r|o|g)\]\s*(.*)$/)
    const Icon = ic ? (ic[1] === 'r' ? AlertCircle : ic[1] === 'o' ? AlertTriangle : CheckCircle2) : null
    out.push(<p key={out.length}>{Icon && <Icon size={13} className={ic![1]} />}<span><Rich s={ic ? ic[2] : l} /></span></p>)
  })
  flush()
  return <>{out}</>
}

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
function VerseShot({ m }: { m: M }) {
  const L = ['using { /Fortnite.com/Devices }', 'using { /Verse.org/Simulation }', '', 'score_manager := class(creative_device):', '    @editable Bouton : button_device = button_device{}', '    var Points : int = 0', '', '    OnBegin<override>()<suspends> : void =', '        Bouton.InteractedWithEvent.Subscribe(OnClic)', '', '    OnClic(Agent : agent) : void =', '        set Points += 1.5']
  return <div className="mock code" aria-hidden dir="ltr">
    <div className="mbar"><i /><i /><i /><span>score_manager.verse · {m.sent}</span></div>
    <pre>{L.map((l, i) => <div key={i} className={i === 11 ? 'lerr' : ''}><span className="ln">{i + 1}</span>{l || ' '}</div>)}</pre>
    <div className="cerr"><AlertCircle size={14} /> {m.err} <b>{m.fix}</b></div>
  </div>
}
function OrganizerShot({ m }: { m: M }) {
  const rows = [['Spawn_Rouge_01', 'Player Spawner', m.red], ['Spawn_Rouge_02', 'Player Spawner', m.red], ['Bouton_Score', 'Button', 'Gameplay'], ['Minuteur_Manche', 'Timer', 'Gameplay'], ['Barriere_Zone2', 'Barrier', 'Zone 2']]
  return <div className="mock" aria-hidden><div className="mpad">
    <div className="msearch" dir="ltr">tag:equipe_rouge type:spawner</div>
    {rows.map((r, i) => <div key={r[0]} className={'mact' + (i < 2 ? ' sel' : '')}><span className="mono">{r[0]}</span><em>{r[1]}</em><small>{r[2]}</small></div>)}
    <div className="mbulk">{m.selected} · <span>{m.rename}</span><span>{m.move}</span><span>{m.dup}</span></div>
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

const FEATURES = [
  { n: '01', icon: ListChecks, Shot: TasksShot },
  { n: '02', icon: Code2, Shot: VerseShot },
  { n: '03', icon: Boxes, Shot: OrganizerShot },
  { n: '04', icon: PenTool, Shot: BoardShot },
  { n: '05', icon: LineChart, Shot: MarketShot },
  { n: '06', icon: Coins, Shot: MoneyShot },
]

export default function Landing() {
  const nav = useNavigate()
  const { t, lang } = useI18n()
  const root = useRef<HTMLDivElement>(null)
  const [faq, setFaq] = useState<number | null>(0)
  const [demo, setDemo] = useState(0)
  const [scrolled, setScrolled] = useState(false)
  useReveal()
  useEffect(() => { document.title = t.meta.title }, [t])
  useEffect(() => resetHtmlLang, [])
  useEffect(() => { const i = setInterval(() => setDemo(d => (d + 1) % t.bou.demo.length), 6000); return () => clearInterval(i) }, [demo, t])
  const go = (id: string) => (e: React.MouseEvent) => { e.preventDefault(); document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' }) }
  const [wait, setWait] = useState(false)
  const signup = () => setWait(true)
  const P = t.pricing
  const cell = (v: string | boolean) => v === true ? <Check size={16} className="ok" /> : v === false ? <Minus size={16} className="no" />
    : v === 'S' ? <span className="ltag soon">{P.soon}</span> : <span>{v === 'U' ? P.unlimited : v}</span>
  const D = t.bou.demo[demo] || t.bou.demo[0]
  return <div className="lp" ref={root} dir={lang.rtl ? 'rtl' : 'ltr'} lang={lang.html} onScroll={e => setScrolled((e.target as HTMLDivElement).scrollTop > 10)}>
    <nav className={'lnav' + (scrolled ? ' solid' : '')}>
      <a href="#top" onClick={go('top')} className="lbrand" aria-label={t.nav.home}><img src="/logo.png" alt="POP" /></a>
      <div className="llinks">
        <a href="#fonctionnalites" onClick={go('fonctionnalites')}>{t.nav.features}</a>
        <a href="#boussole" onClick={go('boussole')}>{t.nav.boussole}</a>
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
      <a className="lannounce" href="#boussole" onClick={go('boussole')}><Sparkles size={14} /> {t.hero.announce} <ArrowRight size={14} /></a>
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
      <div className="lfeats">{FEATURES.map((f, i) => { const it = t.feat.items[i]; return <article key={f.n} className={'lfeat' + (i % 2 ? ' rev' : '')} data-reveal>
        <div className="ltxt">
          <span className="lnum"><b>{f.n}</b><f.icon size={15} />{it.tag}</span>
          <h3>{it.h}</h3><p>{it.p}</p>
          <ul>{it.pts.map(p => <li key={p}><Check size={15} />{p}</li>)}</ul>
        </div>
        <figure><f.Shot m={t.mock} /></figure>
      </article> })}</div>
      <div className="lai" data-reveal>
        <Bot size={22} />
        <div><b>{t.feat.aiB}</b> {t.feat.aiText}</div>
      </div>
    </section>

    <section className="lsec lboussole" id="boussole">
      <div className="lsplit">
        <div className="ltxt" data-reveal>
          <span className="leyebrow">{t.bou.eyebrow}</span>
          <h2>{t.bou.h2a}<br />{t.bou.h2b}</h2>
          <p>{t.bou.p}</p>
          <div className="lchips">{t.bou.demo.map((d, i) => <button key={d.q} className={i === demo ? 'on' : ''} onClick={() => setDemo(i)}>{d.q}</button>)}</div>
        </div>
        <div className="lchat" data-reveal>
          <div className="lchath"><span className="gico"><Compass size={15} /></span>Boussole<small>{t.bou.sub}</small></div>
          <div className="lmsgs" key={demo + lang.code}>
            <div className="lm me">{D.q}</div>
            <div className="lm bot"><Answer lines={D.a} /></div>
          </div>
          <div className="lask"><span>{t.bou.ask}</span><Send size={15} /></div>
        </div>
      </div>
    </section>

    <section className="lstats" data-reveal>
      {t.stats.map(s => <div key={s[0]}><b>{s[0]}</b><span>{s[1]}</span></div>)}
    </section>

    <section className="lsec" id="tarifs">
      <div className="lhead" data-reveal><span className="leyebrow">{P.eyebrow}</span><h2>{P.h2}</h2><p>{P.p}</p></div>
      <div className="lplans" data-reveal>{PLANS.map((p, i) => { const tx = P.plans[i]; return <div key={p.id} className={'lplan' + (p.best ? ' best' : '')}>
        <div className="mrow" style={{ justifyContent: 'space-between' }}><h3>{tx.name}</h3><span className={'ltag' + (p.soon ? ' soon' : '')}>{tx.tag}</span></div>
        <div className="lprice"><b>{p.price}</b><span>{p.per ? P.perMonth : ''}</span></div>
        <p>{tx.desc}</p>
        <button className={'lbtn' + (p.best ? ' pri' : '')} onClick={signup}>{tx.cta} <ArrowRight size={16} /></button>
      </div> })}</div>
      <div className="ltable" data-reveal><table>
        <thead><tr><th>{P.compare}</th>{P.plans.map(p => <th key={p.name}>{p.name}</th>)}</tr></thead>
        <tbody>{ROWS.map((vals, r) => vals.length === 0 ? <tr key={r} className="grp"><td colSpan={4}>{P.rows[r]}</td></tr> :
          <tr key={r}><td>{P.rows[r]}</td>{vals.map((v, i) => <td key={i}>{cell(v)}</td>)}</tr>)}</tbody>
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
        <div><h4>{t.foot.product}</h4><a href="#fonctionnalites" onClick={go('fonctionnalites')}>{t.nav.features}</a><a href="#boussole" onClick={go('boussole')}>{t.nav.boussole}</a><a href="#tarifs" onClick={go('tarifs')}>{t.nav.pricing}</a></div>
        <div><h4>{t.foot.help}</h4><a href="#faq" onClick={go('faq')}>{t.nav.faq}</a></div>
        <div><h4>{t.foot.account}</h4><a href="/connexion" onClick={e => { e.preventDefault(); nav('/connexion') }}>{t.nav.login}</a><a href="#beta" onClick={e => { e.preventDefault(); signup() }}>{t.nav.join}</a></div>
      </div>
    </footer>
  </div>
}
