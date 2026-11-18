// Answering in either place: the agent's `AskUserQuestion` and `ExitPlanMode` calls are sent to
// Yard Log as Claude Code's own dialog shows them, and what the agent waits on with `yard_wait` is
// asked in Claude Code's dialog as well as on the page; whichever the person answers first counts,
// and the other closes (the Features "Answer the agent's questions in either place", "Decide a
// plan in either place" and "Answer a sent submission in either place").

import type { EngineInterface, ToolCallResult } from 'claude-code'

import type { Doc, Ran } from '../loft'
import { askResultOf, givenOf, planId, planPath, planTitle, isCanceled, questionBlocks, rejectionIn, type Asked, type Given } from './answers'
import { dialogOf, recordedOf, type DialogQuestion, type Paging } from './dialog'
import { capture, checkCapture } from './capture'
import { activeStage, idOf, isAsker, lastUnplanned, openSubmissions, pageOf, waitingOf, whoOf, type Asker, type Block } from './log'
import { callerArgs, spawnYard, succeeded, type Caller, type Running } from './run'
import { askerOf, peek, prepare, run, state, type Outcome } from './state'

type Next = ((e: never) => Promise<ToolCallResult>) & { signal: AbortSignal }
type SelectedStage = { id: string; key?: string; blocks?: Block[]; opened?: Record<string, unknown> }

const ENDED: Ran = { exitCode: -1, stderr: '', unrun: 'ended' }

/** Runs `loft yard wait` again after each `pending`, until the submission is no longer open. */
export function waiting($: EngineInterface, cwd: string, submission: string, caller: Caller): Running {
  let current: Running | undefined
  let isEnded = false
  const args = ['wait', '--submission', submission, '--timeout', '540', ...callerArgs(caller)]
  const done = (async (): Promise<Ran> => {
    for (;;) {
      if (isEnded) return ENDED
      current = spawnYard($, state.settings, cwd, args)
      const ran = await current.done
      if (isEnded) return ENDED
      if (!(succeeded(ran) && ran.doc.outcome === 'pending')) return ran
    }
  })()
  return {
    done,
    end: () => {
      isEnded = true
      current?.end()
    },
  }
}

/** The blocks a `loft yard` document lists, by key. */
function blocksOf(doc: Doc | undefined): Map<string, Block> {
  const blocks = Array.isArray(doc?.blocks) ? (doc.blocks as Block[]) : []
  return new Map(blocks.map(block => [block.key, block]))
}

/**
 * Whichever comes first: Claude Code's answer in the terminal, or what the page gives. The other
 * runs on; a terminal that fails after the page came first fails unheard.
 */
function race<T, P>(terminal: Promise<T>, page: Promise<P>): Promise<{ terminal: T } | { page: P }> {
  const answered = terminal.then(ran => ({ terminal: ran }))
  answered.catch(() => {})
  return Promise.race([answered, page.then(ran => ({ page: ran }))])
}

const isAnswered = (ran: ToolCallResult) =>
  ran.deny === undefined &&
  ran.isError !== true &&
  Object.keys((ran.result as { answers?: object } | undefined)?.answers ?? {}).length > 0

/** The stage a batch writes to: the agent's active one, or one the mod opens for it. */
function stageFor(doc: Doc, asker: Asker, minted: string): SelectedStage {
  const active = activeStage(doc, asker)
  if (active) return { id: active.id, key: active.key, blocks: active.blocks }
  const id = idOf('asked', minted)
  const after = lastUnplanned(doc)
  return {
    id,
    opened: { id, kind: 'stage', title: 'Asked in the session', status: 'active', ...(after ? { after: after.id } : {}) },
  }
}

/**
 * A file's revisions belong to the selected stage, including blocks with legacy ids: the plan
 * block's id, and the key of the block it writes again, where the stage holds one.
 */
function planFor(stage: SelectedStage, path: string | undefined, fallback: string): { id: string; key?: string } {
  const base = planId(path, fallback)
  if (!path) return { id: base }
  // The length separates the filename identity from the stage namespace without ambiguity.
  const prefix = idOf('plan', String(base.length), base)
  const held = stage.blocks?.find(block => block.kind === 'plan' && (block.id === base || block.id.startsWith(`${prefix}-`)))
  return held ? { id: held.id, key: held.key } : { id: idOf(prefix, stage.key ?? stage.id) }
}

/**
 * The part of the ids the mod mints for one ask: the id Claude Code gives the tool call, which no
 * other call shares, so that no two asks, by any session or subagent, write to one block.
 */
const mintedFor = (e: { tool_use_id?: unknown }, caller: Caller) =>
  typeof e.tool_use_id === 'string' && e.tool_use_id !== '' ? idOf(e.tool_use_id) : idOf('call', whoOf(caller), String(Date.now()))

/** Sends a batch and returns its submission's key, or undefined where it could not be sent. */
async function send($: EngineInterface, cwd: string, caller: Caller, batch: unknown[]): Promise<string | undefined> {
  const sent = await run($, cwd, ['write', '--input', '-', '--send', ...callerArgs(caller)], JSON.stringify(batch))
  return succeeded(sent) && typeof sent.doc.submission === 'string' ? sent.doc.submission : undefined
}

