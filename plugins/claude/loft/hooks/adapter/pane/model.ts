// The Yard pane's view of the branch's Yard Log, made from the documents `loft yard read` prints:
// a whole read, then each read since the revision shown merged into it (the Feature "Follow Yard
// Log in a pane"). Pure, so that each rule is tested on its own.

import type { PaneBlock, PaneStage, PaneView } from '../../../types'
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
function keptStage(stage: Stage): PaneStage {
  return { ...kept(stage), blocks: (stage.blocks ?? []).map(kept) }
}

const revisionOf = (doc: Doc, otherwise: number) => (typeof doc.revision === 'number' ? doc.revision : otherwise)

function identityOf(doc: Doc) {
  const log = isObject(doc.yardLog) && typeof doc.yardLog.key === 'string' ? doc.yardLog.key : null
  return { tree: textOf(doc.tree), branch: textOf(doc.branch), log }
}

function sentOf(doc: Doc): string[] {
  return openSubmissions(doc).flatMap(each => each.blocks)
}

/** The view of a whole read. */
function whole(doc: Doc): PaneView {
  return {
    ...identityOf(doc),
    revision: revisionOf(doc, 0),
    page: pageOf(doc) ?? null,
    stages: stagesOf(doc).map(keptStage),
    sent: sentOf(doc),
  }
}

/** `blocks` with each of `changed` in place of the block of its key, and each new one last. */
function merged(blocks: readonly PaneBlock[], changed: readonly Block[]): PaneBlock[] {
  const next = [...blocks]
  for (const block of changed.map(kept)) {
    const at = next.findIndex(each => each.key === block.key)
    if (at === -1) next.push(block)
    else next[at] = block
  }
  return next
}

const sameList = (one: readonly string[], other: readonly string[]) => one.length === other.length && one.every((each, n) => each === other[n])

/**
 * The view after a read: a whole read starts it over; a read since the revision shown is merged
 * in, a changed stage keeping its blocks and a new stage going after the one before it in the read.
 * A read that changed nothing gives the view shown, the same object. Undefined where a read since
 * names another tree, branch or Yard Log: the pane then reads it whole.
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
  const stages = [...view.stages]
  let before: string | undefined
  for (const stage of read) {
    const at = stages.findIndex(each => each.key === stage.key)
    if (at === -1) {
      const after = before === undefined ? -1 : stages.findIndex(each => each.key === before)
      if (after === -1) stages.push(keptStage(stage))
      else stages.splice(after + 1, 0, keptStage(stage))
    } else {
      stages[at] = { ...kept(stage), blocks: merged(stages[at]!.blocks, stage.blocks ?? []) }
    }
    before = stage.key
  }
  return { ...view, revision, page, stages, sent }
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
