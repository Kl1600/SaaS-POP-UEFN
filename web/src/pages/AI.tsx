import { useState } from 'react'
import { Bot, Check, Copy, Trash2, Lock, Unlock, X } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead, PageHead } from '../components/ui'

type Client = { kind: string; name: string; where: string; config: (url: string, token: string) => string; note?: string }

const json = (v: any) => JSON.stringify(v, null, 2)
const CLIENTS: Client[] = [
  { kind: 'claude_code', name: 'Claude Code', where: 'Dans un terminal, dans le dossier de votre projet :', config: (u, t) => `claude mcp add --transport http pop ${u} --header "Authorization: Bearer ${t}"` },
  { kind: 'claude_desktop', name: 'Claude (application)', where: 'Paramètres → Connecteurs → Ajouter un connecteur personnalisé, puis collez cette adresse :', config: (u, t) => `${u}?token=${t}`, note: 'Demande que POP soit en ligne en HTTPS (l’application ne joint pas votre ordinateur local).' },
  { kind: 'cursor', name: 'Cursor', where: 'Fichier ~/.cursor/mcp.json (ou .cursor/mcp.json dans le projet) :', config: (u, t) => json({ mcpServers: { pop: { url: u, headers: { Authorization: `Bearer ${t}` } } } }) },
  { kind: 'codex', name: 'Codex (OpenAI)', where: 'Fichier ~/.codex/config.toml :', config: (u, t) => `[mcp_servers.pop]\nurl = "${u}"\nhttp_headers = { "Authorization" = "Bearer ${t}" }` },
  { kind: 'windsurf', name: 'Windsurf', where: 'Fichier ~/.codeium/windsurf/mcp_config.json :', config: (u, t) => json({ mcpServers: { pop: { serverUrl: u, headers: { Authorization: `Bearer ${t}` } } } }) },
  { kind: 'vscode', name: 'VS Code (Copilot)', where: 'Fichier .vscode/mcp.json dans le projet :', config: (u, t) => json({ servers: { pop: { type: 'http', url: u, headers: { Authorization: `Bearer ${t}` } } } }) },
  { kind: 'gemini', name: 'Gemini CLI', where: 'Fichier ~/.gemini/settings.json :', config: (u, t) => json({ mcpServers: { pop: { httpUrl: u, headers: { Authorization: `Bearer ${t}` } } } }) },
  { kind: 'autre', name: 'Autre IA (MCP)', where: 'Serveur MCP « Streamable HTTP » :', config: (u, t) => `Adresse : ${u}\nEn-tête : Authorization: Bearer ${t}\n(ou adresse avec le jeton : ${u}?token=${t})` },
]

