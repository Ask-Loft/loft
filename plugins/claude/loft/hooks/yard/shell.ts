// The names of the `loft` subcommands a shell command runs, for a tool call's event: `loft` by
// its name, by a path to it, or through a shell variable the command sets to either; never a word
// of quoted text or of a heredoc (the Feature "Record a session's events"). Pure.

/** `loft`'s own subcommands: a name is taken only from this list. */
export const LOFT_SUBCOMMANDS: readonly string[] = [
  'init',
  'fmt',
  'check',
  'import',
  'publish',
  'find',
  'terms',
  'reconcile',
  'verify',
  'status',
  'review',
  'merge-driver',
  'skill',
  'record-install',
  'telemetry',
  'update-check',
  'yard',
  'render',
]

type Word = { text: string; quoted: boolean }

const SEPARATORS = new Set([';', '&', '|', '(', ')', '\n'])

/** The command with every heredoc's body left out. */
function withoutHeredocs(command: string): string {
  const lines = command.split('\n')
  const kept: string[] = []
  let ending: string | null = null
  for (const line of lines) {
    if (ending !== null) {
      if (line.trim() === ending) ending = null
      continue
    }
    kept.push(line)
    const opened = /<<-?\s*(['"]?)([A-Za-z_][A-Za-z0-9_]*)\1/.exec(line)
    if (opened) ending = opened[2]!
  }
  return kept.join('\n')
}

/** The simple commands of a shell command, each as its words. */
function commandsOf(command: string): Word[][] {
  const commands: Word[][] = [[]]
  let word = ''
  let quoted = false
  let isWord = false
  const end = () => {
    if (isWord) commands.at(-1)!.push({ text: word, quoted })
    word = ''
    quoted = false
    isWord = false
  }
  const source = withoutHeredocs(command)
  for (let i = 0; i < source.length; i++) {
    const c = source[i]!
    if (c === "'" || c === '"') {
      const close = source.indexOf(c, i + 1)
      const inner = close === -1 ? source.slice(i + 1) : source.slice(i + 1, close)
      word += inner
      quoted = quoted || !(c === '"' && /^\$\{?[A-Za-z_][A-Za-z0-9_]*\}?$/.test(inner))
      isWord = true
      i = close === -1 ? source.length : close
    } else if (c === ' ' || c === '\t') {
      end()
    } else if (SEPARATORS.has(c)) {
      end()
      if (commands.at(-1)!.length > 0) commands.push([])
    } else {
      word += c
      isWord = true
    }
  }
  end()
  return commands.filter(words => words.length > 0)
}

const ASSIGNMENT = /^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/s
const VARIABLE = /^\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?$/

const isLoft = (word: string) => word === 'loft' || /[\\/]loft(\.exe)?$/.test(word)

/** The `loft` subcommands the shell command runs, each named once, in the order they run. */
export function loftSubcommands(command: string): string[] {
  const variables = new Map<string, string>()
  const names: string[] = []
  for (const words of commandsOf(command)) {
    let n = 0
    while (n < words.length && !words[n]!.quoted && ASSIGNMENT.test(words[n]!.text)) n++
    if (n === words.length) {
      for (const word of words) {
        const [, name, value] = ASSIGNMENT.exec(word.text)!
        variables.set(name!, value!)
      }
      continue
    }
    const first = words[n]!
    const variable = VARIABLE.exec(first.text)
    const program = variable ? variables.get(variable[1]!) : first.quoted ? undefined : first.text
    if (program === undefined || !isLoft(program)) continue
    const sub = words.slice(n + 1).find(word => !word.text.startsWith('-'))
    if (sub && !sub.quoted && LOFT_SUBCOMMANDS.includes(sub.text) && !names.includes(sub.text)) names.push(sub.text)
  }
  return names
}
