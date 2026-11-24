// « Mes agents » (#134) : les IA du compte (depuis #145, une IA se connecte au compte et choisit sa map elle-même).
// - Section de l'accueil : une carte par IA (connectée, ou enregistrée et en attente de son premier appel MCP) et
//   « Connecter un agent », qui déplie la connexion dans la section (en un clic #290, ou clé API #280).
// - Pastille dans la barre du haut : aucune IA → « Connecter une IA » ; en attente ; connectée.
// État lu dans /api/me/tokens : last_used vide = en attente de connexion, rempli = premier appel MCP reçu.
import { useEffect, useRef, useState } from 'react'
import { Check, Copy, ArrowLeft, X, Plus, Bot, Settings2 } from 'lucide-react'
import { useLocation, useNavigate } from 'react-router-dom'
import { ago, api } from '../api'
import { useApp } from '../state'
import { tr } from '../i18n/t'
import { CLIENTS, ClientLogo, KeyPanel, OneClick, clientName, type Client } from './AiTokens'

export type AccountAI = { tokens: any[] | null; live: any[]; pending: any[]; reload: () => void }

/** IA du compte, relues toutes les 3 s tant qu'on attend une connexion, toutes les 30 s sinon, et au retour sur l'onglet. */
// La pastille et la section « Mes agents » partagent la dernière lecture : elles changent ensemble.
let last: any[] | null = null
const subs = new Set<(t: any[]) => void>()
export function useAccountAI(waiting: boolean): AccountAI {
  const [tokens, setTokens] = useState<any[] | null>(last)
  useEffect(() => { subs.add(setTokens); return () => { subs.delete(setTokens) } }, [])
  const reload = async () => { try { last = await api.get<any[]>('/api/me/tokens'); subs.forEach(f => f(last!)) } catch {} }
  const live = (tokens ?? []).filter(t => t.last_used)
  const pending = (tokens ?? []).filter(t => !t.last_used)
  const fast = waiting || pending.length > 0
  useEffect(() => {
    reload()
    // Connexion faite dans un autre onglet (Claude, Cursor…) : relue dès qu'on revient sur POP.
    const f = () => { if (document.visibilityState === 'visible') reload() }
    document.addEventListener('visibilitychange', f)
    return () => document.removeEventListener('visibilitychange', f)
  }, [])
  useEffect(() => {
    const h = setInterval(reload, fast ? 3000 : 30000)
    return () => clearInterval(h)
  }, [fast])
  return { tokens, live, pending, reload }
}

/** Pastille d'état de l'IA, à côté de celle d'UEFN Link. Sans IA connectée, elle mène à « Mes agents » sur l'accueil. */
export function AiPill() {
  const nav = useNavigate()
  const ai = useAccountAI(false)
  if (!ai.tokens) return null
  const toAgents = () => nav('/#agents')
  if (ai.live.length) {
    const kinds = [...new Set(ai.live.map(t => t.kind))].slice(0, 3)
    return <button className="upill connected" onClick={toAgents} title={ai.live.map(t => t.name).join(', ')}>
      <span className="udot" />{kinds.map(k => <ClientLogo key={k} kind={k} size={16} />)}
      {ai.live.length === 1 ? tr("{0} connectée", [ai.live[0].name]) : tr("{0} IA connectées", [ai.live.length])}
    </button>
  }
  if (ai.pending.length) {
    const t = ai.pending[ai.pending.length - 1]
    return <button className="upill closed" onClick={toAgents} title={tr("Enregistrée dans POP : elle sera connectée à son premier appel.")}>
      <span className="aiwait-dot" /><ClientLogo kind={t.kind} size={16} />{tr("{0} · en attente de connexion", [t.name])}
    </button>
  }
  return <button className="upill unlinked" onClick={toAgents}><span className="udot" />{tr("Connecter une IA")}</button>
}

