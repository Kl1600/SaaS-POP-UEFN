// IA connectées du compte (#145) : un jeton MCP par IA, lié au compte et pas à une map. L'IA choisit sa map de
// travail elle-même (outils list_maps puis use_map). Depuis #285, l'IA peut aussi se connecter seule par OAuth
// (on colle l'adresse /mcp, POP demande « Autoriser ») ; la clé API reste pour les autres.
import { useState } from 'react'
import { Check, Copy, Trash2, Lock, Unlock, ExternalLink, KeyRound, ChevronDown } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead } from './ui'
import { tr } from '../i18n/t'
import { AiLogo } from './AiLogo'
import { ConnectAiButton, useAccountAI } from './ConnectAI'

export const ClientLogo = ({ kind, size }: { kind: string; size?: number }) => <AiLogo kind={kind} size={size} />

export type Client = { kind: string; name: string; where: string; config: (url: string, token: string) => string; note?: string }

const json = (v: any) => JSON.stringify(v, null, 2)
export const CLIENTS: Client[] = [
  { kind: 'claude_code', name: tr("Claude Code"), where: tr("Dans un terminal, dans le dossier de votre projet :"), config: (u, t) => tr("claude mcp add --transport http pop {0} --header \"Authorization: Bearer {1}\"", [u, t]) },
  { kind: 'claude_desktop', name: 'Claude', where: tr("Paramètres → Connecteurs → Ajouter un connecteur personnalisé, puis collez cette adresse :"), config: (u, t) => `${u}?token=${t}`, note: tr("Demande que POP soit en ligne en HTTPS (l’application ne joint pas votre ordinateur local).") },
  { kind: 'cursor', name: tr("Cursor"), where: tr("Fichier ~/.cursor/mcp.json (ou .cursor/mcp.json dans le projet) :"), config: (u, t) => json({ mcpServers: { pop: { url: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'codex', name: tr("Codex (OpenAI)"), where: tr("Fichier ~/.codex/config.toml :"), config: (u, t) => tr("[mcp_servers.pop]\nurl = \"{0}\"\nhttp_headers = { \"Authorization\" = \"Bearer {1}\" }", [u, t]) },
  { kind: 'windsurf', name: tr("Windsurf"), where: tr("Fichier ~/.codeium/windsurf/mcp_config.json :"), config: (u, t) => json({ mcpServers: { pop: { serverUrl: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'vscode', name: 'VS Code (Copilot)', where: tr("Fichier .vscode/mcp.json dans le projet :"), config: (u, t) => json({ servers: { pop: { type: 'http', url: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'gemini_cli', name: tr("Gemini CLI"), where: tr("Fichier ~/.gemini/settings.json :"), config: (u, t) => json({ mcpServers: { pop: { url: u, type: 'http', headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'gemini', name: 'Gemini', where: tr("Adresse à coller dans Gemini (la clé est dedans) :"), config: (u, t) => `${u}?token=${t}` },
  { kind: 'chatgpt', name: 'ChatGPT', where: tr("Adresse à coller dans ChatGPT (la clé est dedans) :"), config: (u, t) => `${u}?token=${t}` },
  { kind: 'lechat', name: 'Le Chat', where: tr("Clé à coller dans Le Chat (authentification « HTTP Bearer Token ») :"), config: (_u, t) => t },
  { kind: 'autre', name: tr("Autre IA (MCP)"), where: tr("Serveur MCP « Streamable HTTP » :"), config: (u, t) => tr("Adresse : {0}\nEn-tête : Authorization: Bearer {1}\n(ou adresse avec le jeton : {2}?token={3})", [u, t, u, t]) },
]

export const clientName = (kind: string) => CLIENTS.find(c => c.kind === kind)?.name ?? tr("Autre")

/** Paramètres du compte → IA connectées : lister, rôle, lecture seule, déconnecter ; connecter une IA comme sur
 *  l'accueil (« Mes IA », #134 : prompt à coller d'abord, en un clic et clé API repliés). */
export function AccountAI() {
  const ai = useAccountAI(false)
  const data = ai.tokens, reload = ai.reload
  // Rôles d'agents (#111) : consignes et outils permis, choisis par IA.
  const { data: roles } = useLoad<{ id: string; name: string; instructions: string }[]>('/api/agent-roles', [])
  // Une IA ne compte comme connectée qu'une fois qu'elle a appelé POP (#280) ; les autres jetons sont « en attente ».
  const live = ai.live
  const pending = ai.pending
  return (<>
    <section className="card">
      <CardHead title={tr("IA connectées")} sub={tr("Connectez votre IA (Claude, Cursor, Codex…) une seule fois pour tout votre compte : elle voit vos maps perso et celles de vos studios, et choisit elle-même la map sur laquelle travailler.")} />
      <h3 style={{ fontWeight: 500, fontSize: 15, margin: '4px 0 8px' }}>{tr("Vos IA")}</h3>
      <p className="small muted" style={{ marginTop: 0 }}>{tr("Lecture seule : l’IA voit vos maps sans rien modifier, sur toutes les maps. Les suppressions d’acteurs attendent toujours votre validation dans Paramètres → UEFN Link.")}</p>
      {data && live.length === 0 && <p className="muted">{tr("Aucune IA connectée pour l’instant.")}</p>}
      <div className="list">{live.map(t => (
        <div key={t.id} className="li" style={{ flexWrap: 'wrap' }}>
          <ClientLogo kind={t.kind} />
          <span style={{ flex: 1, minWidth: 180 }}>{t.name}<span className="small muted"> · {clientName(t.kind)} · {t.oauth ? tr("connexion OAuth") : tr("clé API")} · {t.map ? tr("travaille sur « {0} »", [t.map.name]) : tr("aucune map choisie")}{ai.agentsForge(t) ? ' · ' + tr("{0} agent(s) dans Forge", [ai.agentsForge(t)]) : ''}</span></span>
          <span className={'badge ' + (t.last_used ? 'b-g' : '')}>{t.last_used ? tr("Utilisée {0} · {1} appel{2}", [ago(t.last_used), t.calls, t.calls > 1 ? 's' : '']) : tr("Jamais utilisée")}</span>
          <select className="field" style={{ height: 34, width: 'auto' }} aria-label={tr("Rôle de {0}", [t.name])} title={roles?.find(r => r.id === t.role)?.instructions ?? tr("Sans rôle : tous les outils de POP.")} value={t.role ?? ''} onChange={async e => { await api.post(`/api/tokens/${t.id}/role`, { role: e.target.value || null }); reload() }}>
            <option value="">{tr("Sans rôle")}</option>
            {roles?.map(r => <option key={r.id} value={r.id}>{tr(r.name)}</option>)}
          </select>
          <button className="btn" style={{ height: 34 }} onClick={async () => { await api.post(`/api/tokens/${t.id}/scope`, { scope: t.scope === 'read' ? 'write' : 'read' }); reload() }}>{t.scope === 'read' ? <><Lock size={14} />{tr("Lecture seule")}</> : <><Unlock size={14} />{tr("Peut modifier")}</>}</button>
          <button className="btn" style={{ height: 34 }} aria-label={tr("Révoquer {0}", [t.name])} onClick={async () => { if (confirm(tr("Révoquer {0} ? Son accès à POP est coupé tout de suite ; pour la reconnecter, refaites la connexion.", [t.name]))) { await api.del(`/api/tokens/${t.id}`); reload() } }}><Trash2 size={14} />{tr("Révoquer")}</button>
        </div>
      ))}</div>
      {pending.length > 0 && <p className="small muted" style={{ marginBottom: 0 }}>
        {tr("En attente de connexion : {0}.", [pending.map(t => clientName(t.kind)).join(', ')])}{' '}
        <button className="linkbtn" onClick={async () => { for (const t of pending) await api.del(`/api/tokens/${t.id}`); reload() }}>{tr("Annuler")}</button>
      </p>}
      <div style={{ marginTop: 16 }}><ConnectAiButton /></div>
    </section>
  </>)
}

/** « Connecter une IA » en 3 étapes (#375, simplifié #379) : on choisit son IA, puis une phrase et un bouton par
 * étape : copier, ouvrir l'IA ou coller, cliquer « Autoriser » dans POP. Étapes vérifiées dans la doc officielle de
 * chaque IA (novembre 2026). La clé API reste en secours, repliée. */
type Tab = {
  id: string; kind: string; name: string
  /** Étape 1 : ce qu'on copie (adresse, commande ou configuration), ou un lien d'installation. */
  copy: (url: string) => string
  install?: { label: string; href: (url: string) => string }
  /** Étape 2 : où le coller, en une phrase ; bouton facultatif pour ouvrir l'IA. */
  s2: string
  open?: { label: string; href: string }
  help?: string
  /** Secours sans OAuth : la même chose avec une clé API. */
  key?: (url: string, k: string) => string
}
const cfg = (v: any) => JSON.stringify(v, null, 2)
const bearer = (k: string) => `Bearer ${k}`
const MAIN = ['claude', 'chatgpt', 'gemini', 'lechat']
export const TABS: Tab[] = [
  { id: 'claude', kind: 'claude_desktop', name: 'Claude', copy: u => u,
    s2: tr("Dans Claude, cliquez « Ajouter un connecteur personnalisé » et collez l’adresse."), open: { label: tr("Ouvrir Claude"), href: 'https://claude.ai/customize/connectors' },
    help: tr("Le bouton ouvre la page des connecteurs de Claude. Sinon : Paramètres → Connecteurs. Offre gratuite : un seul connecteur personnalisé."),
    key: (u, k) => `${u}?token=${k}` },
  { id: 'chatgpt', kind: 'chatgpt', name: 'ChatGPT', copy: u => u,
    s2: tr("Dans ChatGPT, cliquez + puis « Add custom MCP server » et collez l’adresse."), open: { label: tr("Ouvrir ChatGPT"), href: 'https://chatgpt.com/plugins' },
    help: tr("Sur ordinateur (chatgpt.com). Selon OpenAI : écriture avec Business, Enterprise et Edu, lecture seule avec Pro."),
    key: (u, k) => `${u}?token=${k}` },
  { id: 'gemini', kind: 'gemini', name: 'Gemini', copy: u => u,
    s2: tr("Dans Gemini, Paramètres → Connected Apps → « Add a custom app », collez l’adresse."), open: { label: tr("Ouvrir Gemini"), href: 'https://gemini.google.com' },
    help: tr("Selon Google, pour l’instant : 18 ans ou plus, aux États-Unis, compte personnel, Gemini en anglais.") },
  { id: 'lechat', kind: 'lechat', name: 'Le Chat', copy: u => u,
    s2: tr("Dans Le Chat, Connecteurs → « + Add Connector » → « Custom MCP Connector », collez l’adresse."), open: { label: tr("Ouvrir Le Chat"), href: 'https://chat.mistral.ai' },
    help: tr("Réservé à l’administrateur de l’espace (vous, sur les offres Free, Pro et Student)."),
    key: (_u, k) => k },
  { id: 'cursor', kind: 'cursor', name: 'Cursor', copy: u => cfg({ mcpServers: { pop: { url: u } } }),
    install: { label: tr("Installer dans Cursor"), href: u => `cursor://anysphere.cursor-deeplink/mcp/install?name=pop&config=${encodeURIComponent(btoa(JSON.stringify({ url: u })))}` },
    s2: tr("Dans Cursor, Réglages → MCP : cliquez « Connect » à côté de pop."),
    key: (u, k) => cfg({ mcpServers: { pop: { url: u, headers: { Authorization: bearer(k) } } } }) },
  { id: 'vscode', kind: 'vscode', name: 'VS Code', copy: u => cfg({ servers: { pop: { type: 'http', url: u } } }),
    install: { label: tr("Installer dans VS Code"), href: u => `vscode:mcp/install?${encodeURIComponent(JSON.stringify({ name: 'pop', type: 'http', url: u }))}` },
    s2: tr("Dans VS Code, démarrez le serveur pop (mode Agent de Copilot)."),
    key: (u, k) => cfg({ servers: { pop: { type: 'http', url: u, headers: { Authorization: bearer(k) } } } }) },
  { id: 'windsurf', kind: 'windsurf', name: 'Windsurf', copy: u => cfg({ mcpServers: { pop: { serverUrl: u } } }),
    s2: tr("Dans Windsurf : Cascade → MCP → « View raw config », collez, puis « Refresh »."),
    key: (u, k) => cfg({ mcpServers: { pop: { serverUrl: u, headers: { Authorization: bearer(k) } } } }) },
  { id: 'codex', kind: 'codex', name: 'Codex', copy: u => `codex mcp add pop --url ${u}`,
    s2: tr("Collez la commande dans un terminal (sinon ensuite : codex mcp login pop)."),
    key: (u, k) => `[mcp_servers.pop]\nurl = "${u}"\nhttp_headers = { "Authorization" = "Bearer ${k}" }` },
  { id: 'claude_code', kind: 'claude_code', name: 'Claude Code', copy: u => `claude mcp add --transport http --scope user pop ${u}`,
    s2: tr("Collez la commande dans un terminal, puis tapez /mcp dans Claude Code et choisissez pop."),
    key: (u, k) => `claude mcp add --transport http --scope user pop ${u} --header "Authorization: Bearer ${k}"` },
  { id: 'gemini_cli', kind: 'gemini_cli', name: 'Gemini CLI', copy: u => `gemini mcp add --transport http --scope user pop ${u}`,
    s2: tr("Collez la commande dans un terminal, puis tapez /mcp auth pop dans Gemini."),
    key: (u, k) => `gemini mcp add --transport http --scope user pop ${u} --header "Authorization: Bearer ${k}"` },
  { id: 'autre', kind: 'autre', name: tr("Autre IA"), copy: u => u,
    s2: tr("Dans votre IA, ajoutez un serveur MCP « HTTP » avec cette adresse."),
    help: tr("Anciens clients : transport SSE à l’adresse /sse."),
    key: (u, k) => tr("Adresse : {0}\nEn-tête : Authorization: Bearer {1}\nOu adresse avec la clé : {0}?token={1}", [u, k]) },
]

/** Choix de l'IA puis 3 étapes. `linked` : l'IA est connectée (l'étape 3 passe au vert). */
export function ConnectTabs({ url, onKey, linked }: { url: string; onKey: (kind: string, name: string) => Promise<string | null>; linked?: boolean }) {
  const { toast } = useApp()
  const [sel, setSel] = useState('claude')
  const [more, setMore] = useState(false)
  const [copied, setCopied] = useState('')
  const [key, setKey] = useState<{ tab: string; k: string } | null>(null)
  const t = TABS.find(x => x.id === sel)!
  const copy = async (txt: string, what: string) => { try { await navigator.clipboard.writeText(txt); setCopied(what) } catch { toast(tr("Copie impossible : sélectionnez le texte."), true) } }
  const text = t.copy(url)
  const what = text === url ? tr("l’adresse") : text.startsWith('{') ? tr("la configuration") : tr("la commande")
  const pick = (id: string) => { setSel(id); setCopied(''); setKey(null) }
  const btn = (x: Tab, big: boolean) => <button key={x.id} role="tab" aria-selected={x.id === sel} className={(big ? 'aipick' : '') + (x.id === sel ? ' on' : '')} onClick={() => pick(x.id)}><ClientLogo kind={x.kind} size={big ? 22 : 16} />{x.name}</button>
  return <div className="aisteps3">
    <div className="aipicks" role="tablist" aria-label={tr("Votre IA")}>
      {TABS.filter(x => MAIN.includes(x.id)).map(x => btn(x, true))}
      <button className={'aipick' + (!MAIN.includes(sel) ? ' on' : '')} aria-expanded={more} onClick={() => setMore(m => !m)}>{tr("Autres IA")}<ChevronDown size={16} /></button>
    </div>
    {(more || !MAIN.includes(sel)) && <div className="aiquick" role="tablist">{TABS.filter(x => !MAIN.includes(x.id)).map(x => btn(x, false))}</div>}
    <ol className="aistep">
      <li><span className="aistep-n">1</span><div>
        <b>{t.install ? tr("Installez POP dans {0}", [t.name]) : tr("Copiez {0}", [what])}</b>
        {text !== url && !t.install && <pre className="report mono">{text}</pre>}
        {text === url && <code className="aiurl-v">{url}</code>}
        <div className="row" style={{ gap: 8, marginTop: 8, flexWrap: 'wrap' }}>
          {t.install && <a className="btn pri" href={t.install.href(url)}>{t.install.label}</a>}
          <button className={'btn' + (t.install ? '' : ' pri')} onClick={() => copy(text, 'main')}>{copied === 'main' ? <><Check size={15} />{tr("Copié")}</> : <><Copy size={15} />{t.install ? tr("ou copier la configuration") : tr("Copier")}</>}</button>
        </div>
      </div></li>
      <li><span className="aistep-n">2</span><div>
        <b>{t.s2}</b>
        {t.open && <div style={{ marginTop: 8 }}><a className="btn" href={t.open.href} target="_blank" rel="noopener noreferrer"><ExternalLink size={15} />{t.open.label}</a></div>}
      </div></li>
      <li className={linked ? 'ok' : ''}><span className="aistep-n">{linked ? <Check size={16} /> : 3}</span><div>
        <b>{tr("Cliquez « Autoriser » sur la page POP qui s’ouvre.")}</b>
        <div className={'aiwait' + (linked ? ' ok' : '')} role="status">{linked ? <><Check size={16} />{tr("Connectée !")}</> : <><span className="aiwait-dot" />{tr("En attente de votre IA…")}</>}</div>
      </div></li>
    </ol>
    {(t.help || t.key) && <details className="connectai-more">
      <summary className="small">{tr("Besoin d’aide ?")}</summary>
      {t.help && <p className="small muted">{t.help}</p>}
      {t.key && (!key || key.tab !== t.id
        ? <button className="linkbtn small" onClick={async () => { const k = await onKey(t.kind, t.name); if (k) setKey({ tab: t.id, k }) }}><KeyRound size={14} /> {tr("Ça ne marche pas ? Utiliser une clé API à la place")}</button>
        : <>
          <pre className="report mono" style={{ fontSize: 12 }}>{t.key(url, key.k)}</pre>
          <div className="row" style={{ gap: 8 }}>
            <button className="btn" onClick={() => copy(t.key!(url, key.k), 'key')}>{copied === 'key' ? <><Check size={15} />{tr("Copié")}</> : <><Copy size={15} />{tr("Copier")}</>}</button>
            <span className="small muted">{tr("La clé ne sera plus affichée. Révocable dans « IA connectées ».")}</span>
          </div>
        </>)}
    </details>}
  </div>
}
