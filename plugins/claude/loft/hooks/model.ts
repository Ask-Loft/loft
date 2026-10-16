// The Loft status pane's model: pure functions from the JSON documents `loft` prints to what the
// pane draws. Nothing here runs a command or reads a file (the accepted decision "The status
// pane reads Loft only through the command line's JSON").

import type { LoftError, LoftFinding, LoftNode, LoftRead, LoftStatus } from '../types'

type Json = Record<string, unknown>

const isObject = (value: unknown): value is Json =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const records = (value: unknown): Json[] => (Array.isArray(value) ? value.filter(isObject) : [])

const text = (value: unknown): string | undefined => (typeof value === 'string' ? value : undefined)

const whole = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isInteger(value) ? value : undefined

/** One document as `loft --format json` prints it, or undefined for anything else. */
export function parseDocument(stdout: string): Json | undefined {
  try {
    const value: unknown = JSON.parse(stdout)
    return isObject(value) && typeof value.outcome === 'string' ? value : undefined
  } catch {
    return undefined
  }
}

/** The errors a document carries, each with the command that printed it. */
export function documentErrors(command: string, document: Json): LoftError[] {
  return records(document.errors).map(error => ({
    command,
    message: text(error.message) ?? 'an error without a message',
    path: text(error.path),
    line: whole(error.line),
  }))
}

/** Whether a command stopped: its document's outcome is `error`. */
export const stopped = (document: Json): boolean => document.outcome === 'error'

function node(value: Json): LoftNode {
  return {
    key: text(value.key) ?? '',
    label: text(value.label) ?? null,
    kind: text(value.kind) ?? '',
    name: text(value.name) ?? '',
    path: text(value.path) ?? '',
    line: whole(value.line),
    column: whole(value.column),
  }
}

/** `loft status --base <base> --format json` */
export function parseStatus(document: Json): LoftStatus {
  const lock = isObject(document.lock) ? document.lock : undefined
  const repository = isObject(document.repository) ? document.repository : {}
  const base = isObject(document.base) ? document.base : undefined
  return {
    lock: lock
      ? {
          publishSequence: whole(lock.publishSequence) ?? 0,
          matches: lock.matches === true,
          differs: text(lock.differs) ?? null,
        }
      : null,
    branch: text(repository.branch) ?? null,
    modified: Array.isArray(repository.modified) ? repository.modified.filter(p => typeof p === 'string') : [],
    base: base
      ? {
          revision: text(base.revision) ?? '',
          commit: text(base.commit) ?? '',
          added: records(base.added).map(node),
          changed: records(base.changed).map(node),
          removed: records(base.removed).map(node),
        }
      : null,
  }
}

/** The findings of `loft check` or `loft reconcile`, each with the key of its node. */
export function parseFindings(document: Json): LoftFinding[] {
  return records(document.findings).map(finding => {
    const subject = records(finding.subjects)[0]
    return {
      rule: text(finding.rule) ?? '',
      severity: text(finding.severity) ?? '',
      message: text(finding.message) ?? '',
      key: text(finding.key) ?? text(subject?.key),
      label: text(finding.label) ?? null,
      path: text(finding.path),
      line: whole(finding.line),
    }
  })
}

/** The four documents of one read, made into what the pane keeps. */
export function readOf(status: Json, fmt: Json, check: Json, reconcile: Json): LoftRead {
  return {
    status: parseStatus(status),
    canonical: fmt.outcome === 'canonical',
    drift: Array.isArray(fmt.changed) ? fmt.changed.filter(p => typeof p === 'string') : [],
    checkErrors: records(check.errors).length,
    checkFindings: parseFindings(check),
    traceFindings: parseFindings(reconcile),
  }
}

/** The words of the `loftCommand` setting: its first is the program, the rest its first arguments. */
export function commandWords(setting: string): string[] {
  const words = setting.trim().split(/\s+/).filter(word => word !== '')
  return words.length > 0 ? words : ['loft']
}

/** The directories from `dir` up to the file system's root, nearest first. */
export function ancestors(dir: string): string[] {
  const parts = dir.split('/').filter(part => part !== '')
  const dirs: string[] = []
  for (let n = parts.length; n >= 0; n--) dirs.push('/' + parts.slice(0, n).join('/'))
  return dirs
}

/** `path` from `root`, when it lies under it. */
export function relativeTo(root: string, path: string): string | undefined {
  const prefix = root.endsWith('/') ? root : `${root}/`
  return path.startsWith(prefix) ? path.slice(prefix.length) : undefined
}

/** Whether a write of `path` can change what the pane reads: a file under `spec/`, or the trace. */
export function touchesSpecification(root: string, path: string): boolean {
  const relative = relativeTo(root, path)
  return relative !== undefined && (relative.startsWith('spec/') || relative === 'loft.trace.yaml')
}

