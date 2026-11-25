import { useEffect, useRef, useState } from 'react'
import { ChevronDown } from 'lucide-react'

// Sélecteur au style de POP Forge (#376) : remplace le <select> natif, dont la liste suit le style gris de Windows.
// Clavier : Entrée, Espace ou Flèche bas ouvre ; Flèches pour se déplacer ; Entrée choisit ; Échap ferme.
export type Option = { value: string; label: string }

export function Selecteur({ options, placeholder, onChange, label, disabled, value }: { options: Option[]; placeholder: string; onChange: (v: string) => void; label: string; disabled?: boolean; value?: string }) {
  const choisi = options.find(o => o.value === value)
  const [ouvert, setOuvert] = useState(false)
  const [actif, setActif] = useState(0)
  const boite = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!ouvert) return
    const f = (e: MouseEvent) => { if (!boite.current?.contains(e.target as Node)) setOuvert(false) }
    document.addEventListener('mousedown', f); return () => document.removeEventListener('mousedown', f)
  }, [ouvert])
  const choisir = (v: string) => { setOuvert(false); onChange(v) }
  const clavier = (e: React.KeyboardEvent) => {
    if (!ouvert && ['Enter', ' ', 'ArrowDown'].includes(e.key)) { e.preventDefault(); setOuvert(true); setActif(0); return }
    if (!ouvert) return
    if (e.key === 'Escape') { e.preventDefault(); setOuvert(false) }
    else if (e.key === 'ArrowDown') { e.preventDefault(); setActif(i => Math.min(i + 1, options.length - 1)) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setActif(i => Math.max(i - 1, 0)) }
    else if (e.key === 'Enter' && options[actif]) { e.preventDefault(); choisir(options[actif].value) }
  }
  return <div className="sel" ref={boite}>
    <button type="button" className="sel-btn" aria-haspopup="listbox" aria-expanded={ouvert} aria-label={label} disabled={disabled || !options.length}
      onClick={() => { setOuvert(o => !o); setActif(0) }} onKeyDown={clavier}>
      <span>{choisi?.label ?? placeholder}</span><ChevronDown size={16} strokeWidth={1.5} aria-hidden="true" />
    </button>
    {ouvert && <div className="sel-menu" role="listbox" aria-label={label}>
      {options.map((o, i) => <button type="button" key={o.value} role="option" aria-selected={o.value === value} className={'sel-opt' + (i === actif ? ' on' : '')}
        onMouseEnter={() => setActif(i)} onClick={() => choisir(o.value)}>{o.label}</button>)}
    </div>}
  </div>
}
