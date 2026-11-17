// Choix de la langue : bouton drapeau + code, puis fenêtre avec recherche de pays et grille de drapeaux.
import { useEffect, useRef, useState } from 'react'
import { ChevronDown, Search, X } from 'lucide-react'
import { LANGS, matchLang, setLang, useI18n } from '../i18n'

export default function LangPicker() {
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
  return <div className="llang" ref={box}>
    <button className="llangbtn" onClick={() => setOpen(o => !o)} aria-haspopup="dialog" aria-expanded={open} aria-label={`${t.nav.lang} : ${lang.language}`}>
      <img src={lang.flag} alt="" /><span>{lang.short}</span><ChevronDown size={14} />
    </button>
    {open && <div className="llangpop" role="dialog" aria-label={t.nav.lang}>
      <div className="llanghead">
        <label className="llangsearch"><Search size={16} /><input ref={input} value={q} onChange={e => setQ(e.target.value)} placeholder={t.nav.search} aria-label={t.nav.search}
          onKeyDown={e => { if (e.key === 'Enter' && list[0]) { setLang(list[0].code); setOpen(false) } }} /></label>
        <button className="llangx" onClick={() => setOpen(false)} aria-label="×"><X size={18} /></button>
      </div>
      <div className="llanggrid">
        {list.map(l => <button key={l.code} className={'llangopt' + (l.code === lang.code ? ' on' : '')} lang={l.html}
          onClick={() => { setLang(l.code); setOpen(false) }} aria-pressed={l.code === lang.code}>
          <img src={l.flag} alt="" /><span><b>{l.country}</b><small>{l.language}</small></span>
        </button>)}
        {list.length === 0 && <p className="llangnone">—</p>}
      </div>
    </div>}
  </div>
}
