// Nouveautés de POP (#557) : seulement des changements livrés, du plus récent au plus ancien.
// Pour un changement visible par les utilisateurs, ajoutez une entrée en tête (date du jour de la fusion).
import { tr } from './i18n/t'

export type Nouveaute = { date: string; tag: 'nouveau' | 'ameliore' | 'corrige'; titre: string; texte: string; lien?: string }

export const NOUVEAUTES = (): Nouveaute[] => [
  { date: '2026-11-28', tag: 'nouveau', titre: tr("Léo fait ce que vous lui demandez"), texte: tr("« Crée une tâche test pour demain », « ouvre le planning », « marque #12 comme faite », « assigne #8 à Léa »… Léo vous montre ce qu’il va faire et attend votre clic."), },
  { date: '2026-11-28', tag: 'nouveau', titre: tr("Popy devient Léo"), texte: tr("L’assistant a un nouveau nom et un visage : ses yeux suivent la souris. Son panneau s’ouvre en bas à droite, depuis son bouton."), },
  { date: '2026-11-28', tag: 'nouveau', titre: tr("Un didacticiel sur chaque onglet"), texte: tr("À la première visite d’un onglet, quelques bulles montrent l’essentiel ; le reste de la page est flouté et la page défile toute seule jusqu’à la zone montrée."), lien: '/parametres' },
  { date: '2026-11-28', tag: 'nouveau', titre: tr("Barre latérale à votre façon"), texte: tr("Repliez une catégorie en cliquant son titre, déplacez un onglet dans sa catégorie avec la poignée ⋮⋮. L’ordre est gardé sur votre compte."), },
  { date: '2026-11-28', tag: 'ameliore', titre: tr("Nouvelles icônes et barre latérale plus large"), texte: tr("Des icônes au trait fin, une barre latérale plus aérée, et les menus des maps et du compte qui s’ouvrent à droite, dans la page."), },
  { date: '2026-11-28', tag: 'nouveau', titre: tr("Abonnement annuel"), texte: tr("Le prix ramené au jour sous chaque plan, et le choix Mensuel / Annuel avec 2 mois offerts quand il est proposé."), lien: '/parametres?onglet=abonnement' },
  { date: '2026-11-28', tag: 'ameliore', titre: tr("UEFN Link s’appelle maintenant POP Forge"), texte: tr("Partout dans POP, le relais vers UEFN porte le nom de l’application qui le contient : POP Forge."), lien: '/parametres?onglet=uefn' },
  { date: '2026-11-27', tag: 'ameliore', titre: tr("Calendrier de Planifier plus clair"), texte: tr("L’avis de faisabilité n’encombre plus le calendrier ; il reste dans « Équipe et faisabilité » et sur le Dashboard."), lien: '/planifier' },
  { date: '2026-11-26', tag: 'ameliore', titre: tr("Ouvrir une tâche depuis le calendrier"), texte: tr("Dans Planifier, « Ouvrir la tâche » affiche son détail sur place, sans quitter le calendrier."), lien: '/planifier' },
  { date: '2026-11-26', tag: 'ameliore', titre: tr("Plus de crédits IA"), texte: tr("POP n’utilise plus de crédits : l’IA de POP reste locale, et vous connectez votre propre IA si vous le souhaitez."), },
  { date: '2026-11-25', tag: 'nouveau', titre: tr("Toutes les IA se connectent à POP"), texte: tr("Claude, ChatGPT, Codex, Gemini, Cursor… une seule adresse, une connexion en 3 étapes, et la liste des IA connectées pour les révoquer."), lien: '/parametres?onglet=ia' },
  { date: '2026-11-25', tag: 'ameliore', titre: tr("Dashboard épuré"), texte: tr("Le Dashboard va à l’essentiel : chiffres clés, tâches du jour, objectifs et avancement."), },
  { date: '2026-11-25', tag: 'corrige', titre: tr("Styles du site"), texte: tr("Le site s’affichait sans mise en page après une mise à jour : c’est réparé."), },
]

/** Date de la nouveauté la plus récente (pour le point « non vu »). */
export const DERNIERE = () => NOUVEAUTES()[0]?.date ?? ''
