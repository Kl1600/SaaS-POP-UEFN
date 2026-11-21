import { SecuritySettings } from '../components/Security'
import { useState } from 'react'
import { Navigate, useNavigate } from 'react-router-dom'
import { Copy, KeyRound, Moon, Plus, Sun, Trash2 } from 'lucide-react'
import { api, euros } from '../api'
import { useApp, useLoad, studioName } from '../state'
import { Avatar, CardHead, PageHead, ROLE, Tabs } from '../components/ui'
import { tr } from '../i18n/t'
import LangPicker from '../components/LangPicker'
import { openCreate } from '../components/MapSwitcher'

/** Pages du studio (ou de l'espace perso) : /studio/:sid/equipe | integrations | maps. */
export default function Team({ tab }: { tab: 'members' | 'integ' | 'maps' }) {
  const { studio } = useApp()
  const nav = useNavigate()
  if (!studio) return null
  if (studio.personal && tab === 'members') return <Navigate to={`/studio/${studio.id}/maps`} replace />
  const items: [string, string][] = studio.personal
    ? [['maps', tr("Mes maps")], ['integ', tr("Intégrations")]]
    : [['members', tr("Membres et rôles")], ['maps', tr("Maps du studio")], ['integ', tr("Intégrations")]]
  const sub = { members: 'equipe', integ: 'integrations', maps: 'maps' } as const
  return (
    <main className="content">
      <PageHead title={studio.personal ? tr("Mes outils") : studio.name} help={studio.personal ? tr("Vos maps perso et les outils reliés à votre compte (Discord, GitHub, Lore).") : tr("Qui fait partie du studio, ce que chacun peut faire, ses maps et les outils reliés.")} />
      <Tabs value={tab} onChange={t => nav(`/studio/${studio.id}/${sub[t as keyof typeof sub]}`)} items={items} />
      {tab === 'members' && <Members />}
      {tab === 'integ' && <Integrations />}
      {tab === 'maps' && <Maps />}
    </main>
  )
}

