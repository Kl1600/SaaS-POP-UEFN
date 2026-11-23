// Logos des IA connectées (#260) : bibliothèque @lobehub/icons-static-svg (licence MIT), en couleur quand le
// logo officiel en a une ; Cursor, Windsurf et MCP n'existent qu'en une couleur et suivent le thème (currentColor).
import claudeCode from '@lobehub/icons-static-svg/icons/claudecode-color.svg?raw'
import claude from '@lobehub/icons-static-svg/icons/claude-color.svg?raw'
import cursor from '@lobehub/icons-static-svg/icons/cursor.svg?raw'
import codex from '@lobehub/icons-static-svg/icons/codex-color.svg?raw'
import windsurf from '@lobehub/icons-static-svg/icons/windsurf.svg?raw'
import copilot from '@lobehub/icons-static-svg/icons/copilot-color.svg?raw'
import gemini from '@lobehub/icons-static-svg/icons/geminicli-color.svg?raw'
import mcp from '@lobehub/icons-static-svg/icons/mcp.svg?raw'

const SVG: Record<string, string> = {
  claude_code: claudeCode, claude_desktop: claude, cursor, codex, windsurf, vscode: copilot, gemini, autre: mcp,
}

/** Logo d'une IA par son type de client ; un type inconnu prend le logo MCP. Les fichiers viennent du paquet, pas d'une saisie. */
export function AiLogo({ kind, size = 22 }: { kind: string; size?: number }) {
  const svg = (SVG[kind] ?? mcp).replace(/height="1em"/, `height="${size}"`).replace(/width="1em"/, `width="${size}"`)
  return <span className="ailogo" role="img" aria-hidden="true" style={{ width: size, height: size }} dangerouslySetInnerHTML={{ __html: svg }} />
}
