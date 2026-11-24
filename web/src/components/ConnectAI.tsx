// « Connecte ton IA » au niveau du compte (#134) : depuis #145, une IA se connecte au compte et choisit sa map elle-même.
// - Pastille dans la barre du haut : aucune IA → « Connecter une IA » ; IA enregistrée (clé ou autorisation OAuth créée,
//   aucun appel encore) → « en attente de connexion » ; premier appel MCP reçu (last_used) → « connectée ».
// - Fenêtre ouverte à l'arrivée tant qu'aucune IA n'a appelé POP, sauf après « Passer » (gardé pour le compte).
// Reprend la connexion en un clic (#290) et les clés API (#280) de Paramètres → IA connectées.
import { useEffect, useRef, useState } from 'react'
import { Check, Copy, ArrowLeft, X } from 'lucide-react'
import { api } from '../api'
import { useApp } from '../state'
import { tr } from '../i18n/t'
import { CLIENTS, ClientLogo, KeyPanel, OneClick, type Client } from './AiTokens'
import { Modal } from './ui'

export type AccountAI = { tokens: any[] | null; live: any[]; pending: any[]; reload: () => void }

/** IA du compte, relues toutes les 3 s tant qu'on attend une connexion, toutes les 30 s sinon, et au retour sur l'onglet. */
export function useAccountAI(waiting: boolean): AccountAI {
  const [tokens, setTokens] = useState<any[] | null>(null)
  const reload = async () => { try { setTokens(await api.get<any[]>('/api/me/tokens')) } catch {} }
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

/** Pastille d'état de l'IA, à côté de celle d'UEFN Link. */
export function AiPill({ ai, onOpen, onSettings }: { ai: AccountAI; onOpen: () => void; onSettings: () => void }) {
  if (!ai.tokens) return null
  if (ai.live.length) {
    const kinds = [...new Set(ai.live.map(t => t.kind))].slice(0, 3)
    return <button className="upill connected" onClick={onSettings} title={ai.live.map(t => t.name).join(', ')}>
      <span className="udot" />{kinds.map(k => <ClientLogo key={k} kind={k} size={16} />)}
      {ai.live.length === 1 ? tr("{0} connectée", [ai.live[0].name]) : tr("{0} IA connectées", [ai.live.length])}
    </button>
  }
  if (ai.pending.length) {
    const t = ai.pending[ai.pending.length - 1]
    return <button className="upill closed" onClick={onOpen} title={tr("Enregistrée dans POP : elle sera connectée à son premier appel.")}>
      <span className="aiwait-dot" /><ClientLogo kind={t.kind} size={16} />{tr("{0} · en attente de connexion", [t.name])}
    </button>
  }
  return <button className="upill unlinked" onClick={onOpen}><span className="udot" />{tr("Connecter une IA")}</button>
}

const KEY_KINDS = ['claude_desktop', 'claude_code', 'cursor', 'codex', 'autre']

/** Fenêtre « Connecte ton IA ». Passe seule à « IA connectée » dès le premier appel MCP d'une nouvelle IA. */
export function ConnectAIModal({ ai, onClose, onSkip }: { ai: AccountAI; onClose: () => void; onSkip?: () => void }) {
  const { map, toast } = useApp()
  const [key, setKey] = useState<{ c: Client; token: string; id: number } | null>(null)
  const [copied, setCopied] = useState(false)
  // IA déjà connectées à l'ouverture : seule une nouvelle connexion fait passer à la suite.
  const before = useRef<Set<number> | null>(null)
  if (before.current === null && ai.tokens) before.current = new Set(ai.live.map(t => t.id))
  const fresh = before.current ? ai.live.find(t => !before.current!.has(t.id)) : undefined
  useEffect(() => {
    if (!fresh) return
    const h = setTimeout(onClose, 4000)
    return () => clearTimeout(h)
  }, [fresh?.id])
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
    <h2 className="connectai-t">{title}</h2>
    <button className="iconbtn" aria-label={tr("Fermer")} onClick={onClose}><X size={18} /></button>
  </div>

  return <Modal onClose={onClose} label={tr("Connecte ton IA")}>
    <div className="connectai">
      {fresh ? <>
        {head(tr("IA connectée"))}
        <div className="aiwait ok connectai-ok" role="status"><ClientLogo kind={fresh.kind} size={18} />{tr("{0} est connectée à POP.", [fresh.name])}</div>
        <p className="small">{tr("Demandez-lui maintenant :")} <b style={{ fontWeight: 500 }}>{ask}</b></p>
        <div className="row"><button className="btn pri" onClick={onClose}>{tr("Continuer")}</button></div>
      </> : key ? <>
        {head(tr("Connecter {0}", [key.c.name]))}
        <KeyPanel c={key.c} token={key.token} linked={false} />
        <button className="linkbtn" onClick={() => setKey(null)}><ArrowLeft size={14} /> {tr("Choisir une autre IA")}</button>
      </> : <>
        {head(tr("Connecte ton IA"))}
        <p className="muted connectai-sub">{tr("POP se pilote avec votre IA (Claude, Cursor, Codex…) : elle voit vos maps, prend les tâches et agit dans UEFN. Connectez-la une fois pour tout votre compte.")}</p>
        <OneClick url={`${location.origin}/mcp`} />
        <div className={'aiwait' + (waiting ? ' wait' : '')} role="status">
          <span className="aiwait-dot" />
          {waiting ? tr("{0} est enregistrée, en attente de connexion… Envoyez-lui un message pour qu’elle appelle POP.", [waiting.name]) : tr("En attente de votre IA…")}
        </div>
        <div className="connectai-ask">
          <span className="small">{tr("Une fois connectée, demandez-lui :")}</span>
          <code className="aiurl-v">{ask}</code>
          <button className="btn" onClick={copyAsk}>{copied ? <><Check size={15} />{tr("Copié")}</> : <><Copy size={15} />{tr("Copier")}</>}</button>
        </div>
        <p className="small muted" style={{ margin: '18px 0 8px' }}>{tr("Votre IA ne propose pas de se connecter ? Copiez sa configuration avec une clé API :")}</p>
        <div className="aigrid">
          {CLIENTS.filter(c => KEY_KINDS.includes(c.kind)).map(c => <button key={c.kind} className="aicard" onClick={() => createKey(c)}>
            <ClientLogo kind={c.kind} size={20} /><b style={{ fontWeight: 500, flex: 1 }}>{c.name}</b><span className="small">{tr("Clé API")}</span>
          </button>)}
        </div>
        {onSkip && <p style={{ margin: '18px 0 0' }}><button className="linkbtn" onClick={onSkip}>{tr("Passer, je connecterai mon IA plus tard")}</button></p>}
      </>}
    </div>
  </Modal>
}
