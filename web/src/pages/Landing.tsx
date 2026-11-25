// Site vitrine de POP (visiteurs non connectés). Sombre, accent orange, maquettes de l'app en HTML.
import { LEGAL_PAGES } from './Legal'
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  ArrowRight, Check, ChevronDown, Compass, ListChecks, Boxes, PenTool, LineChart, Coins, Bot, Minus, Sparkles, AlertTriangle, AlertCircle,
} from 'lucide-react'
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
/** Première phrase seulement : une ligne par fonction (#362). */
const first = (t: string) => { const m = t.match(/^.*?[.!?。؟](?=\s|$)/); return m ? m[0] : t }

// Les fonctions, en liste courte : icône, titre, une phrase (#362).
const FEATURES = [
  { n: '01', id: 'f-taches', icon: ListChecks },
  { n: '02', id: 'f-organiseur', icon: Boxes },
  { n: '03', id: 'f-tableau', icon: PenTool },
  { n: '04', id: 'f-analyse', icon: LineChart },
  { n: '05', id: 'f-gains', icon: Coins },
]

export default function Landing() {
  const nav = useNavigate()
  const { t, lang } = useI18n()
  const root = useRef<HTMLDivElement>(null)
  const [faq, setFaq] = useState<number | null>(null)
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
        <a href="#fonctionnalites" onClick={go('fonctionnalites')}>{t.nav.features}</a>
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
        <button className="lbtn ghost" onClick={go('fonctionnalites')}>{t.hero.see}</button>
      </div>
      <div className="lshot" data-reveal><AppShot m={t.mock} /></div>
    </header>

    <section className="lsec" id="fonctionnalites">
      <div className="lhead" data-reveal><span className="leyebrow">{t.feat.eyebrow}</span><h2>{t.feat.h2}</h2></div>
      <div className="lsimple" data-reveal>
        {FEATURES.map((f, i) => { const it = t.feat.items[i]; return <div key={f.n} id={f.id} className="lsimple-row">
          <span className="lsimple-ic"><f.icon size={20} strokeWidth={1.6} /></span>
          <div><h3>{it.h}</h3><p>{first(it.p)}</p></div>
        </div> })}
        <div className="lsimple-row ai" id="ia">
          <span className="lsimple-ic"><Bot size={20} strokeWidth={1.6} /></span>
          <div><h3>{t.feat.aiB}</h3><p>{t.feat.aiText}</p></div>
        </div>
      </div>
    </section>

    <section className="lsec" id="tarifs">
      <div className="lhead" data-reveal><span className="leyebrow">{P.eyebrow}</span><h2>{P.h2}</h2><p>{P.p}</p></div>
      <div className="lplans" data-reveal>{PLANS.map((p, i) => { const tx = P.plans[i]; return <div key={p.id} className={'lplan' + (p.best ? ' best' : '')}>
        <div className="mrow" style={{ justifyContent: 'space-between' }}><h3>{tx.name}</h3><span className="ltag">{tx.tag}</span></div>
        <div className="lprice"><b>{p.price}</b><span>{p.per ? P.perMonth : ''}</span></div>
        <p>{tx.desc}</p>
        <button className={'lbtn' + (p.best ? ' pri' : '')} onClick={signup}>{tx.cta} <ArrowRight size={16} /></button>
      </div> })}</div>
      <details className="ltable-wrap" data-reveal>
        <summary>{P.compare}<ChevronDown size={16} /></summary>
        <div className="ltable"><table>
          <thead><tr><th>{P.compare}</th>{P.plans.map(p => <th key={p.name}>{p.name}</th>)}</tr></thead>
          <tbody>{ROWS.map((vals, r) => vals.length === 0 ? <tr key={r} className="grp"><td colSpan={3}>{P.rows[r]}</td></tr> :
            <tr key={r}><td>{P.rows[r]}</td>{vals.map((v, i) => <td key={i}>{cell(v)}</td>)}</tr>)}</tbody>
        </table></div>
      </details>
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
