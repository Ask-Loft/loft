// The Yard pane's view of the branch's Yard Log, made from the documents `loft yard read` prints:
// a whole read, then each read since the revision shown merged into it (the Feature "Follow Yard
// Log in a pane"). Pure, so that each rule is tested on its own.

import type { PaneActor, PaneBlock, PaneStage, PaneView } from '../../../types'
import type { Doc } from '../../loft'
import { isObject, textOf } from '../../model'
import { openSubmissions, pageOf, stagesOf, waitingAmong, type Block, type Stage } from '../log'

/** Of a block, only what the pane draws. */
function kept(block: Block): PaneBlock {
  return {
    id: block.id,
    key: block.key,
    kind: block.kind,
    version: { body: block.version?.body ?? {} },
    ...(block.answer ? { answer: block.answer } : {}),
    ...(block.outcome ? { outcome: block.outcome } : {}),
  }
}

/** Of a stage, only what the pane draws, with the blocks the read gives of it. */
function keptStage(stage: Stage, before?: PaneStage): PaneStage {
  return {
    ...kept(stage),
    owner: isObject(stage.owner) ? (stage.owner as PaneActor) : null,
    blocks: before ? merged(before.blocks, stage.blocks ?? []) : (stage.blocks ?? []).map(kept),
  }
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

/** The blocks of the open submissions, the oldest submission first, each in its own order. */
function sentOf(doc: Doc): string[] {
  return openSubmissions(doc).flatMap(each => each.blocks)
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
  }
}

/** `blocks` with each of `changed` in place of the block of its key. */
function merged(blocks: readonly PaneBlock[], changed: readonly Block[]): PaneBlock[] {
  const replaced = new Map(changed.map(block => [block.key, kept(block)]))
  return blocks.map(block => replaced.get(block.key) ?? block)
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
  if (read.length === 0 && revision === view.revision && page === view.page && sameList(sent, view.sent)) return view
  if (bringsNew(view, read)) return undefined
  const changed = new Map(read.map(stage => [stage.key, stage]))
  const stages = view.stages.map(stage => {
    const fresh = changed.get(stage.key)
    return fresh ? keptStage(fresh, stage) : stage
  })
  return { ...view, revision, page, stages, sent }
}

/** The blocks of the open submissions that wait on the person, in the submissions' order. */
export function waitingIn(view: PaneView): PaneBlock[] {
  return waitingAmong(new Map(view.stages.flatMap(stage => stage.blocks.map(block => [block.key, block] as const))), view.sent)
}

/**
 * The Yard page's address as a link a surface makes of it: Claude Code Desktop links plain http
 * only to `localhost`, which the page server answers as it answers 127.0.0.1.
 */
export function linkOf(page: string): string {
  return page.replace(/^http:\/\/127\.0\.0\.1([:/])/, 'http://localhost$1')
}

/** A path's last part. */
const baseOf = (path: string) => path.split('/').filter(part => part !== '').at(-1) ?? path

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

/** The agent that owns a stage, as the page names it: the main agent, or a subagent by its type. */
export function ownerOf(owner: PaneStage['owner']): string | null {
  if (!owner) return null
  return typeof owner.agent === 'string' ? `${textOf(owner.agentType) || 'a'} subagent` : 'main agent'
}

/** The letter of the option at `index`: A to Z, then AA, AB and on, as the page letters them. */
export function letterOf(index: number): string {
  let text = ''
  for (let n = index; n >= 0; n = Math.floor(n / 26) - 1) text = String.fromCharCode(65 + (n % 26)) + text
  return text
}

/**
 * What `/yard-pane` answers: that the pane is open, or opens once there is room; then, once a read
 * gave the view, the Yard page's address for its tree and branch with a line on what it shows,
 * opening in Desktop's browser pane on Desktop, or that no page server serves it. With no view
 * read yet, the pane's line alone (the Feature "Keep the Yard page beside the session").
 */
export function answerOf(view: PaneView | null, isPlaced: boolean, isDesktop: boolean): string {
  const pane = isPlaced ? "Yard Log's pane is open." : "Yard Log's pane opens once there is room for it."
  if (view === null) return pane
  if (view.page === null) return `${pane} No page server serves the Yard page yet.`
  const where = isDesktop ? "It opens in Desktop's browser pane." : 'Open it in your browser.'
  return `${pane}\nThe Yard page for ${view.branch} shows its Yard Log in full: ${view.page}\n${where}`
}
