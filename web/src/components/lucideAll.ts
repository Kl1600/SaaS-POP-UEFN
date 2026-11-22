// Toute la bibliothèque Lucide, chargée à part (import dynamique) pour les icônes de page (#197).
import { icons } from 'lucide-react'

export const pascal = (n: string) => n.split('-').map(s => s.charAt(0).toUpperCase() + s.slice(1)).join('')
export const lucideIcon = (name: string): any => (icons as Record<string, any>)[pascal(name)]
