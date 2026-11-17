// Langage Verse pour Monaco : coloration, autocomplétion tirée des digests, infobulles, snippets,
// modèles de modules et fiches d'aide des erreurs de compilation fréquentes.
import * as monaco from 'monaco-editor'
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import { tr } from './i18n/t'

;(self as any).MonacoEnvironment = { getWorker: () => new editorWorker() }

export type Sym = { name: string; kind: string; signature: string; effects: string[]; doc: string; container: string; module: string }
export type Ctx = { symbols: Sym[]; tags: string[]; devices: { name: string; class: string }[] }

let registered = false
export const ctx: Ctx = { symbols: [], tags: [], devices: [] }

const KEYWORDS = ['using', 'class', 'struct', 'enum', 'interface', 'module', 'if', 'else', 'then', 'for', 'loop', 'block', 'break', 'return', 'var', 'set', 'case', 'spawn', 'sync', 'race', 'rush', 'branch', 'defer', 'not', 'and', 'or', 'where', 'of', 'ref', 'external', 'self', 'super', 'option', 'array', 'map', 'tuple', 'type', 'subtype', 'false', 'true', 'at', 'do']
const TYPES = ['int', 'float', 'string', 'logic', 'void', 'agent', 'any', 'char', 'rational', 'comparable', 'message', 'player', 'fort_character', 'creative_device', 'listenable', 'cancelable', 'event', 'transform', 'vector3', 'rotation']
const EFFECT_HELP: Record<string, string> = {
  decides: tr("peut échouer : à appeler dans un contexte qui gère l’échec (if, [ ], or…)."),
  transacts: tr("peut lire et écrire l’état ; annulable en cas d’échec."),
  suspends: tr("peut attendre (asynchrone) : à appeler depuis une fonction <suspends>, spawn ou une branche concurrente."),
  computes: tr("calcul pur, sans effet de bord."),
  varies: tr("peut renvoyer un résultat différent à chaque appel."),
  no_rollback: tr("ne peut pas être annulé : interdit dans un contexte <decides>."),
}

export const SNIPPETS: [string, string, string][] = [
  ['creative_device', tr("Appareil Verse"), tr("using { /Fortnite.com/Devices }\nusing { /Verse.org/Simulation }\n\n${1:my_device} := class(creative_device):\n\n    OnBegin<override>()<suspends> : void =\n        ${0:Print(\"Prêt\")}\n")],
  ['class', tr("Classe"), tr("${1:my_class} := class:\n    var ${2:Value} : ${3:int} = ${4:0}\n\n    ${5:Method}() : void =\n        ${0}\n")],
  ['for', tr("Boucle for"), tr("for (${1:Item} : ${2:Items}):\n    ${0}\n")],
  ['if', tr("Condition if"), tr("if (${1:Condition[]}):\n    ${2}\nelse:\n    ${0}\n")],
  ['loop', tr("Boucle loop"), tr("loop:\n    ${1:Sleep(1.0)}\n    ${0}\n")],
  ['spawn', tr("spawn (tâche asynchrone)"), 'spawn{ ${1:Task}() }\n'],
  ['subscribe', tr("Abonnement à un événement"), '${1:Device}.${2:InteractedWithEvent}.Subscribe(${3:OnInteracted})\n'],
  ['handler', tr("Classe handler"), tr("${1:my}_handler := class:\n    ${2:Target} : ${3:creative_device}\n\n    Handle(Agent : agent) : void =\n        ${0}\n")],
]

