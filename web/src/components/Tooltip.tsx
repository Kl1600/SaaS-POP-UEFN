// Infobulles de POP (#192) : un seul style pour tous les boutons à icône. S'affiche au survol (après un court
// délai) et au focus clavier, au-dessus de l'élément (ou en dessous s'il n'y a pas la place), sans sortir de l'écran.
// Texte : `data-tip`, sinon l'`aria-label` d'un bouton à icône (.iconbtn, .plusbtn, .popy-fab, outils du tableau).
import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { cssRect, uiScale } from '../zoom'

const SEL = '[data-tip], [title].iconbtn, [title].tool, .iconbtn[aria-label], .plusbtn[aria-label], .popy-fab[aria-label], .tool[aria-label]'
type Tip = { text: string; x: number; y: number; below: boolean }

export function Tooltips() {
  const [tip, setTip] = useState<Tip | null>(null)
  const timer = useRef<number>()
  const bubble = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const show = (el: Element, now = false) => {
      // Un `title` ferait une deuxième bulle (celle du navigateur) : on le reprend comme texte.
      const t = el.getAttribute('title')
      if (t) { el.setAttribute('data-tip', t); el.removeAttribute('title') }
      const text = el.getAttribute('data-tip') || el.getAttribute('aria-label') || ''
      if (!text) return
      clearTimeout(timer.current)
      timer.current = window.setTimeout(() => {
        const r = cssRect(el)
        const below = r.top < 44
        setTip({ text, x: r.left + r.width / 2, y: below ? r.bottom + 8 : r.top - 8, below })
      }, now ? 0 : 300)
    }
    const hide = () => { clearTimeout(timer.current); setTip(null) }
    const over = (e: Event) => { const el = (e.target as Element).closest?.(SEL); if (el) show(el); else hide() }
    const focus = (e: Event) => { const el = (e.target as Element).closest?.(SEL); if (el && (e.target as Element).matches(':focus-visible')) show(el, true) }
    document.addEventListener('pointerover', over)
    document.addEventListener('focusin', focus)
    document.addEventListener('focusout', hide)
    document.addEventListener('pointerdown', hide)
    window.addEventListener('scroll', hide, true)
    return () => { document.removeEventListener('pointerover', over); document.removeEventListener('focusin', focus); document.removeEventListener('focusout', hide); document.removeEventListener('pointerdown', hide); window.removeEventListener('scroll', hide, true) }
  }, [])
  // Garde la bulle dans l'écran : décale à gauche ou à droite après mesure.
  useEffect(() => {
    const b = bubble.current
    if (!b || !tip) return
    const w = b.offsetWidth, vw = window.innerWidth / uiScale()
    const left = Math.min(Math.max(8, tip.x - w / 2), vw - w - 8)
    b.style.left = `${left}px`
    b.style.setProperty('--ax', `${tip.x - left}px`)
  }, [tip])
  if (!tip) return null
  return createPortal(
    <div ref={bubble} className={'tipb' + (tip.below ? ' below' : '')} role="tooltip" style={{ top: tip.y, left: tip.x }}>{tip.text}</div>,
    document.querySelector('.app') ?? document.body)
}
