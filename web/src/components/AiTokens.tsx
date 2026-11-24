// IA connectées du compte (#145) : un jeton MCP par IA, lié au compte et pas à une map. L'IA choisit sa map de
// travail elle-même (outils list_maps puis use_map). Depuis #285, l'IA peut aussi se connecter seule par OAuth
// (on colle l'adresse /mcp, POP demande « Autoriser ») ; la clé API reste pour les autres.
import { useEffect, useState } from 'react'
import { Check, Copy, Trash2, Lock, Unlock, X } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLoad } from '../state'
import { CardHead, Modal } from './ui'
import { tr } from '../i18n/t'
import { AiLogo } from './AiLogo'

export const ClientLogo = ({ kind, size }: { kind: string; size?: number }) => <AiLogo kind={kind} size={size} />

type Client = { kind: string; name: string; where: string; config: (url: string, token: string) => string; note?: string }

const json = (v: any) => JSON.stringify(v, null, 2)
export const CLIENTS: Client[] = [
  { kind: 'claude_code', name: tr("Claude Code"), where: tr("Dans un terminal, dans le dossier de votre projet :"), config: (u, t) => tr("claude mcp add --transport http pop {0} --header \"Authorization: Bearer {1}\"", [u, t]) },
  { kind: 'claude_desktop', name: tr("Claude (application)"), where: tr("Paramètres → Connecteurs → Ajouter un connecteur personnalisé, puis collez cette adresse :"), config: (u, t) => `${u}?token=${t}`, note: tr("Demande que POP soit en ligne en HTTPS (l’application ne joint pas votre ordinateur local).") },
  { kind: 'cursor', name: tr("Cursor"), where: tr("Fichier ~/.cursor/mcp.json (ou .cursor/mcp.json dans le projet) :"), config: (u, t) => json({ mcpServers: { pop: { url: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'codex', name: tr("Codex (OpenAI)"), where: tr("Fichier ~/.codex/config.toml :"), config: (u, t) => tr("[mcp_servers.pop]\nurl = \"{0}\"\nhttp_headers = { \"Authorization\" = \"Bearer {1}\" }", [u, t]) },
  { kind: 'windsurf', name: tr("Windsurf"), where: tr("Fichier ~/.codeium/windsurf/mcp_config.json :"), config: (u, t) => json({ mcpServers: { pop: { serverUrl: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'vscode', name: 'VS Code (Copilot)', where: tr("Fichier .vscode/mcp.json dans le projet :"), config: (u, t) => json({ servers: { pop: { type: 'http', url: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'gemini', name: tr("Gemini CLI"), where: tr("Fichier ~/.gemini/settings.json :"), config: (u, t) => json({ mcpServers: { pop: { httpUrl: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'autre', name: tr("Autre IA (MCP)"), where: tr("Serveur MCP « Streamable HTTP » :"), config: (u, t) => tr("Adresse : {0}\nEn-tête : Authorization: Bearer {1}\n(ou adresse avec le jeton : {2}?token={3})", [u, t, u, t]) },
]

export const clientName = (kind: string) => CLIENTS.find(c => c.kind === kind)?.name ?? tr("Autre")

/** Paramètres du compte → IA connectées : connecter, lister, passer en lecture seule, révoquer. */
export function AccountAI() {
  const { toast } = useApp()
  const { data, reload } = useLoad<any[]>('/api/me/tokens', [])
  const [open, setOpen] = useState<{ c: Client; token: string; id: number } | null>(null)
  const [copied, setCopied] = useState(false)
  const url = `${location.origin}/mcp`
  // Une IA ne compte comme connectée qu'une fois qu'elle a appelé POP (#280) ; les autres jetons sont « en attente ».
  const live = (data ?? []).filter(t => t.last_used)
  const pending = (data ?? []).filter(t => !t.last_used)
  const connect = async (c: Client) => {
    try {
      // Un seul jeton en attente par IA : l'ancien, jamais utilisé, est remplacé.
      for (const t of pending.filter(t => t.kind === c.kind)) await api.del(`/api/tokens/${t.id}`)
      const used = live.filter(t => t.kind === c.kind).length
      const r = await api.post('/api/me/tokens', { name: used ? `${c.name} ${used + 1}` : c.name, kind: c.kind, scope: 'write' })
      setOpen({ c, token: r.token, id: r.id }); setCopied(false); reload()
    } catch (e: any) { toast(e.message, true) }
  }
  const linked = !!open && live.some(t => t.id === open.id)
  // Pendant que la fenêtre est ouverte, on regarde toutes les 3 s si l'IA a appelé POP.
  useEffect(() => {
    if (!open || linked) return
    const h = setInterval(reload, 3000)
    return () => clearInterval(h)
  }, [open, linked])
  const copy = async (t: string) => { try { await navigator.clipboard.writeText(t); setCopied(true) } catch { toast(tr("Copie impossible : sélectionnez le texte."), true) } }
  return (<>
    <section className="card">
      <CardHead title={tr("IA connectées")} sub={tr("Connectez votre IA (Claude, Cursor, Codex…) une seule fois pour tout votre compte : elle voit vos maps perso et celles de vos studios, et choisit elle-même la map sur laquelle travailler.")} />
      <OneClick url={url} />
      <p className="small muted" style={{ margin: '14px 0 8px' }}>{tr("Votre IA ne propose pas de se connecter ? Créez une clé API pour elle :")}</p>
      <div className="aigrid">
        {CLIENTS.map(c => {
          const n = live.filter(t => t.kind === c.kind && !t.oauth).length
          return <button key={c.kind} className="aicard" onClick={() => connect(c)}>
            <ClientLogo kind={c.kind} size={20} /><span style={{ flex: 1, textAlign: 'left' }}><b style={{ fontWeight: 500 }}>{c.name}</b>{n > 0 && <span className="small muted">{' '}{tr("· {0} connectée{1}", [n, n > 1 ? 's' : ''])}</span>}</span><span className="small">{tr("Clé API")}</span>
          </button>
        })}
      </div>
      <h3 style={{ fontWeight: 500, fontSize: 15, margin: '20px 0 8px' }}>{tr("Vos IA")}</h3>
      <p className="small muted" style={{ marginTop: 0 }}>{tr("Lecture seule : l’IA voit vos maps sans rien modifier, sur toutes les maps. Les suppressions d’acteurs attendent toujours votre validation dans Paramètres → UEFN Link.")}</p>
      {data && live.length === 0 && <p className="muted">{tr("Aucune IA connectée pour l’instant.")}</p>}
      <div className="list">{live.map(t => (
        <div key={t.id} className="li" style={{ flexWrap: 'wrap' }}>
          <ClientLogo kind={t.kind} />
          <span style={{ flex: 1, minWidth: 180 }}>{t.name}<span className="small muted"> · {t.oauth ? tr("autorisée depuis l’IA") : clientName(t.kind)} · {t.map ? tr("travaille sur « {0} »", [t.map.name]) : tr("aucune map choisie")}</span></span>
          <span className={'badge ' + (t.last_used ? 'b-g' : '')}>{t.last_used ? tr("Utilisée {0} · {1} appel{2}", [ago(t.last_used), t.calls, t.calls > 1 ? 's' : '']) : tr("Jamais utilisée")}</span>
          <button className="btn" style={{ height: 34 }} onClick={async () => { await api.post(`/api/tokens/${t.id}/scope`, { scope: t.scope === 'read' ? 'write' : 'read' }); reload() }}>{t.scope === 'read' ? <><Lock size={14} />{tr("Lecture seule")}</> : <><Unlock size={14} />{tr("Peut modifier")}</>}</button>
          <button className="iconbtn" aria-label={tr("Déconnecter {0}", [t.name])} title={tr("Déconnecter")} onClick={async () => { if (confirm(tr("Déconnecter {0} ? Son jeton ne marchera plus.", [t.name]))) { await api.del(`/api/tokens/${t.id}`); reload() } }}><Trash2 size={16} /></button>
        </div>
      ))}</div>
      {pending.length > 0 && <p className="small muted" style={{ marginBottom: 0 }}>
        {tr("En attente de connexion : {0}.", [pending.map(t => clientName(t.kind)).join(', ')])}{' '}
        <button className="linkbtn" onClick={async () => { for (const t of pending) await api.del(`/api/tokens/${t.id}`); reload() }}>{tr("Annuler")}</button>
      </p>}
    </section>

    {open && <Modal onClose={() => setOpen(null)} label={tr("Connecter {0}", [open.c.name])}>
      <div className="row" style={{ justifyContent: 'space-between' }}><h2 style={{ margin: 0, fontWeight: 500, fontSize: 20 }}>{tr("Connecter {0}", [open.c.name])}</h2><button className="iconbtn" aria-label={tr("Fermer")} onClick={() => setOpen(null)}><X size={18} /></button></div>
      <p className="small" style={{ margin: '12px 0 6px' }}>{open.c.where}</p>
      <pre className="report mono" style={{ fontSize: 12 }}>{open.c.config(url, open.token)}</pre>
      <div className="row" style={{ marginTop: 10 }}>
        <button className="btn pri" onClick={() => copy(open.c.config(url, open.token))}>{copied ? <><Check size={16} />{tr("Copié")}</> : <><Copy size={16} />{tr("Copier")}</>}</button>
        <span className="small muted">{tr("Le jeton ne sera plus affiché : copiez-le maintenant.")}</span>
      </div>
      {open.c.note && <p className="small muted">{open.c.note}</p>}
      <div className={'aiwait' + (linked ? ' ok' : '')}>{linked ? <><Check size={16} />{tr("{0} est connectée à POP.", [open.c.name])}</> : <><span className="aiwait-dot" />{tr("En attente de connexion… L’IA apparaîtra dans « Vos IA » dès son premier appel.")}</>}</div>
      <p className="small muted">{tr("Redémarrez l’IA puis demandez-lui par exemple : « Travaille sur ma map POP GrassCutter ». Elle liste vos maps (list_maps) et choisit la bonne (use_map) ; vous pourrez lui faire changer de map sans la reconnecter.")}</p>
    </Modal>}
  </>)
}

/** Connexion en un clic par IA (#290) : l'IA ne connaît que l'adresse /mcp ; POP demande ensuite « Autoriser ». */
type Quick = { id: string; kind: string; name: string; steps: string[]; action: string; run: (url: string) => string | void }
const QUICK: Quick[] = [
  { id: 'claude', kind: 'claude_desktop', name: 'Claude', action: tr("Copier et ouvrir Claude"),
    steps: [tr("Dans Claude : Paramètres → Connecteurs → « Ajouter un connecteur personnalisé »."), tr("Nom : POP, adresse : collez-la (elle est copiée), puis « Ajouter » et « Connecter »."), tr("Dans la page POP qui s’ouvre, cliquez « Autoriser ». Activez POP avec le bouton + sous le message.")],
    run: () => { window.open('https://claude.ai/settings/connectors', '_blank', 'noopener') } },
  { id: 'chatgpt', kind: 'codex', name: 'ChatGPT', action: tr("Copier l’adresse"),
    steps: [tr("Dans ChatGPT : Paramètres → Applications et connecteurs → Avancé : activez le mode développeur."), tr("« Créer » un connecteur : nom POP, adresse : collez-la, authentification OAuth."), tr("Dans la page POP qui s’ouvre, cliquez « Autoriser ».")],
    run: () => {} },
  { id: 'cursor', kind: 'cursor', name: 'Cursor', action: tr("Installer dans Cursor"),
    steps: [tr("Cursor s’ouvre et propose d’installer POP : acceptez."), tr("Dans Cursor (Réglages → MCP), cliquez « Connect » à côté de POP."), tr("Dans la page POP qui s’ouvre, cliquez « Autoriser ».")],
    run: u => `cursor://anysphere.cursor-deeplink/mcp/install?name=POP&config=${encodeURIComponent(btoa(JSON.stringify({ url: u })))}` },
  { id: 'vscode', kind: 'vscode', name: 'VS Code', action: tr("Installer dans VS Code"),
    steps: [tr("VS Code s’ouvre et propose d’installer POP : cliquez « Install »."), tr("Au premier usage, VS Code demande de vous connecter : acceptez."), tr("Dans la page POP qui s’ouvre, cliquez « Autoriser ».")],
    run: u => `vscode:mcp/install?${encodeURIComponent(JSON.stringify({ name: 'pop', type: 'http', url: u }))}` },
  { id: 'cc', kind: 'claude_code', name: 'Claude Code', action: tr("Copier la commande"),
    steps: [tr("Collez la commande dans un terminal, dans le dossier de votre projet."), tr("Lancez Claude Code, tapez /mcp, choisissez pop puis « Authenticate »."), tr("Dans la page POP qui s’ouvre, cliquez « Autoriser ».")],
    run: () => {} },
  { id: 'autre', kind: 'autre', name: tr("Autre IA"), action: tr("Copier l’adresse"),
    steps: [tr("Ajoutez un serveur MCP « HTTP » (ou « Streamable HTTP ») avec cette adresse."), tr("Si l’IA propose de se connecter, acceptez et cliquez « Autoriser » dans POP."), tr("Sinon, créez une clé API ci-dessous.")],
    run: () => {} },
]

function OneClick({ url }: { url: string }) {
  const { toast } = useApp()
  const [sel, setSel] = useState('claude')
  const q = QUICK.find(x => x.id === sel)!
  const copyText = q.id === 'cc' ? `claude mcp add --transport http pop ${url}` : url
  const go = async () => {
    if (q.id === 'cursor' || q.id === 'vscode') { location.href = q.run(url) as string; return }
    // On copie d'abord : ouvrir un onglet ferait perdre le focus, et la copie échouerait.
    try { await navigator.clipboard.writeText(copyText); toast(q.id === 'cc' ? tr("Commande copiée") : tr("Adresse copiée")) } catch { toast(tr("Copie impossible : sélectionnez le texte."), true) }
    q.run(url)
  }
  return <div className="aiurl">
    <b style={{ fontWeight: 500 }}>{tr("Connecter une IA en un clic")}</b>
    <p className="small muted" style={{ margin: '2px 0 10px' }}>{tr("Choisissez votre IA : elle ouvre POP, vous cliquez sur « Autoriser », c’est connecté. Aucune clé à copier.")}</p>
    <div className="aiquick" role="tablist">{QUICK.map(x => <button key={x.id} role="tab" aria-selected={x.id === sel} className={x.id === sel ? 'on' : ''} onClick={() => setSel(x.id)}><ClientLogo kind={x.kind} size={16} />{x.name}</button>)}</div>
    <ol className="aisteps small">{q.steps.map((t, i) => <li key={i}>{t}</li>)}</ol>
    <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
      <code className="aiurl-v" style={{ flex: 1, minWidth: 200 }}>{copyText}</code>
      <button className="btn pri" onClick={go}>{q.id === 'cursor' || q.id === 'vscode' ? null : <Copy size={15} />}{q.action}</button>
    </div>
  </div>
}
