// Logos des services reliés à POP (#200, #205) : icônes de la bibliothèque simple-icons (CC0), en une couleur
// (currentColor) pour rester dans le style noir et blanc de POP. Un service sans logo garde une icône générique.
import { siClaude, siCursor, siDiscord, siEpicgames, siGithub, siGithubcopilot, siGooglegemini, siWindsurf } from 'simple-icons'
import { Bot } from 'lucide-react'

const LOGOS: Record<string, { path: string; title: string }> = {
  discord: siDiscord, github: siGithub, lore: siEpicgames, claude: siClaude, cursor: siCursor,
  windsurf: siWindsurf, gemini: siGooglegemini, copilot: siGithubcopilot,
}

export function Brand({ name, size = 18 }: { name: string; size?: number }) {
  const l = LOGOS[name]
  if (!l) return <Bot size={size} strokeWidth={1.6} aria-hidden="true" />
  return <svg className="blogo" width={size} height={size} viewBox="0 0 24 24" fill="currentColor" role="img" aria-label={l.title}><path d={l.path} /></svg>
}
