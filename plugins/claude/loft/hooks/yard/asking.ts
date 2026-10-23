// Answering in either place: the agent's `AskUserQuestion` and `ExitPlanMode` calls are sent to
// Yard Log as Claude Code's own dialog shows them, and what the agent waits on with `yard_wait` is
// asked in Claude Code's dialog as well as on the page; whichever the person answers first counts,
// and the other closes (the Features "Answer the agent's questions in either place", "Decide a
// plan in either place" and "Answer a sent submission in either place").

import type { EngineInterface, ToolCallResult } from 'claude-code'

import type { Doc, Ran } from '../loft'
import { askResultOf, feedbackIn, givenOf, planId, planTitle, questionBlocks, type Asked, type Given } from './answers'
import { dialogOf, recordedOf, type DialogQuestion, type Paging } from './dialog'
import { activeStage, blockById, idOf, lastUnplanned, openSubmissions, waitingOf, whoOf, type Asker, type Block } from './log'
import { callerArgs, spawnYard, succeeded, type Caller, type Running } from './run'
import { askerOf, peek, prepare, run, state, type Outcome, type Presenting } from './state'

type Next = ((e: never) => Promise<ToolCallResult>) & { signal: AbortSignal }

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
function stageFor(doc: Doc, asker: Asker, minted: string): { id: string; opened?: Record<string, unknown> } {
  const active = activeStage(doc, asker)
  if (active) return { id: active.id }
  const id = idOf('asked', minted)
  const after = lastUnplanned(doc)
  return {
    id,
    opened: { id, kind: 'stage', title: 'Asked in the session', status: 'active', ...(after ? { after: after.id } : {}) },
  }
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

async function record($: EngineInterface, cwd: string, caller: Caller, given: Given[]) {
  await run($, cwd, ['answer', '--input', '-', ...callerArgs(caller)], JSON.stringify(given))
}

async function withdraw($: EngineInterface, cwd: string, caller: Caller, submission: string) {
  await run($, cwd, ['withdraw', '--submission', submission, ...callerArgs(caller)])
}

/** The agent's own questions: sent to Yard Log, and answered in the terminal or on the page. */
export async function askQuestions($: EngineInterface, e: { questions: readonly Asked[]; agentId?: string; tool_use_id?: string }, next: Next) {
  const asking = await prepare($, e.agentId)
  if (!asking) return next(e as never)
  const { cwd, caller, doc } = asking
  const minted = mintedFor(e, caller)
  const ids = e.questions.map((_, n) => idOf('ask', minted, String(n + 1)))
  const stage = stageFor(doc, askerOf(caller), minted)
  const batch = [...(stage.opened ? [stage.opened] : []), ...questionBlocks(e.questions, ids, stage.id)]
  const submission = await send($, cwd, caller, batch)
  if (submission === undefined) return next(e as never)
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

/** A plan's outcome in a wait's document, where the person decided it. */
function outcomeOf(ran: Ran | undefined, id: string): Outcome | undefined {
  if (!ran || !succeeded(ran)) return undefined
  const block = [...blocksOf(ran.doc).values()].find(each => each.id === id)
  return block?.outcome ?? undefined
}

const rejection = (reason: unknown) =>
  `The person rejected the plan on the Yard page${typeof reason === 'string' && reason !== '' ? `: ${reason}` : '.'}`

/**
 * The agent's plan: sent to Yard Log, and decided in the terminal or on the page. The page's
 * decision reaches Claude Code through its permission request (`decideOnPage`), which waits for it.
 */
export async function presentPlan(
  $: EngineInterface,
  e: { plan?: unknown; planFilePath?: unknown; agentId?: string; tool_use_id?: string },
  next: Next,
) {
  const plan = typeof e.plan === 'string' ? e.plan : ''
  const path = typeof e.planFilePath === 'string' ? e.planFilePath : undefined
  if (plan.trim() === '') return next(e as never)
  const asking = await prepare($, e.agentId)
  if (!asking) return next(e as never)
  const { cwd, caller, doc } = asking
  const minted = mintedFor(e, caller)
  const id = planId(path, idOf('plan', minted))
  const held = blockById(doc, id)
  const stage = held ? { id: held.stage.id } : stageFor(doc, askerOf(caller), minted)
  const block = { id, kind: 'plan', stage: stage.id, title: planTitle(plan, path), markdown: plan, ...(path ? { path } : {}), planMode: true }
  const submission = await send($, cwd, caller, [...('opened' in stage && stage.opened ? [stage.opened] : []), block])
  if (submission === undefined) return next(e as never)
  const presenting: Presenting = { id, cwd, caller, submission }
  state.presenting = presenting
  try {
    const ran = await next(e as never)
    presenting.end?.()
    const page = presenting.decided
    if (page) return page.verdict === 'rejected' ? { deny: rejection(page.reason) } : ran
    if (next.signal.aborted) {
      await withdraw($, cwd, caller, submission)
      return ran
    }
    const isApproved = ran.deny === undefined && ran.isError !== true
    const reason = isApproved ? undefined : feedbackIn(ran.deny ?? ran.text ?? '')
    await record($, cwd, caller, [
      { block: id, verdict: isApproved ? 'approved' : 'rejected', ...(reason ? { reason } : {}) },
    ])
    return ran
  } finally {
    state.presenting = null
    presenting.end?.()
  }
}

/** Claude Code's permission for a plan the page approved: plan mode ends and auto mode begins. */
export function approval(input: unknown) {
  return {
    behavior: 'allow' as const,
    // Claude Code drops an allow for a tool that needs the person unless it gives the input back.
    updatedInput: (input ?? {}) as Record<string, unknown>,
    updatedPermissions: [{ type: 'setMode' as const, mode: 'auto' as const, destination: 'session' as const }],
  }
}

/**
 * Answers Claude Code's request to approve the plan where the page decides it first. Claude Code
 * shows its dialog apart from this request and answers beneath it at once with no decision, so the
 * request runs the plan's wait itself: the wait's process is this hook's own, and a hook's time
 * stands still while its own `$` calls run, so it waits as long as the person takes. It ends once
 * the page decides, the terminal decided and the call went on, or Claude Code drops the request.
 */
export async function decideOnPage<R>(
  $: EngineInterface,
  presented: Presenting,
  beneath: () => Promise<R>,
  input: unknown,
  signal: AbortSignal,
) {
  const wait = waiting($, presented.cwd, presented.submission, presented.caller)
  presented.end = wait.end
  const dropped = new Promise<Ran>(done => {
    if (signal.aborted) done(ENDED)
    signal.addEventListener('abort', () => done(ENDED), { once: true })
  })
  const ran = await Promise.race([wait.done, dropped])
  wait.end()
  const decided = outcomeOf(ran, presented.id)
  if (decided?.channel !== 'page') return beneath()
  presented.decided = decided
  if (decided.verdict === 'approved') return { decision: approval(input) }
  return { decision: { behavior: 'deny' as const, message: rejection(decided.reason) } }
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
  const aborted = new Promise<Ran>(done => {
    if (signal.aborted) done(ENDED)
    signal.addEventListener('abort', () => done(ENDED), { once: true })
  })
  const ended = Promise.race([wait.done, aborted])
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
    const recorded = await run($, cwd, ['answer', '--input', '-', ...callerArgs(caller)], JSON.stringify(given))
    if (!succeeded(recorded)) return
  }
}
