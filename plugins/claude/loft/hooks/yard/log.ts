// What the Yard hooks read in the documents `loft yard` prints: the stages, the stage active for
// an agent, the blocks of a submission and which of them still wait on the person. Pure, so that
// each rule is tested on its own.

import type { Doc } from './run'

/** The agent a record names: its session, and for a subagent its id. */
export type Actor = { session?: string; agent?: string; agentType?: string }

/** A block as a `loft yard` document gives it. */
export type Block = {
  id: string
  key: string
  kind: string
  stage?: string
  version: { body: Record<string, unknown> }
  answer?: Record<string, unknown> | null
  outcome?: Record<string, unknown> | null
}

/** A stage as `loft yard read` gives it: a block that holds blocks. */
export type Stage = Block & { owner?: Actor | null; blocks: Block[] }

/** Who is asking: the session and its earlier ids in this process, and the subagent, if one. */
export type Asker = { sessions: readonly string[]; agent?: string }

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

export function stagesOf(doc: Doc): Stage[] {
  return Array.isArray(doc.stages) ? (doc.stages.filter(isObject) as unknown as Stage[]) : []
}

export function statusOf(block: Block): string | undefined {
  const status = block.version?.body?.status
  return typeof status === 'string' ? status : undefined
}

/** Whether a record's actor is the asking agent. */
export function isAsker(actor: Actor | null | undefined, asker: Asker): boolean {
  if (!actor?.session || !asker.sessions.includes(actor.session)) return false
  return (actor.agent ?? undefined) === asker.agent
}

/** The stage `active` for the asking agent, if it has one. */
export function activeStage(doc: Doc, asker: Asker): Stage | undefined {
  return stagesOf(doc).find(stage => statusOf(stage) === 'active' && isAsker(stage.owner, asker))
}

/** The last stage that is no longer `planned`: a stage the mod opens goes after it. */
export function lastUnplanned(doc: Doc): Stage | undefined {
  return stagesOf(doc)
    .filter(stage => statusOf(stage) !== 'planned')
    .at(-1)
}

/** Every block of every stage, by key. */
export function blocksByKey(doc: Doc): Map<string, Block> {
  const found = new Map<string, Block>()
  for (const stage of stagesOf(doc)) for (const block of stage.blocks ?? []) found.set(block.key, block)
  return found
}

/** The block with this id, and the id of the stage that holds it. */
export function blockById(doc: Doc, id: string): { block: Block; stage: Stage } | undefined {
  for (const stage of stagesOf(doc)) {
    const block = (stage.blocks ?? []).find(each => each.id === id)
    if (block) return { block, stage }
  }
  return undefined
}

/** An open submission, as `loft yard read` lists it under the agent that sent it. */
export type Submission = { key: string; blocks: string[]; agent: Actor }

export function openSubmissions(doc: Doc): Submission[] {
  const groups = Array.isArray(doc.submissions) ? doc.submissions.filter(isObject) : []
  return groups.flatMap(group => {
    const agent = (isObject(group.agent) ? group.agent : {}) as Actor
    const open = Array.isArray(group.open) ? group.open.filter(isObject) : []
    return open
      .filter(each => each.state === undefined || each.state === 'open')
      .map(each => ({
        key: String(each.key),
        blocks: Array.isArray(each.blocks) ? each.blocks.map(String) : [],
        agent,
      }))
  })
}

/** The kinds of block the person answers or decides. */
const ASKED = new Set(['question', 'approval', 'plan'])

/** Whether the person has yet to answer or decide the block. */
export function waits(block: Block): boolean {
  return ASKED.has(block.kind) && !block.answer && !block.outcome
}

/** The blocks of a submission that still wait on the person, in the submission's order. */
export function waitingOf(doc: Doc, submission: string): Block[] {
  const sent = openSubmissions(doc).find(each => each.key === submission)
  if (!sent) return []
  const blocks = blocksByKey(doc)
  return sent.blocks.flatMap(key => {
    const block = blocks.get(key)
    return block && waits(block) ? [block] : []
  })
}

/** The page's address for the branch's own Yard Log, where a page server serves it. */
export function pageOf(doc: Doc | undefined): string | undefined {
  const page = doc?.page
  return isObject(page) && page.available === true && typeof page.url === 'string' ? page.url : undefined
}

/** A block id made of words: lowercase letters, digits and hyphens, starting with a letter. */
export function idOf(...words: string[]): string {
  const joined = words
    .join('-')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return /^[a-z]/.test(joined) ? joined : `x-${joined}`
}
