// The Yard pane's view of the branch's Yard Log, made from the documents `loft yard read` prints:
// a whole read, then each read since the revision shown merged into it (the Feature "Follow Yard
// Log in a pane"). Pure, so that each rule is tested on its own.

import type { PaneActor, PaneBlock, PaneDiff, PaneFoot, PaneStage, PaneSubmission, PaneTimes, PaneView } from '../../../types'
import type { Doc } from '../../loft'
import { isObject, records, textOf } from '../../model'
import { pageOf, stagesOf, waitingAmong, type Block, type Stage } from '../log'

type Act = { at?: unknown; by?: unknown }
type Version = { act?: Act; number?: unknown }

const timeOf = (act: Act | undefined) => (typeof act?.at === 'string' ? act.at : null)

/**
 * What a card's foot says of a block: who wrote its current version, its number, and when it was
 * created, from its first version where the read gives every one, else from what the pane kept.
 */
function footOf(block: Block, before: PaneFoot | undefined): PaneFoot {
  const version = block.version as Version
  const versions = (block as { versions?: Version[] }).versions
  const number = typeof version.number === 'number' ? version.number : 1
  const first = Array.isArray(versions) && versions.length > 0 ? timeOf(versions[0]!.act) : undefined
  const created = first ?? before?.created ?? (number === 1 ? timeOf(version.act) : null)
  const by = isObject(version.act?.by) ? (version.act.by as PaneActor) : null
  return { by, number, created, updated: timeOf(version.act) }
}

function timesOf(block: Block): PaneTimes | undefined {
  const times = (block as { times?: unknown }).times
  if (!isObject(times)) return undefined
  const kept: PaneTimes = {}
  for (const name of ['listed', 'started', 'ended'] as const) if (typeof times[name] === 'string') kept[name] = times[name] as string
  return kept
}

/** The most nodes of a `spec-change` report the pane keeps, and the most of each side's words. */
const DIFF_NODES = 200
const DIFF_WORDS = 2000

const cut = (words: unknown) => {
  const text = textOf(words)
  if (text === '') return null
  return text.length > DIFF_WORDS ? `${text.slice(0, DIFF_WORDS)}…` : text
}

/** Of a `spec-change` report, the nodes it names, before and after; none for another view or a block read without it. */
function diffOf(body: Record<string, unknown>): PaneDiff | undefined {
  const report = body.report
  if (body.view !== 'spec-change' || !isObject(report) || !isObject(report.graph)) return undefined
  const nodes = records(report.graph.diff)
  return {
    nodes: nodes.slice(0, DIFF_NODES).map(each => {
      const words = isObject(each.words) ? each.words : {}
      return { change: textOf(each.change), kind: textOf(each.kind), label: textOf(each.label) || null, before: cut(words.before), after: cut(words.after) }
    }),
    more: Math.max(0, nodes.length - DIFF_NODES),
  }
}

/** Of a block, only what the pane draws; `before`, the block as the pane held it, where it did. */
function kept(block: Block, before?: PaneBlock): PaneBlock {
  const times = timesOf(block)
  const { report: _report, ...body } = (block.version?.body ?? {}) as Record<string, unknown>
  const diff = diffOf(block.version?.body ?? {}) ?? before?.diff
  return {
    id: block.id,
    key: block.key,
    kind: block.kind,
    version: { body },
    ...(block.answer ? { answer: block.answer } : {}),
    ...(block.outcome ? { outcome: block.outcome } : {}),
    foot: footOf(block, before?.foot),
    ...(times ? { times } : {}),
    ...(diff ? { diff } : {}),
  }
}

/** Of a stage, only what the pane draws, with the blocks the read gives of it. */
function keptStage(stage: Stage, before?: PaneStage): PaneStage {
  return {
    ...kept(stage, before),
    owner: isObject(stage.owner) ? (stage.owner as PaneActor) : null,
    blocks: before ? merged(before.blocks, stage.blocks ?? []) : (stage.blocks ?? []).map(each => kept(each)),
  }
}

/** Every submission the read lists with `--submissions`, oldest first, with when it was sent and settled. */
function submissionsOf(doc: Doc): PaneSubmission[] {
  return records(doc.sent)
    .map(each => {
      const changes = records(each.changes)
      const sent = changes.find(change => change.what === 'sent')
      const settled = changes.filter(change => change.what !== 'sent').at(-1)
      return {
        key: String(each.key),
        blocks: Array.isArray(each.blocks) ? each.blocks.map(String) : [],
        sent: timeOf(sent?.act as Act | undefined),
        settled: timeOf(settled?.act as Act | undefined),
        isOpen: each.state === 'open',
      }
    })
    .sort((one, other) => (one.sent ?? '').localeCompare(other.sent ?? ''))
}

const revisionOf = (doc: Doc, otherwise: number) => (typeof doc.revision === 'number' ? doc.revision : otherwise)

function identityOf(doc: Doc) {
  const log = isObject(doc.yardLog) && typeof doc.yardLog.key === 'string' ? doc.yardLog.key : null
  return { tree: textOf(doc.tree), branch: textOf(doc.branch), log }
}

