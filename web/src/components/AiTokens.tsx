// IA connectées du compte (#145) : un jeton MCP par IA, lié au compte et pas à une map. L'IA choisit sa map de
// travail elle-même (outils list_maps puis use_map). Depuis #285, l'IA peut aussi se connecter seule par OAuth
// (on colle l'adresse /mcp, POP demande « Autoriser ») ; la clé API reste pour les autres.
import { useState } from 'react'
import { Check, Copy, Trash2, Lock, Unlock, ExternalLink, KeyRound } from 'lucide-react'
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
          <span style={{ flex: 1, minWidth: 180 }}>{t.name}<span className="small muted"> · {clientName(t.kind)} · {t.oauth ? tr("connexion OAuth") : tr("clé API")} · {t.map ? tr("travaille sur « {0} »", [t.map.name]) : tr("aucune map choisie")}</span></span>
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

/** « Connecter une IA » par onglet (#375) : une seule adresse MCP pour toutes les IA ; chaque onglet donne les étapes
 * de l'IA, vérifiées dans sa documentation officielle (novembre 2026), et ce qu'il faut copier. OAuth d'abord
 * (l'IA ouvre POP, on clique « Autoriser ») ; une clé API en secours pour les IA qui ne gèrent pas OAuth. */
type Tab = {
  id: string; kind: string; name: string
  steps: string[]
  /** Ce qu'on copie : l'adresse, une commande ou une configuration (OAuth, sans clé). */
  copy: (url: string) => string
  open?: { label: string; href: (url: string) => string }
  note?: string
  /** Même chose avec une clé API, pour les IA sans OAuth. */
  key?: (url: string, k: string) => string
  keyWhere?: string
}
const cfg = (v: any) => JSON.stringify(v, null, 2)
const bearer = (k: string) => `Bearer ${k}`
export const TABS: Tab[] = [
  { id: 'claude', kind: 'claude_desktop', name: 'Claude',
    steps: [tr("Sur claude.ai ou dans l’application Claude : Personnaliser → Connecteurs."), tr("Cliquez « Ajouter un connecteur personnalisé », collez l’adresse de POP, puis « Ajouter »."), tr("Cliquez « Connecter » : la page POP s’ouvre, cliquez « Autoriser ».")],
    copy: u => u, open: { label: tr("Ouvrir Claude"), href: () => 'https://claude.ai/customize/connectors' },
    note: tr("Offre gratuite : un seul connecteur personnalisé. Équipe et Entreprise : un propriétaire ajoute d’abord POP dans Paramètres de l’organisation → Connecteurs."),
    key: (u, k) => `${u}?token=${k}`, keyWhere: tr("Adresse à coller à la place (la clé est dedans) :") },
  { id: 'chatgpt', kind: 'chatgpt', name: 'ChatGPT',
    steps: [tr("Sur chatgpt.com (ordinateur), ouvrez Plugins (chatgpt.com/plugins)."), tr("Bouton + → « Add custom MCP server » : nom POP, collez l’adresse de POP sous « Connection », authentification OAuth."), tr("Cochez l’avertissement, « Create as a plugin », installez-le, puis cliquez « Autoriser » dans la page POP."), tr("Dans une conversation, tapez @ et choisissez POP.")],
    copy: u => u, open: { label: tr("Ouvrir ChatGPT"), href: () => 'https://chatgpt.com/plugins' },
    note: tr("Selon OpenAI : sur le web seulement ; outils en écriture avec les offres Business, Enterprise et Edu, lecture seule avec Pro.") },
  { id: 'gemini', kind: 'gemini', name: 'Gemini',
    steps: [tr("Sur gemini.google.com (ordinateur) : Paramètres → Connected Apps (ou Personal Intelligence → Connected Apps)."), tr("Sous « Custom apps », cliquez « Add a custom app » et collez l’adresse de POP, puis « Next »."), tr("Suivez les instructions : la page POP s’ouvre, cliquez « Autoriser ».")],
    copy: u => u, open: { label: tr("Ouvrir Gemini"), href: () => 'https://gemini.google.com' },
    note: tr("Selon Google, pour l’instant : 18 ans ou plus, aux États-Unis, compte Google personnel, Gemini en anglais, historique activé. Ensuite, POP marche aussi dans l’application mobile.") },
  { id: 'lechat', kind: 'lechat', name: 'Le Chat',
    steps: [tr("Dans Le Chat (Mistral) : panneau de gauche → Intelligence → Connecteurs."), tr("« + Add Connector », onglet « Custom MCP Connector » : nom pop, serveur : collez l’adresse de POP, authentification OAuth."), tr("Cliquez « Connect » : la page POP s’ouvre, cliquez « Autoriser ».")],
    copy: u => u, open: { label: tr("Ouvrir Le Chat"), href: () => 'https://chat.mistral.ai' },
    note: tr("Réservé aux administrateurs de l’espace (par défaut sur les offres Free, Pro et Student)."),
    key: (_u, k) => k, keyWhere: tr("Ou choisissez l’authentification « HTTP Bearer Token » et collez cette clé :") },
  { id: 'cursor', kind: 'cursor', name: 'Cursor',
    steps: [tr("Cliquez « Installer dans Cursor » et acceptez, ou ajoutez la configuration dans ~/.cursor/mcp.json."), tr("Dans Cursor, Réglages → MCP : cliquez « Connect » à côté de pop."), tr("La page POP s’ouvre : cliquez « Autoriser ».")],
    copy: u => cfg({ mcpServers: { pop: { url: u } } }),
    open: { label: tr("Installer dans Cursor"), href: u => `cursor://anysphere.cursor-deeplink/mcp/install?name=pop&config=${encodeURIComponent(btoa(JSON.stringify({ url: u })))}` },
    key: (u, k) => cfg({ mcpServers: { pop: { url: u, headers: { Authorization: bearer(k) } } } }), keyWhere: tr("Fichier ~/.cursor/mcp.json :") },
  { id: 'vscode', kind: 'vscode', name: 'VS Code',
    steps: [tr("Cliquez « Installer dans VS Code », ou ajoutez la configuration dans .vscode/mcp.json (commande « MCP: Add Server »)."), tr("Démarrez le serveur pop : VS Code ouvre la connexion dans le navigateur."), tr("La page POP s’ouvre : cliquez « Autoriser ». Les outils sont dans le mode Agent de Copilot.")],
    copy: u => cfg({ servers: { pop: { type: 'http', url: u } } }),
    open: { label: tr("Installer dans VS Code"), href: u => `vscode:mcp/install?${encodeURIComponent(JSON.stringify({ name: 'pop', type: 'http', url: u }))}` },
    key: (u, k) => cfg({ servers: { pop: { type: 'http', url: u, headers: { Authorization: bearer(k) } } } }), keyWhere: tr("Fichier .vscode/mcp.json :") },
  { id: 'windsurf', kind: 'windsurf', name: 'Windsurf',
    steps: [tr("Dans Cascade, menu MCP → « View raw config » (fichier ~/.codeium/windsurf/mcp_config.json) et collez la configuration."), tr("Enregistrez et cliquez « Refresh » : Windsurf ouvre la page POP."), tr("Cliquez « Autoriser ».")],
    copy: u => cfg({ mcpServers: { pop: { serverUrl: u } } }),
    key: (u, k) => cfg({ mcpServers: { pop: { serverUrl: u, headers: { Authorization: bearer(k) } } } }), keyWhere: tr("Fichier ~/.codeium/windsurf/mcp_config.json :") },
  { id: 'codex', kind: 'codex', name: 'Codex',
    steps: [tr("Dans un terminal, lancez la commande (elle ajoute pop à ~/.codex/config.toml, partagé par la CLI et l’extension)."), tr("Puis lancez : codex mcp login pop"), tr("La page POP s’ouvre : cliquez « Autoriser ».")],
    copy: u => `codex mcp add pop --url ${u}`,
    key: (u, k) => `[mcp_servers.pop]\nurl = "${u}"\nhttp_headers = { "Authorization" = "Bearer ${k}" }`, keyWhere: tr("Dans ~/.codex/config.toml :") },
  { id: 'claude_code', kind: 'claude_code', name: 'Claude Code',
    steps: [tr("Dans un terminal, lancez la commande."), tr("Lancez Claude Code et tapez /mcp, choisissez pop puis « Authenticate » (ou : claude mcp login pop)."), tr("La page POP s’ouvre : cliquez « Autoriser ».")],
    copy: u => `claude mcp add --transport http --scope user pop ${u}`,
    key: (u, k) => `claude mcp add --transport http --scope user pop ${u} --header "Authorization: Bearer ${k}"`, keyWhere: tr("Dans un terminal :") },
  { id: 'gemini_cli', kind: 'gemini_cli', name: 'Gemini CLI',
    steps: [tr("Dans un terminal, lancez la commande."), tr("Lancez gemini et tapez /mcp auth pop."), tr("La page POP s’ouvre : cliquez « Autoriser ».")],
    copy: u => `gemini mcp add --transport http --scope user pop ${u}`,
    key: (u, k) => `gemini mcp add --transport http --scope user pop ${u} --header "Authorization: Bearer ${k}"`, keyWhere: tr("Dans un terminal :") },
  { id: 'autre', kind: 'autre', name: tr("Autre client MCP"),
    steps: [tr("Ajoutez un serveur MCP distant « Streamable HTTP » (parfois « HTTP ») avec l’adresse de POP."), tr("Si votre IA gère la connexion OAuth de MCP, elle ouvre POP : cliquez « Autoriser »."), tr("Sinon, créez une clé : en-tête Authorization: Bearer, ou adresse avec ?token=. Anciens clients : transport SSE à l’adresse /sse.")],
    copy: u => u,
    key: (u, k) => tr("Adresse : {0}\nEn-tête : Authorization: Bearer {1}\nOu adresse avec la clé : {0}?token={1}\nAncien transport SSE : {2}?token={1}", [u, k, u.replace(/\/mcp$/, '/sse')]), keyWhere: tr("À copier dans votre client :") },
]