/** Map perso → map d'équipe : la map passe dans un studio (existant ou nouveau) avec tout son contenu. */
function GoTeam({ m, onDone }: { m: { id: number; name: string }; onDone: () => void }) {
  const { me, reload, toast } = useApp()
  const nav = useNavigate()
  const teams = (me?.studios ?? []).filter(s => !s.personal && (s.role === 'owner' || s.role === 'admin'))
  const [mode, setMode] = useState<'new' | 'existing'>(teams.length ? 'existing' : 'new')
  const [sid, setSid] = useState(teams[0]?.id ?? 0)
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  return <form className="stack" style={{ marginTop: 14, paddingTop: 14, borderTop: '1px solid var(--border)' }} onSubmit={async e => {
    e.preventDefault(); if (busy) return; setBusy(true)
    try {
      const r = await api.post(`/api/maps/${m.id}/move`, mode === 'new' ? { new_studio: name } : { studio_id: sid })
      await reload(); toast(tr("« {0} » est maintenant une map d’équipe", [m.name])); nav(`/studio/${r.studio_id}/equipe`)
    } catch (er: any) { toast(er.message, true); setBusy(false) }
  }}>
    <span className="small muted">{tr("La map passe dans un studio avec tout son contenu (tâches, pages, tableaux, gains). Vous pourrez ensuite inviter des membres.")}</span>
    {teams.length > 0 && <div className="row" style={{ gap: 16, flexWrap: 'wrap' }}>
      <label className="row" style={{ gap: 6 }}><input type="radio" checked={mode === 'existing'} onChange={() => setMode('existing')} />{tr("Dans un studio existant")}</label>
      <label className="row" style={{ gap: 6 }}><input type="radio" checked={mode === 'new'} onChange={() => setMode('new')} />{tr("Dans un nouveau studio")}</label>
    </div>}
    {mode === 'existing' && teams.length > 0
      ? <label className="label">{tr("Studio")}<select className="field" value={sid} onChange={e => setSid(Number(e.target.value))}>{teams.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>
      : <label className="label">{tr("Nom du studio")}<input className="field" value={name} onChange={e => setName(e.target.value)} required maxLength={60} placeholder={tr("Ex. Studio Nova")} /></label>}
    <div className="row"><button type="button" className="btn" onClick={onDone}>{tr("Annuler")}</button><button className="btn pri" disabled={busy}>{tr("Passer en équipe")}</button></div>
  </form>
}

function Members() {
  const { studio, me, isAdmin, toast } = useApp()
  const { data, reload } = useLoad<any>(studio ? `/api/studios/${studio.id}/members` : null, [studio?.id])
  const [inv, setInv] = useState({ email: '', role: 'member' })
  const [link, setLink] = useState('')
  if (!data) return null
  const patch = async (uid: number, b: any) => { try { await api.patch(`/api/studios/${studio!.id}/members/${uid}`, b); reload() } catch (e: any) { toast(e.message, true) } }
  return <>
    <section className="card">
      <CardHead title={tr("Membres")} sub={tr("{0} membres · {1} invitation(s) en attente", [data.members.length, data.invitations.length])} />
      <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>{tr("Membre")}</th><th>{tr("Rôle")}</th><th>{tr("Spécialité")}</th><th>{tr("Heures / sem.")}</th><th>{tr("Portfolio court")}</th><th /></tr></thead><tbody>
        {data.members.map((m: any) => {
          const self = m.id === me?.user.id, editable = isAdmin || self
          return (
            <tr key={m.id}>
              <td><span className="row" style={{ gap: 10 }}><Avatar name={m.name} /><span><span style={{ display: 'block', fontWeight: 500 }}>{m.name}</span><span className="small muted">{m.email}</span></span></span></td>
              <td>{isAdmin && m.role !== 'owner' ? <select className="field" value={m.role} onChange={e => patch(m.id, { role: e.target.value })}>{['admin', 'member', 'freelance', 'viewer'].map(r => <option key={r} value={r}>{ROLE[r]}</option>)}</select> : <span className="badge">{ROLE[m.role]}</span>}</td>
              <td><input className="field" style={{ width: 140 }} defaultValue={m.specialty} disabled={!editable} onBlur={e => e.target.value !== m.specialty && patch(m.id, { specialty: e.target.value })} aria-label={tr("Spécialité")} /></td>
              <td><input className="field" type="number" min={0} max={80} style={{ width: 80 }} defaultValue={m.hours_per_week} disabled={!editable} onBlur={e => Number(e.target.value) !== m.hours_per_week && patch(m.id, { hours_per_week: Number(e.target.value) })} aria-label={tr("Heures")} /></td>
              <td><input className="field" style={{ width: '100%', minWidth: 220 }} defaultValue={m.portfolio} disabled={!editable} maxLength={280} onBlur={e => e.target.value !== m.portfolio && patch(m.id, { portfolio: e.target.value })} aria-label={tr("Portfolio")} /></td>
              <td style={{ whiteSpace: 'nowrap' }}>{isAdmin && !self && <button className="iconbtn" aria-label={tr("Lien de nouveau mot de passe pour {0}", [m.name])} title={tr("Lien de nouveau mot de passe")} onClick={async () => { try { const r = await api.post(`/api/studios/${studio!.id}/members/${m.id}/reset-link`); const link = location.origin + r.link; await navigator.clipboard?.writeText(link).catch(() => {}); prompt(tr("Lien à envoyer à {0} (valable 2 heures, déjà copié) :", [m.name]), link) } catch (e: any) { toast(e.message, true) } }}><KeyRound size={16} /></button>}{isAdmin && m.role !== 'owner' && !self && <button className="iconbtn" aria-label={tr("Retirer {0}", [m.name])} onClick={async () => { if (confirm(tr("Retirer {0} du studio ?", [m.name]))) { await api.del(`/api/studios/${studio!.id}/members/${m.id}`); reload() } }}><Trash2 size={16} /></button>}</td>
            </tr>
          )
        })}
        {data.invitations.map((i: any) => <tr key={'i' + i.id}><td className="muted">{i.email}</td><td><span className="badge">{ROLE[i.role]}</span></td><td colSpan={3} className="muted small">{tr("Invitation en attente")}</td><td>{isAdmin && <button className="iconbtn" aria-label={tr("Annuler l’invitation de {0}", [i.email])} onClick={async () => { await api.del(`/api/invitations/${i.id}`); reload() }}><Trash2 size={16} /></button>}</td></tr>)}
      </tbody></table></div>
    </section>
    <div className="two">
      {isAdmin && <section className="card">
        <CardHead title={tr("Inviter")} sub={tr("Le lien expire après 7 jours. Envoyez-le vous-même (e-mail, Discord).")} />
        <form className="stack" onSubmit={async e => { e.preventDefault(); try { const r = await api.post(`/api/studios/${studio!.id}/invitations`, inv); setLink(location.origin + r.link); reload() } catch (er: any) { toast(er.message, true) } }}>
          <label className="label">E-mail<input className="field" type="email" value={inv.email} onChange={e => setInv({ ...inv, email: e.target.value })} required /></label>
          <label className="label">{tr("Rôle")}<select className="field" value={inv.role} onChange={e => setInv({ ...inv, role: e.target.value })}>{['admin', 'member', 'freelance', 'viewer'].map(r => <option key={r} value={r}>{ROLE[r]}</option>)}</select></label>
          <button className="btn pri" style={{ alignSelf: 'flex-start' }}>{tr("Créer le lien d'invitation")}</button>
          {link && <div className="row"><input className="field mono" style={{ flex: 1 }} readOnly value={link} onFocus={e => e.target.select()} /><button type="button" className="btn" onClick={() => { navigator.clipboard?.writeText(link); toast(tr("Lien copié")) }}><Copy size={16} />{tr("Copier")}</button></div>}
        </form>
      </section>}
      <section className="card">
        <CardHead title={tr("Rôles")} sub={tr("Ce que chaque rôle peut faire")} />
        <div className="list">{[['owner', tr("Tout, y compris la paie et les intégrations")], ['admin', tr("Membres, intégrations, gains et règles de partage")], ['member', tr("Pages, tableaux, tâches, planning ; voit son relevé")], ['freelance', tr("Ses tâches, les pages et tableaux ; voit son relevé")], ['viewer', tr("Lecture seule ; peut commenter les pages")]].map(([r, d]) => <div key={r} className="li"><span style={{ width: 150, fontWeight: 500 }}>{ROLE[r]}</span><span className="muted">{d}</span></div>)}</div>
        <p className="small muted">{tr("Chaque page peut aussi être privée, ouverte à l'équipe ou partagée par lien en lecture (onglet Partage de la page).")}</p>
      </section>
    </div>
  </>
}

function Integrations() {
  const { studio, map, isAdmin, toast } = useApp()
  const { data, reload } = useLoad<any>(studio ? `/api/studios/${studio.id}/integrations` : null, [studio?.id])
  const { data: report } = useLoad<any>(map ? `/api/maps/${map.id}/report` : null, [map?.id])
  if (!data) return null
  const set = async (kind: string, enabled: boolean, config: any) => { try { await api.put(`/api/studios/${studio!.id}/integrations/${kind}`, { enabled, config }); reload() } catch (e: any) { toast(e.message, true) } }
  const lore = data.lore
  const hook = `${location.origin}/api/hooks/lore/${studio!.id}?token=${lore.config.token ?? '…'}`
  return <>
    <div className="grid2">
      <Discord data={data.discord} set={set} />
      <GitHub data={data.github} set={set} />
      <section className="card">
        <CardHead title={tr("Lore / UEFN")} sub={tr("Check-ins reliés aux tâches, fichiers verrouillés")}><button className={'btn' + (lore.enabled ? '' : ' pri')} disabled={!isAdmin} onClick={() => set('lore', !lore.enabled, lore.config)}>{lore.enabled ? tr("Désactiver") : tr("Activer")}</button></CardHead>
        <span className={'badge' + (lore.enabled ? ' b-g' : '')}>{lore.enabled ? tr("Activé") : tr("Désactivé")}</span>
        {lore.enabled && isAdmin && <>
          <p className="small muted">{tr("Appelez ce webhook depuis votre script de check-in. « #15 » relie le check-in à la tâche 15, « fini #15 » la passe à Fait.")}</p>
          <pre className="report mono" style={{ fontSize: 12 }}>{tr("curl -X POST \"{0}\" \\\n  -H \"content-type: application/json\" \\\n  -d '{\"map_id\": {1}, \"user_email\": \"{2}\", \"action\": \"checkin\", \"message\": \"fini #15\", \"files\": [\"Content/Maps/City.umap\"]}'", [hook, map?.id, 'vous@exemple.fr'])}</pre>
          <p className="small muted">{tr("Actions : checkin, lock, unlock. Gardez le jeton secret.")}</p>
        </>}
      </section>
    </div>
    <section className="card">
      <CardHead title={tr("Rapport du soir")} sub={tr("À poster dans {0}", [data.discord.config.channel || tr("votre salon")])}><button className="btn" onClick={() => { navigator.clipboard?.writeText(report?.text ?? ''); toast(tr("Rapport copié")) }}><Copy size={16} />{tr("Copier")}</button></CardHead>
      <pre className="report">{report?.text}</pre>
    </section>
  </>
}

function Maps() {
  const { studio, isAdmin, reload: reloadMe, toast } = useApp()
  const nav = useNavigate()
  const [going, setGoing] = useState<number | null>(null)
  const { data, reload } = useLoad<any[]>(studio ? `/api/studios/${studio.id}/maps` : null, [studio?.id])
  return <>
    <div className="two">
      {data?.map(m => (
        <section key={m.id} className="card">
          <CardHead title={m.name} sub={m.genre}><span className="badge">{m.status === 'early_access' ? tr("Accès anticipé") : m.status === 'live' ? tr("Publiée") : tr("En production")}</span></CardHead>
          <div className="tiles" style={{ gridTemplateColumns: 'repeat(3, 1fr)' }}>
            <div><div className="num" style={{ marginTop: 0, fontSize: 28 }}>{euros(m.last_gains_cents, 0)}</div><div className="lab">{tr("Gains du dernier mois")}</div></div>
            <div><div className="num" style={{ marginTop: 0, fontSize: 28 }}>{m.members}</div><div className="lab">{tr("Membres")}</div></div>
            <div><div className="num" style={{ marginTop: 0, fontSize: 28 }}>{m.progress} %</div><div className="lab">{tr("{0} / {1} tâches", [m.done, m.tasks])}</div></div>
          </div>
          <div className="bar" style={{ margin: '16px 0' }}><span style={{ width: `${m.progress}%` }} /></div>
          <div className="row">
            <button className="btn" onClick={() => nav(`/map/${m.id}`)}>{tr("Ouvrir")}</button>
            {studio?.personal && going !== m.id && <button className="btn" onClick={() => setGoing(m.id)}>{tr("Passer en équipe")}</button>}
            {isAdmin && (data?.length ?? 0) > 1 && <button className="btn" onClick={async () => { const c = prompt(tr("Supprimer définitivement « {0} » et tout son contenu (tâches, pages, Verse, gains) ?\nTapez le nom de la map pour confirmer :", [m.name])); if (c === null) return; try { await api.del(`/api/maps/${m.id}`, { confirm: c }); await reloadMe(); reload(); toast(tr("Map supprimée")) } catch (er: any) { toast(er.message, true) } }}>{tr("Supprimer")}</button>}
            {isAdmin && <input className="field" type="date" defaultValue={m.release_date ?? ''} aria-label={tr("Date de sortie")} onBlur={async e => { await api.patch(`/api/maps/${m.id}`, { release_date: e.target.value }); reloadMe(); reload() }} />}
          </div>
          {going === m.id && <GoTeam m={m} onDone={() => setGoing(null)} />}
        </section>
      ))}
    </div>
    <div className="two">
      <section className="card">
        <CardHead title={tr("Créer")} sub={studio?.personal ? tr("Une nouvelle map perso, ou un studio pour travailler en équipe.") : tr("Une nouvelle map pour ce studio, ou un nouveau studio pour une autre équipe.")} />
        <div className="row" style={{ flexWrap: 'wrap' }}>
          {isAdmin && <button className="btn pri" onClick={() => openCreate({ kind: 'map', studioId: studio?.id })}><Plus size={16} />{tr("Nouvelle map")}</button>}
          <button className="btn" onClick={() => openCreate({ kind: 'studio' })}><Plus size={16} />{tr("Nouveau studio")}</button>
        </div>
      </section>
      {!studio?.personal && <StudioSettings />}
    </div>
  </>
}

export function Account() {
  const { me, toast, reload, dark, toggleDark } = useApp()
  const [name, setName] = useState(me?.user.name ?? '')
  const [pw, setPw] = useState({ current: '', new: '' })
  return (<>
    <div className="grid2">
      <section className="card">
        <CardHead title={tr("Mon compte")} />
        <div className="props"><span className="muted">E-mail</span><span>{me?.user.email}</span><span className="muted">{tr("Langue")}</span><span><LangPicker app /></span><span className="muted">{tr("Thème")}</span><span className="row" style={{ gap: 6 }}><button type="button" className={dark ? 'btn' : 'btn pri'} aria-pressed={!dark} onClick={() => { if (dark) toggleDark() }}><Sun size={16} />{tr("Clair")}</button><button type="button" className={dark ? 'btn pri' : 'btn'} aria-pressed={dark} onClick={() => { if (!dark) toggleDark() }}><Moon size={16} />{tr("Sombre")}</button></span><span className="muted">{tr("Crédits de playtest")}</span><span>{me?.credits}</span><span className="muted">{tr("Studios")}</span><span>{me?.studios.map(s => s.personal ? studioName(s) : `${s.name} (${ROLE[s.role]})`).join(', ')}</span></div>
        <form className="row" style={{ marginTop: 16 }} onSubmit={async e => { e.preventDefault(); try { await api.post('/api/me/profile', { name }); await reload(); toast(tr("Nom enregistré")) } catch (er: any) { toast(er.message, true) } }}>
          <label className="label" style={{ flex: 1 }}>{tr("Nom affiché")}<input className="field" value={name} onChange={e => setName(e.target.value)} maxLength={80} required /></label>
          <button className="btn" style={{ alignSelf: 'flex-end' }}>{tr("Enregistrer")}</button>
        </form>
        <div className="row" style={{ marginTop: 20, flexWrap: 'wrap' }}>
          <button className="btn" onClick={async () => { await api.post('/api/auth/logout'); location.href = '/' }}>{tr("Se déconnecter")}</button>
          <a className="btn" href="/api/me/export" download>{tr("Télécharger mes données")}</a>
          <button className="btn" style={{ color: 'var(--tr)' }} onClick={async () => { const p = prompt(tr("Supprimer définitivement votre compte ? Vos studios sans autre membre seront supprimés aussi.\nTapez votre mot de passe pour confirmer :")); if (!p) return; try { await api.del('/api/me', { password: p }); location.href = '/' } catch (er: any) { toast(er.message, true) } }}>{tr("Supprimer mon compte")}</button>
        </div>
      </section>
      <section className="card">
        <CardHead title={tr("Mot de passe")} sub={tr("Changer le mot de passe déconnecte vos autres appareils.")} />
        <form className="stack" onSubmit={async e => { e.preventDefault(); try { const r: any = await api.post('/api/me/password', pw); toast(r?.tokens_revoked ? tr("Mot de passe changé : reconnectez-vous. {0} jeton(s) d’IA connectée révoqué(s) : recréez-les dans « IA connectées ».", [r.tokens_revoked]) : tr("Mot de passe changé : reconnectez-vous")); setTimeout(() => (location.href = '/'), 1200) } catch (er: any) { toast(er.message, true) } }}>
          <label className="label">{tr("Mot de passe actuel")}<input className="field" type="password" value={pw.current} onChange={e => setPw({ ...pw, current: e.target.value })} required autoComplete="current-password" /></label>
          <label className="label">{tr("Nouveau (10 caractères minimum)")}<input className="field" type="password" value={pw.new} onChange={e => setPw({ ...pw, new: e.target.value })} minLength={10} required autoComplete="new-password" /></label>
          <button className="btn pri">{tr("Changer le mot de passe")}</button>
        </form>
      </section>
    </div>
    <SecuritySettings />
  </>)
}

function McpTokens() {
  const { map, canEdit, toast } = useApp()
  const { data, reload } = useLoad<any[]>(map ? `/api/maps/${map.id}/tokens` : null, [map?.id])
  const [tok, setTok] = useState<string | null>(null)
  const cfg = tok ? JSON.stringify({ mcpServers: { pop: { type: 'http', url: `${location.origin}/mcp`, headers: { Authorization: tr("Bearer {0}", [tok]) } } } }, null, 2) : ''
  return (
    <section className="card">
      <CardHead title={tr("IA connectée (serveur MCP de POP)")} sub={tr("Votre IA (Claude Code, Cursor…) peut lire et modifier les acteurs et le Verse comme vous : tout est journalisé, et une suppression attend la validation d’un humain dans l’Organiseur.")}>
        {canEdit && <button className="btn pri" onClick={async () => { const n = prompt(tr("Nom du jeton"), tr("Mon IA")); if (n) { try { setTok((await api.post(`/api/maps/${map!.id}/tokens`, { name: n })).token); reload() } catch (e: any) { toast(e.message, true) } } }}>{tr("Créer un jeton")}</button>}
      </CardHead>
      {tok && <><p className="small muted">{tr("Copiez ce fichier .mcp.json maintenant : le jeton ne sera plus affiché.")}</p><pre className="report mono">{cfg}</pre></>}
      <div className="list">{data?.map(t => <div key={t.id} className="li"><span style={{ flex: 1 }}>{t.name}</span><span className="small muted">{t.created_at}</span><button className="iconbtn" aria-label={tr("Révoquer")} onClick={async () => { await api.del(`/api/tokens/${t.id}`); reload() }}><Trash2 size={16} /></button></div>)}</div>
    </section>
  )
}

function useMembersOfStudio() {
  const { map } = useApp()
  return useLoad<any>(map ? `/api/maps/${map.id}/people` : null, [map?.id]).data?.people ?? []
}

function F({ label, value, k, cfg, onSave, disabled, placeholder, mono, type }: { label: string; value?: string; k: string; cfg: any; onSave: (c: any) => void; disabled: boolean; placeholder?: string; mono?: boolean; type?: string }) {
  return <label className="label" style={{ marginTop: 10 }}>{label}<input className={'field' + (mono ? ' mono' : '')} type={type ?? 'text'} defaultValue={value ?? cfg[k] ?? ''} disabled={disabled} placeholder={placeholder} onBlur={e => e.target.value !== (cfg[k] ?? '') && onSave({ ...cfg, [k]: e.target.value })} /></label>
}

function Discord({ data, set }: { data: any; set: (k: string, e: boolean, c: any) => void }) {
  const { studio, isAdmin, toast } = useApp()
  const people = useMembersOfStudio()
  const cfg = data.config ?? {}
  const save = (c: any) => set('discord', data.enabled, c)
  const hook = `${location.origin}/api/hooks/discord/${studio!.id}`
  return <section className="card">
    <CardHead title={tr("Discord")} sub={tr("Rapport du soir automatique, commandes /tache-suivante, /fini, /rapport")}><button className={'btn' + (data.enabled ? '' : ' pri')} disabled={!isAdmin} onClick={() => set('discord', !data.enabled, cfg)}>{data.enabled ? tr("Désactiver") : tr("Activer")}</button></CardHead>
    <span className={'badge' + (data.enabled ? ' b-g' : '')}>{data.enabled ? tr("Activé") : tr("Désactivé")}</span>
    {data.enabled && isAdmin && <>
      <div className="dsec">{tr("1 · Rapport du soir")}</div>
      <p className="small muted" style={{ margin: 0 }}>{tr("Discord → Paramètres du salon → Intégrations → Webhooks → Nouveau webhook → Copier l’URL.")}</p>
      <F label={tr("URL du webhook")} k="webhook" cfg={cfg} onSave={save} disabled={!isAdmin} placeholder="https://discord.com/api/webhooks/…" mono />
      <F label={tr("Heure d’envoi (heure de Paris, vide = jamais)")} k="report_time" cfg={cfg} onSave={save} disabled={!isAdmin} type="time" />
      <button className="btn" style={{ marginTop: 10 }} onClick={async () => { try { const r = await api.post(`/api/studios/${studio!.id}/discord/send`); toast(tr("{0} rapport(s) envoyé(s) sur Discord", [r.sent])) } catch (e: any) { toast(e.message, true) } }}>{tr("Envoyer le rapport maintenant")}</button>
      <div className="dsec">2 · Commandes (application Discord)</div>
      <p className="small muted" style={{ margin: 0 }}>{tr("discord.com/developers → New Application. Copiez l’Application ID et la Public Key (General Information), puis un jeton de bot (Bot → Reset Token). Collez ensuite l’adresse ci-dessous dans « Interactions Endpoint URL » : POP doit être en ligne en HTTPS. Invitez le bot avec le scope applications.commands.")}</p>
      <F label={tr("Application ID")} k="app_id" cfg={cfg} onSave={save} disabled={!isAdmin} mono />
      <F label={tr("Public Key")} k="public_key" cfg={cfg} onSave={save} disabled={!isAdmin} mono />
      <F label={tr("Jeton du bot")} k="bot_token" cfg={cfg} onSave={save} disabled={!isAdmin} mono type="password" />
      <pre className="report mono" style={{ fontSize: 12, marginTop: 10 }}>{hook}</pre>
      <button className="btn" onClick={async () => { try { await api.post(`/api/studios/${studio!.id}/discord/register`); toast(tr("Commandes enregistrées sur Discord")) } catch (e: any) { toast(e.message, true) } }}>{tr("Enregistrer les commandes")}</button>
      <div className="dsec">{tr("Pseudo Discord de chaque membre")}</div>
      {people.map((p: any) => <label key={p.id} className="row small" style={{ marginTop: 6 }}><span style={{ width: 160 }}>{p.name}</span><input className="field" style={{ flex: 1 }} defaultValue={cfg.handles?.[p.id] ?? ''} placeholder="pseudo" onBlur={e => save({ ...cfg, handles: { ...(cfg.handles ?? {}), [p.id]: e.target.value.trim() } })} /></label>)}
    </>}
  </section>
}

function GitHub({ data, set }: { data: any; set: (k: string, e: boolean, c: any) => void }) {
  const { studio, map, isAdmin, toast } = useApp()
  const cfg = data.config ?? {}
  const save = (c: any) => set('github', data.enabled, c)
  const [busy, setBusy] = useState(false)
  return <section className="card">
    <CardHead title={tr("GitHub")} sub={tr("Issues ↔ tâches dans les deux sens")}><button className={'btn' + (data.enabled ? '' : ' pri')} disabled={!isAdmin} onClick={() => set('github', !data.enabled, cfg)}>{data.enabled ? tr("Désactiver") : tr("Activer")}</button></CardHead>
    <span className={'badge' + (data.enabled ? ' b-g' : '')}>{data.enabled ? tr("Activé") : tr("Désactivé")}</span>
    {data.enabled && isAdmin && <>
      <F label={tr("Dépôt (propriétaire/nom)")} k="repo" cfg={cfg} onSave={save} disabled={!isAdmin} placeholder="mon-studio/ma-map" mono />
      <p className="small muted" style={{ margin: '8px 0 0' }}>{tr("Jeton : GitHub → Settings → Developer settings → Fine-grained tokens, accès à ce dépôt, permission Issues en lecture et écriture.")}</p>
      <F label={tr("Jeton GitHub")} k="token" cfg={cfg} onSave={save} disabled={!isAdmin} mono type="password" />
      <button className="btn pri" style={{ marginTop: 10 }} disabled={busy} onClick={async () => { setBusy(true); try { const r = await api.post(`/api/studios/${studio!.id}/github/sync?map=${map!.id}`); toast(tr("GitHub : {0} issue(s) créée(s), {1} importée(s), {2} fermée(s), {3} tâche(s) faite(s)", [r.created, r.imported, r.closed, r.done])) } catch (e: any) { toast(e.message, true) } finally { setBusy(false) } }}>{busy ? tr("Synchronisation…") : tr("Synchroniser maintenant")}</button>
      <div className="dsec">{tr("Mise à jour instantanée (POP en ligne)")}</div>
      <p className="small muted" style={{ margin: 0 }}>{tr("Dépôt → Settings → Webhooks → Add webhook : Payload URL ci-dessous, Content type application/json, Secret ci-dessous, événement « Issues ».")}</p>
      <pre className="report mono" style={{ fontSize: 12 }}>{`${location.origin}/api/hooks/github/${studio!.id}\nSecret : ${cfg.secret ?? tr("(activez puis rechargez)")}`}</pre>
    </>}
  </section>
}

function StudioSettings() {
  const { studio, me, isAdmin, reload: reloadMe, toast } = useApp()
  const owner = me?.studios.find(s => s.id === studio?.id)?.role === 'owner'
  const { data } = useLoad<any>(studio ? `/api/studios/${studio.id}/members` : null, [studio?.id])
  const [name, setName] = useState(studio?.name ?? '')
  const [to, setTo] = useState('')
  if (!studio) return null
  return <section className="card">
    <CardHead title={tr("Studio {0}", [studio.name])} sub={tr("Nom, sauvegarde, propriété")} />
    {isAdmin && <form className="row" onSubmit={async e => { e.preventDefault(); try { await api.patch(`/api/studios/${studio.id}`, { name }); await reloadMe(); toast(tr("Studio renommé")) } catch (er: any) { toast(er.message, true) } }}>
      <label className="label" style={{ flex: 1 }}>{tr("Nom du studio")}<input className="field" value={name} onChange={e => setName(e.target.value)} required maxLength={80} /></label>
      <button className="btn" style={{ alignSelf: 'flex-end' }}>{tr("Renommer")}</button>
    </form>}
    {isAdmin && <><div className="dsec">{tr("Sauvegarde")}</div><p className="small muted" style={{ margin: 0 }}>{tr("Téléchargez tout le travail du studio (maps, tâches, pages, tableaux, Verse, gains) dans un fichier.")}</p>
      <a className="btn" style={{ marginTop: 8 }} href={`/api/studios/${studio.id}/export`} download>{tr("Exporter le studio")}</a></>}
    {owner && <><div className="dsec">{tr("Transférer la propriété")}</div>
      <div className="row"><select className="field" style={{ flex: 1 }} value={to} onChange={e => setTo(e.target.value)} aria-label={tr("Nouveau propriétaire")}><option value="">{tr("Choisir un membre…")}</option>{data?.members.filter((m: any) => m.id !== me?.user.id).map((m: any) => <option key={m.id} value={m.id}>{m.name}</option>)}</select>
        <button className="btn" disabled={!to} onClick={async () => { if (!confirm(tr("Transférer la propriété ? Vous deviendrez admin."))) return; try { await api.post(`/api/studios/${studio.id}/transfer`, { user_id: Number(to) }); await reloadMe(); toast(tr("Propriété transférée")) } catch (er: any) { toast(er.message, true) } }}>{tr("Transférer")}</button></div></>}
    <div className="dsec">{tr("Zone sensible")}</div>
    <div className="row" style={{ flexWrap: 'wrap' }}>
      {!owner && <button className="btn" onClick={async () => { if (!confirm(tr("Quitter le studio {0} ?", [studio.name]))) return; try { await api.post(`/api/studios/${studio.id}/leave`); await reloadMe(); location.href = '/' } catch (er: any) { toast(er.message, true) } }}>{tr("Quitter le studio")}</button>}
      {owner && <button className="btn" style={{ color: 'var(--tr)' }} onClick={async () => { const c = prompt(tr("Supprimer définitivement le studio et TOUTES ses maps ?\nTapez « {0} » pour confirmer :", [studio.name])); if (c === null) return; try { await api.del(`/api/studios/${studio.id}`, { confirm: c }); await reloadMe(); location.href = '/' } catch (er: any) { toast(er.message, true) } }}>{tr("Supprimer le studio")}</button>}
    </div>
  </section>
}