/** Records what the person gave in the terminal with `loft yard answer`. */
export function record($: EngineInterface, cwd: string, caller: Caller, given: Given[]) {
  return run($, cwd, ['answer', '--input', '-', ...callerArgs(caller)], JSON.stringify(given))
}

/** `ENDED` once `signal` aborts, as Claude Code drops a call or a request. */
function endedBy(signal: AbortSignal) {
  return new Promise<Ran>(done => {
    if (signal.aborted) done(ENDED)
    signal.addEventListener('abort', () => done(ENDED), { once: true })
  })
}

async function withdraw($: EngineInterface, cwd: string, caller: Caller, submission: string) {
  await run($, cwd, ['withdraw', '--submission', submission, ...callerArgs(caller)])
}

/** The agent's own questions: sent to Yard Log, and answered in the terminal or on the page. */
export async function askQuestions($: EngineInterface, e: { questions: readonly Asked[]; agentId?: string; tool_use_id?: string }, next: Next) {
  const captured = await capture(e.agentId, next.signal, async () => {
    const asking = await prepare($, e.agentId)
    checkCapture(next.signal)
    if (!asking) return undefined
    const { cwd, caller, doc } = asking
    const minted = mintedFor(e, caller)
    const ids = e.questions.map((_, n) => idOf('ask', minted, String(n + 1)))
    const stage = stageFor(doc, askerOf(caller), minted)
    const batch = [...(stage.opened ? [stage.opened] : []), ...questionBlocks(e.questions, ids, stage.id)]
    const submission = await send($, cwd, caller, batch)
    return submission === undefined ? undefined : { cwd, caller, ids, submission }
  })
  if (!captured) return next(e as never)
  const { cwd, caller, ids, submission } = captured
  const wait = waiting($, cwd, submission, caller)
  const terminal = next(e as never)
  const raced = await race(terminal, wait.done)
  if ('page' in raced && succeeded(raced.page) && raced.page.doc.outcome === 'answered') {
    return { result: askResultOf(e.questions, blocksOf(raced.page.doc), ids) }
  }
  wait.end()
  const ran = 'terminal' in raced ? raced.terminal : await terminal
  if (isAnswered(ran)) await record($, cwd, caller, givenOf(e.questions, ids, ran.result))
  else await withdraw($, cwd, caller, submission)
  return ran
}

/** A plan's outcome as Yard Log holds it, where the page decided it. */
async function decidedOnPage($: EngineInterface, cwd: string, caller: Caller, id: string): Promise<Outcome | undefined> {
  const read = await peek($, cwd, caller)
  if (!succeeded(read)) return undefined
  const stages = Array.isArray(read.doc.stages) ? (read.doc.stages as { blocks?: Block[] }[]) : []
  const block = stages.flatMap(stage => stage.blocks ?? []).find(each => each.id === id)
  return block?.outcome?.channel === 'page' ? block.outcome : undefined
}

/** The agent's own open submission that holds the block `key`, which a new version of it needs withdrawn. */
function heldBy(doc: Doc, key: string | undefined, asker: Asker): string | undefined {
  if (key === undefined) return undefined
  return openSubmissions(doc).find(each => each.blocks.includes(key) && isAsker(each.agent, asker))?.key
}

/**
 * A note telling the agent that nobody decided the plan and where it waits, then the host's own
 * words: an Agent SDK host may answer `ExitPlanMode` itself, and a tool error may end it, before
 * the person saw it. The note leads, since a host such as T3 Code tells the agent to stop.
 */
function undecided(ran: ToolCallResult, submission: string, address: string | undefined): ToolCallResult {
  const where = address ? `the Yard page (${address})` : 'the Yard page'
  const host = ran.deny ?? ran.text
  const note =
    `Nobody has decided this plan: ExitPlanMode ended before the person did. The plan waits on ${where} ` +
    `as submission ${submission}; wait on it with yard_wait for the person's decision before you end the turn.`
  // A deny: core gives the agent its own messages for a result it got back unchanged.
  return { deny: host ? `${note} The host's own words follow.\n\n${host}` : note }
}

const rejection = (reason: unknown) =>
  `The person rejected the plan on the Yard page${typeof reason === 'string' && reason !== '' ? `: ${reason}` : '.'}`

/**
 * The agent's plan: sent to Yard Log, and decided in the terminal or on the page. The page's
 * decision reaches Claude Code through the plugin's `PermissionRequest` command hook, which waits
 * for it beside Claude Code's dialog; once Claude Code goes on, a decision the page made is
 * Yard Log's already, and only one made in the terminal is recorded here. A call that ends with
 * neither, as when the host answers it, leaves the plan open on the page for the agent to wait on.
 */