/** The repository's main worktree: the folder that holds git's common directory, where it is `.git`. */
function repositoryOf(doc: Doc): string | null {
  const common = textOf(doc.commonDir)
  return common.endsWith('/.git') ? common.slice(0, -'/.git'.length) : null
}

/** The open submissions, the oldest sent first, each with its blocks in its own order. */
function openOf(doc: Doc): { key: string; blocks: string[] }[] {
  return records(doc.submissions)
    .flatMap(group => records(group.open))
    .filter(each => each.state === undefined || each.state === 'open')
    .map((each, n) => ({
      key: String(each.key),
      blocks: Array.isArray(each.blocks) ? each.blocks.map(String) : [],
      at: isObject(each.sent) ? textOf(each.sent.at) : '',
      n,
    }))
    .sort((one, other) => one.at.localeCompare(other.at) || one.n - other.n)
    .map(({ key, blocks }) => ({ key, blocks }))
}

function sentOf(doc: Doc): string[] {
  return openOf(doc).flatMap(each => each.blocks)
}

/** The view of a whole read. */
function whole(doc: Doc): PaneView {
  const identity = identityOf(doc)
  const title = isObject(doc.yardLog) && typeof doc.yardLog.title === 'string' ? doc.yardLog.title : identity.branch
  return {
    ...identity,
    repository: repositoryOf(doc),
    title,
    revision: revisionOf(doc, 0),
    page: pageOf(doc) ?? null,
    stages: stagesOf(doc).map(stage => keptStage(stage)),
    sent: sentOf(doc),
    asked: openOf(doc).map(each => each.key),
    submissions: submissionsOf(doc),
  }
}

/** `blocks` with each of `changed` in place of the block of its key. */
function merged(blocks: readonly PaneBlock[], changed: readonly Block[]): PaneBlock[] {
  const replaced = new Map(changed.map(block => [block.key, block]))
  return blocks.map(block => {
    const fresh = replaced.get(block.key)
    return fresh ? kept(fresh, block) : block
  })
}

/**
 * Whether a read since brings a stage or a block the view does not hold: the read gives no place
 * for it among what did not change, so only a whole read shows it in its place.
 */
function bringsNew(view: PaneView, read: readonly Stage[]): boolean {
  const known = new Set(view.stages.flatMap(stage => [stage.key, ...stage.blocks.map(block => block.key)]))
  return read.some(stage => !known.has(stage.key) || (stage.blocks ?? []).some(block => !known.has(block.key)))
}

const sameList = (one: readonly string[], other: readonly string[]) => one.length === other.length && one.every((each, n) => each === other[n])

/**
 * The view after a read: a whole read starts it over; a read since the revision shown is merged
 * in, each changed stage and block in its place, a changed stage keeping its blocks. A read that
 * changed nothing gives the view shown, the same object. Undefined where a read since names another
 * tree, branch or Yard Log, or brings a stage or a block the view does not hold: the pane then reads
 * it whole.
 */
export function followed(view: PaneView | undefined, doc: Doc): PaneView | undefined {
  if (typeof doc.since !== 'number') return whole(doc)
  if (view === undefined) return undefined
  const identity = identityOf(doc)
  if (identity.tree !== view.tree || identity.branch !== view.branch || identity.log !== view.log) return undefined
  const read = stagesOf(doc)
  const revision = revisionOf(doc, view.revision)
  const page = pageOf(doc) ?? view.page
  const sent = sentOf(doc)
  const asked = openOf(doc).map(each => each.key)
  if (read.length === 0 && revision === view.revision && page === view.page && sameList(sent, view.sent) && sameList(asked, view.asked)) return view
  if (bringsNew(view, read)) return undefined
  const changed = new Map(read.map(stage => [stage.key, stage]))
  const stages = view.stages.map(stage => {
    const fresh = changed.get(stage.key)
    return fresh ? keptStage(fresh, stage) : stage
  })
  return { ...view, revision, page, stages, sent, asked, submissions: submissionsOf(doc) }
}

/** The blocks of the open submissions that wait on the person, in the submissions' order. */
export function waitingIn(view: PaneView): PaneBlock[] {
  return waitingAmong(new Map(view.stages.flatMap(stage => stage.blocks.map(block => [block.key, block] as const))), view.sent)
}

/** The channel a block was answered or decided on, once it was. */
export function answeredOn(block: PaneBlock): string | undefined {
  const given = block.answer ?? block.outcome
  return given && typeof given.channel === 'string' ? given.channel : undefined
}

/**
 * The Yard page's address as a link a surface makes of it: Claude Code Desktop links plain http
 * only to `localhost`, which the page server answers as it answers 127.0.0.1.
 */
export function linkOf(page: string): string {
  return page.replace(/^http:\/\/127\.0\.0\.1([:/])/, 'http://localhost$1')
}

/** A path's last part. */
export const baseOf = (path: string) => path.split('/').filter(part => part !== '').at(-1) ?? path

/**
 * What the head names above the title, as the page does: the repository, the worktree where it is
 * not the main one, and the branch where the title is not its name.
 */