/** Section « Mes agents » de l'accueil. */
export function AgentsSection() {
  const nav = useNavigate()
  const { toast } = useApp()
  const loc = useLocation()
  const ref = useRef<HTMLElement>(null)
  const [open, setOpen] = useState(false)
  const ai = useAccountAI(open)
  const none = !!ai.tokens && ai.tokens.length === 0
  // /#agents (pastille de la barre du haut) : on descend jusqu'à la section et on ouvre la connexion s'il n'y a pas d'IA.
  useEffect(() => {
    if (loc.hash !== '#agents' || !ai.tokens) return
    ref.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
    if (!ai.live.length) setOpen(true)
  }, [loc.key, !!ai.tokens])
  // Aucune IA : la connexion est dépliée d'office, et le reste quand la clé est créée (elle ne s'affiche qu'une fois).
  useEffect(() => { if (none) setOpen(true) }, [none])
  if (!ai.tokens) return null
  const settings = () => nav('/parametres?onglet=ia')
  return <section className="home-block" id="agents" ref={ref}>
    <div className="home-head">
      <span className="home-ic"><Bot size={18} /></span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <h2>{tr("Mes agents")}</h2>
        <span className="small muted">{tr("Les IA connectées à votre compte : elles voient vos maps et choisissent celle sur laquelle travailler.")}</span>
      </div>
      {!none && <button className="btn" onClick={settings}><Settings2 size={16} />{tr("Gérer")}</button>}
    </div>
    <div className="home-grid">
      {ai.tokens.map(t => {
        const live = !!t.last_used
        return <div key={t.id} role="button" tabIndex={0} className="home-map agent-card" onClick={settings} onKeyDown={e => { if (e.key === 'Enter') settings() }}>
          <span className="home-map-top"><span className="agent-name"><ClientLogo kind={t.kind} size={20} /><b>{t.name}</b></span>
            {!live && <button className="iconbtn agent-drop" aria-label={tr("Retirer {0}", [t.name])} data-tip={tr("Retirer cette tentative")} onClick={async e => {
              // Tentative jamais aboutie : on retire sa clé (elle n'a jamais servi), sans passer par Paramètres.
              e.stopPropagation()
              try { await api.del(`/api/tokens/${t.id}`); ai.reload(); toast(tr("{0} retirée", [t.name])) } catch (er: any) { toast(er.message, true) }
            }}><X size={16} /></button>}
          </span>
          <span className="small muted">{t.oauth ? tr("Autorisée depuis l’IA") : clientName(t.kind)}{t.scope === 'read' ? ` · ${tr("Lecture seule")}` : ''}</span>
          <span className={'agent-state' + (live ? ' on' : '')}>{live ? <span className="udot" /> : <span className="aiwait-dot" />}{live ? tr("Connectée · {0}", [ago(t.last_used)]) : tr("En attente de connexion…")}</span>
          <span className="small muted">{t.map ? tr("Travaille sur « {0} »", [t.map.name]) : live ? tr("Aucune map choisie") : tr("Sera connectée à son premier appel")}</span>
        </div>
      })}
      {!open && !none && <button className="home-map add" onClick={() => setOpen(true)}><Plus size={20} /><b>{tr("Connecter un agent")}</b></button>}
    </div>
    {(open || none) && <div className="card agents-connect"><ConnectPanel ai={ai} onClose={none ? undefined : () => setOpen(false)} /></div>}
  </section>
}

/** « Connecter un agent » replié en bouton, déplié d'office tant que le compte n'a aucune IA (Paramètres → IA connectées). */
export function ConnectAgent() {
  const [open, setOpen] = useState(false)
  const ai = useAccountAI(open)
  const none = !!ai.tokens && ai.tokens.length === 0
  useEffect(() => { if (none) setOpen(true) }, [none])
  if (!ai.tokens) return null
  if (!open && !none) return <button className="btn" onClick={() => setOpen(true)}><Plus size={16} />{tr("Connecter un agent")}</button>
  return <div className="card agents-connect"><ConnectPanel ai={ai} onClose={none ? undefined : () => setOpen(false)} /></div>
}

const KEY_KINDS = ['claude_desktop', 'claude_code', 'cursor', 'codex', 'autre']