export async function presentPlan(
  $: EngineInterface,
  e: { plan?: unknown; planFilePath?: unknown; agentId?: string; tool_use_id?: string },
  next: Next,
) {
  const plan = typeof e.plan === 'string' ? e.plan : ''
  const path = typeof e.planFilePath === 'string' ? e.planFilePath : undefined
  if (plan.trim() === '') return next(e as never)
  const captured = await capture(e.agentId, next.signal, async () => {
    const asking = await prepare($, e.agentId)
    checkCapture(next.signal)
    if (!asking) return undefined
    const { cwd, caller, doc } = asking
    const minted = mintedFor(e, caller)
    const asker = askerOf(caller)
    const stage = stageFor(doc, asker, minted)
    const { id, key } = planFor(stage, path, idOf('plan', minted))
    const held = heldBy(doc, key, asker)
    if (held) await withdraw($, cwd, caller, held)
    const kept = planPath(typeof doc?.tree === 'string' ? doc.tree : undefined, path)
    const block = { id, kind: 'plan', stage: stage.id, title: planTitle(plan, path), markdown: plan, ...(kept ? { path: kept } : {}), planMode: true }
    const submission = await send($, cwd, caller, [...('opened' in stage && stage.opened ? [stage.opened] : []), block])
    return submission === undefined ? undefined : { id, cwd, caller, submission, address: pageOf(doc) }
  })
  if (!captured) return next(e as never)
  const { id, cwd, caller, submission, address } = captured
  const ran = await next(e as never)
  const page = await decidedOnPage($, cwd, caller, id)
  if (page) return page.verdict === 'rejected' ? { deny: rejection(page.reason) } : ran
  const denial = ran.deny ?? ran.text ?? ''
  if (next.signal.aborted || isCanceled(denial)) {
    await withdraw($, cwd, caller, submission)
    return ran
  }
  if (ran.deny === undefined && ran.isError !== true) {
    await record($, cwd, caller, [{ block: id, verdict: 'approved' }])
    return ran
  }
  const rejected = rejectionIn(denial)
  if (!rejected) return undecided(ran, submission, address)
  await record($, cwd, caller, [{ block: id, verdict: 'rejected', ...rejected }])
  return ran
}

/**
 * The dialogs the mod itself opens, by their first question: the questions the dialog asks, the
 * end of the wait that closes it, and its result once answered. `$.ui.ask` asks one question
 * with its labels alone, and Claude Code refuses `AskUserQuestion` through `$.tool.call`, so the
 * mod's own hook asks the whole dialog in its place.
 */
type OwnDialog = { questions: DialogQuestion[]; ended: Promise<void>; result?: ToolCallResult }
export const dialogs = new Map<string, OwnDialog>()

/** One of the mod's own dialogs: Claude Code asks it whole, and it closes once its wait ends first. */
export async function ownDialog(e: { questions: readonly Asked[] }, next: Next) {
  const own = dialogs.get(String(e.questions[0]?.question))
  if (!own) return next(e as never)
  const first = await race(next({ ...e, questions: own.questions } as never), own.ended)
  if ('page' in first) return { result: { questions: own.questions, answers: {} } }
  own.result = first.terminal
  return first.terminal
}

/** `yard_wait`: waits in Yard Log, asking in Claude Code's dialog meanwhile. */
export async function waitOn($: EngineInterface, cwd: string, caller: Caller, submission: string, signal: AbortSignal) {
  const wait = waiting($, cwd, submission, caller)
  const ended = Promise.race([wait.done, endedBy(signal)])
  const asking = askInTerminal($, cwd, caller, submission, ended.then(() => undefined))
  const ran = await ended
  wait.end()
  await asking
  return ran
}

/** Asks the submission's blocks that still wait, dialog after dialog, until it is over. */
async function askInTerminal($: EngineInterface, cwd: string, caller: Caller, submission: string, over: Promise<void>) {
  let isOver = false
  void over.then(() => (isOver = true))
  let paging: Paging = {}
  while (!isOver) {
    const read = await peek($, cwd, caller)
    if (!succeeded(read) || isOver) return
    const sender = openSubmissions(read.doc).find(each => each.key === submission)?.agent
    const who = sender?.agent ? `subagent ${sender.agentType ?? ''} ${sender.agent}`.replace(/\s+/g, ' ') : undefined
    const dialog = dialogOf(waitingOf(read.doc, submission), paging, who)
    if (!dialog) return
    const [first] = dialog.questions
    const own: OwnDialog = { questions: dialog.questions, ended: over }
    dialogs.set(first!.question, own)
    try {
      await $.ui.ask(first!.question, {
        options: first!.options.map(option => option.label),
        header: first!.header,
        ...(first!.multiSelect ? { multiSelect: true as const } : {}),
      })
    } catch {
      // Dismissed, or closed once the wait ended.
    } finally {
      dialogs.delete(first!.question)
    }
    const answered = own.result
    if (isOver || !answered || !isAnswered(answered)) return
    const { given, paging: after } = recordedOf(dialog, answered.result, paging)
    paging = after
    if (given.length === 0) continue
    if (!succeeded(await record($, cwd, caller, given))) return
  }
}
