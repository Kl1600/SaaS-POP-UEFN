import { SecuritySettings } from '../components/Security'
import { useState } from 'react'
import { Navigate, useSearchParams } from 'react-router-dom'
import { Copy, KeyRound, Trash2 } from 'lucide-react'
import { api, euros } from '../api'
import { useApp, useLoad } from '../state'
import { Avatar, CardHead, PageHead, ROLE, Tabs } from '../components/ui'

export default function Team() {
  const [params, setParams] = useSearchParams()
  const tab = (params.get('tab') as any) || 'members'
  return (
    <main className="content">
      <PageHead title="Équipe et intégrations" help="Qui fait partie du studio, ce que chacun peut faire, et les outils reliés." />
      <Tabs value={tab} onChange={t => setParams({ tab: t })} items={[['members', 'Membres et rôles'], ['integ', 'Intégrations'], ['maps', 'Maps du studio']]} />
      {tab === 'members' && <Members />}
      {tab === 'integ' && <Integrations />}
      {tab === 'maps' && <Maps />}
      {tab === 'compte' && <Navigate to="/parametres" replace />}
    </main>
  )
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
      <CardHead title="Membres" sub={`${data.members.length} membres · ${data.invitations.length} invitation(s) en attente`} />
      <div style={{ overflowX: 'auto' }}><table className="tbl"><thead><tr><th>Membre</th><th>Rôle</th><th>Spécialité</th><th>Heures / sem.</th><th>Portfolio court</th><th /></tr></thead><tbody>
        {data.members.map((m: any) => {
          const self = m.id === me?.user.id, editable = isAdmin || self
          return (
            <tr key={m.id}>
              <td><span className="row" style={{ gap: 10 }}><Avatar name={m.name} /><span><span style={{ display: 'block', fontWeight: 500 }}>{m.name}</span><span className="small muted">{m.email}</span></span></span></td>
              <td>{isAdmin && m.role !== 'owner' ? <select className="field" value={m.role} onChange={e => patch(m.id, { role: e.target.value })}>{['admin', 'member', 'freelance', 'viewer'].map(r => <option key={r} value={r}>{ROLE[r]}</option>)}</select> : <span className="badge">{ROLE[m.role]}</span>}</td>
              <td><input className="field" style={{ width: 140 }} defaultValue={m.specialty} disabled={!editable} onBlur={e => e.target.value !== m.specialty && patch(m.id, { specialty: e.target.value })} aria-label="Spécialité" /></td>
              <td><input className="field" type="number" min={0} max={80} style={{ width: 80 }} defaultValue={m.hours_per_week} disabled={!editable} onBlur={e => Number(e.target.value) !== m.hours_per_week && patch(m.id, { hours_per_week: Number(e.target.value) })} aria-label="Heures" /></td>
              <td><input className="field" style={{ width: '100%', minWidth: 220 }} defaultValue={m.portfolio} disabled={!editable} maxLength={280} onBlur={e => e.target.value !== m.portfolio && patch(m.id, { portfolio: e.target.value })} aria-label="Portfolio" /></td>
              <td style={{ whiteSpace: 'nowrap' }}>{isAdmin && !self && <button className="iconbtn" aria-label={`Lien de nouveau mot de passe pour ${m.name}`} title="Lien de nouveau mot de passe" onClick={async () => { try { const r = await api.post(`/api/studios/${studio!.id}/members/${m.id}/reset-link`); const link = location.origin + r.link; await navigator.clipboard?.writeText(link).catch(() => {}); prompt(`Lien à envoyer à ${m.name} (valable 2 heures, déjà copié) :`, link) } catch (e: any) { toast(e.message, true) } }}><KeyRound size={16} /></button>}{isAdmin && m.role !== 'owner' && !self && <button className="iconbtn" aria-label={`Retirer ${m.name}`} onClick={async () => { if (confirm(`Retirer ${m.name} du studio ?`)) { await api.del(`/api/studios/${studio!.id}/members/${m.id}`); reload() } }}><Trash2 size={16} /></button>}</td>
            </tr>
          )
        })}
        {data.invitations.map((i: any) => <tr key={'i' + i.id}><td className="muted">{i.email}</td><td><span className="badge">{ROLE[i.role]}</span></td><td colSpan={3} className="muted small">Invitation en attente</td><td>{isAdmin && <button className="iconbtn" aria-label={`Annuler l’invitation de ${i.email}`} onClick={async () => { await api.del(`/api/invitations/${i.id}`); reload() }}><Trash2 size={16} /></button>}</td></tr>)}
      </tbody></table></div>
    </section>
    <div className="two">
      {isAdmin && <section className="card">
        <CardHead title="Inviter" sub="Le lien expire après 7 jours. Envoyez-le vous-même (e-mail, Discord)." />
        <form className="stack" onSubmit={async e => { e.preventDefault(); try { const r = await api.post(`/api/studios/${studio!.id}/invitations`, inv); setLink(location.origin + r.link); reload() } catch (er: any) { toast(er.message, true) } }}>
          <label className="label">E-mail<input className="field" type="email" value={inv.email} onChange={e => setInv({ ...inv, email: e.target.value })} required /></label>
          <label className="label">Rôle<select className="field" value={inv.role} onChange={e => setInv({ ...inv, role: e.target.value })}>{['admin', 'member', 'freelance', 'viewer'].map(r => <option key={r} value={r}>{ROLE[r]}</option>)}</select></label>
          <button className="btn pri" style={{ alignSelf: 'flex-start' }}>Créer le lien d'invitation</button>
          {link && <div className="row"><input className="field mono" style={{ flex: 1 }} readOnly value={link} onFocus={e => e.target.select()} /><button type="button" className="btn" onClick={() => { navigator.clipboard?.writeText(link); toast('Lien copié') }}><Copy size={16} />Copier</button></div>}
        </form>
      </section>}
      <section className="card">
        <CardHead title="Rôles" sub="Ce que chaque rôle peut faire" />
        <div className="list">{[['owner', 'Tout, y compris la paie et les intégrations'], ['admin', 'Membres, intégrations, gains et règles de partage'], ['member', 'Pages, tableaux, tâches, planning ; voit son relevé'], ['freelance', 'Ses tâches, les pages et tableaux ; voit son relevé'], ['viewer', 'Lecture seule ; peut commenter les pages']].map(([r, d]) => <div key={r} className="li"><span style={{ width: 150, fontWeight: 500 }}>{ROLE[r]}</span><span className="muted">{d}</span></div>)}</div>
        <p className="small muted">Chaque page peut aussi être privée, ouverte à l'équipe ou partagée par lien en lecture (onglet Partage de la page).</p>
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
        <CardHead title="Lore / UEFN" sub="Check-ins reliés aux tâches, fichiers verrouillés"><button className={'btn' + (lore.enabled ? '' : ' pri')} disabled={!isAdmin} onClick={() => set('lore', !lore.enabled, lore.config)}>{lore.enabled ? 'Désactiver' : 'Activer'}</button></CardHead>
        <span className={'badge' + (lore.enabled ? ' b-g' : '')}>{lore.enabled ? 'Activé' : 'Désactivé'}</span>
        {lore.enabled && isAdmin && <>
          <p className="small muted">Appelez ce webhook depuis votre script de check-in. « #15 » relie le check-in à la tâche 15, « fini #15 » la passe à Fait.</p>
          <pre className="report mono" style={{ fontSize: 12 }}>{`curl -X POST "${hook}" \\\n  -H "content-type: application/json" \\\n  -d '{"map_id": ${map?.id}, "user_email": "${'vous@exemple.fr'}", "action": "checkin", "message": "fini #15", "files": ["Content/Maps/City.umap"]}'`}</pre>
          <p className="small muted">Actions : checkin, lock, unlock. Gardez le jeton secret.</p>
        </>}
      </section>
    </div>
    <section className="card">
      <CardHead title="Rapport du soir" sub={`À poster dans ${data.discord.config.channel || 'votre salon'}`}><button className="btn" onClick={() => { navigator.clipboard?.writeText(report?.text ?? ''); toast('Rapport copié') }}><Copy size={16} />Copier</button></CardHead>
      <pre className="report">{report?.text}</pre>
    </section>
  </>
}

