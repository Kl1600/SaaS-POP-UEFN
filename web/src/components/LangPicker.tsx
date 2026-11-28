// Choix de la langue : bouton drapeau + code, puis fenêtre avec recherche de pays et grille de drapeaux.
// Sur le site vitrine, la langue change tout de suite ; dans l'application (`app`), la page se recharge.
import { useEffect, useRef, useState } from 'react'
import { ChevronDown, Search, X } from '../icons'
import { LANGS, matchLang, setLang, useI18n } from '../i18n'
import { LANG, setAppLang, tr } from '../i18n/t'
import './langpicker.css'

export default function LangPicker({ app }: { app?: boolean }) {
  const { lang, t } = useI18n()
  const [open, setOpen] = useState(false)
  const [q, setQ] = useState('')
  const box = useRef<HTMLDivElement>(null)
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (!open) return
    setQ('')
    const t1 = setTimeout(() => input.current?.focus(), 30)
    const down = (e: MouseEvent) => { if (box.current && !box.current.contains(e.target as Node)) setOpen(false) }
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false) }
    document.addEventListener('mousedown', down); document.addEventListener('keydown', key)
    return () => { clearTimeout(t1); document.removeEventListener('mousedown', down); document.removeEventListener('keydown', key) }
  }, [open])
  const list = LANGS.filter(l => matchLang(l, q))
  const label = app ? tr('Langue') : t.nav.lang
  const search = app ? tr('Chercher un pays') : t.nav.search
  const choose = (code: string) => {
    setOpen(false)
    if (app) { if (code !== lang.code) setAppLang(code) } else setLang(code)
  }
  return <div className={'llang' + (app ? ' inapp' : '')} ref={box}>
    <button className="llangbtn" onClick={() => setOpen(o => !o)} aria-haspopup="dialog" aria-expanded={open} aria-label={`${label} : ${lang.language}`} title={label}>
      <img src={lang.flag} alt="" /><span>{lang.short}</span><ChevronDown size={14} />
    </button>
    {open && <div className="llangpop" role="dialog" aria-label={label}>
      <div className="llanghead">
        <label className="llangsearch"><Search size={16} /><input ref={input} value={q} onChange={e => setQ(e.target.value)} placeholder={search} aria-label={search}
          onKeyDown={e => { if (e.key === 'Enter' && list[0]) choose(list[0].code) }} /></label>
        <button className="llangx" onClick={() => setOpen(false)} aria-label="×"><X size={18} /></button>
      </div>
      {app && <p className="llangnote">{tr('L’application est disponible en français et en anglais. Pour les autres langues, elle s’affiche en anglais en attendant leur traduction.')}</p>}
      <div className="llanggrid">
        {list.map(l => <button key={l.code} className={'llangopt' + (l.code === lang.code ? ' on' : '')} lang={l.html}
          onClick={() => choose(l.code)} aria-pressed={l.code === lang.code}>
          <img src={l.flag} alt="" /><span><b>{l.country}</b><small>{l.language}</small></span>
        </button>)}
        {list.length === 0 && <p className="llangnone">—</p>}
      </div>
    </div>}
  </div>
}

/** Langue de l'application (pour l'attribut lang de la page). */
export const appHtmlLang = () => LANG
