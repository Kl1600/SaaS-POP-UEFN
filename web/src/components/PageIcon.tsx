// Icône d'une page (#197) : un emoji, ou « lucide:<nom> » (icône SVG chargée à part).
import { lazy, useEffect, useState } from 'react'
import { FileText } from 'lucide-react'

let lib: Promise<typeof import('./lucideAll')> | null = null
const loadLib = () => (lib ??= import('./lucideAll'))

export const IconPicker = lazy(() => import('./IconPicker'))

export function PageIcon({ icon, size = 14 }: { icon?: string | null; size?: number }) {
  const [C, setC] = useState<any>(null)
  const name = icon?.startsWith('lucide:') ? icon.slice(7) : null
  useEffect(() => {
    if (!name) { setC(null); return }
    let live = true
    loadLib().then(m => { if (live) setC(() => m.lucideIcon(name) ?? null) })
    return () => { live = false }
  }, [name])
  if (!icon) return <FileText size={size} />
  if (name) return C ? <C size={size} /> : <span style={{ width: size, height: size, display: 'inline-block' }} />
  return <span style={{ fontSize: size, lineHeight: 1 }}>{icon}</span>
}