/** Onglets « Connecter une IA ». `onKey` crée une clé API pour l'onglet (secours sans OAuth). */
export function ConnectTabs({ url, onKey }: { url: string; onKey: (kind: string, name: string) => Promise<string | null> }) {
  const { toast } = useApp()
  const [sel, setSel] = useState('claude')
  const [copied, setCopied] = useState('')
  const [key, setKey] = useState<{ tab: string; k: string } | null>(null)
  const t = TABS.find(x => x.id === sel)!
  const copy = async (txt: string, what: string) => { try { await navigator.clipboard.writeText(txt); setCopied(what) } catch { toast(tr("Copie impossible : sélectionnez le texte."), true) } }
  const text = t.copy(url)
  const deep = t.open && t.open.href(url).match(/^(cursor|vscode):/)
  return <div className="aiurl aitabs">
    <div className="row" style={{ gap: 8, flexWrap: 'wrap', marginBottom: 12 }}>
      <b style={{ fontWeight: 500 }}>{tr("Adresse MCP de POP, la même pour toutes les IA :")}</b>
      <code className="aiurl-v" style={{ flex: 1, minWidth: 220 }}>{url}</code>
      <button className="btn" onClick={() => copy(url, 'url')}>{copied === 'url' ? <><Check size={15} />{tr("Copiée")}</> : <><Copy size={15} />{tr("Copier")}</>}</button>
    </div>
    <div className="aiquick" role="tablist" aria-label={tr("Choisissez votre IA")}>{TABS.map(x => <button key={x.id} role="tab" aria-selected={x.id === sel} className={x.id === sel ? 'on' : ''} onClick={() => { setSel(x.id); setCopied(''); setKey(null) }}><ClientLogo kind={x.kind} size={16} />{x.name}</button>)}</div>
    <div role="tabpanel" aria-label={t.name}>
      <ol className="aisteps small">{t.steps.map((s, i) => <li key={i}>{s}</li>)}</ol>
      {text !== url && <pre className="report mono" style={{ fontSize: 12, margin: '0 0 10px' }}>{text}</pre>}
      <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
        {t.open && deep && <a className="btn pri" href={t.open.href(url)}>{t.open.label}</a>}
        <button className={'btn' + (t.open && deep ? '' : ' pri')} onClick={() => copy(text, 'main')}>{copied === 'main' ? <><Check size={15} />{tr("Copié")}</> : <><Copy size={15} />{text === url ? tr("Copier l’adresse") : text.startsWith('{') ? tr("Copier la configuration") : tr("Copier la commande")}</>}</button>
        {t.open && !deep && <a className="btn" href={t.open.href(url)} target="_blank" rel="noopener noreferrer"><ExternalLink size={15} />{t.open.label}</a>}
      </div>
      {t.note && <p className="small muted" style={{ marginBottom: 0 }}>{t.note}</p>}
      {t.key && <div className="aikey">
        {!key || key.tab !== t.id
          ? <button className="linkbtn small" onClick={async () => { const k = await onKey(t.kind, t.name); if (k) setKey({ tab: t.id, k }) }}><KeyRound size={14} /> {tr("Pas de connexion OAuth ? Utiliser une clé API à la place")}</button>
          : <>
            <p className="small" style={{ margin: '10px 0 6px' }}>{t.keyWhere}</p>
            <pre className="report mono" style={{ fontSize: 12, margin: 0 }}>{t.key(url, key.k)}</pre>
            <div className="row" style={{ marginTop: 8, gap: 8 }}>
              <button className="btn" onClick={() => copy(t.key!(url, key.k), 'key')}>{copied === 'key' ? <><Check size={15} />{tr("Copié")}</> : <><Copy size={15} />{tr("Copier")}</>}</button>
              <span className="small muted">{tr("La clé ne sera plus affichée : copiez-la maintenant. Révocable dans « IA connectées ».")}</span>
            </div>
          </>}
      </div>}
    </div>
  </div>
}
