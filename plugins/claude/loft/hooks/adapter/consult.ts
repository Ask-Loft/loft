// What a tool call consults: the class of each path it reads or edits, `spec` under the `spec/`
// of the known tree that holds it, `code` elsewhere in that tree and `other` outside every known
// tree or within its `.git/` or `.yard/`; and the classes a shell command's reading programs read
// (the Feature "Classify what a tool call consults"). Nothing of a path or a command leaves here
// but its classes. Pure: no file is read, so a path is classified by its words alone.

import { ASSIGNMENT, commandsOf, type Word } from './shell'

export type Class = 'spec' | 'code' | 'other'

const ORDER: readonly Class[] = ['spec', 'code', 'other']

/** The distinct classes of `classes`, in the order `spec`, `code`, `other`. */
export const ordered = (classes: Iterable<Class>): Class[] => {
  const seen = new Set(classes)
  return ORDER.filter(each => seen.has(each))
}

/** `path` made absolute against `dir`, with `.` and `..` resolved; none for a path from `~`. */
export function resolve(path: string, dir: string): string | undefined {
  if (path === '~' || path.startsWith('~/')) return undefined
  const parts: string[] = []
  for (const part of `${path.startsWith('/') ? '' : `${dir}/`}${path}`.split('/')) {
    if (part === '' || part === '.') continue
    if (part === '..') parts.pop()
    else parts.push(part)
  }
  return `/${parts.join('/')}`
}

/** The class of `path`, named by an agent in `dir`, among the known `trees`. */
export function classOf(path: string, dir: string, trees: Iterable<string>): Class {
  const absolute = resolve(path, dir)
  if (absolute === undefined) return 'other'
  // The nearest tree is the one that leaves the shortest path within it.
  const within = [...trees]
    .flatMap(tree => {
      const root = resolve(tree, '/')!
      if (absolute === root) return ['']
      return absolute.startsWith(`${root === '/' ? '' : root}/`) ? [absolute.slice(root.length + 1)] : []
    })
    .sort((a, b) => a.length - b.length)[0]
  if (within === undefined) return 'other'
  const top = within.split('/')[0]
  return top === 'spec' ? 'spec' : top === '.git' || top === '.yard' ? 'other' : 'code'
}

/** How a reading program takes its words. */
type Reader = {
  /** Options whose value is the next word, with the number of words each takes. */
  values?: Record<string, number>
  /** Its first word that is not an option is a pattern, a script or a filter, not a path. */
  patternFirst?: boolean
  /** Options that give the pattern, after which the first word is a path. */
  patternOptions?: readonly string[]
  /** Naming no path, it reads the directory it runs in rather than its standard input. */
  searches?: boolean
}

const GREP: Reader = { values: { '-e': 1, '-f': 1, '-A': 1, '-B': 1, '-C': 1, '-m': 1, '--regexp': 1, '--file': 1 }, patternFirst: true, patternOptions: ['-e', '-f', '--regexp', '--file'] }

const READERS: Record<string, Reader> = {
  cat: {},
  head: { values: { '-n': 1, '-c': 1 } },
  tail: { values: { '-n': 1, '-c': 1 } },
  less: {},
  more: {},
  bat: { values: { '-l': 1, '-r': 1, '--language': 1, '--line-range': 1 } },
  nl: {},
  sed: { values: { '-e': 1, '-f': 1, '--expression': 1, '--file': 1 }, patternFirst: true, patternOptions: ['-e', '-f', '--expression', '--file'] },
  awk: { values: { '-F': 1, '-v': 1, '-f': 1 }, patternFirst: true, patternOptions: ['-f'] },
  cut: { values: { '-d': 1, '-f': 1, '-c': 1, '-b': 1 } },
  sort: { values: { '-k': 1, '-t': 1, '-o': 1 } },
  uniq: { values: { '-f': 1, '-s': 1, '-w': 1 } },
  wc: {},
  diff: {},
  grep: GREP,
  egrep: GREP,
  fgrep: GREP,
  rg: {
    values: { '-e': 1, '-f': 1, '-g': 1, '-t': 1, '-T': 1, '-A': 1, '-B': 1, '-C': 1, '-m': 1, '-M': 1, '-j': 1, '--glob': 1, '--type': 1, '--type-not': 1, '--regexp': 1, '--file': 1 },
    patternFirst: true,
    patternOptions: ['-e', '-f', '--regexp', '--file'],
    searches: true,
  },
  ag: { values: { '-G': 1, '-A': 1, '-B': 1, '-C': 1, '-m': 1 }, patternFirst: true, searches: true },
  fd: { values: { '-e': 1, '-t': 1, '-E': 1, '-d': 1, '--type': 1, '--extension': 1, '--exclude': 1, '--max-depth': 1 }, patternFirst: true, searches: true },
  ls: { searches: true },
  tree: { values: { '-L': 1, '-I': 1, '-P': 1 }, searches: true },
  jq: { values: { '--arg': 2, '--argjson': 2, '-f': 1, '--from-file': 1 }, patternFirst: true, patternOptions: ['-f', '--from-file'] },
  yq: { patternFirst: true },
}