export const MODULES: [string, string, string][] = [
  ['economy', tr("Économie"), tr("using { /Fortnite.com/Devices }\nusing { /Verse.org/Simulation }\n\n# Monnaie par joueur : gagner, dépenser, consulter.\neconomy_manager := class(creative_device):\n\n    var Balances : [agent]int = map{}\n\n    Earn(Agent : agent, Amount : int) : void =\n        Current := Balances[Agent] or 0\n        if (set Balances[Agent] = Current + Amount) {}\n\n    Spend(Agent : agent, Amount : int)<decides><transacts> : void =\n        Current := Balances[Agent]\n        Current >= Amount\n        set Balances[Agent] = Current - Amount\n\n    GetBalance(Agent : agent) : int =\n        Balances[Agent] or 0\n")],
  ['inventory', tr("Inventaire"), tr("using { /Verse.org/Simulation }\n\n# Objets possédés par joueur.\ninventory_manager := class(creative_device):\n\n    var Items : [agent][]string = map{}\n\n    Add(Agent : agent, Item : string) : void =\n        Current := Items[Agent] or array{}\n        if (set Items[Agent] = Current + array{Item}) {}\n\n    Has(Agent : agent, Item : string)<decides><transacts> : void =\n        List := Items[Agent]\n        List.Find[Item]\n")],
  ['hud', 'HUD', tr("using { /Fortnite.com/Devices }\nusing { /Fortnite.com/UI }\nusing { /UnrealEngine.com/Temporary/UI }\nusing { /Verse.org/Simulation }\n\n# Affiche un texte à l’écran du joueur.\nhud_manager := class(creative_device):\n\n    MakeLabel<localizes>(Value : string) : message = \"{Value}\"\n\n    Show(Player : player, Value : string) : void =\n        if (PlayerUI := GetPlayerUI[Player]):\n            Widget := text_block{DefaultText := MakeLabel(Value)}\n            PlayerUI.AddWidget(Widget)\n")],
  ['save', tr("Sauvegarde du profil"), tr("using { /Verse.org/Simulation }\n\n# Données persistantes par joueur (weak_map de session persistante).\nplayer_profile := class<final><persistable>:\n    Gold : int = 0\n    Level : int = 1\n\nvar Profiles : weak_map(player, player_profile) = map{}\n\nprofile_manager := class(creative_device):\n\n    Load(Player : player) : player_profile =\n        Profiles[Player] or player_profile{}\n\n    Save(Player : player, Profile : player_profile) : void =\n        if (set Profiles[Player] = Profile) {}\n")],
  ['teams', tr("Équipes"), tr("using { /Fortnite.com/Devices }\nusing { /Fortnite.com/Teams }\nusing { /Verse.org/Simulation }\n\n# Répartition et lecture des équipes.\nteam_manager := class(creative_device):\n\n    GetTeamIndex(Agent : agent)<decides><transacts> : int =\n        Teams := GetPlayspace().GetTeamCollection().GetTeams()\n        Team := GetPlayspace().GetTeamCollection().GetTeam[Agent]\n        for (Index -> T : Teams, T = Team):\n            Index\n        0\n")],
  ['timer', tr("Minuteur"), tr("using { /Fortnite.com/Devices }\nusing { /Verse.org/Simulation }\n\n# Compte à rebours de manche.\nround_timer := class(creative_device):\n\n    @editable\n    Seconds : float = 480.0\n\n    OnBegin<override>()<suspends> : void =\n        Sleep(Seconds)\n        Print(\"Fin de la manche\")\n")],
]

// Fiches d'aide des erreurs fréquentes (le code dépend de la version d'UEFN ; la détection se fait aussi par mots-clés).
export const ERROR_HELP: { match: (code: string, msg: string) => boolean; title: string; why: string; before: string; after: string }[] = [
  { match: (c, m) => c === '3512' || /no_rollback/i.test(m), title: tr("Effet no_rollback dans une condition"), why: tr("Une condition (if, [ ]) peut être annulée si elle échoue. Une fonction no_rollback (comme Print ou un appel d’appareil) ne peut pas l’être : sortez-la de la condition."), before: tr("if (Print(\"x\"), Player := Agent[]):\n    …"), after: tr("if (Player := player[Agent]):\n    Print(\"x\")") },
  { match: (c, m) => /ambig/i.test(m), title: tr("Identifiant ambigu"), why: tr("Le même nom existe dans plusieurs modules importés. Précisez le chemin complet ou renommez votre déclaration."), before: tr("Teams := GetTeams()"), after: tr("Teams := GetPlayspace().GetTeamCollection().GetTeams()") },
  { match: (c, m) => /incompatible|expected .* but|type mismatch|n’est pas du type/i.test(m), title: tr("Type incompatible"), why: tr("La valeur n’a pas le type attendu. Convertissez-la (Floor, Int, ToString) ou changez le type déclaré."), before: tr("var Score : int = 1.5"), after: tr("var Score : float = 1.5   # ou Floor[1.5]") },
  { match: (c, m) => /effect|effet/i.test(m) && !/no_rollback/i.test(m), title: tr("Effet manquant"), why: tr("La fonction appelle quelque chose qui a un effet (<suspends>, <decides>, <transacts>) qu’elle ne déclare pas. Ajoutez l’effet à sa signature ou appelez-la depuis le bon contexte."), before: tr("Wait() : void =\n    Sleep(1.0)"), after: tr("Wait()<suspends> : void =\n    Sleep(1.0)") },
  { match: (c, m) => c === '3506' || /unknown identifier|identifiant inconnu/i.test(m), title: tr("Identifiant inconnu"), why: tr("Le nom n’est pas visible ici : il manque souvent un using { … } en haut du fichier, ou il y a une faute de frappe."), before: tr("my_device := class(creative_device):"), after: tr("using { /Fortnite.com/Devices }\n\nmy_device := class(creative_device):") },
  { match: (c) => c === '3100', title: tr("Parenthèse, crochet ou accolade non fermé"), why: tr("Chaque ( [ { doit être refermé sur la même expression. Vérifiez la ligne indiquée et la précédente."), before: tr("Print(\"x\""), after: tr("Print(\"x\")") },
]

