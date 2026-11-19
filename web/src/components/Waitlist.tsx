// Demande d'accès à la bêta (liste d'attente). Utilisé sur le site vitrine et sur /inscription.
// Les libellés suivent la langue choisie ; les valeurs envoyées restent en français pour le panel admin.
import { useEffect, useState } from 'react'
import { ArrowRight, CheckCircle2, X } from 'lucide-react'
import { api } from '../api'
import { useI18n, resetHtmlLang } from '../i18n'
import fr from '../i18n/fr'
import LangPicker from './LangPicker'
import '../landing.css'

export function WaitlistForm({ onDone }: { onDone?: () => void }) {
  const { t, lang } = useI18n()
  const w = t.wl
  const [f, setF] = useState({ name: '', email: '', studio: '', team: fr.wl.teams[0], experience: fr.wl.exps[0], links: '', message: '', pop_hp: '' })
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState('')
  const [done, setDone] = useState<null | { already?: boolean; status?: string }>(null)
  const up = (k: keyof typeof f) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) => setF({ ...f, [k]: e.target.value })
  if (done) return <div className="lwdone">
    <CheckCircle2 size={36} />
    <h3>{done.already ? (done.status === 'approved' ? w.approvedTitle : w.alreadyTitle) : w.okTitle}</h3>
    <p>{done.already && done.status === 'approved' ? w.approvedText : w.okText}</p>
    {onDone && <button className="lbtn" onClick={onDone}>{w.close}</button>}
  </div>
  const opt = () => <small>{w.optional}</small>
  return <form className="lwform" onSubmit={async e => {
    e.preventDefault(); setBusy(true); setErr('')
    const message = lang.code === 'fr' ? f.message : `[${lang.code}] ${f.message}`.trim()
    try { const r = await api.post('/api/waitlist', { ...f, message }); setDone(r) } catch (er: any) { setErr(er.message) } finally { setBusy(false) }
  }}>
    <div className="lwrow">
      <label>{w.name}<input value={f.name} onChange={up('name')} required maxLength={80} autoComplete="name" placeholder="Léa" /></label>
      <label>{w.email}<input type="email" value={f.email} onChange={up('email')} required maxLength={200} autoComplete="email" placeholder="lea@exemple.fr" dir="ltr" /></label>
    </div>
    <div className="lwrow">
      <label><span>{w.studio} {opt()}</span><input value={f.studio} onChange={up('studio')} maxLength={80} placeholder={w.studioPh} /></label>
      <label>{w.team}<select value={f.team} onChange={up('team')}>{fr.wl.teams.map((x, i) => <option key={x} value={x}>{w.teams[i]}</option>)}</select></label>
    </div>
    <label>{w.exp}<select value={f.experience} onChange={up('experience')}>{fr.wl.exps.map((x, i) => <option key={x} value={x}>{w.exps[i]}</option>)}</select></label>
    <label><span>{w.links} {opt()}</span><input value={f.links} onChange={up('links')} maxLength={400} placeholder={w.linksPh} /></label>
    <label><span>{w.msg} {opt()}</span><textarea value={f.message} onChange={up('message')} maxLength={1480} rows={3} placeholder={w.msgPh} /></label>
    {/* Champ piège : invisible et hors du parcours clavier, seuls les robots le remplissent. */}
    <input name="pop_hp" value={f.pop_hp} onChange={up('pop_hp')} tabIndex={-1} autoComplete="off" aria-hidden="true" style={{ position: 'absolute', left: '-10000px', width: 1, height: 1, opacity: 0 }} />
    {err && <p className="lwerr" role="alert">{err}</p>}
    <button className="lbtn pri" disabled={busy}>{busy ? w.sending : <>{w.send} <ArrowRight size={18} /></>}</button>
    <p className="lwsmall">{w.nospam} <a href="/confidentialite" target="_blank" rel="noopener">{w.privacy}</a></p>
  </form>
}

export function WaitlistModal({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  return <div className="lwmodal" role="dialog" aria-label={t.wl.title} onMouseDown={e => { if (e.target === e.currentTarget) onClose() }}>
    <div className="lwbox">
      <button className="lwx" aria-label={t.wl.close} onClick={onClose}><X size={20} /></button>
      <span className="leyebrow">{t.wl.eyebrow}</span>
      <h2>{t.wl.title}</h2>
      <p className="lwintro">{t.wl.intro}</p>
      <WaitlistForm onDone={onClose} />
    </div>
  </div>
}

/** Page /inscription sans lien d'accès : formulaire de liste d'attente au style du site vitrine. */
export function WaitlistPage({ invalid }: { invalid?: boolean }) {
  const { t, lang } = useI18n()
  useEffect(() => { document.title = t.wl.title + ' · POP' }, [t])
  useEffect(() => resetHtmlLang, [])
  return <div className="lp lwpage" dir={lang.rtl ? 'rtl' : 'ltr'} lang={lang.html}>
    <div className="lwtop"><a href="/"><img src="/logo.png" alt="POP" /></a><div className="lwtopr"><LangPicker /><a href="/connexion">{t.nav.login}</a></div></div>
    <div className="lwbox">
      <span className="leyebrow">{t.wl.eyebrow}</span>
      <h2>{t.wl.title}</h2>
      {invalid && <p className="lwerr" style={{ marginTop: 10 }}>{t.wl.invalid}</p>}
      <p className="lwintro">{t.wl.pageIntro}</p>
      <WaitlistForm />
    </div>
  </div>
}
