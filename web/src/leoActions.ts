// Léo comprend des demandes d'action, comme Siri, sans IA externe (#552) : « crée une tâche test pour demain »,
// « ouvre le planning », « marque #12 comme faite », « assigne #8 à Léa », « déplace #5 à vendredi »…
// Rien ne change avant le clic de l'utilisateur : chaque action est d'abord proposée, puis lancée sur « Confirmer ».
import { api } from './api'
import { tr, LOCALE } from './i18n/t'

export type Ctx = { mapId?: number; studioId?: number; meId: number }
/** Réponse de Léo à une demande d'action : aller sur une page, ou une action à confirmer. */
export type Reponse =
  | { kind: 'nav'; to: string; text: string }
  | { kind: 'action'; text: string; bouton: string; run: () => Promise<{ text: string; to?: string }> }
  | { kind: 'info'; text: string }

/** Minuscules, sans accents ni apostrophes typographiques. */
export const norm = (s: string) => s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[’`]/g, "'")
const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
const jour = (s: string) => new Date(s + 'T12:00:00').toLocaleDateString(LOCALE, { weekday: 'long', day: 'numeric', month: 'long' })
const plus = (n: number) => { const d = new Date(); d.setDate(d.getDate() + n); return d }

const JOURS: Record<string, number> = { dimanche: 0, lundi: 1, mardi: 2, mercredi: 3, jeudi: 4, vendredi: 5, samedi: 6, sunday: 0, monday: 1, tuesday: 2, wednesday: 3, thursday: 4, friday: 5, saturday: 6 }
const MOIS: Record<string, number> = { janvier: 1, janv: 1, jan: 1, january: 1, fevrier: 2, fevr: 2, fev: 2, feb: 2, february: 2, mars: 3, mar: 3, march: 3, avril: 4, avr: 4, apr: 4, april: 4, mai: 5, may: 5, juin: 6, june: 6, jun: 6, juillet: 7, juil: 7, july: 7, jul: 7, aout: 8, aug: 8, august: 8, septembre: 9, sept: 9, sep: 9, september: 9, octobre: 10, oct: 10, october: 10, novembre: 11, nov: 11, november: 11, decembre: 12, dec: 12, december: 12 }

/** Trouve une date dans le texte (déjà normalisé) ; renvoie la date et le texte sans elle. */
export function trouveDate(t: string): { date?: string; reste: string } {
  const prefixe = "(?:(?:pour|pr|a|au|le|d'ici|avant|on|for|by|due)\\s+)?"
  const essais: [RegExp, (m: RegExpMatchArray) => Date | null][] = [
    [/(\d{4})-(\d{2})-(\d{2})/, m => new Date(+m[1], +m[2] - 1, +m[3])],
    [/\b(\d{1,2})\/(\d{1,2})(?:\/(\d{2,4}))?\b/, m => { const y = m[3] ? (+m[3] < 100 ? 2000 + +m[3] : +m[3]) : new Date().getFullYear(); const d = new Date(y, +m[2] - 1, +m[1]); if (!m[3] && d < plus(-1)) d.setFullYear(y + 1); return d }],
    [/\bapres[- ]demain\b|\bday after tomorrow\b/, () => plus(2)],
    [/\bdemain\b|\btomorrow\b/, () => plus(1)],
    [/\baujourd'?hui\b|\bce soir\b|\btoday\b|\btonight\b/, () => plus(0)],
    [/\bdans (\d+) jours?\b|\bin (\d+) days?\b/, m => plus(+(m[1] ?? m[2]))],
    [/\bdans (\d+) semaines?\b|\bin (\d+) weeks?\b/, m => plus(7 * +(m[1] ?? m[2]))],
    [/\b(?:la )?semaine prochaine\b|\bnext week\b/, () => { const d = plus(1); while (d.getDay() !== 1) d.setDate(d.getDate() + 1); return d }],
    [/\b(\d{1,2})(?:er)? (janvier|janv|fevrier|fevr|fev|mars|avril|avr|mai|juin|juillet|juil|aout|septembre|sept|octobre|oct|novembre|nov|decembre|dec|january|february|march|april|may|june|july|august|september|october|november|december|jan|feb|mar|apr|jun|jul|aug|sep)\b/, m => { const y = new Date().getFullYear(); const d = new Date(y, MOIS[m[2]] - 1, +m[1]); if (d < plus(-1)) d.setFullYear(y + 1); return d }],
    [/\b(?:(?:ce|next|this) )?(lundi|mardi|mercredi|jeudi|vendredi|samedi|dimanche|monday|tuesday|wednesday|thursday|friday|saturday|sunday)(?: prochain)?\b/, m => { const d = plus(1); while (d.getDay() !== JOURS[m[1]]) d.setDate(d.getDate() + 1); return d }],
  ]
  for (const [re, f] of essais) {
    const full = new RegExp(prefixe + re.source)
    const m = t.match(full)
    if (!m) continue
    const inner = m[0].match(re)!
    const d = f(inner)
    if (d && !isNaN(+d)) return { date: iso(d), reste: (t.slice(0, m.index) + ' ' + t.slice(m.index! + m[0].length)).replace(/\s+/g, ' ').trim() }
  }
  return { reste: t }
}

type Membre = { id: number; name: string }
let cacheMembres: { sid: number; at: number; list: Membre[] } | null = null
async function membres(sid?: number): Promise<Membre[]> {
  if (!sid) return []
  if (cacheMembres?.sid === sid && Date.now() - cacheMembres.at < 60_000) return cacheMembres.list
  const r = await api.get(`/api/studios/${sid}/members`).catch(() => null)
  const list = (r?.members ?? []).map((m: any) => ({ id: m.id, name: m.name }))
  cacheMembres = { sid, at: Date.now(), list }
  return list
}
/** « pour Léa », « à moi », « to Hugo » : la personne et le texte sans elle. */
async function trouvePersonne(t: string, ctx: Ctx): Promise<{ who?: Membre; reste: string }> {
  const m = t.match(/\b(?:pour|a|assigne[e]?s? a|to|for)\s+(moi|me|myself|[a-z][a-z'-]+)\b/)
  if (!m) return { reste: t }
  const cut = () => (t.slice(0, m.index) + ' ' + t.slice(m.index! + m[0].length)).replace(/\s+/g, ' ').trim()
  if (['moi', 'me', 'myself'].includes(m[1])) return { who: { id: ctx.meId, name: tr("vous") }, reste: cut() }
  const who = (await membres(ctx.studioId)).find(x => norm(x.name).split(/\s+/)[0] === m[1] || norm(x.name) === m[1])
  return who ? { who, reste: cut() } : { reste: t }
}

/** Numéro de tâche : « #12 », « tâche 12 », « task 12 ». */
const numero = (t: string) => { const m = t.match(/#\s?(\d+)|\b(?:tache|task|ticket)\s+(?:n[°o]?\s?)?(\d+)\b/); return m ? +(m[1] ?? m[2]) : null }
async function tache(mapId: number, n: number): Promise<any | null> {
  const list: any[] = await api.get(`/api/maps/${mapId}/tasks`).catch(() => [])
  return (Array.isArray(list) ? list : (list as any)?.tasks ?? []).find((x: any) => x.number === n) ?? null
}
/** Garde la casse d'origine pour un titre trouvé dans le texte normalisé. */
const original = (src: string, normTitre: string) => {
  const n = norm(src), i = n.indexOf(normTitre)
  return i >= 0 ? src.slice(i, i + normTitre.length) : normTitre
}
const nettoie = (s: string) => s.replace(/^[\s:,-]+|[\s:,.!?-]+$/g, '').replace(/^(?:nommee?|appelee?|intitulee?|qui s'appelle|called|named|titled)\s+/, '').replace(/^["«“]\s*|\s*["»”]$/g, '').trim()

const PAGES: [RegExp, string, () => string][] = [
  [/\b(planning|planifier|calendrier|calendar|plan)\b/, '/planifier', () => tr("Planifier")],
  [/\b(taches|tache|kanban|tasks?)\b/, '/taches', () => tr("Tâches")],
  [/\b(pages?|docs?|documentation)\b/, '/pages', () => tr("Pages")],
  [/\b(tableau blanc|tableau|whiteboard|board)\b/, '/tableau', () => tr("Tableau blanc")],
  [/\b(lancer|lancement|sortie|launch|discover)\b/, '/lancer', () => tr("Lancer et suivre")],
  [/\b(remunerer|remuneration|gains|paiements?|revenus|payouts?|earnings)\b/, '/remunerer', () => tr("Rémunérer")],
  [/\b(abonnement|facturation|subscription|billing)\b/, '/parametres?onglet=abonnement', () => tr("Abonnement")],
  [/\b(parametres|reglages|settings)\b/, '/parametres', () => tr("Paramètres")],
  [/\b(aide|support|help)\b/, '/aide', () => tr("Aide et support")],
  [/\b(maps a faire|idees|analyse|tendances|ideas|trends)\b/, '/analyse/a-faire', () => tr("Maps à faire")],
  [/\b(forge|relais|uefn)\b/, '/parametres?onglet=uefn', () => tr("POP Forge")],
  [/\b(dashboard|accueil|tableau de bord|home)\b/, '', () => tr("Dashboard")],
]

const STATUTS: [RegExp, string, () => string][] = [
  [/\b(termine[er]?|finie?s?|faite?s?|fait|done|complete[d]?|finish(?:ed)?|valide[er]?)\b/, 'done', () => tr("Fait")],
  [/\b(commence[r]?|demarre[r]?|en cours|start(?:ed)?|in progress|doing)\b/, 'doing', () => tr("En cours")],
  [/\b(bloquee?s?|bloque[r]?|blocked|block)\b/, 'blocked', () => tr("Bloqué")],
  [/\b(a faire|rouvre|rouvrir|reouvre|todo|to do|reopen)\b/, 'todo', () => tr("À faire")],
]

/** Comprend une demande d'action. Renvoie null si ce n'est pas une action (Léo répond alors avec ses fiches). */
export async function comprendre(texte: string, ctx: Ctx): Promise<Reponse | null> {
  const t = norm(texte).replace(/\s+/g, ' ').trim()
  // Une question sur la manière de faire (« comment créer une tâche ? ») reste une question.
  if (/^(comment|pourquoi|c'est quoi|qu'est-ce|how|why|what is)\b/.test(t)) return null
  const creer = /\b(cree[rz]?|creer|ajoute[rz]?|ajouter|nouvelle?|nouveau|mets?|programme[rz]?|create|add|new|make)\b/

  // 1. Créer une tâche.
  if (creer.test(t) && /\b(taches?|tasks?|todo)\b/.test(t)) {
    if (!ctx.mapId) return { kind: 'info', text: tr("Ouvrez d’abord une map : la tâche y sera créée.") }
    const apres = t.replace(/^.*?\b(?:taches?|tasks?|todo)\b/, '')
    const d = trouveDate(apres)
    const p = await trouvePersonne(d.reste, ctx)
    const titre = original(texte, nettoie(p.reste))
    if (!titre) return { kind: 'info', text: tr("Quel est le titre de la tâche ? Par exemple : « crée une tâche Ajouter le spawn pour demain ».") }
    const detail = [d.date ? tr("pour {0}", [jour(d.date)]) : '', p.who ? tr("assignée à {0}", [p.who.name]) : ''].filter(Boolean).join(', ')
    return {
      kind: 'action', bouton: tr("Créer la tâche"),
      text: tr("Je crée la tâche « {0} »{1}.", [titre, detail ? ' ' + detail : '']),
      run: async () => {
        const r = await api.post(`/api/maps/${ctx.mapId}/tasks`, { title: titre, planned_day: d.date, assignee_id: p.who?.id })
        return { text: tr("C’est fait : tâche #{0} « {1} » créée.", [String(r.number ?? ''), titre]), to: r.id ? `/taches?t=${r.id}` : '/taches' }
      },
    }
  }

  // 2. Créer une page ou un tableau blanc.
  const estTableau = /\b(tableau blanc|tableau|whiteboard|board)\b/.test(t)
  if (creer.test(t) && (estTableau || /\b(page|doc|document|note)\b/.test(t)) && !/\b(objectif|jalon|milestone)\b/.test(t)) {
    if (!ctx.mapId) return { kind: 'info', text: tr("Ouvrez d’abord une map.") }
    const apres = t.replace(/^.*?\b(?:tableau blanc|tableau|whiteboard|board|page|doc|document|note)\b/, '')
    const titre = original(texte, nettoie(apres))
    return {
      kind: 'action', bouton: estTableau ? tr("Créer le tableau") : tr("Créer la page"),
      text: estTableau ? (titre ? tr("Je crée le tableau blanc « {0} ».", [titre]) : tr("Je crée un tableau blanc vide.")) : (titre ? tr("Je crée la page « {0} ».", [titre]) : tr("Je crée une page vide.")),
      run: async () => {
        if (estTableau) {
          const r = await api.post(`/api/maps/${ctx.mapId}/boards`, titre ? { title: titre } : {})
          return { text: tr("Tableau blanc créé."), to: `/tableau/${r.id}` }
        }
        const r = await api.post(`/api/maps/${ctx.mapId}/pages`, titre ? { title: titre } : {})
        return { text: tr("Page créée."), to: `/pages/${r.id}` }
      },
    }
  }

  // 3. Ajouter un objectif (jalon) à une date.
  if (creer.test(t) && /\b(objectif|jalon|date de sortie|milestone|deadline|sortie)\b/.test(t)) {
    if (!ctx.mapId) return { kind: 'info', text: tr("Ouvrez d’abord une map.") }
    const apres = t.replace(/^.*?\b(?:objectif|jalon|date de sortie|milestone|deadline|sortie)\b/, '')
    const d = trouveDate(apres)
    if (!d.date) return { kind: 'info', text: tr("Pour quelle date ? Par exemple : « ajoute un objectif Bêta le 15 décembre ».") }
    const titre = original(texte, nettoie(d.reste)) || tr("Objectif")
    return {
      kind: 'action', bouton: tr("Ajouter l’objectif"),
      text: tr("J’ajoute l’objectif « {0} » pour {1}.", [titre, jour(d.date)]),
      run: async () => { await api.post(`/api/maps/${ctx.mapId}/milestones`, { title: titre, date: d.date }); return { text: tr("Objectif ajouté."), to: '/planifier' } },
    }
  }

  const n = numero(t)
  // 4. Changer le statut, assigner, replanifier ou commenter une tâche existante (« #12 »).
  if (n != null && ctx.mapId) {
    const charge = async () => { const x = await tache(ctx.mapId!, n); if (!x) throw new Error(tr("Je ne trouve pas la tâche #{0} dans cette map.", [String(n)])); return x }
    if (/\b(commente[rz]?|comment(?:e|er)?|note)\b/.test(t)) {
      const corps = original(texte, nettoie(t.replace(/^.*?#\s?\d+|^.*?\b(?:tache|task)\s+\d+/, '')))
      if (!corps) return { kind: 'info', text: tr("Que voulez-vous écrire ? Par exemple : « commente #12 : spawn corrigé ».") }
      return { kind: 'action', bouton: tr("Commenter"), text: tr("J’ajoute ce commentaire sur #{0} : « {1} ».", [String(n), corps]),
        run: async () => { const x = await charge(); await api.post(`/api/tasks/${x.id}/comments`, { body: corps }); return { text: tr("Commentaire ajouté sur #{0}.", [String(n)]), to: `/taches?t=${x.id}` } } }
    }
    if (/\b(assigne[rz]?|attribue[rz]?|donne[rz]?|confie[rz]?|assign|give)\b/.test(t)) {
      const p = await trouvePersonne(t, ctx)
      if (!p.who) return { kind: 'info', text: tr("À qui ? Donnez le prénom d’un membre du studio, ou « moi ».") }
      return { kind: 'action', bouton: tr("Assigner"), text: tr("J’assigne #{0} à {1}.", [String(n), p.who.name]),
        run: async () => { const x = await charge(); await api.patch(`/api/tasks/${x.id}`, { assignee_id: p.who!.id }); return { text: tr("#{0} est assignée à {1}.", [String(n), p.who!.name]), to: `/taches?t=${x.id}` } } }
    }
    if (/\b(deplace[rz]?|decale[rz]?|reporte[rz]?|planifie[rz]?|prevois|move|reschedule|schedule|postpone)\b/.test(t)) {
      const d = trouveDate(t.replace(/#\s?\d+/, ''))
      if (!d.date) return { kind: 'info', text: tr("Pour quel jour ? Par exemple : « déplace #{0} à vendredi ».", [String(n)]) }
      return { kind: 'action', bouton: tr("Déplacer"), text: tr("Je prévois #{0} pour {1}.", [String(n), jour(d.date)]),
        run: async () => { const x = await charge(); await api.patch(`/api/tasks/${x.id}`, { planned_day: d.date }); return { text: tr("#{0} est prévue pour {1}.", [String(n), jour(d.date!)]), to: `/taches?t=${x.id}` } } }
    }
    const s = STATUTS.find(([re]) => re.test(t.replace(/#\s?\d+/, '')))
    if (s) return { kind: 'action', bouton: tr("Confirmer"), text: tr("Je passe #{0} en « {1} ».", [String(n), s[2]()]),
      run: async () => { const x = await charge(); await api.post(`/api/tasks/${x.id}/status`, { status: s[1] }); return { text: tr("#{0} est maintenant « {1} ».", [String(n), s[2]()]), to: `/taches?t=${x.id}` } } }
    if (/\b(ouvre|affiche|montre|voir|open|show)\b/.test(t)) {
      const x = await tache(ctx.mapId, n)
      return x ? { kind: 'nav', to: `/taches?t=${x.id}`, text: tr("J’ouvre la tâche #{0}.", [String(n)]) } : { kind: 'info', text: tr("Je ne trouve pas la tâche #{0} dans cette map.", [String(n)]) }
    }
  }

  // 5. Aller sur une page de POP.
  if (/^(ouvre|ouvrir|va|aller|allons|affiche|montre|emmene|amene|direction|go|open|show|take me)\b/.test(t)) {
    const p = PAGES.find(([re]) => re.test(t))
    if (p) {
      const to = p[1] === '' ? (ctx.mapId ? `/map/${ctx.mapId}` : '/') : p[1]
      return { kind: 'nav', to, text: tr("Direction {0}.", [p[2]()]) }
    }
  }
  return null
}
