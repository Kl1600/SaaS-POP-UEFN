import { useEffect, useRef, useState } from 'react'
import { Hammer, X } from '../icons'
import { tr } from '../i18n/t'

// « Ouvrir dans Forge » (#297) : lien pop-forge://map/{id}[/tache/{n}], que Forge déclare à l'installation. Si la
// page garde la main quelques secondes (aucun logiciel n'a pris le lien), une bulle propose de télécharger Forge.
// Pour tous les comptes : Forge est gratuit en mode relais, ses écrans de construction restent Pro (#404, #416).
export function OuvrirForge({ mapId, tache, compact }: { mapId: number; tache?: number; compact?: boolean }) {
  const [bulle, setBulle] = useState(false)
  const minuteur = useRef<number>()
  useEffect(() => () => window.clearTimeout(minuteur.current), [])

  const ouvrir = () => {
    let parti = false
    const quitte = () => { parti = true }
    window.addEventListener('blur', quitte, { once: true })
    document.addEventListener('visibilitychange', quitte, { once: true })
    window.location.href = `pop-forge://map/${mapId}${tache ? `/tache/${tache}` : ''}`
    window.clearTimeout(minuteur.current)
    minuteur.current = window.setTimeout(() => {
      window.removeEventListener('blur', quitte)
      document.removeEventListener('visibilitychange', quitte)
      if (!parti && document.hasFocus()) setBulle(true)
    }, 3000)
  }
  return <span className="ouvrir-forge">
    <button className={compact ? 'iconbtn' : 'btn'} onClick={ouvrir} title={tr("Ouvrir dans Forge")} aria-label={tr("Ouvrir dans Forge")}>
      <Hammer size={compact ? 20 : 16} strokeWidth={1.5} />{!compact && tr("Ouvrir dans Forge")}
    </button>
    {bulle && <span className="ouvrir-forge-bulle" role="dialog" aria-label={tr("Forge ne s’est pas ouvert")}>
      <span>{tr("Forge ne s’est pas ouvert ? Il n’est peut-être pas installé sur ce PC.")}</span>
      <a className="btn pri" href="/forge" target="_blank" rel="noopener">{tr("Télécharger Forge")}</a>
      <button className="iconbtn" onClick={() => setBulle(false)} aria-label={tr("Fermer")}><X size={16} /></button>
    </span>}
  </span>
}
