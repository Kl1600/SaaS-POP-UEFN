// Dictée vocale locale dans Forge (#356) : garder le raccourci appuyé (Ctrl+Espace par défaut) dans un champ de texte
// pour dicter ; Forge transcrit sur le PC (whisper.cpp) et le texte s'ajoute au champ, modifiable avant l'envoi.
// Le modèle (547 Mio) n'est téléchargé qu'après l'accord de l'utilisateur.
import { useEffect, useRef, useState } from 'react'
import { Mic } from 'lucide-react'
import { tr, LANG } from '../i18n/t'
import { Layer, Modal } from './ui'
import { useApp } from '../state'

type Invoke = <T = any>(cmd: string, args?: Record<string, unknown>) => Promise<T>
type Etat = { modele: boolean; taille: number; recu: number | null }

export const RACCOURCIS = { 'ctrl-espace': 'Ctrl+Espace', 'alt-espace': 'Alt+Espace', 'f9': 'F9' } as const
export type Raccourci = keyof typeof RACCOURCIS
const CLE = 'pop-forge-dictee-touche'
export const raccourci = (): Raccourci => { try { const v = localStorage.getItem(CLE) as Raccourci; return v in RACCOURCIS ? v : 'ctrl-espace' } catch { return 'ctrl-espace' } }
export const choisirRaccourci = (r: Raccourci) => { try { localStorage.setItem(CLE, r) } catch { /* rien */ } }

const appuye = (e: KeyboardEvent, r: Raccourci) =>
  r === 'f9' ? e.key === 'F9' : e.code === 'Space' && (r === 'ctrl-espace' ? e.ctrlKey : e.altKey)
const relache = (e: KeyboardEvent, r: Raccourci) =>
  r === 'f9' ? e.key === 'F9' : e.code === 'Space' || (r === 'ctrl-espace' ? e.key === 'Control' : e.key === 'Alt')

type Champ = HTMLInputElement | HTMLTextAreaElement
const champDeTexte = (el: Element | null): el is Champ =>
  !!el && (el instanceof HTMLTextAreaElement || (el instanceof HTMLInputElement && ['text', 'search', ''].includes(el.type))) && !el.readOnly && !el.disabled

/** Ajoute le texte dicté à l'emplacement du curseur, de façon que React voie le changement. */
export function inserer(el: Champ, texte: string) {
  const a = el.selectionStart ?? el.value.length, b = el.selectionEnd ?? a
  const avant = el.value.slice(0, a), apres = el.value.slice(b)
  const t = (avant && !/\s$/.test(avant) ? ' ' : '') + texte
  const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype
  Object.getOwnPropertyDescriptor(proto, 'value')!.set!.call(el, avant + t + apres)
  el.dispatchEvent(new Event('input', { bubbles: true }))
  el.selectionStart = el.selectionEnd = a + t.length
  el.focus()
}

const mio = (o: number) => Math.round(o / 1048576)

export function ForgeDictee({ invoke }: { invoke: Invoke | null }) {
  const app = useApp()
  const [phase, setPhase] = useState<'' | 'ecoute' | 'ecrit'>('')
  const [proposer, setProposer] = useState<Etat | null>(null)
  const [recu, setRecu] = useState<number | null>(null)
  const cible = useRef<Champ | null>(null)
  const actif = useRef(false)

  useEffect(() => {
    if (!invoke) return
    const bas = async (e: KeyboardEvent) => {
      if (actif.current || e.repeat || !appuye(e, raccourci())) return
      const el = document.activeElement
      if (!champDeTexte(el)) return
      e.preventDefault()
      actif.current = true
      cible.current = el
      try {
        const etat = await invoke<Etat>('dictee_etat')
        if (!etat.modele) { actif.current = false; setProposer(etat); if (etat.recu != null) suivre(); return }
        await invoke('dictee_debut')
        setPhase('ecoute')
      } catch (er: any) {
        actif.current = false
        app.toast(/not allowed|not found|unknown/i.test(String(er)) ? tr("Dictée indisponible : mettez Forge à jour depuis pop-uefn.com/forge.") : String(er), true)
      }
    }
    const haut = async (e: KeyboardEvent) => {
      if (!actif.current || !relache(e, raccourci())) return
      e.preventDefault()
      actif.current = false
      setPhase('ecrit')
      try {
        const texte = await invoke<string>('dictee_fin', { langue: LANG })
        if (texte && cible.current) inserer(cible.current, texte)
        else if (!texte) app.toast(tr("Rien entendu : gardez le raccourci appuyé pendant que vous parlez."))
      } catch (er: any) { app.toast(String(er), true) } finally { setPhase('') }
    }
    window.addEventListener('keydown', bas, true)
    window.addEventListener('keyup', haut, true)
    return () => { window.removeEventListener('keydown', bas, true); window.removeEventListener('keyup', haut, true) }
  }, [invoke])

  const suivre = () => {
    const t = setInterval(async () => {
      const e = await invoke!<Etat>('dictee_etat').catch(() => null)
      if (!e || e.recu == null) { clearInterval(t); setRecu(null); if (e?.modele) { setProposer(null); app.toast(tr("Dictée prête : gardez {0} appuyé dans un champ pour dicter.", [RACCOURCIS[raccourci()]])) } return }
      setRecu(e.recu)
    }, 700)
  }
  const telecharger = async () => {
    setRecu(0); suivre()
    try { await invoke!('dictee_telecharger') } catch (er: any) { app.toast(String(er), true); setRecu(null) }
  }

  return <>
    {phase && <Layer><div className="forge-dictee" role="status" aria-live="polite">
      <Mic size={16} strokeWidth={1.6} className={phase === 'ecoute' ? 'forge-dictee-micro' : ''} />
      {phase === 'ecoute' ? tr("Je vous écoute… relâchez pour écrire") : tr("Transcription sur votre PC…")}
    </div></Layer>}
    {proposer && <Layer><Modal label={tr("Dictée vocale")} onClose={() => recu == null && setProposer(null)}>
      <h2 style={{ marginTop: 0 }}>{tr("Dictée vocale")}</h2>
      <p>{tr("La dictée transcrit votre voix sur votre PC : aucun son ne quitte l'ordinateur. Elle a besoin d'un modèle de reconnaissance vocale à télécharger une seule fois.")}</p>
      <p className="small muted">{tr("Modèle Whisper large-v3-turbo (whisper.cpp, licence MIT) · {0} Mio", [mio(proposer.taille)])}</p>
      {recu != null ? <div role="status">
        <progress max={proposer.taille} value={recu} style={{ width: '100%' }} />
        <p className="small">{tr("Téléchargement : {0} / {1} Mio", [mio(recu), mio(proposer.taille)])}</p>
      </div> : <div className="row" style={{ justifyContent: 'flex-end', gap: 8 }}>
        <button className="btn" onClick={() => setProposer(null)}>{tr("Pas maintenant")}</button>
        <button className="btn pri" onClick={telecharger}>{tr("Télécharger ({0} Mio)", [mio(proposer.taille)])}</button>
      </div>}
    </Modal></Layer>}
  </>
}