/** Connexion d'un agent, dépliée dans la section. Passe seule à « connectée » dès le premier appel MCP d'une nouvelle IA. */
function ConnectPanel({ ai, onClose }: { ai: AccountAI; onClose?: () => void }) {
  const { map, toast } = useApp()
  const [key, setKey] = useState<{ c: Client; token: string; id: number } | null>(null)
  const [copied, setCopied] = useState(false)
  // IA déjà connectées à l'ouverture : seule une nouvelle connexion fait passer à « connectée ».
  const before = useRef<Set<number> | null>(null)
  if (before.current === null && ai.tokens) before.current = new Set(ai.live.map(t => t.id))
  const fresh = before.current ? ai.live.find(t => !before.current!.has(t.id)) : undefined
  const ask = map ? tr("Travaille sur ma map POP « {0} »", [map.name]) : tr("Liste mes maps POP")
  const copyAsk = async () => { try { await navigator.clipboard.writeText(ask); setCopied(true) } catch { toast(tr("Copie impossible : sélectionnez le texte."), true) } }
  const createKey = async (c: Client) => {
    try {
      // Un seul jeton en attente par IA : l'ancien, jamais utilisé, est remplacé (comme dans Paramètres).
      for (const t of ai.pending.filter(t => t.kind === c.kind && !t.oauth)) await api.del(`/api/tokens/${t.id}`)
      const r = await api.post('/api/me/tokens', { name: c.name, kind: c.kind, scope: 'write' })
      setKey({ c, token: r.token, id: r.id }); ai.reload()
    } catch (e: any) { toast(e.message, true) }
  }
  const waiting = ai.pending[ai.pending.length - 1]
  const head = (title: string) => <div className="row" style={{ justifyContent: 'space-between' }}>
    <h3 className="connectai-t">{title}</h3>
    {onClose && <button className="iconbtn" aria-label={tr("Fermer")} onClick={onClose}><X size={18} /></button>}
  </div>

  if (fresh) return <div className="connectai">
    <div className="aiwait ok connectai-ok" role="status"><ClientLogo kind={fresh.kind} size={18} />{tr("{0} est connectée à POP.", [fresh.name])}</div>
    <p className="small" style={{ margin: '0 0 12px' }}>{tr("Demandez-lui maintenant :")} <b style={{ fontWeight: 500 }}>{ask}</b></p>
    {onClose && <button className="btn pri" onClick={onClose}><Check size={16} />{tr("Terminé")}</button>}
  </div>
  if (key) return <div className="connectai">
    {head(tr("Connecter {0}", [key.c.name]))}
    <KeyPanel c={key.c} token={key.token} linked={false} />
    <button className="linkbtn" onClick={() => setKey(null)}><ArrowLeft size={14} /> {tr("Choisir une autre IA")}</button>
  </div>
  return <div className="connectai">
    {head(tr("Connecter un agent"))}
    <p className="muted connectai-sub">{tr("POP se pilote avec votre IA (Claude, Cursor, Codex…) : elle voit vos maps, prend les tâches et agit dans UEFN. Connectez-la une fois pour tout votre compte.")}</p>
    <PromptBlock ai={ai} />
    <div className="aiwait" role="status">
      <span className="aiwait-dot" />
      {waiting ? tr("{0} est enregistrée, en attente de connexion… Envoyez-lui un message pour qu’elle appelle POP.", [waiting.name]) : tr("En attente de votre IA…")}
    </div>
    <div className="connectai-ask">
      <span className="small">{tr("Une fois connectée, demandez-lui :")}</span>
      <code className="aiurl-v">{ask}</code>
      <button className="btn" onClick={copyAsk}>{copied ? <><Check size={15} />{tr("Copié")}</> : <><Copy size={15} />{tr("Copier")}</>}</button>
    </div>
    <details className="connectai-more">
      <summary className="small">{tr("Autres façons de connecter : en un clic depuis l’IA, ou configuration à copier")}</summary>
      <OneClick url={`${location.origin}/mcp`} />
      <p className="small muted" style={{ margin: '14px 0 8px' }}>{tr("Votre IA ne propose pas de se connecter ? Copiez sa configuration avec une clé API :")}</p>
      <div className="aigrid">
        {CLIENTS.filter(c => KEY_KINDS.includes(c.kind)).map(c => <button key={c.kind} className="aicard" onClick={() => createKey(c)}>
          <ClientLogo kind={c.kind} size={20} /><b style={{ fontWeight: 500, flex: 1 }}>{c.name}</b><span className="small">{tr("Clé API")}</span>
        </button>)}
      </div>
    </details>
  </div>
}