/** Whether a shell command runs `loft` or `git`, each of which can change what the pane reads. */
export function runsLoftOrGit(command: string): boolean {
  return /(^|[\s;&|(/])(loft|git)(\s|$)/.test(command)
}

/** Whether a skill or a slash command is the `loft-spec` skill, by any plugin prefix. */
export function isLoftSpecSkill(name: string | undefined): boolean {
  return name !== undefined && /(^|:)loft-spec$/.test(name)
}

const TRACE_WORK = new Set(['TR-001', 'TR-002', 'TR-003', 'TR-006', 'TR-009'])
const TRACE_BROKEN = new Set(['TR-004', 'TR-005', 'TR-010'])

/** A criterion the branch added or changed, with the finding that leaves it unreviewed. */
export type ToVerify = { node: LoftNode; open: LoftFinding | undefined }

/** Each criterion the branch added or changed, in the order of its file and line. */
export function criteriaToVerify(read: LoftRead): ToVerify[] {
  const base = read.status.base
  if (!base) return []
  const seen = new Set<string>()
  const touched = [...base.added, ...base.changed]
    .filter(node => node.kind === 'criterion' && !seen.has(node.key) && seen.add(node.key))
    .sort((a, b) => a.path.localeCompare(b.path) || (a.line ?? 0) - (b.line ?? 0))
  return touched.map(node => ({
    node,
    open: read.traceFindings.find(finding => finding.key === node.key && TRACE_WORK.has(finding.rule)),
  }))
}

/** The trace findings that say a link is no longer true, wherever they stand. */
export const brokenLinks = (read: LoftRead): LoftFinding[] =>
  read.traceFindings.filter(finding => TRACE_BROKEN.has(finding.rule))

/** The warnings of `loft check`: its findings at warning. */
export const checkWarnings = (read: LoftRead): number =>
  read.checkFindings.filter(finding => finding.severity === 'warning').length

export type CheckId = 'format' | 'check' | 'committed' | 'trace' | 'lock'

export type Check = { id: CheckId; label: string; passes: boolean; detail: string }

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`

/** The checks of the loop, in their order. */
export function checks(read: LoftRead): Check[] {
  const warnings = checkWarnings(read)
  const toVerify = criteriaToVerify(read)
  const reviewed = toVerify.filter(each => !each.open).length
  const broken = brokenLinks(read).length
  const lock = read.status.lock
  return [
    {
      id: 'format',
      label: 'Format',
      passes: read.canonical,
      detail: read.canonical ? 'canonical' : `${plural(read.drift.length, 'file', 'files')} to format`,
    },
    {
      id: 'check',
      label: 'Check',
      passes: read.checkErrors === 0 && warnings === 0,
      detail: `${plural(read.checkErrors, 'error', 'errors')} · ${plural(warnings, 'warning', 'warnings')}`,
    },
    {
      id: 'committed',
      label: 'Spec committed',
      passes: read.status.modified.length === 0,
      detail:
        read.status.modified.length === 0
          ? 'clean'
          : `${plural(read.status.modified.length, 'path', 'paths')} uncommitted`,
    },
    {
      id: 'trace',
      label: 'Trace',
      passes: reviewed === toVerify.length && broken === 0,
      detail: `${reviewed}/${toVerify.length} criteria reviewed${broken > 0 ? ` · ${plural(broken, 'broken link', 'broken links')}` : ''}`,
    },
    {
      id: 'lock',
      label: 'Lock',
      passes: lock?.matches === true,
      detail: !lock
        ? 'not published yet'
        : lock.matches
          ? `matches publish ${lock.publishSequence}`
          : `unpublished changes since publish ${lock.publishSequence}`,
    },
  ]
}

const NEXT: Record<CheckId, string> = {
  format: 'run `loft fmt`',
  check: 'fix what `loft check` reports',
  committed: 'commit `spec/`',
  trace: 'link and review the tests of the criteria to verify',
  lock: 'merge the branch, then publish on the publish branch',
}

/** The action the first check that does not pass asks for. */
export function nextStep(all: Check[]): string {
  const first = all.find(check => !check.passes)
  return first ? NEXT[first.id] : 'nothing left to do'
}

/** The nodes the branch changed in one file, added then changed. */
export function nodesIn(read: LoftRead, path: string): { change: 'added' | 'changed'; node: LoftNode }[] {
  const base = read.status.base
  if (!base) return []
  return [
    ...base.added.filter(node => node.path === path).map(node => ({ change: 'added' as const, node })),
    ...base.changed.filter(node => node.path === path).map(node => ({ change: 'changed' as const, node })),
  ]
}

/** How a node is named in a list: its label, or `new` without one. */
export const labelOf = (node: LoftNode): string => node.label ?? 'new'

/** The status line's entry for a read. */
export function statusLine(read: LoftRead): string {
  const base = read.status.base
  const delta = base ? `+${base.added.length} ~${base.changed.length} −${base.removed.length}` : 'no base'
  const toVerify = criteriaToVerify(read)
  const reviewed = toVerify.filter(each => !each.open).length
  return [
    read.status.branch ?? 'detached HEAD',
    delta,
    `check ${read.checkErrors}e/${checkWarnings(read)}w`,
    `trace ${reviewed}/${toVerify.length}`,
  ].join(' · ')
}

/** The first `size` items of a list and how many it leaves out. */
export function window<T>(items: readonly T[], size: number): { shown: T[]; more: number } {
  const room = Math.max(1, size)
  if (items.length <= room) return { shown: [...items], more: 0 }
  return { shown: items.slice(0, room - 1), more: items.length - (room - 1) }
}
