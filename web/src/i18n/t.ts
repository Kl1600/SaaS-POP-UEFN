// Traduction de l'application (dashboard). Le texte source est le français ; l'anglais est dans app-en.ts
// (clé = phrase française, valeur = phrase anglaise, avec {0}, {1}… pour les valeurs variables).
// La langue est fixée au chargement de la page : en changer recharge la page.
import EN from './app-en'

const KEY = 'pop-lang'
export type AppLang = 'fr' | 'en'

function pick(): AppLang {
  let saved: string | null = null
  try { saved = localStorage.getItem(KEY) } catch { /* stockage indisponible */ }
  if (saved) return saved === 'fr' ? 'fr' : 'en'
  const list = typeof navigator !== 'undefined' ? (navigator.languages?.length ? navigator.languages : [navigator.language || 'fr']) : ['fr']
  return list[0]?.toLowerCase().startsWith('fr') ? 'fr' : 'en'
}

/** Langue de l'application : français, ou anglais pour toutes les autres langues (en attendant leur traduction). */
export const LANG: AppLang = pick()
/** Locale pour les dates et les nombres. */
export const LOCALE = LANG === 'fr' ? 'fr-FR' : 'en-GB'

const fill = (s: string, v?: readonly unknown[] | Record<string, unknown>) =>
  v ? s.replace(/\{(\w+)\}/g, (m, k) => { const x = (v as any)[k]; return x === undefined || x === null ? m : String(x) }) : s

/** Traduit un texte de l'interface. `tr('{0} tâches', [n])`. */
export function t(s: string, v?: readonly unknown[] | Record<string, unknown>): string {
  return fill(LANG === 'en' ? (EN[s] ?? s) : s, v)
}
/** Alias utilisé dans les composants (beaucoup ont déjà une variable `t`). */
export const tr = t

// Textes venus du serveur (messages d'erreur, Boussole, analyse) : correspondance exacte, puis modèles avec {0}…
let patterns: [RegExp, string][] | null = null
const esc = (s: string) => s.replace(/[.*+?^$()|[\]\\]/g, '\\$&')
function compile() {
  patterns = Object.keys(EN).filter(k => /\{\d+\}/.test(k)).map(k => {
    const re = new RegExp('^' + esc(k).replace(/\{(\d+)\}/g, '(.*?)') + '$', 's')
    return [re, k] as [RegExp, string]
  })
}
/** Traduit un texte reçu du serveur (en français). Laisse le texte tel quel s'il est inconnu. */
export function tx(s: string | null | undefined): string {
  if (!s || LANG === 'fr') return s ?? ''
  if (EN[s]) return EN[s]
  if (!patterns) compile()
  for (const [re, k] of patterns!) {
    const m = s.match(re)
    if (m) return fill(EN[k], m.slice(1).map(x => tx(x)))
  }
  return s
}

/** Change la langue (et recharge la page pour l'appliquer partout). */
export function setAppLang(code: string) {
  try { localStorage.setItem(KEY, code) } catch { /* rien */ }
  location.reload()
}
