// Demande d'accès à la bêta (liste d'attente). Utilisé sur le site vitrine et sur /inscription.
import { useState } from 'react'
import { ArrowRight, CheckCircle2, X } from 'lucide-react'
import { api } from '../api'
import '../landing.css'

export function WaitlistForm({ onDone }: { onDone?: () => void }) {
  const [f, setF] = useState({ name: '', email: '', studio: '', team: 'Seul', experience: 'Débutant UEFN', links: '', message: '', pop_hp: '' })
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState('')
  const [done, setDone] = useState<null | { already?: boolean; status?: string }>(null)
  const up = (k: keyof typeof f) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) => setF({ ...f, [k]: e.target.value })
  if (done) return <div className="lwdone">
    <CheckCircle2 size={36} />
    <h3>{done.already ? (done.status === 'approved' ? 'Votre accès est déjà validé.' : 'Vous êtes déjà sur la liste.') : 'C’est noté, merci !'}</h3>
    <p>{done.already && done.status === 'approved' ? 'Vérifiez vos e-mails : le lien pour créer votre compte vous a été envoyé. Sinon, écrivez-nous.' : 'Votre demande est arrivée chez l’équipe POP. Vous recevrez un lien pour créer votre compte dès qu’une place se libère.'}</p>
    {onDone && <button className="lbtn" onClick={onDone}>Fermer</button>}
  </div>
  return <form className="lwform" onSubmit={async e => {
    e.preventDefault(); setBusy(true); setErr('')
    try { const r = await api.post('/api/waitlist', f); setDone(r) } catch (er: any) { setErr(er.message) } finally { setBusy(false) }
  }}>
    <div className="lwrow">
      <label>Nom ou pseudo<input value={f.name} onChange={up('name')} required maxLength={80} autoComplete="name" placeholder="Léa" /></label>
      <label>E-mail<input type="email" value={f.email} onChange={up('email')} required maxLength={200} autoComplete="email" placeholder="lea@exemple.fr" /></label>
    </div>
    <div className="lwrow">
      <label><span>Studio <small>(facultatif)</small></span><input value={f.studio} onChange={up('studio')} maxLength={80} placeholder="Nom du studio" /></label>
      <label>Taille de l’équipe<select value={f.team} onChange={up('team')}>{['Seul', '2 à 3', '4 à 10', 'Plus de 10'].map(x => <option key={x}>{x}</option>)}</select></label>
    </div>
    <label>Expérience<select value={f.experience} onChange={up('experience')}>{['Débutant UEFN', 'Quelques maps publiées', 'Créateur confirmé', 'Studio professionnel'].map(x => <option key={x}>{x}</option>)}</select></label>
    <label><span>Vos maps ou réseaux <small>(facultatif)</small></span><input value={f.links} onChange={up('links')} maxLength={400} placeholder="Codes d’îles, TikTok, YouTube…" /></label>
    <label><span>Ce que vous voulez faire avec POP <small>(facultatif)</small></span><textarea value={f.message} onChange={up('message')} maxLength={1500} rows={3} placeholder="Ex. : on est 4 sur une map tycoon et on se perd entre Discord et les Google Docs." /></label>
    {/* Champ piège : invisible et hors du parcours clavier, seuls les robots le remplissent. */}
    <input name="pop_hp" value={f.pop_hp} onChange={up('pop_hp')} tabIndex={-1} autoComplete="off" aria-hidden="true" style={{ position: 'absolute', left: '-10000px', width: 1, height: 1, opacity: 0 }} />
    {err && <p className="lwerr" role="alert">{err}</p>}
    <button className="lbtn pri" disabled={busy}>{busy ? 'Envoi…' : <>Demander mon accès <ArrowRight size={18} /></>}</button>
    <p className="lwsmall">Pas de spam : on vous écrit seulement pour vous donner votre accès.</p>
  </form>
}

export function WaitlistModal({ onClose }: { onClose: () => void }) {
  return <div className="lwmodal" role="dialog" aria-label="Rejoindre la liste d’attente" onMouseDown={e => { if (e.target === e.currentTarget) onClose() }}>
    <div className="lwbox">
      <button className="lwx" aria-label="Fermer" onClick={onClose}><X size={20} /></button>
      <span className="leyebrow">Bêta privée</span>
      <h2>Rejoindre la liste d’attente</h2>
      <p className="lwintro">On ouvre POP petit à petit pour accompagner chaque studio. Dites-nous qui vous êtes, on vous envoie votre accès dès que possible.</p>
      <WaitlistForm onDone={onClose} />
    </div>
  </div>
}

/** Page /inscription sans lien d'accès : formulaire de liste d'attente au style du site vitrine. */
export function WaitlistPage({ invalid }: { invalid?: boolean }) {
  return <div className="lp lwpage">
    <div className="lwtop"><a href="/"><img src="/logo.png" alt="POP" /></a><a href="/connexion">Se connecter</a></div>
    <div className="lwbox">
      <span className="leyebrow">Bêta privée</span>
      <h2>Rejoindre la liste d’attente</h2>
      {invalid && <p className="lwerr" style={{ marginTop: 10 }}>Ce lien d’accès est invalide, déjà utilisé ou expiré. Refaites une demande, ou écrivez à l’équipe POP.</p>}
      <p className="lwintro">Les comptes POP s’ouvrent sur invitation. Dites-nous qui vous êtes : vous recevrez un lien pour créer votre compte dès que votre accès est validé.</p>
      <WaitlistForm />
    </div>
  </div>
}