function Maps() {
  const { studio, isAdmin, setMapId, reload: reloadMe, toast } = useApp()
  const { data, reload } = useLoad<any[]>(studio ? `/api/studios/${studio.id}/maps` : null, [studio?.id])
  const [f, setF] = useState({ name: '', genre: '', release_date: '' })
  const [s, setS] = useState({ name: '', map_name: '' })
  return <>
    <div className="two">
      {data?.map(m => (
        <section key={m.id} className="card">
          <CardHead title={m.name} sub={m.genre}><span className="badge">{m.status === 'early_access' ? 'Accès anticipé' : m.status === 'live' ? 'Publiée' : 'En production'}</span></CardHead>
          <div className="tiles" style={{ gridTemplateColumns: 'repeat(3, 1fr)' }}>
            <div><div className="num" style={{ marginTop: 0, fontSize: 28 }}>{euros(m.last_gains_cents, 0)}</div><div className="lab">Gains du dernier mois</div></div>
            <div><div className="num" style={{ marginTop: 0, fontSize: 28 }}>{m.members}</div><div className="lab">Membres</div></div>
            <div><div className="num" style={{ marginTop: 0, fontSize: 28 }}>{m.progress} %</div><div className="lab">{m.done} / {m.tasks} tâches</div></div>
          </div>
          <div className="bar" style={{ margin: '16px 0' }}><span style={{ width: `${m.progress}%` }} /></div>
          <div className="row">
            <button className="btn" onClick={() => setMapId(m.id)}>Ouvrir</button>
            {isAdmin && (data?.length ?? 0) > 1 && <button className="btn" onClick={async () => { const c = prompt(`Supprimer définitivement « ${m.name} » et tout son contenu (tâches, pages, Verse, gains) ?\nTapez le nom de la map pour confirmer :`); if (c === null) return; try { await api.del(`/api/maps/${m.id}`, { confirm: c }); await reloadMe(); reload(); toast('Map supprimée') } catch (er: any) { toast(er.message, true) } }}>Supprimer</button>}
            {isAdmin && <input className="field" type="date" defaultValue={m.release_date ?? ''} aria-label="Date de sortie" onBlur={async e => { await api.patch(`/api/maps/${m.id}`, { release_date: e.target.value }); reloadMe(); reload() }} />}
          </div>
        </section>
      ))}
    </div>
    <div className="two">
      {isAdmin && <section className="card">
        <CardHead title="Nouvelle map" sub={`Dans ${studio?.name} · chaque map a ses gains, son équipe et son avancement`} />
        <form className="stack" onSubmit={async e => { e.preventDefault(); try { const r = await api.post(`/api/studios/${studio!.id}/maps`, f); await reloadMe(); setMapId(r.map_id); reload(); setF({ name: '', genre: '', release_date: '' }) } catch (er: any) { toast(er.message, true) } }}>
          <label className="label">Nom<input className="field" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} required /></label>
          <label className="label">Genre<input className="field" value={f.genre} onChange={e => setF({ ...f, genre: e.target.value })} /></label>
          <label className="label">Sortie visée<input className="field" type="date" value={f.release_date} onChange={e => setF({ ...f, release_date: e.target.value })} /></label>
          <button className="btn pri" style={{ alignSelf: 'flex-start' }}>Créer la map</button>
        </form>
      </section>}
      <section className="card">
        <CardHead title="Nouveau studio" sub="Pour une autre équipe" />
        <form className="stack" onSubmit={async e => { e.preventDefault(); try { const r = await api.post('/api/studios', s); await reloadMe(); setMapId(r.map_id) } catch (er: any) { toast(er.message, true) } }}>
          <label className="label">Nom du studio<input className="field" value={s.name} onChange={e => setS({ ...s, name: e.target.value })} required /></label>
          <label className="label">Première map<input className="field" value={s.map_name} onChange={e => setS({ ...s, map_name: e.target.value })} required /></label>
          <button className="btn" style={{ alignSelf: 'flex-start' }}>Créer le studio</button>
        </form>
      </section>
      <StudioSettings />
    </div>
  </>
}