export default function AI() {
  const { map, canEdit, toast } = useApp()
  const { data, reload } = useLoad<any[]>(map ? `/api/maps/${map.id}/tokens` : null, [map?.id])
  useLive(['ai'], reload)
  const [open, setOpen] = useState<{ c: Client; token: string } | null>(null)
  const [copied, setCopied] = useState(false)
  const url = `${location.origin}/mcp`
  const connect = async (c: Client) => {
    const used = (data ?? []).filter(t => t.kind === c.kind).length
    try {
      const r = await api.post(`/api/maps/${map!.id}/tokens`, { name: used ? `${c.name} ${used + 1}` : c.name, kind: c.kind, scope: 'write' })
      setOpen({ c, token: r.token }); setCopied(false); reload()
    } catch (e: any) { toast(e.message, true) }
  }
  const copy = async (t: string) => { try { await navigator.clipboard.writeText(t); setCopied(true) } catch { toast('Copie impossible : sélectionnez le texte.', true) } }
  return (
    <main className="content">
      <PageHead title="IA connectées" help="Branchez une ou plusieurs IA (Claude, Cursor, Codex…) sur cette map : elles lisent et modifient le Verse et les acteurs UEFN comme vous, avec un journal et l’annulation." />
      <section className="card">
        <CardHead title="Connecter une IA" sub={`Chaque IA reçoit son propre jeton, lié à la map « ${map?.name} ». Vous pouvez en connecter autant que vous voulez.`} />
        <div className="aigrid">
          {CLIENTS.map(c => {
            const n = (data ?? []).filter(t => t.kind === c.kind).length
            return <button key={c.kind} className="aicard" disabled={!canEdit} onClick={() => connect(c)}>
              <Bot size={20} /><span style={{ flex: 1, textAlign: 'left' }}><b style={{ fontWeight: 500 }}>{c.name}</b>{n > 0 && <span className="small muted"> · {n} connectée{n > 1 ? 's' : ''}</span>}</span><span className="small">Connecter</span>
            </button>
          })}
        </div>
      </section>

      <section className="card">
        <CardHead title="Vos IA" sub="Lecture seule : l’IA voit le projet sans rien modifier. Les suppressions d’acteurs attendent toujours votre validation dans l’Organiseur." />
        {data?.length === 0 && <p className="muted">Aucune IA connectée pour l’instant.</p>}
        <div className="list">{data?.map(t => (
          <div key={t.id} className="li" style={{ flexWrap: 'wrap' }}>
            <Bot size={18} />
            <span style={{ flex: 1, minWidth: 180 }}>{t.name}<span className="small muted"> · {CLIENTS.find(c => c.kind === t.kind)?.name ?? 'Autre'}</span></span>
            <span className={'badge ' + (t.last_used ? 'b-g' : '')}>{t.last_used ? `Utilisée ${ago(t.last_used)} · ${t.calls} appel${t.calls > 1 ? 's' : ''}` : 'Jamais utilisée'}</span>
            <button className="btn" style={{ height: 34 }} onClick={async () => { await api.post(`/api/tokens/${t.id}/scope`, { scope: t.scope === 'read' ? 'write' : 'read' }); reload() }}>{t.scope === 'read' ? <><Lock size={14} />Lecture seule</> : <><Unlock size={14} />Peut modifier</>}</button>
            <button className="iconbtn" aria-label={`Déconnecter ${t.name}`} title="Déconnecter" onClick={async () => { if (confirm(`Déconnecter ${t.name} ? Son jeton ne marchera plus.`)) { await api.del(`/api/tokens/${t.id}`); reload() } }}><Trash2 size={16} /></button>
          </div>
        ))}</div>
      </section>

      <section className="card">
        <CardHead title="Ce que vos IA peuvent faire" />
        <p className="small muted" style={{ margin: 0, lineHeight: 1.7 }}>État d’UEFN · lister, lire, modifier, dupliquer les acteurs · demander une suppression (validée par un humain) · lire et écrire les fichiers Verse · compiler · pousser en session de test · enregistrer le niveau · archiver dans Lore · annuler une action. Tout apparaît dans le journal de l’Organiseur avec la mention « IA ».</p>
      </section>

      {open && <>
        <div className="scrim" onClick={() => setOpen(null)} />
        <div className="modal" role="dialog" aria-label={`Connecter ${open.c.name}`}>
          <div className="row" style={{ justifyContent: 'space-between' }}><h2 style={{ margin: 0, fontWeight: 500, fontSize: 20 }}>Connecter {open.c.name}</h2><button className="iconbtn" aria-label="Fermer" onClick={() => setOpen(null)}><X size={18} /></button></div>
          <p className="small" style={{ margin: '12px 0 6px' }}>{open.c.where}</p>
          <pre className="report mono" style={{ fontSize: 12 }}>{open.c.config(url, open.token)}</pre>
          <div className="row" style={{ marginTop: 10 }}>
            <button className="btn pri" onClick={() => copy(open.c.config(url, open.token))}>{copied ? <><Check size={16} />Copié</> : <><Copy size={16} />Copier</>}</button>
            <span className="small muted">Le jeton ne sera plus affiché : copiez-le maintenant.</span>
          </div>
          {open.c.note && <p className="small muted">{open.c.note}</p>}
          <p className="small muted">Redémarrez l’IA puis demandez-lui par exemple : « Liste les acteurs de ma map POP ». Cette page indiquera qu’elle est utilisée.</p>
        </div>
      </>}
    </main>
  )
}
