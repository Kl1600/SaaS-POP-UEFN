import { useCallback, useEffect, useState } from 'react'
import { CircleCheck, CircleX, CircleDashed, Copy, RefreshCw, X } from '../icons'
import { useApp } from '../state'
import { tr } from '../i18n/t'

// Assistant de premier lancement de Forge (#299), d'après la maquette « Liaison à UEFN » : il vérifie le poste, règle
// ce qu'il peut avec l'accord de l'utilisateur (démarrage automatique du serveur MCP) et dit quoi faire pour le reste.
type Invoke = <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T>
const FAIT = 'pop-forge-assistant-fait'
const MCP = tr("dans UEFN : Édition → Préférences de l'éditeur → Agentic Workflows → Model Context Protocol → Start Server")

type Etape = { id: string; titre: string; ok: boolean | null; aide: string; action?: [string, () => void] }

export function ForgeAssistant({ invoke, liens, etat }: {
  invoke: Invoke; liens: { projet: string; map: string }[]; etat: string
}) {
  const app = useApp()
  const [b, setB] = useState<any>(null)
  const [occupe, setOccupe] = useState(false)
  const [ferme, setFerme] = useState(() => { try { return localStorage.getItem(FAIT) === '1' } catch { return false } })
  const verifier = useCallback(async () => {
    setOccupe(true)
    try { setB(await invoke('bilan_poste')) } catch (e: any) { setB({ erreur: String(e) }) }
    setOccupe(false)
  }, [invoke])
  useEffect(() => { if (!ferme) verifier() }, [ferme, verifier])

  const ouvert: string | null = b?.projet_ouvert ?? null
  const relie = !!ouvert && liens.some(l => l.projet.toLowerCase() === ouvert.toLowerCase())
  // Tant que ce n'est pas fini, on revérifie toutes les 5 s (l'utilisateur ouvre UEFN, démarre le serveur…).
  useEffect(() => {
    if (ferme || relie) return
    const t = setInterval(verifier, 5000); return () => clearInterval(t)
  }, [ferme, relie, verifier])
  if (ferme || !b) return null

  const activer = async () => {
    if (!confirm(tr("Forge va régler UEFN pour qu'il démarre son serveur MCP tout seul à chaque lancement. Une copie du fichier de réglages est gardée. Continuer ?"))) return
    try { await invoke('activer_mcp_auto'); app.toast(tr("Démarrage automatique activé : il servira au prochain lancement d'UEFN.")); verifier() }
    catch (e: any) { app.toast(String(e), true) }
  }
  const mcpOff = !b.mcp
  const etapes: Etape[] = [
    { id: 'windows', titre: tr("Windows"), ok: !!b.windows, aide: tr("UEFN et Forge ne fonctionnent que sous Windows.") },
    { id: 'compte', titre: tr("Compte POP et abonnement Pro"), ok: !!app.me?.forge, aide: tr("Forge est inclus dans l'abonnement Pro.") },
    { id: 'uefn', titre: tr("UEFN installé"), ok: !!b.uefn || !!b.uefn_ouvert, aide: tr("Installez Fortnite depuis l'Epic Games Launcher : UEFN est dans la bibliothèque du launcher (« Unreal Editor for Fortnite »).") },
    { id: 'projet', titre: tr("Projet ouvert dans UEFN"), ok: !!ouvert, aide: tr("Ouvrez votre projet dans UEFN. Forge le trouve tout seul.") },
    { id: 'mcp', titre: tr("Serveur MCP d'UEFN démarré"), ok: !!b.mcp, aide: tr("Démarrez-le {0}.", [MCP]) },
    { id: 'auto', titre: tr("Démarrage automatique du serveur MCP"), ok: b.demarrage_auto ?? null,
      // Réglage absent des fichiers d'UEFN = valeur par défaut, active (#528).
      aide: b.demarrage_auto === false ? tr("Il est désactivé : sans lui, il faut démarrer le serveur à la main à chaque lancement.") : b.demarrage_auto_defaut ? tr("Actif (réglage par défaut d'UEFN).") : tr("Actif."),
      action: b.demarrage_auto === false ? [tr("Activer"), activer] : undefined },
    { id: 'toolsets', titre: tr("Réglage du projet : UEFN MCP Toolsets"), ok: mcpOff ? null : (b.toolsets ?? 0) > 0, aide: tr("Activez les toolsets MCP d'UEFN dans les paramètres du projet, puis redémarrez le serveur MCP.") },
    { id: 'python', titre: tr("Réglage du projet : Python Editor Scripting"), ok: mcpOff ? null : b.python ?? null, aide: tr("Activez « Enable Project Python Scripts » dans les paramètres du projet, puis redémarrez UEFN.") },
    { id: 'map', titre: tr("Projet relié à une map POP"), ok: relie, aide: ouvert ? tr("Choisissez sa map ci-dessous et cliquez Relier, ou « Créer la map dans POP ».") : tr("Ouvrez d'abord le projet dans UEFN.") },
  ]
  const fini = etapes.every(e => e.ok !== false)
  const diagnostic = () => {
    const txt = [`POP Forge · ${new Date().toLocaleString('fr-FR')}`, `${tr("Relais")} : ${etat}`,
      ...etapes.map(e => `${e.ok === true ? '✓' : e.ok === false ? '✗' : '?'} ${e.titre}${e.ok === false ? ' : ' + e.aide : ''}`),
      `${tr("Projet")} : ${ouvert ?? '—'}`, `UEFN : ${b.uefn ?? '—'}`, `${tr("Réglages UEFN")} : ${b.reglages_uefn ?? '—'}`].join('\n')
    navigator.clipboard.writeText(txt).then(() => app.toast(tr("Diagnostic copié")), () => app.toast(tr("Copie impossible"), true))
  }
  const terminer = () => { try { localStorage.setItem(FAIT, '1') } catch { /* stockage indisponible */ } setFerme(true) }

  return <section className="card stack forge-assistant" style={{ padding: 20, gap: 12 }} aria-label={tr("Assistant de premier lancement")}>
    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <h2 style={{ margin: 0, fontWeight: 500, fontSize: 17, flex: 1 }}>{fini ? tr("Tout est prêt") : tr("Liaison à UEFN")}</h2>
      <button className="btn" onClick={verifier} disabled={occupe}><RefreshCw size={15} strokeWidth={1.5} className={occupe ? 'spin' : ''} />{tr("Revérifier")}</button>
      <button className="btn" onClick={diagnostic}><Copy size={15} strokeWidth={1.5} />{tr("Copier le diagnostic")}</button>
      <button className="iconbtn" onClick={terminer} aria-label={tr("Fermer l'assistant")} title={tr("Fermer l'assistant")}><X size={18} /></button>
    </div>
    <ol className="assistant-etapes">
      {etapes.map(e => <li key={e.id} className={e.ok === false ? 'non' : ''}>
        {e.ok === true ? <CircleCheck size={19} strokeWidth={1.5} className="verif-ok" aria-label={tr("Fait")} />
          : e.ok === false ? <CircleX size={19} strokeWidth={1.5} className="verif-bloq" aria-label={tr("À faire")} />
          : <CircleDashed size={19} strokeWidth={1.5} className="muted" aria-label={tr("Pas encore vérifiable")} />}
        <div style={{ flex: 1 }}>
          <div>{e.titre}</div>
          {e.ok === false && <small className="muted">{e.aide}</small>}
          {(e.ok === null || (e.ok === true && b.demarrage_auto_defaut)) && e.id === 'auto' && <small className="muted">{e.aide}</small>}
        </div>
        {e.action && <button className="btn pri" onClick={e.action[1]}>{e.action[0]}</button>}
      </li>)}
    </ol>
    {fini && <div><button className="btn pri" onClick={terminer}>{tr("Terminer")}</button></div>}
  </section>
}
