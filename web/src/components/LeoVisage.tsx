// Léo, l'assistant de POP (#543), rond (#548) : un petit bonhomme dont les yeux clignent et regardent autour (et vers la souris).
import { useEffect, useRef, useState } from 'react'

const calme = () => typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches

export function LeoVisage({ size = 40, suivre = true }: { size?: number; suivre?: boolean }) {
  const ref = useRef<SVGSVGElement>(null)
  const [r, setR] = useState({ x: 0, y: 0 })
  useEffect(() => {
    if (calme()) return
    let derniere = 0, raf = 0
    // Regard vers la souris (borné à ±2,5 unités), sinon un coup d'œil au hasard de temps en temps.
    const bouge = (e: PointerEvent) => {
      if (!suivre || raf) return
      raf = requestAnimationFrame(() => {
        raf = 0
        const b = ref.current?.getBoundingClientRect(); if (!b) return
        const dx = e.clientX - (b.left + b.width / 2), dy = e.clientY - (b.top + b.height / 2), d = Math.hypot(dx, dy) || 1
        const k = Math.min(1, d / 300) * 2.5
        setR({ x: (dx / d) * k, y: (dy / d) * k * 0.8 }); derniere = Date.now()
      })
    }
    const coup = setInterval(() => {
      if (Date.now() - derniere < 2500) return
      const a = Math.random() * Math.PI * 2, k = Math.random() < 0.35 ? 0 : 2
      setR({ x: Math.cos(a) * k, y: Math.sin(a) * k * 0.7 })
    }, 2600)
    window.addEventListener('pointermove', bouge)
    return () => { window.removeEventListener('pointermove', bouge); clearInterval(coup); cancelAnimationFrame(raf) }
  }, [suivre])
  return <svg ref={ref} className="leo" width={size} height={size} viewBox="0 0 40 40" aria-hidden="true">
    <circle cx="20" cy="20" r="19" className="leo-tete" />
    <g className="leo-yeux" style={{ transform: `translate(${r.x}px, ${r.y}px)` }}>
      <rect className="leo-oeil" x="12" y="13" width="5.5" height="13" rx="2.75" />
      <rect className="leo-oeil" x="22.5" y="13" width="5.5" height="13" rx="2.75" />
    </g>
  </svg>
}
