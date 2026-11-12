// The Yard pane's view of the branch's Yard Log, made from the documents `loft yard read` prints:
// a whole read, then each read since the revision shown merged into it (the Feature "Follow Yard
// Log in a pane"). Pure, so that each rule is tested on its own.

import type { PaneActor, PaneBlock, PaneFoot, PaneStage, PaneSubmission, PaneTimes, PaneView } from '../../../types'
import type { Doc } from '../../loft'
import { isObject, records, textOf } from '../../model'
import { openSubmissions, pageOf, stagesOf, waitingAmong, type Block, type Stage } from '../log'

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

/** Of a block, only what the pane draws; `before`, the block as the pane held it, where it did. */
function kept(block: Block, before?: PaneBlock): PaneBlock {
  const times = timesOf(block)
  return {
    id: block.id,
    key: block.key,
    kind: block.kind,
    version: { body: block.version?.body ?? {} },
    ...(block.answer ? { answer: block.answer } : {}),
    ...(block.outcome ? { outcome: block.outcome } : {}),
    foot: footOf(block, before?.foot),
    ...(times ? { times } : {}),
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

function sentOf(doc: Doc): string[] {
  return openSubmissions(doc).flatMap(each => each.blocks)
}

/** The view of a whole read. */
function whole(doc: Doc): PaneView {
  return {
    ...identityOf(doc),
    revision: revisionOf(doc, 0),
    page: pageOf(doc) ?? null,
    stages: stagesOf(doc).map(stage => keptStage(stage)),
    sent: sentOf(doc),
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
  if (read.length === 0 && revision === view.revision && page === view.page && sameList(sent, view.sent)) return view
  if (bringsNew(view, read)) return undefined
  const changed = new Map(read.map(stage => [stage.key, stage]))
  const stages = view.stages.map(stage => {
    const fresh = changed.get(stage.key)
    return fresh ? keptStage(fresh, stage) : stage
  })
  return { ...view, revision, page, stages, sent, submissions: submissionsOf(doc) }
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
