import { useState } from 'react'
import { Bot, Check, Copy, Trash2, Lock, Unlock, X } from 'lucide-react'
import { ago, api } from '../api'
import { useApp, useLive, useLoad } from '../state'
import { CardHead, Modal, PageHead } from '../components/ui'
import { tr } from '../i18n/t'
import { Brand } from '../components/Brand'

const LOGO: Record<string, string> = { claude_code: 'claude', claude_desktop: 'claude', cursor: 'cursor', windsurf: 'windsurf', vscode: 'copilot', gemini: 'gemini' }

type Client = { kind: string; name: string; where: string; config: (url: string, token: string) => string; note?: string }

const json = (v: any) => JSON.stringify(v, null, 2)
const CLIENTS: Client[] = [
  { kind: 'claude_code', name: tr("Claude Code"), where: tr("Dans un terminal, dans le dossier de votre projet :"), config: (u, t) => tr("claude mcp add --transport http pop {0} --header \"Authorization: Bearer {1}\"", [u, t]) },
  { kind: 'claude_desktop', name: tr("Claude (application)"), where: tr("Paramètres → Connecteurs → Ajouter un connecteur personnalisé, puis collez cette adresse :"), config: (u, t) => `${u}?token=${t}`, note: tr("Demande que POP soit en ligne en HTTPS (l’application ne joint pas votre ordinateur local).") },
  { kind: 'cursor', name: tr("Cursor"), where: tr("Fichier ~/.cursor/mcp.json (ou .cursor/mcp.json dans le projet) :"), config: (u, t) => json({ mcpServers: { pop: { url: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'codex', name: tr("Codex (OpenAI)"), where: tr("Fichier ~/.codex/config.toml :"), config: (u, t) => tr("[mcp_servers.pop]\nurl = \"{0}\"\nhttp_headers = { \"Authorization\" = \"Bearer {1}\" }", [u, t]) },
  { kind: 'windsurf', name: tr("Windsurf"), where: tr("Fichier ~/.codeium/windsurf/mcp_config.json :"), config: (u, t) => json({ mcpServers: { pop: { serverUrl: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'vscode', name: 'VS Code (Copilot)', where: tr("Fichier .vscode/mcp.json dans le projet :"), config: (u, t) => json({ servers: { pop: { type: 'http', url: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'gemini', name: tr("Gemini CLI"), where: tr("Fichier ~/.gemini/settings.json :"), config: (u, t) => json({ mcpServers: { pop: { httpUrl: u, headers: { Authorization: tr("Bearer {0}", [t]) } } } }) },
  { kind: 'autre', name: tr("Autre IA (MCP)"), where: tr("Serveur MCP « Streamable HTTP » :"), config: (u, t) => tr("Adresse : {0}\nEn-tête : Authorization: Bearer {1}\n(ou adresse avec le jeton : {2}?token={3})", [u, t, u, t]) },
]

/** IA connectées : dans Paramètres → IA connectées (#205), pour la map ouverte. */
export default function AI({ embedded }: { embedded?: boolean } = {}) {
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
  const copy = async (t: string) => { try { await navigator.clipboard.writeText(t); setCopied(true) } catch { toast(tr("Copie impossible : sélectionnez le texte."), true) } }
  return (
    <Wrap embedded={embedded}>
      {!embedded && <PageHead title={tr("IA connectées")} help={tr("Branchez une ou plusieurs IA (Claude, Cursor, Codex…) sur cette map : elles lisent et modifient le Verse et les acteurs UEFN comme vous, avec un journal et l’annulation.")} />}
      <section className="card">
        <CardHead title={tr("Connecter une IA")} sub={tr("Chaque IA reçoit son propre jeton, lié à la map « {0} ». Vous pouvez en connecter autant que vous voulez.", [map?.name])} />
        <div className="aigrid">
          {CLIENTS.map(c => {
            const n = (data ?? []).filter(t => t.kind === c.kind).length
            return <button key={c.kind} className="aicard" disabled={!canEdit} onClick={() => connect(c)}>
              <Brand name={LOGO[c.kind] ?? ''} size={20} /><span style={{ flex: 1, textAlign: 'left' }}><b style={{ fontWeight: 500 }}>{c.name}</b>{n > 0 && <span className="small muted">{' '}{tr("· {0} connectée{1}", [n, n > 1 ? 's' : ''])}</span>}</span><span className="small">{tr("Connecter")}</span>
            </button>
          })}
        </div>
      </section>

      <section className="card">
        <CardHead title={tr("Vos IA")} sub={tr("Lecture seule : l’IA voit le projet sans rien modifier. Les suppressions d’acteurs attendent toujours votre validation dans Paramètres → UEFN Link.")} />
        {data?.length === 0 && <p className="muted">{tr("Aucune IA connectée pour l’instant.")}</p>}
        <div className="list">{data?.map(t => (
          <div key={t.id} className="li" style={{ flexWrap: 'wrap' }}>
            <Brand name={LOGO[t.kind] ?? ''} />
            <span style={{ flex: 1, minWidth: 180 }}>{t.name}<span className="small muted"> · {CLIENTS.find(c => c.kind === t.kind)?.name ?? tr("Autre")}</span></span>
            <span className={'badge ' + (t.last_used ? 'b-g' : '')}>{t.last_used ? tr("Utilisée {0} · {1} appel{2}", [ago(t.last_used), t.calls, t.calls > 1 ? 's' : '']) : tr("Jamais utilisée")}</span>
            <button className="btn" style={{ height: 34 }} onClick={async () => { await api.post(`/api/tokens/${t.id}/scope`, { scope: t.scope === 'read' ? 'write' : 'read' }); reload() }}>{t.scope === 'read' ? <><Lock size={14} />{tr("Lecture seule")}</> : <><Unlock size={14} />{tr("Peut modifier")}</>}</button>
            <button className="iconbtn" aria-label={tr("Déconnecter {0}", [t.name])} title={tr("Déconnecter")} onClick={async () => { if (confirm(tr("Déconnecter {0} ? Son jeton ne marchera plus.", [t.name]))) { await api.del(`/api/tokens/${t.id}`); reload() } }}><Trash2 size={16} /></button>
          </div>
        ))}</div>
      </section>

      <section className="card">
        <CardHead title={tr("Ce que vos IA peuvent faire")} />
        <p className="small muted" style={{ margin: 0, lineHeight: 1.7 }}>{tr("État d’UEFN · lister, lire, modifier, dupliquer les acteurs · demander une suppression (validée par un humain) · lire et écrire les fichiers Verse · compiler · pousser en session de test · enregistrer le niveau · archiver dans Lore · annuler une action. Tout apparaît dans le journal d’UEFN Link (Paramètres) avec la mention « IA ».")}</p>
      </section>

      {open && <>
        <Modal onClose={() => setOpen(null)} label={tr("Connecter {0}", [open.c.name])}>
          <div className="row" style={{ justifyContent: 'space-between' }}><h2 style={{ margin: 0, fontWeight: 500, fontSize: 20 }}>{tr("Connecter {0}", [open.c.name])}</h2><button className="iconbtn" aria-label={tr("Fermer")} onClick={() => setOpen(null)}><X size={18} /></button></div>
          <p className="small" style={{ margin: '12px 0 6px' }}>{open.c.where}</p>
          <pre className="report mono" style={{ fontSize: 12 }}>{open.c.config(url, open.token)}</pre>
          <div className="row" style={{ marginTop: 10 }}>
            <button className="btn pri" onClick={() => copy(open.c.config(url, open.token))}>{copied ? <><Check size={16} />{tr("Copié")}</> : <><Copy size={16} />{tr("Copier")}</>}</button>
            <span className="small muted">{tr("Le jeton ne sera plus affiché : copiez-le maintenant.")}</span>
          </div>
          {open.c.note && <p className="small muted">{open.c.note}</p>}
          <p className="small muted">{tr("Redémarrez l’IA puis demandez-lui par exemple : « Liste les acteurs de ma map POP ». Cette page indiquera qu’elle est utilisée.")}</p>
        </Modal>
      </>}
    </Wrap>
  )
}

const Wrap = ({ embedded, children }: { embedded?: boolean; children: React.ReactNode }) => embedded ? <>{children}</> : <main className="content">{children}</main>
