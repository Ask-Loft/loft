// The Yard pane's view of the branch's Yard Log, made from the documents `loft yard read` prints:
// a whole read, then each read since the revision shown merged into it (the Feature "Follow Yard
// Log in a pane"). Pure, so that each rule is tested on its own.

import type { Doc } from '../../loft'
import { isObject } from '../../model'
import { blocksByKey, openSubmissions, pageOf, stagesOf, waits, type Block } from '../log'

/** A stage as the pane keeps it: its own block and its blocks, in order. */
export type PaneStage = Block & { blocks: Block[] }

/** What the pane shows: which Yard Log, at which revision, and its stages. */
export type PaneView = {
  tree: string
  branch: string
  /** The key of Yard Log shown; none where the branch has no Yard Log yet. */
  log: string | null
  hasLog: boolean
  revision: number
  page: string | null
  stages: PaneStage[]
  /** The blocks of the open submissions, by key, in each submission's order. */
  sent: string[]
}

/** Of a block, only what the pane draws. */
function kept(block: Block): Block {
  return {
    id: block.id,
    key: block.key,
    kind: block.kind,
    version: { body: block.version?.body ?? {} },
    ...(block.answer ? { answer: block.answer } : {}),
    ...(block.outcome ? { outcome: block.outcome } : {}),
  }
}

const textOf = (value: unknown) => (typeof value === 'string' ? value : '')

function identityOf(doc: Doc) {
  const log = isObject(doc.yardLog) && typeof doc.yardLog.key === 'string' ? doc.yardLog.key : null
  return { tree: textOf(doc.tree), branch: textOf(doc.branch), log }
}

function sentOf(doc: Doc): string[] {
  return openSubmissions(doc).flatMap(each => each.blocks)
}

/** The view of a whole read. */
function whole(doc: Doc): PaneView {
  const identity = identityOf(doc)
  return {
    ...identity,
    hasLog: identity.log !== null,
    revision: typeof doc.revision === 'number' ? doc.revision : 0,
    page: pageOf(doc) ?? null,
    stages: stagesOf(doc).map(stage => ({ ...kept(stage), blocks: (stage.blocks ?? []).map(kept) })),
    sent: sentOf(doc),
  }
}

/** `blocks` with each of `changed` in place of the block of its key, and each new one last. */
function merged(blocks: readonly Block[], changed: readonly Block[]): Block[] {
  const next = [...blocks]
  for (const block of changed.map(kept)) {
    const at = next.findIndex(each => each.key === block.key)
    if (at === -1) next.push(block)
    else next[at] = block
  }
  return next
}

/**
 * The view after a read: a whole read starts it over; a read since the revision shown is merged
 * in, a changed stage keeping its blocks and a new stage going after the one before it in the read.
 * Undefined where a read since names another tree, branch or Yard Log: the pane then reads it whole.
 */
export function followed(view: PaneView | undefined, doc: Doc): PaneView | undefined {
  if (typeof doc.since !== 'number' || view === undefined) return typeof doc.since === 'number' ? undefined : whole(doc)
  const identity = identityOf(doc)
  if (identity.tree !== view.tree || identity.branch !== view.branch || identity.log !== view.log) return undefined
  const stages = [...view.stages]
  let before: string | undefined
  for (const read of stagesOf(doc)) {
    const at = stages.findIndex(each => each.key === read.key)
    if (at === -1) {
      const after = before === undefined ? -1 : stages.findIndex(each => each.key === before)
      const stage = { ...kept(read), blocks: (read.blocks ?? []).map(kept) }
      if (after === -1) stages.push(stage)
      else stages.splice(after + 1, 0, stage)
    } else {
      stages[at] = { ...kept(read), blocks: merged(stages[at]!.blocks, read.blocks ?? []) }
    }
    before = read.key
  }
  return {
    ...view,
    revision: typeof doc.revision === 'number' ? doc.revision : view.revision,
    page: pageOf(doc) ?? view.page,
    stages,
    sent: sentOf(doc),
  }
}

/** The revision a read since asks from: the one shown; none before the first read. */
export function sinceOf(view: PaneView | undefined): number | undefined {
  return view?.revision
}

/** The blocks of the open submissions that wait on the person, in the submissions' order. */
export function waitingOf(view: PaneView): Block[] {
  const blocks = blocksByKey({ outcome: 'read', stages: view.stages })
  return view.sent.flatMap(key => {
    const block = blocks.get(key)
    return block && waits(block) ? [block] : []
  })
}

/** The channel a block was answered or decided on, once it was. */
export function answeredOn(block: Block): string | undefined {
  const given = block.answer ?? block.outcome
  return given && typeof given.channel === 'string' ? given.channel : undefined
}