export function helpFor(code: string, message: string) {
  return ERROR_HELP.find(h => h.match(code ?? '', message ?? ''))
}

export function setup() {
  if (registered) return
  registered = true
  monaco.languages.register({ id: 'verse', extensions: ['.verse'] })
  monaco.languages.setLanguageConfiguration('verse', {
    comments: { lineComment: '#', blockComment: ['<#', '#>'] },
    brackets: [['{', '}'], ['[', ']'], ['(', ')']],
    autoClosingPairs: [{ open: '{', close: '}' }, { open: '[', close: ']' }, { open: '(', close: ')' }, { open: '"', close: '"' }],
    indentationRules: { increaseIndentPattern: /:\s*$|=\s*$/, decreaseIndentPattern: /^\s*(else)\b/ },
    onEnterRules: [{ beforeText: /[:=]\s*$/, action: { indentAction: monaco.languages.IndentAction.Indent } }],
  })
  monaco.languages.setMonarchTokensProvider('verse', {
    keywords: KEYWORDS, typeKeywords: TYPES,
    tokenizer: {
      root: [
        [/<#/, 'comment', '@block'],
        [/#.*$/, 'comment'],
        [/@[a-z_]+/, 'annotation'],
        [/<(public|private|protected|internal|override|final|concrete|abstract|unique|persistable|localizes|native|epic_internal|decides|transacts|suspends|computes|varies|reads|writes|allocates|converges|no_rollback|constructor|module_scoped_var_weak_map_key)>/, 'keyword.effect'],
        [/[a-z_][\w]*(?=\s*:=\s*(class|struct|enum|interface|module))/, 'type.identifier'],
        [/[A-Za-z_][\w]*/, { cases: { '@keywords': 'keyword', '@typeKeywords': 'type', '@default': 'identifier' } }],
        [/\d+\.\d+([eE][-+]?\d+)?/, 'number.float'],
        [/0x[0-9a-fA-F]+|\d+/, 'number'],
        [/"/, 'string', '@string'],
        [/:=|=>|->|[=<>!+\-*/:.]+/, 'operator'],
      ],
      string: [[/\{[^}]*\}/, 'variable'], [/[^"{]+/, 'string'], [/"/, 'string', '@pop']],
      block: [[/#>/, 'comment', '@pop'], [/./, 'comment']],
    },
  } as any)
  monaco.editor.defineTheme('pop-light', { base: 'vs', inherit: true, rules: [{ token: 'keyword.effect', foreground: '8a5a00' }, { token: 'annotation', foreground: '7a3e9d' }, { token: 'type.identifier', foreground: '0a6a8a', fontStyle: 'bold' }], colors: {} })
  monaco.editor.defineTheme('pop-dark', { base: 'vs-dark', inherit: true, rules: [{ token: 'keyword.effect', foreground: 'e5b567' }, { token: 'annotation', foreground: 'c39ad9' }, { token: 'type.identifier', foreground: '6cc3e0', fontStyle: 'bold' }], colors: { 'editor.background': '#0a0a0a' } })

  monaco.languages.registerCompletionItemProvider('verse', {
    triggerCharacters: ['.', '{'],
    provideCompletionItems: (model, pos) => {
      const word = model.getWordUntilPosition(pos)
      const range = new monaco.Range(pos.lineNumber, word.startColumn, pos.lineNumber, word.endColumn)
      const before = model.getLineContent(pos.lineNumber).slice(0, word.startColumn - 1)
      const K = monaco.languages.CompletionItemKind
      const kindOf: Record<string, number> = { function: K.Function, event: K.Event, field: K.Field, class: K.Class, struct: K.Struct, enum: K.Enum, interface: K.Interface, module: K.Module }
      const afterDot = before.endsWith('.')
      const items: monaco.languages.CompletionItem[] = []
      const seen = new Set<string>()
      for (const s of ctx.symbols) {
        if (afterDot && !['function', 'event', 'field'].includes(s.kind)) continue
        const key = s.name + s.container
        if (seen.has(key)) continue
        seen.add(key)
        items.push({ label: { label: s.name, description: s.container || s.module }, kind: kindOf[s.kind] ?? K.Text, insertText: s.name, range, detail: s.signature, documentation: { value: docOf(s) } })
        if (items.length > 4000) break
      }
      if (!afterDot) {
        ctx.devices.forEach(d => items.push({ label: { label: d.name, description: d.class }, kind: K.Reference, insertText: d.name, range, detail: tr("Appareil de la map · {0}", [d.class]) }))
        ctx.tags.forEach(t => items.push({ label: { label: t, description: 'tag' }, kind: K.Constant, insertText: t, range, detail: tr("Tag Verse du projet") }))
        KEYWORDS.forEach(k => items.push({ label: k, kind: K.Keyword, insertText: k, range }))
        SNIPPETS.forEach(([k, l, body]) => items.push({ label: { label: k, description: 'snippet' }, kind: K.Snippet, insertText: body, insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet, range, detail: l }))
      }
      return { suggestions: items }
    },
  })
  monaco.languages.registerHoverProvider('verse', {
    provideHover: (model, pos) => {
      const w = model.getWordAtPosition(pos)
      if (!w) return null
      const hits = ctx.symbols.filter(s => s.name === w.word).slice(0, 3)
      if (hits.length) return { contents: hits.map(s => ({ value: docOf(s) })) }
      const dev = ctx.devices.find(d => d.name === w.word)
      if (dev) return { contents: [{ value: `**${dev.name}** · appareil de la map (${dev.class})` }] }
      if (ctx.tags.includes(w.word)) return { contents: [{ value: `**${w.word}** · tag Verse du projet` }] }
      return null
    },
  })
  // Aller à la définition et renommer partout : sur tous les fichiers ouverts comme modèles.
  monaco.languages.registerDefinitionProvider('verse', {
    provideDefinition: (model, pos) => {
      const w = model.getWordAtPosition(pos)
      if (!w) return null
      const re = new RegExp(`^\\s*(var\\s+)?${w.word}\\b\\s*(<[^>]*>)*\\s*(\\(|:=|:)`)
      const out: monaco.languages.Location[] = []
      for (const m of monaco.editor.getModels()) {
        if (m.getLanguageId() !== 'verse') continue
        const lines = m.getLinesContent()
        lines.forEach((l, i) => { if (re.test(l)) out.push({ uri: m.uri, range: new monaco.Range(i + 1, l.indexOf(w.word) + 1, i + 1, l.indexOf(w.word) + 1 + w.word.length) }) })
      }
      return out
    },
  })
  monaco.languages.registerRenameProvider('verse', {
    provideRenameEdits: (model, pos, newName) => {
      const w = model.getWordAtPosition(pos)
      if (!w) return { edits: [] }
      const re = new RegExp(`\\b${w.word}\\b`, 'g')
      const edits: monaco.languages.IWorkspaceTextEdit[] = []
      for (const m of monaco.editor.getModels()) {
        if (m.getLanguageId() !== 'verse') continue
        m.getLinesContent().forEach((l, i) => {
          let x: RegExpExecArray | null
          re.lastIndex = 0
          while ((x = re.exec(l))) edits.push({ resource: m.uri, versionId: undefined, textEdit: { range: new monaco.Range(i + 1, x.index + 1, i + 1, x.index + 1 + w.word.length), text: newName } })
        })
      }
      return { edits }
    },
  })
}

function docOf(s: Sym) {
  const fx = (s.effects ?? []).map(e => `- \`<${e}>\` ${EFFECT_HELP[e] ?? ''}`).join('\n')
  return `\`\`\`verse\n${s.signature}\n\`\`\`\n${s.doc ? s.doc + '\n\n' : ''}${fx}${s.module ? `\n\n*${s.module}*` : ''}`
}

export { monaco }