/** IA qui savent modifier leur propre configuration (agents de code) : on leur donne un prompt, elles font le reste. */
const PROMPT_KINDS = ['claude_code', 'codex', 'cursor', 'autre']

/** Le prompt à coller dans l'IA : adresse, clé du compte et commandes par outil. La clé n'est montrée qu'ici, une fois. */
function promptText(url: string, token: string) {
  const name = /^(localhost|127\.0\.0\.1)$/.test(location.hostname) ? 'pop-local' : 'pop'
  return tr(`Connecte-toi à POP (mon outil de production de maps Fortnite / UEFN) par son serveur MCP, puis montre-moi mes maps.

Adresse du serveur MCP (Streamable HTTP) : {0}
Clé : {1} (en-tête HTTP « Authorization: Bearer {1} »)

1. Ajoute ce serveur MCP sous le nom « {2} » dans ta configuration utilisateur (pas dans un fichier du dépôt git : la clé est personnelle) :
   - Claude Code : claude mcp add --scope user --transport http {2} {0} --header "Authorization: Bearer {1}"
   - Codex : dans ~/.codex/config.toml, [mcp_servers.{2}] avec url = "{0}" et http_headers = { "Authorization" = "Bearer {1}" }
   - Cursor : dans ~/.cursor/mcp.json, "mcpServers": { "{2}": { "url": "{0}", "headers": { "Authorization": "Bearer {1}" } } }
   - Autre outil : un serveur MCP HTTP avec cette adresse et cet en-tête.
   Si un serveur « {2} » existe déjà, remplace-le.
2. Si les outils de POP n'apparaissent qu'après un redémarrage, dis-moi exactement quoi faire (par exemple ouvrir une nouvelle session).
3. Une fois connecté, appelle l'outil list_maps de POP et donne-moi la liste de mes maps.`, [url, token, name])
}

function PromptBlock({ ai }: { ai: AccountAI }) {
  const { toast } = useApp()
  const [kind, setKind] = useState('claude_code')
  const [text, setText] = useState('')
  const [copied, setCopied] = useState(false)
  const copy = async (t: string) => { try { await navigator.clipboard.writeText(t); setCopied(true) } catch { setCopied(false); toast(tr("Copie impossible : sélectionnez le texte."), true) } }
  const make = async () => {
    try {
      const c = CLIENTS.find(c => c.kind === kind)!
      // Un seul jeton en attente par IA : l'ancien, jamais utilisé, est remplacé.
      for (const t of ai.pending.filter(t => t.kind === kind && !t.oauth)) await api.del(`/api/tokens/${t.id}`)
      const r = await api.post('/api/me/tokens', { name: c.name, kind, scope: 'write' })
      const p = promptText(`${location.origin}/mcp`, r.token)
      setText(p); ai.reload(); await copy(p)
    } catch (e: any) { toast(e.message, true) }
  }
  return <div className="aiurl connectai-prompt">
    <b style={{ fontWeight: 500 }}>{tr("Le plus simple : donnez ce prompt à votre IA")}</b>
    <p className="small muted" style={{ margin: '2px 0 10px' }}>{tr("Il contient l’adresse de POP et une clé pour votre compte : votre IA s’ajoute POP toute seule. Pour les IA qui travaillent sur votre ordinateur (Claude Code, Codex, Cursor…).")}</p>
    <div className="aiquick" role="tablist">{PROMPT_KINDS.map(k => <button key={k} role="tab" aria-selected={k === kind} className={k === kind ? 'on' : ''} onClick={() => { setKind(k); setText(''); setCopied(false) }}><ClientLogo kind={k} size={16} />{CLIENTS.find(c => c.kind === k)!.name}</button>)}</div>
    {!text
      ? <button className="btn pri" onClick={make}><Copy size={15} />{tr("Créer la clé et copier le prompt")}</button>
      : <>
        <pre className="report mono connectai-ptext">{text}</pre>
        <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
          <button className="btn pri" onClick={() => copy(text)}>{copied ? <><Check size={15} />{tr("Prompt copié")}</> : <><Copy size={15} />{tr("Copier le prompt")}</>}</button>
          <span className="small muted">{tr("Collez-le dans votre IA. La clé ne sera plus affichée.")}</span>
        </div>
      </>}
  </div>
}