export function Account() {
  const { me, toast, reload } = useApp()
  const [name, setName] = useState(me?.user.name ?? '')
  const [pw, setPw] = useState({ current: '', new: '' })
  return (<>
    <div className="grid2">
      <section className="card">
        <CardHead title="Mon compte" />
        <div className="props"><span className="muted">E-mail</span><span>{me?.user.email}</span><span className="muted">Crédits de playtest</span><span>{me?.credits}</span><span className="muted">Studios</span><span>{me?.studios.map(s => `${s.name} (${ROLE[s.role]})`).join(', ')}</span></div>
        <form className="row" style={{ marginTop: 16 }} onSubmit={async e => { e.preventDefault(); try { await api.post('/api/me/profile', { name }); await reload(); toast('Nom enregistré') } catch (er: any) { toast(er.message, true) } }}>
          <label className="label" style={{ flex: 1 }}>Nom affiché<input className="field" value={name} onChange={e => setName(e.target.value)} maxLength={80} required /></label>
          <button className="btn" style={{ alignSelf: 'flex-end' }}>Enregistrer</button>
        </form>
        <div className="row" style={{ marginTop: 20, flexWrap: 'wrap' }}>
          <button className="btn" onClick={async () => { await api.post('/api/auth/logout'); location.href = '/' }}>Se déconnecter</button>
          <a className="btn" href="/api/me/export" download>Télécharger mes données</a>
          <button className="btn" style={{ color: 'var(--tr)' }} onClick={async () => { const p = prompt('Supprimer définitivement votre compte ? Vos studios sans autre membre seront supprimés aussi.\nTapez votre mot de passe pour confirmer :'); if (!p) return; try { await api.del('/api/me', { password: p }); location.href = '/' } catch (er: any) { toast(er.message, true) } }}>Supprimer mon compte</button>
        </div>
      </section>
      <section className="card">
        <CardHead title="Mot de passe" sub="Changer le mot de passe déconnecte vos autres appareils." />
        <form className="stack" onSubmit={async e => { e.preventDefault(); try { const r: any = await api.post('/api/me/password', pw); toast(r?.tokens_revoked ? `Mot de passe changé : reconnectez-vous. ${r.tokens_revoked} jeton(s) d’IA connectée révoqué(s) : recréez-les dans « IA connectées ».` : 'Mot de passe changé : reconnectez-vous'); setTimeout(() => (location.href = '/'), 1200) } catch (er: any) { toast(er.message, true) } }}>
          <label className="label">Mot de passe actuel<input className="field" type="password" value={pw.current} onChange={e => setPw({ ...pw, current: e.target.value })} required autoComplete="current-password" /></label>
          <label className="label">Nouveau (10 caractères minimum)<input className="field" type="password" value={pw.new} onChange={e => setPw({ ...pw, new: e.target.value })} minLength={10} required autoComplete="new-password" /></label>
          <button className="btn pri">Changer le mot de passe</button>
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
  const cfg = tok ? JSON.stringify({ mcpServers: { pop: { type: 'http', url: `${location.origin}/mcp`, headers: { Authorization: `Bearer ${tok}` } } } }, null, 2) : ''
  return (
    <section className="card">
      <CardHead title="IA connectée (serveur MCP de POP)" sub="Votre IA (Claude Code, Cursor…) peut lire et modifier les acteurs et le Verse comme vous : tout est journalisé, et une suppression attend la validation d’un humain dans l’Organiseur.">
        {canEdit && <button className="btn pri" onClick={async () => { const n = prompt('Nom du jeton', 'Mon IA'); if (n) { try { setTok((await api.post(`/api/maps/${map!.id}/tokens`, { name: n })).token); reload() } catch (e: any) { toast(e.message, true) } } }}>Créer un jeton</button>}
      </CardHead>
      {tok && <><p className="small muted">Copiez ce fichier .mcp.json maintenant : le jeton ne sera plus affiché.</p><pre className="report mono">{cfg}</pre></>}
      <div className="list">{data?.map(t => <div key={t.id} className="li"><span style={{ flex: 1 }}>{t.name}</span><span className="small muted">{t.created_at}</span><button className="iconbtn" aria-label="Révoquer" onClick={async () => { await api.del(`/api/tokens/${t.id}`); reload() }}><Trash2 size={16} /></button></div>)}</div>
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
    <CardHead title="Discord" sub="Rapport du soir automatique, commandes /tache-suivante, /fini, /rapport"><button className={'btn' + (data.enabled ? '' : ' pri')} disabled={!isAdmin} onClick={() => set('discord', !data.enabled, cfg)}>{data.enabled ? 'Désactiver' : 'Activer'}</button></CardHead>
    <span className={'badge' + (data.enabled ? ' b-g' : '')}>{data.enabled ? 'Activé' : 'Désactivé'}</span>
    {data.enabled && isAdmin && <>
      <div className="dsec">1 · Rapport du soir</div>
      <p className="small muted" style={{ margin: 0 }}>Discord → Paramètres du salon → Intégrations → Webhooks → Nouveau webhook → Copier l’URL.</p>
      <F label="URL du webhook" k="webhook" cfg={cfg} onSave={save} disabled={!isAdmin} placeholder="https://discord.com/api/webhooks/…" mono />
      <F label="Heure d’envoi (heure de Paris, vide = jamais)" k="report_time" cfg={cfg} onSave={save} disabled={!isAdmin} type="time" />
      <button className="btn" style={{ marginTop: 10 }} onClick={async () => { try { const r = await api.post(`/api/studios/${studio!.id}/discord/send`); toast(`${r.sent} rapport(s) envoyé(s) sur Discord`) } catch (e: any) { toast(e.message, true) } }}>Envoyer le rapport maintenant</button>
      <div className="dsec">2 · Commandes (application Discord)</div>
      <p className="small muted" style={{ margin: 0 }}>discord.com/developers → New Application. Copiez l’Application ID et la Public Key (General Information), puis un jeton de bot (Bot → Reset Token). Collez ensuite l’adresse ci-dessous dans « Interactions Endpoint URL » : POP doit être en ligne en HTTPS. Invitez le bot avec le scope applications.commands.</p>
      <F label="Application ID" k="app_id" cfg={cfg} onSave={save} disabled={!isAdmin} mono />
      <F label="Public Key" k="public_key" cfg={cfg} onSave={save} disabled={!isAdmin} mono />
      <F label="Jeton du bot" k="bot_token" cfg={cfg} onSave={save} disabled={!isAdmin} mono type="password" />
      <pre className="report mono" style={{ fontSize: 12, marginTop: 10 }}>{hook}</pre>
      <button className="btn" onClick={async () => { try { await api.post(`/api/studios/${studio!.id}/discord/register`); toast('Commandes enregistrées sur Discord') } catch (e: any) { toast(e.message, true) } }}>Enregistrer les commandes</button>
      <div className="dsec">Pseudo Discord de chaque membre</div>
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
    <CardHead title="GitHub" sub="Issues ↔ tâches dans les deux sens"><button className={'btn' + (data.enabled ? '' : ' pri')} disabled={!isAdmin} onClick={() => set('github', !data.enabled, cfg)}>{data.enabled ? 'Désactiver' : 'Activer'}</button></CardHead>
    <span className={'badge' + (data.enabled ? ' b-g' : '')}>{data.enabled ? 'Activé' : 'Désactivé'}</span>
    {data.enabled && isAdmin && <>
      <F label="Dépôt (propriétaire/nom)" k="repo" cfg={cfg} onSave={save} disabled={!isAdmin} placeholder="mon-studio/ma-map" mono />
      <p className="small muted" style={{ margin: '8px 0 0' }}>Jeton : GitHub → Settings → Developer settings → Fine-grained tokens, accès à ce dépôt, permission Issues en lecture et écriture.</p>
      <F label="Jeton GitHub" k="token" cfg={cfg} onSave={save} disabled={!isAdmin} mono type="password" />
      <button className="btn pri" style={{ marginTop: 10 }} disabled={busy} onClick={async () => { setBusy(true); try { const r = await api.post(`/api/studios/${studio!.id}/github/sync?map=${map!.id}`); toast(`GitHub : ${r.created} issue(s) créée(s), ${r.imported} importée(s), ${r.closed} fermée(s), ${r.done} tâche(s) faite(s)`) } catch (e: any) { toast(e.message, true) } finally { setBusy(false) } }}>{busy ? 'Synchronisation…' : 'Synchroniser maintenant'}</button>
      <div className="dsec">Mise à jour instantanée (POP en ligne)</div>
      <p className="small muted" style={{ margin: 0 }}>Dépôt → Settings → Webhooks → Add webhook : Payload URL ci-dessous, Content type application/json, Secret ci-dessous, événement « Issues ».</p>
      <pre className="report mono" style={{ fontSize: 12 }}>{`${location.origin}/api/hooks/github/${studio!.id}\nSecret : ${cfg.secret ?? '(activez puis rechargez)'}`}</pre>
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
    <CardHead title={`Studio ${studio.name}`} sub="Nom, sauvegarde, propriété" />
    {isAdmin && <form className="row" onSubmit={async e => { e.preventDefault(); try { await api.patch(`/api/studios/${studio.id}`, { name }); await reloadMe(); toast('Studio renommé') } catch (er: any) { toast(er.message, true) } }}>
      <label className="label" style={{ flex: 1 }}>Nom du studio<input className="field" value={name} onChange={e => setName(e.target.value)} required maxLength={80} /></label>
      <button className="btn" style={{ alignSelf: 'flex-end' }}>Renommer</button>
    </form>}
    {isAdmin && <><div className="dsec">Sauvegarde</div><p className="small muted" style={{ margin: 0 }}>Téléchargez tout le travail du studio (maps, tâches, pages, tableaux, Verse, gains) dans un fichier.</p>
      <a className="btn" style={{ marginTop: 8 }} href={`/api/studios/${studio.id}/export`} download>Exporter le studio</a></>}
    {owner && <><div className="dsec">Transférer la propriété</div>
      <div className="row"><select className="field" style={{ flex: 1 }} value={to} onChange={e => setTo(e.target.value)} aria-label="Nouveau propriétaire"><option value="">Choisir un membre…</option>{data?.members.filter((m: any) => m.id !== me?.user.id).map((m: any) => <option key={m.id} value={m.id}>{m.name}</option>)}</select>
        <button className="btn" disabled={!to} onClick={async () => { if (!confirm('Transférer la propriété ? Vous deviendrez admin.')) return; try { await api.post(`/api/studios/${studio.id}/transfer`, { user_id: Number(to) }); await reloadMe(); toast('Propriété transférée') } catch (er: any) { toast(er.message, true) } }}>Transférer</button></div></>}
    <div className="dsec">Zone sensible</div>
    <div className="row" style={{ flexWrap: 'wrap' }}>
      {!owner && <button className="btn" onClick={async () => { if (!confirm(`Quitter le studio ${studio.name} ?`)) return; try { await api.post(`/api/studios/${studio.id}/leave`); await reloadMe(); location.href = '/' } catch (er: any) { toast(er.message, true) } }}>Quitter le studio</button>}
      {owner && <button className="btn" style={{ color: 'var(--tr)' }} onClick={async () => { const c = prompt(`Supprimer définitivement le studio et TOUTES ses maps ?\nTapez « ${studio.name} » pour confirmer :`); if (c === null) return; try { await api.del(`/api/studios/${studio.id}`, { confirm: c }); await reloadMe(); location.href = '/' } catch (er: any) { toast(er.message, true) } }}>Supprimer le studio</button>}
    </div>
  </section>
}