export function kickerOf(view: PaneView): string[] {
  const repository = view.repository ?? view.tree
  return [
    baseOf(repository),
    ...(view.repository !== null && view.tree !== view.repository ? [baseOf(view.tree)] : []),
    ...(view.title !== view.branch ? [view.branch] : []),
  ]
}

const STATUSES = ['done', 'active', 'planned', 'dropped'] as const

/** The head's line that sums up the stages, as `3 stages: 2 done, 1 active, 0 planned`; dropped only where any is. */
export function summaryOf(view: PaneView): string {
  const counts = STATUSES.map(status => [status, view.stages.filter(stage => stage.version.body.status === status).length] as const)
  const tally = counts.filter(([status, n]) => n > 0 || status !== 'dropped').map(([status, n]) => `${n} ${status}`)
  const n = view.stages.length
  return `${n} ${n === 1 ? 'stage' : 'stages'}: ${tally.join(', ')}`
}

/** Where the planned stages begin among `stages`: the first of the planned ones that end the list. */
export function plannedFrom(stages: readonly PaneStage[]): number {
  let from = stages.length
  while (from > 0 && stages[from - 1]!.version.body.status === 'planned') from--
  return from
}

/** The stage chosen: the one the person chose, else the first `active` stage, else the first stage. */
export function chosenOf(stages: readonly PaneStage[], chosen: string | null): PaneStage | undefined {
  return stages.find(stage => stage.key === chosen) ?? stages.find(stage => stage.version.body.status === 'active') ?? stages[0]
}

/** The views of a `loft` block, in words, as the page names them. */
export const VIEW_NAMES: Record<string, string> = {
  'spec-change': 'Spec change',
  findings: 'Check findings',
  trace: 'Trace coverage',
  decision: 'Decision',
  publish: 'Publish state',
}

/** What a block's row on the rail calls it, by its kind, as the page does, a `loft` view in words. */
export function blockTitle(block: PaneBlock): string {
  const body = block.version.body
  switch (block.kind) {
    case 'question':
      return textOf(body.header) || textOf(body.prompt) || block.id
    case 'loft':
      return VIEW_NAMES[textOf(body.view)] ?? (textOf(body.view) || block.id)
    case 'note':
      return textOf(body.title) || firstLine(textOf(body.markdown)) || block.id
    default:
      return textOf(body.title) || block.id
  }
}

function firstLine(text: string): string {
  const line = text.split('\n').find(each => each.trim() !== '') ?? ''
  return line.replace(/[#*_`>]/g, '').trim().slice(0, 80)
}

/** A time as the page gives one: its hour and minute, as `02:00 PM`. */
export function clockOf(at: string): string {
  const date = new Date(at)
  if (Number.isNaN(date.getTime())) return at
  try {
    return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' })
  } catch {
    return at.slice(11, 16)
  }
}

/** The agent that owns a stage, as the page names it: the main agent, or a subagent by its type. */
export function ownerOf(owner: PaneStage['owner']): string | null {
  if (!owner) return null
  return typeof owner.agent === 'string' ? `${textOf(owner.agentType) || 'a'} subagent` : 'main agent'
}

const STATES: Record<string, string> = { open: 'Open', active: 'Active', done: 'Done', dropped: 'Dropped' }

/**
 * A block's state as the page words it, with the status whose colour it takes: that it waits on
 * the person, a task's state, whether a question was answered, an approval's or a plan's verdict.
 * A note, a decision or a `loft` block has none.
 */
export function blockStateOf(block: PaneBlock, isWaiting: boolean): [string, string] | null {
  if (isWaiting) return ['Waits on you', 'waiting']
  const body = block.version.body
  if (block.kind === 'task') {
    const state = textOf(body.state) || 'open'
    return [STATES[state] ?? state, state === 'open' ? 'planned' : state]
  }
  if (block.kind === 'question') return block.answer ? ['Answered', 'done'] : ['Open', 'planned']
  if (block.kind === 'approval' || block.kind === 'plan') {
    const verdict = block.outcome?.verdict
    if (verdict === 'approved') return ['Approved', 'done']
    if (verdict === 'rejected') return ['Rejected', 'dropped']
    return ['Open', 'planned']
  }
  return null
}

/** Who acted, as the page names them: the person by name, a subagent by its type, else the main agent. */
export function whoOf(by: PaneActor | null | undefined): string {
  if (!by) return 'unknown'
  if (textOf(by.name)) return textOf(by.name)
  if (textOf(by.agentType)) return textOf(by.agentType)
  return typeof by.agent === 'string' ? 'subagent' : 'main agent'
}

/** Where a block was answered or decided, in the page's words. */
export function channelWords(channel: string | undefined): string {
  if (channel === 'page') return 'on the page'
  if (channel === 'terminal') return 'in the terminal'
  return channel ? `on ${channel}` : ''
}

/** The letter of the option at `index`: A to Z, then AA, AB and on, as the page letters them. */
export function letterOf(index: number): string {
  let text = ''
  for (let n = index; n >= 0; n = Math.floor(n / 26) - 1) text = String.fromCharCode(65 + (n % 26)) + text
  return text
}
