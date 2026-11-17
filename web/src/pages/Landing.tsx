// Site vitrine de POP (visiteurs non connectés). Sombre, accent orange, maquettes de l'app en HTML.
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  ArrowRight, Check, ChevronDown, Compass, ListChecks, Code2, Boxes, PenTool, LineChart, Coins, Bot, Minus, Sparkles, Send, AlertTriangle, AlertCircle, CheckCircle2,
} from 'lucide-react'
import '../landing.css'
import { WaitlistModal } from '../components/Waitlist'

// Tarifs : à ajuster ici. Accès sur liste d'attente pendant la bêta privée (aucun paiement n'est branché).
const PLANS = [
  { id: 'createur', name: 'Créateur', tag: 'Pour se lancer', price: '0 €', per: '', desc: 'Une map, une petite équipe, tout l’essentiel.', cta: 'Rejoindre la liste' },
  { id: 'studio', name: 'Studio', tag: 'Le plus complet', price: '19 €', per: '/ mois', desc: 'Pour les studios qui sortent des maps en équipe.', cta: 'Rejoindre la liste', best: true },
  { id: 'forge', name: 'Studio+', tag: 'Bientôt', price: '49 €', per: '/ mois', desc: 'Tout Studio, plus POP Forge : l’app de bureau qui construit dans UEFN.', cta: 'Rejoindre la liste', soon: true },
]
const ROWS: [string, (string | boolean)[]][] = [
  ['Organisation', []],
  ['Tâches d’une heure, Kanban, dépendances', [true, true, true]],
  ['Boussole, le guide du jour', [true, true, true]],
  ['Pages, tableau blanc, planning et jalons', [true, true, true]],
  ['Maps', ['1', 'Illimitées', 'Illimitées']],
  ['Membres', ['3', 'Illimités', 'Illimités']],
  ['UEFN', []],
  ['UEFN Link : projets synchronisés', [true, true, true]],
  ['Éditeur Verse relié (compilation, erreurs à la ligne)', [true, true, true]],
  ['Organiseur d’acteurs et modifications en lot', [false, true, true]],
  ['IA connectées (Claude, Cursor, Codex…)', ['1', 'Illimitées', 'Illimitées']],
  ['POP Forge : construction assistée dans UEFN', [false, false, 'Bientôt']],
  ['Croissance', []],
  ['Analyse du marché et maps qui marchent', [true, true, true]],
  ['Tendances vidéo TikTok et YouTube Shorts', [false, true, true]],
  ['Échange de playtests et jour J', [true, true, true]],
  ['Partage des gains et relevés', [false, true, true]],
  ['Discord, GitHub, Lore', [false, true, true]],
]
const FAQ: [string, string][] = [
  ['Faut-il un PC Windows ?', 'Pour relier UEFN, oui : UEFN ne tourne que sur Windows, et UEFN Link se lance sur ce PC. Le reste de POP s’utilise depuis n’importe quel navigateur, Mac compris.'],
  ['Est-ce que POP publie ma map ?', 'Non. POP prépare tout (vérification avant publication, playtests, plan du jour J), mais la publication se fait toujours dans le portail créateur d’Epic.'],
  ['Boussole, c’est une IA ?', 'Non. Boussole suit des règles simples à partir de vos tâches, de leurs dates, de leurs dépendances et de vos jalons. Elle répond aussi aux questions sur POP à partir de son guide. Aucune donnée n’est envoyée à un service d’IA.'],
  ['Et si je veux utiliser mon IA ?', 'Connectez Claude, Cursor, Codex ou une autre IA compatible MCP dans « IA connectées ». Elle peut lire et modifier vos tâches, votre Verse et vos acteurs UEFN, et chaque action est notée et annulable.'],
  ['D’où viennent les chiffres de l’analyse ?', 'De l’API officielle Fortnite d’Epic : joueurs, pic de joueurs simultanés, temps de jeu et rétention de chaque map publique, sur 7 jours.'],
  ['Comment avoir accès ?', 'POP est en bêta privée. Rejoignez la liste d’attente : on valide les demandes une par une et vous recevez par e-mail un lien pour créer votre compte.'],
  ['Combien ça coûte ?', 'Les tarifs sont affichés plus haut. Ils s’appliquent à l’ouverture de POP ; les membres de la bêta sont prévenus avant tout paiement.'],
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
function AppShot() {
  return <div className="mock appshot" aria-hidden>
    <div className="mbar"><i /><i /><i /><span>pop · Vue d’ensemble</span></div>
    <div className="mbody">
      <aside className="mside">
        <img src="/logo.png" alt="" />
        {['Vue d’ensemble', 'Pages', 'Tableau blanc', 'Tâches', 'Éditeur Verse', 'Organiseur', 'Marché'].map((x, i) => <div key={x} className={'mnav' + (i === 0 ? ' on' : '')}>{x}</div>)}
      </aside>
      <div className="mmain">
        <div className="mcard guide">
          <div className="mrow"><span className="gico"><Compass size={14} /></span><b>Bonjour Léa, je vous propose de commencer par #42.</b></div>
          <p><AlertCircle size={13} className="r" />2 tâches en retard (la plus ancienne était prévue le 12 nov.).</p>
          <p><AlertTriangle size={13} className="o" />Jalon « Bêta » dans 5 jours : 9 tâches à finir, retard prévu de 1 jour.</p>
          <div className="msug first"><span><b>#42</b> Système de score</span><em>débloque 3 autres tâches</em><span className="mbtn">Prendre</span></div>
          <div className="msug"><span><b>#38</b> Décor de la zone 2</span><em>prévue aujourd’hui</em><span className="mbtn ghost">Commencer</span></div>
        </div>
        <div className="mkan">
          {[['À faire', ['#44 Écran de fin', '#45 Playtest interne']], ['En cours', ['#42 Système de score']], ['Fait', ['#40 Spawn des joueurs', '#41 Tempête']]].map(([t, cs]) => <div key={t as string} className="mcol"><span>{t as string}</span>{(cs as string[]).map(c => <div key={c} className="mtask">{c}<i>1 h</i></div>)}</div>)}
        </div>
      </div>
    </div>
  </div>
}
function TasksShot() {
  return <div className="mock" aria-hidden><div className="mpad">
    <div className="mkan three">
      {[['À faire', [['#44', 'Écran de fin', 'attend #42'], ['#45', 'Playtest interne', '']]], ['En cours', [['#42', 'Système de score', 'Léa']]], ['Fait', [['#40', 'Spawn des joueurs', 'Hugo'], ['#41', 'Tempête', 'Léa']]]].map(([t, cs]) =>
        <div key={t as string} className="mcol"><span>{t as string}</span>{(cs as string[][]).map(c => <div key={c[0]} className="mtask big"><small>{c[0]} · 1 h</small>{c[1]}{c[2] && <em>{c[2]}</em>}</div>)}</div>)}
    </div>
  </div></div>
}
function VerseShot() {
  const L = ['using { /Fortnite.com/Devices }', 'using { /Verse.org/Simulation }', '', 'score_manager := class(creative_device):', '    @editable Bouton : button_device = button_device{}', '    var Points : int = 0', '', '    OnBegin<override>()<suspends> : void =', '        Bouton.InteractedWithEvent.Subscribe(OnClic)', '', '    OnClic(Agent : agent) : void =', '        set Points += 1.5']
  return <div className="mock code" aria-hidden>
    <div className="mbar"><i /><i /><i /><span>score_manager.verse · envoyé à UEFN</span></div>
    <pre>{L.map((l, i) => <div key={i} className={i === 11 ? 'lerr' : ''}><span className="ln">{i + 1}</span>{l || ' '}</div>)}</pre>
    <div className="cerr"><AlertCircle size={14} /> Ligne 12 : int et float ne s’additionnent pas. <b>Corriger : Points += 1</b></div>
  </div>
}
function OrganizerShot() {
  const rows = [['Spawn_Rouge_01', 'Player Spawner', 'Équipe rouge'], ['Spawn_Rouge_02', 'Player Spawner', 'Équipe rouge'], ['Bouton_Score', 'Button', 'Gameplay'], ['Minuteur_Manche', 'Timer', 'Gameplay'], ['Barriere_Zone2', 'Barrier', 'Zone 2']]
  return <div className="mock" aria-hidden><div className="mpad">
    <div className="msearch">tag:equipe_rouge type:spawner</div>
    {rows.map((r, i) => <div key={r[0]} className={'mact' + (i < 2 ? ' sel' : '')}><span className="mono">{r[0]}</span><em>{r[1]}</em><small>{r[2]}</small></div>)}
    <div className="mbulk">2 sélectionnés · <span>Renommer</span><span>Déplacer</span><span>Dupliquer</span></div>
  </div></div>
}
function BoardShot() {
  return <div className="mock wbshot" aria-hidden>
    <div className="bfmt"><b>B</b><i>I</i><u>U</u><s>S</s><span className="bsw" style={{ background: '#ED753E' }} /><span className="bsw" style={{ background: '#0090FF' }} /></div>
    <div className="bst" style={{ left: '8%', top: '30%', background: '#FFF3A3' }}>Mode équipe 4v4</div>
    <div className="bst" style={{ left: '38%', top: '22%', background: '#FFD8D6' }}><b>Boss final</b> au centre</div>
    <div className="bst" style={{ left: '66%', top: '40%', background: '#D3F2D4' }}>Économie : 1 pièce / élimination</div>
    <div className="bshape">Zone 2</div>
  </div>
}
function MarketShot() {
  const G = [['Tycoon', 'Bonne opportunité', 'g', '1 650', '+12 %'], ['Horror', 'Populaire mais disputé', 'o', '1 480', '+4 %'], ['Box fight', 'Saturé', 'r', '220', '−8 %'], ['Parkour', 'Niche', 'b', '640', '+21 %']]
  return <div className="mock" aria-hidden><div className="mpad mgen">
    {G.map(g => <div key={g[0]} className="mgcard"><div className="mrow"><b>{g[0]}</b><em className={g[4].startsWith('−') ? 'down' : 'up'}>{g[4]}</em></div><span className={'mbadge ' + g[2]}>{g[1]}</span><div className="mbig"><b>{g[3]}</b> joueurs / map typique</div><div className="spk">{[3, 4, 4, 5, 6, 7, 8].map((h, i) => <i key={i} style={{ height: h * 4 }} />)}</div></div>)}
  </div></div>
}
function MoneyShot() {
  return <div className="mock" aria-hidden><div className="mpad">
    <div className="mrow" style={{ justifyContent: 'space-between' }}><b>Gains d’octobre</b><b>4 820 €</b></div>
    {[['Léa', 'Code · 40 %', '1 928 €'], ['Hugo', 'Carte · 35 %', '1 687 €'], ['Sam', 'Musique · fixe', '300 €'], ['Studio', 'Reste', '905 €']].map(r => <div key={r[0]} className="mpay"><span className="mav">{r[0][0]}</span><span>{r[0]}<small>{r[1]}</small></span><b>{r[2]}</b></div>)}
    <p className="mnote">POP calcule, il ne fait aucun virement.</p>
  </div></div>
}

const FEATURES = [
  { n: '01', icon: ListChecks, tag: 'Tâches et Boussole', h: 'Des tâches d’une heure. Un guide chaque matin.', p: 'Le générateur découpe la map en tâches d’une heure, avec leurs dépendances. Boussole résume la journée, signale les retards et propose quoi prendre en premier.', pts: ['Kanban et dépendances', 'Retards et jalons suivis', '« Prendre » en un clic'], shot: <TasksShot /> },
  { n: '02', icon: Code2, tag: 'Éditeur Verse', h: 'Ctrl+S, et c’est dans UEFN.', p: 'Le Verse s’écrit dans POP et part directement dans votre projet : compilation, erreurs à la bonne ligne, session de test. L’assistant explique et corrige, sans service externe.', pts: ['Compilation dans UEFN', 'Erreurs à la ligne', 'Corrections proposées'], shot: <VerseShot /> },
  { n: '03', icon: Boxes, tag: 'Organiseur', h: 'Tous vos appareils. Rangés en trois secondes.', p: 'Cherchez par tag, type ou dossier, sélectionnez-en vingt et renommez, déplacez ou dupliquez d’un coup. Ctrl+Z annule. Si UEFN est fermé, les changements attendent.', pts: ['Recherche tag:, type:, dossier:', 'Modifications en lot', 'Tout est annulable'], shot: <OrganizerShot /> },
  { n: '04', icon: PenTool, tag: 'Tableau blanc', h: 'Dessinez la map à plusieurs.', p: 'Post-it, formes, flèches, couleurs, gras, souligné : tout ce qu’il faut pour poser les idées. Votez, puis transformez les meilleurs post-it en tâches.', pts: ['Mise en forme complète', 'Temps réel à plusieurs', 'Post-it → tâches'], shot: <BoardShot /> },
  { n: '05', icon: LineChart, tag: 'Analyse', h: 'Sachez quoi construire avant de construire.', p: 'Quels genres attirent les joueurs, où la place est libre, quelles maps décollent dès leur sortie, et ce qui buzze en vidéo. Chiffres de l’API officielle d’Epic.', pts: ['Verdict par genre', 'Maps qui marchent', 'Tendances TikTok et Shorts'], shot: <MarketShot /> },
  { n: '06', icon: Coins, tag: 'Rémunérer', h: 'Chacun sait ce qu’il touche.', p: 'Règles de partage au pourcentage, au fixe ou à la tâche, import des gains, relevé par personne. Plus de tableur qui traîne sur Discord.', pts: ['Pourcentage, fixe, par tâche', 'Import CSV', 'Relevé de chacun'], shot: <MoneyShot /> },
]

const DEMO: { q: string; a: React.ReactNode }[] = [
  { q: 'Qu’est-ce que je fais maintenant ?', a: <><p><b>Bonjour Léa, on rattrape le retard ?</b></p><p><AlertCircle size={13} className="r" /> 2 tâches en retard.</p><p><CheckCircle2 size={13} className="g" /> Hier, vous avez terminé 3 tâches.</p><p>À faire maintenant : <b>#42 Système de score</b>, qui débloque 3 autres tâches.</p></> },
  { q: 'Comment relier UEFN ?', a: <><p><b>Comment relier UEFN à POP ?</b></p><ol><li>Organiseur → Télécharger UEFN Link.</li><li>Double-cliquez sur pop-link.exe sur le PC.</li><li>Dans UEFN : Model Context Protocol → Start Server.</li></ol></> },
  { q: 'On est en retard pour la bêta ?', a: <><p><AlertTriangle size={13} className="o" /> Jalon « Bêta » dans 5 jours : 9 tâches à finir.</p><p>Au rythme actuel (1,5 par jour), il faudrait 6 jours. <b>Retard prévu : 1 jour.</b></p></> },
  { q: 'Mon IA peut fermer les tâches ?', a: <><p>Oui : connectée dans « IA connectées », elle utilise <b>next_task</b>, <b>set_task_status</b> et <b>comment_task</b>. Chaque action est notée « IA » dans l’historique.</p></> },
]

export default function Landing() {
  const nav = useNavigate()
  const root = useRef<HTMLDivElement>(null)
  const [faq, setFaq] = useState<number | null>(0)
  const [demo, setDemo] = useState(0)
  const [scrolled, setScrolled] = useState(false)
  useReveal()
  useEffect(() => { document.title = 'POP · Le studio des créateurs Fortnite' }, [])
  useEffect(() => { const t = setInterval(() => setDemo(d => (d + 1) % DEMO.length), 6000); return () => clearInterval(t) }, [demo])
  const go = (id: string) => (e: React.MouseEvent) => { e.preventDefault(); document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' }) }
  const [wait, setWait] = useState(false)
  const signup = () => setWait(true)
  return <div className="lp" ref={root} onScroll={e => setScrolled((e.target as HTMLDivElement).scrollTop > 10)}>
    <nav className={'lnav' + (scrolled ? ' solid' : '')}>
      <a href="#top" onClick={go('top')} className="lbrand" aria-label="POP, accueil"><img src="/logo.png" alt="POP" /></a>
      <div className="llinks">
        <a href="#fonctionnalites" onClick={go('fonctionnalites')}>Fonctionnalités</a>
        <a href="#boussole" onClick={go('boussole')}>Boussole</a>
        <a href="#tarifs" onClick={go('tarifs')}>Tarifs</a>
        <a href="#faq" onClick={go('faq')}>FAQ</a>
      </div>
      <div className="lacts">
        <button className="lghost" onClick={() => nav('/connexion')}>Se connecter</button>
        <button className="lpill" onClick={signup}>Rejoindre la bêta</button>
      </div>
    </nav>

    <header className="lhero" id="top">
      <div className="lglow" />
      <a className="lannounce" href="#boussole" onClick={go('boussole')}><Sparkles size={14} /> Nouveau : Boussole, votre guide du jour <ArrowRight size={14} /></a>
      <h1 data-reveal>Vos maps Fortnite.<br /><span className="grad">À plusieurs. Sans le chaos.</span></h1>
      <p className="lsub" data-reveal>POP réunit tout le studio UEFN au même endroit : <b>tâches d’une heure</b>, Verse relié à UEFN, tableau blanc, analyse du marché et partage des gains.</p>
      <div className="lctas" data-reveal>
        <button className="lbtn pri" onClick={signup}>Rejoindre la liste d’attente <ArrowRight size={18} /></button>
        <button className="lbtn" onClick={go('fonctionnalites')}>Voir les fonctionnalités</button>
      </div>
      <p className="lmicro" data-reveal><span className="ldot" /> <b>Bêta privée</b> · Accès sur liste d’attente · Relié à UEFN</p>
      <div className="lshot" data-reveal><AppShot /></div>
    </header>

    <section className="lmarquee" aria-label="Fonctionne avec">
      <p>Fonctionne avec vos outils</p>
      <div className="mq"><div className="mqt">{[0, 1].map(k => <span key={k}>{['UEFN', 'Verse', 'Discord', 'GitHub', 'Lore', 'Claude', 'Cursor', 'Codex', 'TikTok', 'YouTube Shorts', 'API Fortnite d’Epic'].map(x => <em key={x}>{x}</em>)}</span>)}</div></div>
    </section>

    <section className="lsec lpain">
      <div className="lhead" data-reveal><span className="leyebrow">Le problème</span><h2>Discord, trois Google Docs,<br />et personne ne sait qui fait quoi.</h2></div>
      <div className="lvs" data-reveal>
        <div className="lcol bad"><span className="lcolh">Sans POP</span>
          <ul>{['« Qui a touché au spawner ? »', 'Le Verse passe par Discord en .txt', 'Une liste de tâches sur un Google Doc', 'Le tableur des gains, version 7', '« On sort quand déjà ? »', 'On construit sans savoir si le genre marche'].map(x => <li key={x}><Minus size={14} />{x}</li>)}</ul></div>
        <div className="lcol good"><span className="lcolh">Avec POP</span>
          <ul>{['Chaque action est tracée et annulable', 'Ctrl+S envoie le Verse dans UEFN', 'Des tâches d’une heure, avec dépendances', 'Un relevé clair pour chacun', 'Boussole suit le jalon et le retard', 'Un verdict par genre, avec les vrais chiffres'].map(x => <li key={x}><Check size={14} />{x}</li>)}</ul></div>
      </div>
    </section>

    <section className="lsec" id="fonctionnalites">
      <div className="lhead" data-reveal><span className="leyebrow">Fonctionnalités</span><h2>Tout le studio, de l’idée au jour J.</h2><p>Ce que vous voyez ici, ce sont les vrais écrans de POP, en plus simple.</p></div>
      <div className="lfeats">{FEATURES.map((f, i) => <article key={f.n} className={'lfeat' + (i % 2 ? ' rev' : '')} data-reveal>
        <div className="ltxt">
          <span className="lnum"><b>{f.n}</b><f.icon size={15} />{f.tag}</span>
          <h3>{f.h}</h3><p>{f.p}</p>
          <ul>{f.pts.map(p => <li key={p}><Check size={15} />{p}</li>)}</ul>
        </div>
        <figure>{f.shot}</figure>
      </article>)}</div>
      <div className="lai" data-reveal>
        <Bot size={22} />
        <div><b>Vous avez déjà une IA ?</b> Connectez Claude, Cursor, Codex ou une autre IA compatible MCP : elle prend une tâche, écrit le Verse, compile, puis ferme la tâche avec une note. Chaque action est notée et annulable.</div>
      </div>
    </section>

    <section className="lsec lboussole" id="boussole">
      <div className="lsplit">
        <div className="ltxt" data-reveal>
          <span className="leyebrow">Boussole</span>
          <h2>Un guide, pas une IA.</h2>
          <p>Chaque matin, Boussole lit vos tâches, leurs dates, leurs dépendances et vos jalons. Elle dit ce qui est en retard, ce qui coince et par quoi commencer. Et si quelqu’un ne sait pas utiliser POP, il lui pose la question.</p>
          <div className="lchips">{DEMO.map((d, i) => <button key={d.q} className={i === demo ? 'on' : ''} onClick={() => setDemo(i)}>{d.q}</button>)}</div>
        </div>
        <div className="lchat" data-reveal>
          <div className="lchath"><span className="gico"><Compass size={15} /></span>Boussole<small>votre guide du jour</small></div>
          <div className="lmsgs" key={demo}>
            <div className="lm me">{DEMO[demo].q}</div>
            <div className="lm bot">{DEMO[demo].a}</div>
          </div>
          <div className="lask"><span>Comment je… ?</span><Send size={15} /></div>
        </div>
      </div>
    </section>

    <section className="lstats" data-reveal>
      {[['1 h', 'par tâche, pour avancer chaque jour'], ['0', 'service d’IA externe obligatoire'], ['7 jours', 'de stats par map, de l’API d’Epic'], ['100 %', 'des actions UEFN annulables']].map(s => <div key={s[0]}><b>{s[0]}</b><span>{s[1]}</span></div>)}
    </section>

    <section className="lsec" id="tarifs">
      <div className="lhead" data-reveal><span className="leyebrow">Tarifs</span><h2>Choisissez votre plan.</h2><p>POP est en bêta privée : l’accès se fait sur liste d’attente. Les tarifs s’appliquent à l’ouverture, et vous serez prévenu avant tout paiement.</p></div>
      <div className="lplans" data-reveal>{PLANS.map(p => <div key={p.id} className={'lplan' + (p.best ? ' best' : '')}>
        <div className="mrow" style={{ justifyContent: 'space-between' }}><h3>{p.name}</h3><span className={'ltag' + (p.soon ? ' soon' : '')}>{p.tag}</span></div>
        <div className="lprice"><b>{p.price}</b><span>{p.per}</span></div>
        <p>{p.desc}</p>
        <button className={'lbtn' + (p.best ? ' pri' : '')} onClick={signup}>{p.cta} <ArrowRight size={16} /></button>
      </div>)}</div>
      <div className="ltable" data-reveal><table>
        <thead><tr><th>Comparer les plans</th>{PLANS.map(p => <th key={p.id}>{p.name}</th>)}</tr></thead>
        <tbody>{ROWS.map(([label, vals]) => vals.length === 0 ? <tr key={label} className="grp"><td colSpan={4}>{label}</td></tr> :
          <tr key={label}><td>{label}</td>{vals.map((v, i) => <td key={i}>{v === true ? <Check size={16} className="ok" /> : v === false ? <Minus size={16} className="no" /> : <span className={v === 'Bientôt' ? 'ltag soon' : ''}>{v}</span>}</td>)}</tr>)}</tbody>
      </table></div>
    </section>

    <section className="lsec lfaq" id="faq">
      <div className="lhead" data-reveal><span className="leyebrow">FAQ</span><h2>Les questions qu’on nous pose.</h2></div>
      <div className="lqs" data-reveal>{FAQ.map(([q, a], i) => <div key={q} className={'lq' + (faq === i ? ' open' : '')}>
        <button onClick={() => setFaq(faq === i ? null : i)} aria-expanded={faq === i}>{q}<ChevronDown size={18} /></button>
        <div className="la"><p>{a}</p></div>
      </div>)}</div>
    </section>

    <section className="lfinal" data-reveal>
      <div className="lglow" />
      <h2>Votre prochaine map<br /><span className="grad">commence ici.</span></h2>
      <p>Rejoignez la liste d’attente : on vous envoie votre accès dès qu’une place se libère.</p>
      <div className="lctas"><button className="lbtn pri" onClick={signup}>Rejoindre la liste d’attente <ArrowRight size={18} /></button><button className="lbtn" onClick={() => nav('/connexion')}>Se connecter</button></div>
    </section>

    {wait && <WaitlistModal onClose={() => setWait(false)} />}
    <footer className="lfoot">
      <div className="lfb"><img src="/logo.png" alt="POP" /><span>© {new Date().getFullYear()} POP · Fait par des créateurs Fortnite, pour des créateurs Fortnite.</span><small>Fortnite et UEFN sont des marques d’Epic Games. POP n’est ni affilié à Epic Games ni approuvé par Epic Games.</small></div>
      <div className="lfcols">
        <div><h4>Produit</h4><a href="#fonctionnalites" onClick={go('fonctionnalites')}>Fonctionnalités</a><a href="#boussole" onClick={go('boussole')}>Boussole</a><a href="#tarifs" onClick={go('tarifs')}>Tarifs</a></div>
        <div><h4>Aide</h4><a href="#faq" onClick={go('faq')}>FAQ</a></div>
        <div><h4>Compte</h4><a href="/connexion" onClick={e => { e.preventDefault(); nav('/connexion') }}>Se connecter</a><a href="#beta" onClick={e => { e.preventDefault(); signup() }}>Rejoindre la bêta</a></div>
      </div>
    </footer>
  </div>
}