/** git's subcommands that read, whose paths are the words after `--`. */
const GIT_READERS = new Set(['diff', 'show', 'log', 'grep'])

const REDIRECTION = /^(?:\d*|&)(>>?|<<<|<<-?|<)(.*)$/s

/** The words of a command that are not redirections, and the files its input is redirected from. */
function redirections(words: Word[]): { words: Word[]; inputs: string[] } {
  const kept: Word[] = []
  const inputs: string[] = []
  for (let i = 0; i < words.length; i++) {
    const word = words[i]!
    const redirected = word.quoted ? null : REDIRECTION.exec(word.text)
    if (!redirected) {
      kept.push(word)
      continue
    }
    const [, operator, attached] = redirected
    const target = attached !== '' ? attached : words[++i]?.text
    if (operator === '<' && target !== undefined && !target.startsWith('&')) inputs.push(target)
  }
  return { words: kept, inputs }
}

/** A directory no tree holds, for a command that moved to `~`. */
const OUTSIDE = '/~'

const programOf = (word: string) => word.slice(word.lastIndexOf('/') + 1)

/** The paths a reading program's arguments name, where it names any. */
function readerPaths(reader: Reader, args: Word[]): string[] {
  const paths: string[] = []
  let patternGiven = false
  let options = true
  for (let i = 0; i < args.length; i++) {
    const { text } = args[i]!
    if (options && text === '--') {
      options = false
      continue
    }
    if (options && text.startsWith('-') && text !== '-') {
      if (reader.patternOptions?.includes(text)) patternGiven = true
      i += reader.values?.[text] ?? 0
      continue
    }
    if (reader.patternFirst && !patternGiven) {
      patternGiven = true
      continue
    }
    paths.push(text)
  }
  return paths
}

/** The paths `find` searches: its words before its expression. */
function findPaths(args: Word[]): string[] {
  const end = args.findIndex(({ text }) => text.startsWith('-') || text === '(' || text === '\\(' || text === '!')
  return (end === -1 ? args : args.slice(0, end)).map(({ text }) => text)
}

/** The paths a git command reads, where it reads: none for a subcommand that does not. */
function gitPaths(args: Word[]): { reads: boolean; paths: string[]; dir?: string } {
  let dir: string | undefined
  let i = 0
  for (; i < args.length && args[i]!.text.startsWith('-'); i++) {
    if (args[i]!.text === '-C') dir = args[++i]?.text
    else if (args[i]!.text === '-c') i++
  }
  if (!GIT_READERS.has(args[i]?.text ?? '')) return { reads: false, paths: [] }
  const dashes = args.findIndex(({ text }) => text === '--')
  return { reads: true, paths: dashes === -1 ? [] : args.slice(dashes + 1).map(({ text }) => text), dir }
}

/** The classes of what a shell command's reading programs read, run from `dir`. */
export function shellReads(command: string, dir: string, trees: Iterable<string>): Class[] {
  const known = [...trees]
  const classes: Class[] = []
  let here = dir
  for (const all of commandsOf(command)) {
    let n = 0
    while (n < all.length && !all[n]!.quoted && ASSIGNMENT.test(all[n]!.text)) n++
    const { words, inputs } = redirections(all.slice(n))
    if (words.length === 0) continue
    const program = programOf(words[0]!.text)
    const args = words.slice(1)
    if (program === 'cd') {
      here = resolve(args[0]?.text ?? '~', here) ?? OUTSIDE
      continue
    }
    let paths: string[]
    let searches = false
    let from = here
    if (program === 'git') {
      const git = gitPaths(args)
      if (!git.reads) continue
      paths = git.paths
      searches = true
      if (git.dir !== undefined) from = resolve(git.dir, here) ?? OUTSIDE
    } else if (program === 'find') {
      paths = findPaths(args)
      searches = true
    } else {
      const reader = READERS[program]
      if (reader === undefined) continue
      paths = readerPaths(reader, args)
      searches = reader.searches === true
    }
    paths.push(...inputs)
    if (paths.length === 0 && searches) paths.push('.')
    for (const path of paths) classes.push(classOf(path, from, known))
  }
  return ordered(classes)
}
