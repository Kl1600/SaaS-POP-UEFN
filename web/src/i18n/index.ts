// Langue du site vitrine et de la liste d'attente.
// Le français est chargé d'office ; les autres langues sont chargées à la demande.
import { useSyncExternalStore } from 'react'
import fr, { type Dict } from './fr'
import { LANG as APP_LANG } from './t'
import fFR from 'flag-icons/flags/4x3/fr.svg'
import fGB from 'flag-icons/flags/4x3/gb.svg'
import fES from 'flag-icons/flags/4x3/es.svg'
import fBR from 'flag-icons/flags/4x3/br.svg'
import fPT from 'flag-icons/flags/4x3/pt.svg'
import fDE from 'flag-icons/flags/4x3/de.svg'
import fIT from 'flag-icons/flags/4x3/it.svg'
import fNL from 'flag-icons/flags/4x3/nl.svg'
import fPL from 'flag-icons/flags/4x3/pl.svg'
import fRU from 'flag-icons/flags/4x3/ru.svg'
import fTR from 'flag-icons/flags/4x3/tr.svg'
import fSA from 'flag-icons/flags/4x3/sa.svg'
import fTH from 'flag-icons/flags/4x3/th.svg'
import fRO from 'flag-icons/flags/4x3/ro.svg'
import fCZ from 'flag-icons/flags/4x3/cz.svg'
import fHU from 'flag-icons/flags/4x3/hu.svg'

export type { Dict }
export type Lang = { code: string; html: string; short: string; country: string; language: string; flag: string; rtl?: boolean; load: () => Promise<{ default: Dict }> }

// Pays et langue écrits dans leur propre langue, comme sur les sites de référence.
export const LANGS: Lang[] = [
  { code: 'fr', html: 'fr', short: 'FR', country: 'France', language: 'Français', flag: fFR, load: async () => ({ default: fr }) },
  { code: 'en', html: 'en', short: 'EN', country: 'United Kingdom', language: 'English', flag: fGB, load: () => import('./en') },
  { code: 'es', html: 'es', short: 'ES', country: 'España', language: 'Español', flag: fES, load: () => import('./es') },
  { code: 'pt-BR', html: 'pt-BR', short: 'BR', country: 'Brasil', language: 'Português', flag: fBR, load: () => import('./pt') },
  { code: 'pt-PT', html: 'pt-PT', short: 'PT', country: 'Portugal', language: 'Português', flag: fPT, load: () => import('./ptpt') },
  { code: 'de', html: 'de', short: 'DE', country: 'Deutschland', language: 'Deutsch', flag: fDE, load: () => import('./de') },
  { code: 'it', html: 'it', short: 'IT', country: 'Italia', language: 'Italiano', flag: fIT, load: () => import('./it') },
  { code: 'nl', html: 'nl', short: 'NL', country: 'Nederland', language: 'Nederlands', flag: fNL, load: () => import('./nl') },
  { code: 'pl', html: 'pl', short: 'PL', country: 'Polska', language: 'Polski', flag: fPL, load: () => import('./pl') },
  { code: 'ru', html: 'ru', short: 'RU', country: 'Россия', language: 'Русский', flag: fRU, load: () => import('./ru') },
  { code: 'tr', html: 'tr', short: 'TR', country: 'Türkiye', language: 'Türkçe', flag: fTR, load: () => import('./tr') },
  { code: 'ar', html: 'ar', short: 'AR', country: 'السعودية', language: 'العربية', flag: fSA, rtl: true, load: () => import('./ar') },
  { code: 'th', html: 'th', short: 'TH', country: 'ประเทศไทย', language: 'ไทย', flag: fTH, load: () => import('./th') },
  { code: 'ro', html: 'ro', short: 'RO', country: 'România', language: 'Română', flag: fRO, load: () => import('./ro') },
  { code: 'cs', html: 'cs', short: 'CZ', country: 'Česko', language: 'Čeština', flag: fCZ, load: () => import('./cs') },
  { code: 'hu', html: 'hu', short: 'HU', country: 'Magyarország', language: 'Magyar', flag: fHU, load: () => import('./hu') },
]
// Noms anglais en plus, pour que la recherche trouve « Germany », « Brazil »… quelle que soit la langue affichée.
const ALIASES: Record<string, string> = {
  fr: 'france french francais', en: 'england uk united kingdom great britain english anglais angleterre royaume-uni', es: 'spain spanish espagne espagnol', 'pt-BR': 'brazil brazilian portuguese bresil portugais',
  'pt-PT': 'portugal portuguese portugais', de: 'germany german allemagne allemand', it: 'italy italian italie italien', nl: 'netherlands holland dutch pays-bas hollande neerlandais',
  pl: 'poland polish pologne polonais', ru: 'russia russian russie russe', tr: 'turkey turkish turquie turc', ar: 'saudi arabia arabic arabie saoudite arabe',
  th: 'thailand thai thailande thailandais', ro: 'romania romanian roumanie roumain', cs: 'czech czechia republique tcheque tcheque', hu: 'hungary hungarian hongrie hongrois',
}
const fold = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
export function matchLang(l: Lang, q: string) {
  const f = fold(q.trim())
  return !f || fold(`${l.country} ${l.language} ${l.short} ${ALIASES[l.code] || ''}`).includes(f)
}

const KEY = 'pop-lang'
function initial(): Lang {
  let saved: string | null = null
  try { saved = localStorage.getItem(KEY) } catch { /* stockage indisponible */ }
  const byCode = (c: string | null) => c ? LANGS.find(l => l.code.toLowerCase() === c.toLowerCase()) : undefined
  if (byCode(saved)) return byCode(saved)!
  for (const n of (navigator.languages?.length ? navigator.languages : [navigator.language || 'fr'])) {
    const exact = byCode(n); if (exact) return exact
    const base = n.split('-')[0].toLowerCase()
    if (base === 'pt') return byCode(/pt-pt/i.test(n) ? 'pt-PT' : 'pt-BR')!
    const b = LANGS.find(l => l.code.split('-')[0] === base); if (b) return b
  }
  return LANGS[0]
}

let state: { lang: Lang; t: Dict } = { lang: LANGS[0], t: fr }
let ready = false
const subs = new Set<() => void>()
const emit = () => subs.forEach(f => f())

export async function setLang(code: string, remember = true) {
  const l = LANGS.find(x => x.code === code) || LANGS[0]
  if (remember) try { localStorage.setItem(KEY, l.code) } catch { /* rien */ }
  const d = (await l.load()).default
  state = { lang: l, t: d }
  document.documentElement.lang = l.html
  emit()
}

function boot() {
  if (ready) return
  ready = true
  const l = initial()
  if (l.code !== 'fr') setLang(l.code, false)
  else document.documentElement.lang = 'fr'
}

/** Langue courante et textes : `const { t, lang } = useI18n()`. */
export function useI18n() {
  boot()
  return useSyncExternalStore(cb => { subs.add(cb); return () => { subs.delete(cb) } }, () => state)
}

/** Remet la page en français à la sortie du site (l'app reste en français pour l'instant). */
export function resetHtmlLang() { document.documentElement.lang = APP_LANG }
