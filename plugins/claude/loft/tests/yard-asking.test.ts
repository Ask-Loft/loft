import { expect, test } from 'claude-code/testing'

import { idOf } from '../hooks/adapter/log'
import { askedId, block, callIdOf, deferred, flagOf, readDoc, SESSION, stage, start, TREE, world, yardRuns, type Engine } from './yard-world'

const QUESTIONS = [
  {
    question: 'Which readers?',
    header: 'Readers',
    multiSelect: false,
    options: [
      { label: 'Members (Recommended)', description: 'With a card' },
      { label: 'Visitors', description: '' },
    ],
  },
]

const ask = ($: Engine, agentId?: string) =>
  $.tool.call({ tool: 'AskUserQuestion', questions: QUESTIONS, ...(agentId ? { agentId } : {}) } as never)

const writes = (w: ReturnType<typeof world>) =>
  w.state.runs.filter(run => run.argv.includes('write')).map(run => JSON.parse(run.stdin ?? '[]') as Record<string, unknown>[])

const answeredOnPage = (blocks: Record<string, unknown>[]) => ({ outcome: 'answered', errors: [], blocks, takenUp: true, waited: 3 })

test("the agent's questions go to Yard Log as Claude Code's dialog shows them, and the page's answer reaches the agent by its labels", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  const dialog = deferred<Record<string, unknown>>()
  w.state.terminal.AskUserQuestion = () => dialog.promise
  await start($)
  w.state.waits.push(() => answeredOnPage([{ ...block(askedId(w), 'question', {}), answer: { labels: ['Members'], notes: 'for now', channel: 'page' } }]))
  const answered = await ask($)
  expect(writes(w).at(-1)).toEqual([
    {
      id: `ask-${callIdOf(w, 'AskUserQuestion')}-1`,
      kind: 'question',
      stage: 'shape',
      prompt: 'Which readers?',
      header: 'Readers',
      multiple: false,
      ownWords: true,
      options: [
        { label: 'Members', detail: 'With a card', recommended: true },
        { label: 'Visitors', recommended: false },
      ],
    },
  ])
  const sent = w.state.runs.find(run => run.argv.includes('write'))!
  expect(sent.argv).toContain('--send')
  expect(answered.result).toEqual({
    questions: QUESTIONS,
    answers: { 'Which readers?': 'Members (Recommended)' },
    annotations: { 'Which readers?': { notes: 'for now' } },
  })
  expect(yardRuns(w).some(run => run.startsWith('answer'))).toBe(false)
})

test('an answer given first in the terminal is recorded in Yard Log with the channel of the terminal, and the wait ends', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.AskUserQuestion = () => ({ result: { questions: QUESTIONS, answers: { 'Which readers?': 'Visitors' } }, text: 'ok' })
  await start($)
  const answered = await ask($)
  expect((answered.result as { answers: object }).answers).toEqual({ 'Which readers?': 'Visitors' })
  const recorded = w.state.runs.find(run => run.argv.includes('answer'))!
  expect(JSON.parse(recorded.stdin!)).toEqual([{ block: `ask-${callIdOf(w, 'AskUserQuestion')}-1`, labels: ['Visitors'] }])
  expect(flagOf(recorded.argv, '--session')).toBe(SESSION)
})

test('a dialog closed without an answer withdraws the submission', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.AskUserQuestion = () => ({ deny: 'The person closed the dialog.' })
  await start($)
  await ask($)
  expect(yardRuns(w).filter(run => run.startsWith('withdraw'))).toEqual(['withdraw --submission sub-1 --session session-1 --port 7411 --format json'])
})

test('an agent with no active stage has its questions in a stage the mod opens after the last unplanned stage', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('done', 'done', { session: 'other' }), stage('later', 'planned', null)] })
  w.state.terminal.AskUserQuestion = () => ({ result: { questions: QUESTIONS, answers: { 'Which readers?': 'Visitors' } }, text: 'ok' })
  await start($)
  await ask($)
  const [opened, asked] = writes(w).at(-1)!
  const call = callIdOf(w, 'AskUserQuestion')
  expect(opened).toEqual({ id: `asked-${call}`, kind: 'stage', title: 'Asked in the session', status: 'active', after: 'done' })
  expect(asked!.stage).toBe(`asked-${call}`)
})

test("a subagent's questions go to its own stage, in its own name", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({
    stages: [stage('shape', 'active', { session: SESSION }), stage('dig', 'active', { session: SESSION, agent: 'a9', agentType: 'Explore' })],
  })
  w.state.terminal.AskUserQuestion = () => ({ result: { questions: QUESTIONS, answers: { 'Which readers?': 'Visitors' } }, text: 'ok' })
  await start($)
  await $.classic.SubagentStart({ agent_id: 'a9', agent_type: 'Explore' } as never)
  await ask($, 'a9')
  expect(writes(w).at(-1)![0]!.stage).toBe('dig')
  const sent = w.state.runs.filter(run => run.argv.includes('write')).at(-1)!
  expect(flagOf(sent.argv, '--agent')).toBe('a9')
})

test('where Yard Log takes no write, as on a detached HEAD, the questions are asked in the terminal alone', async ($, on) => {
  const w = world(on)
  w.state.answers.read = { doc: { outcome: 'error', errors: [{ message: 'HEAD is detached' }] }, exitCode: 1 }
  w.state.terminal.AskUserQuestion = () => ({ result: { questions: QUESTIONS, answers: { 'Which readers?': 'Visitors' } }, text: 'ok' })
  await start($)
  const answered = await ask($)
  expect((answered.result as { answers: object }).answers).toEqual({ 'Which readers?': 'Visitors' })
  expect(yardRuns(w).filter(run => run.startsWith('write') || run.startsWith('answer'))).toEqual([])
})

test('a refused write leaves the questions to the terminal', async ($, on) => {
  const w = world(on)
  w.state.answers.write = { doc: { outcome: 'refused', errors: [{ message: 'Yard Log is closed' }] }, exitCode: 1 }
  w.state.terminal.AskUserQuestion = () => ({ result: { questions: QUESTIONS, answers: { 'Which readers?': 'Visitors' } }, text: 'ok' })
  await start($)
  await ask($)
  expect(yardRuns(w).filter(run => run.startsWith('wait') || run.startsWith('answer'))).toEqual([])
})

const PLAN = '# Build the shelf\n\nOne step.'

/**
 * Claude Code asking for a plan's approval: the `ExitPlanMode` call whose dialog shows until it is
 * answered, and the permission request raised beneath it, which the mod's hook answers or leaves
 * to the dialog. `answer` settles the call as the dialog's answer would.
 */
async function planAsked($: Engine, w: ReturnType<typeof world>, input: Record<string, unknown>) {
  const dialog = deferred<Record<string, unknown>>()
  w.state.terminal.ExitPlanMode = () => dialog.promise
  const ran = $.tool.call({ tool: 'ExitPlanMode', ...input } as never)
  await w.clock.settle()
  const waitsBefore = yardRuns(w).filter(run => run.startsWith('wait')).length
  const permission = $.classic.PermissionRequest({ tool_name: 'ExitPlanMode', tool_input: input, permission_mode: 'plan' } as never)
  await w.clock.settle()
  return {
    ran,
    permission,
    waitsBefore,
    answer: (result: Record<string, unknown>) => {
      dialog.resolve(result)
      return ran
    },
  }
}

const approvedOnPage = () => answeredOnPage([{ ...block('plan-shelf', 'plan', {}), outcome: { verdict: 'approved', channel: 'page' } }])

test("the agent's plan goes to Yard Log as a plan block in plan mode, with no path for a file outside the tree, and the terminal's verdict is recorded", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.ExitPlanMode = () => ({ deny: "The user doesn't want to proceed. the user said: split it" })
  await start($)
  const ran = await $.tool.call({ tool: 'ExitPlanMode', plan: PLAN, planFilePath: '/home/reader/.claude/plans/shelf.md' } as never)
  expect(writes(w).at(-1)).toEqual([
    { id: 'plan-shelf', kind: 'plan', stage: 'shape', title: 'Build the shelf', markdown: PLAN, planMode: true },
  ])
  expect(ran.deny ?? ran.text).toMatch(/split it/)
  const recorded = w.state.runs.find(run => run.argv.includes('answer'))!
  expect(JSON.parse(recorded.stdin!)).toEqual([{ block: 'plan-shelf', verdict: 'rejected', reason: 'split it' }])
})

test('a plan rejected in the terminal with no feedback typed is recorded as rejected with no reason', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.ExitPlanMode = () => ({ deny: "The user doesn't want to proceed with this tool use." })
  await start($)
  await $.tool.call({ tool: 'ExitPlanMode', plan: PLAN, planFilePath: '/home/reader/.claude/plans/shelf.md' } as never)
  const recorded = w.state.runs.find(run => run.argv.includes('answer'))!
  expect(JSON.parse(recorded.stdin!)).toEqual([{ block: 'plan-shelf', verdict: 'rejected' }])
})

test('a plan from a file within the tree keeps its path relative to the tree', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  await start($)
  await $.tool.call({ tool: 'ExitPlanMode', plan: PLAN, planFilePath: `${TREE}/docs/plans/shelf.md` } as never)
  expect(writes(w).at(-1)).toEqual([
    { id: 'plan-shelf', kind: 'plan', stage: 'shape', title: 'Build the shelf', markdown: PLAN, path: 'docs/plans/shelf.md', planMode: true },
  ])
})

test("a plan approved on the page first answers Claude Code's permission: it allows the plan, gives the input back and turns auto mode on", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  await start($)
  const input = { plan: PLAN, planFilePath: '/p/shelf.md' }
  const asked = await planAsked($, w, input)
  w.state.page.resolve(approvedOnPage())
  expect(await asked.permission).toEqual({
    decision: { behavior: 'allow', updatedInput: input, updatedPermissions: [{ type: 'setMode', mode: 'auto', destination: 'session' }] },
  })
  expect((await asked.answer({ result: 'approved', text: 'approved' })).text).toBe('approved')
  expect(yardRuns(w).some(run => run.startsWith('answer'))).toBe(false)
})

// A hook's ten-second budget stands still only while its own `$` calls run, so the permission
// request, which waits as long as the person takes, must run the plan's wait itself: no wait runs
// before Claude Code raises it, and its one wait runs under it.
test('the permission request runs the wait for the plan itself, and nothing waits before it is raised', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  await start($)
  const asked = await planAsked($, w, { plan: PLAN, planFilePath: '/p/shelf.md' })
  expect(asked.waitsBefore).toBe(0)
  expect(yardRuns(w).filter(run => run.startsWith('wait'))).toHaveLength(1)
  w.state.page.resolve(approvedOnPage())
  await asked.permission
  await asked.answer({ result: 'approved', text: 'approved' })
})

test('a plan rejected on the page first denies the plan with the reason', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  await start($)
  const asked = await planAsked($, w, { plan: PLAN, planFilePath: '/p/shelf.md' })
  w.state.page.resolve(answeredOnPage([{ ...block('plan-shelf', 'plan', {}), outcome: { verdict: 'rejected', reason: 'too big', channel: 'page' } }]))
  const message = 'The person rejected the plan on the Yard page: too big'
  expect(await asked.permission).toEqual({ decision: { behavior: 'deny', message } })
  const ran = await asked.answer({ deny: message })
  expect(ran.deny ?? ran.text).toBe(message)
})

test('a plan decided in the terminal first ends the wait the permission request runs, and its verdict is recorded', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  await start($)
  const asked = await planAsked($, w, { plan: PLAN, planFilePath: '/p/shelf.md' })
  await asked.answer({ result: 'approved', text: 'approved' })
  expect(await asked.permission).toEqual({})
  expect(w.state.killed).toBe(1)
  const recorded = w.state.runs.find(run => run.argv.includes('answer'))!
  expect(JSON.parse(recorded.stdin!)).toEqual([{ block: 'plan-shelf', verdict: 'approved' }])
})

const sentDoc = (blocks: ReturnType<typeof block>[], agent: Record<string, unknown> = { session: SESSION }) =>
  readDoc({
    stages: [{ ...stage('shape', 'active', { session: SESSION }), blocks }],
    submissions: [{ agent, open: [{ key: 'sub-7', blocks: blocks.map(each => each.key), state: 'open' }] }],
  })

const pick = (id: string, options: number, multiple = false) =>
  block(id, 'question', {
    prompt: `Pick ${id}?`,
    header: 'Pick',
    multiple,
    options: Array.from({ length: options }, (_, n) => ({ label: `O${n + 1}`, recommended: false })),
  })

test('yard_wait runs loft yard wait again after each pending, and gives what it ends with', async ($, on) => {
  const w = world(on)
  w.state.read = sentDoc([pick('q1', 2)])
  w.state.terminal.AskUserQuestion = () => new Promise(() => {})
  await start($)
  w.state.waits.push({ outcome: 'pending', errors: [] }, { outcome: 'pending', errors: [] }, answeredOnPage([]))
  const ran = await $.tool.call({ tool: 'mcp__loft__yard_wait', submission: 'sub-7' })
  expect(JSON.parse(String(ran.result)).outcome).toBe('answered')
  const waits = w.state.runs.filter(run => run.argv.includes('wait'))
  expect(waits).toHaveLength(3)
  expect(waits[0]!.argv).toEqual(['loft', 'yard', 'wait', '--submission', 'sub-7', '--timeout', '540', '--session', SESSION, '--port', '7411', '--format', 'json'])
})

test('while yard_wait waits, what still waits is asked in the dialog, and an answer there is recorded from the terminal', async ($, on) => {
  const w = world(on)
  w.state.read = sentDoc([pick('q1', 2), block('ship', 'approval', { title: 'Ship it?', markdown: '' })])
  w.state.terminal.AskUserQuestion = e => {
    const asked = (e.questions as { question: string }[]).map(each => each.question)
    expect(asked).toEqual(['Pick q1?', 'Ship it?'])
    w.state.read = sentDoc([{ ...pick('q1', 2), answer: { labels: ['O2'] } }])
    w.state.page.resolve(answeredOnPage([]))
    return { result: { questions: e.questions, answers: { 'Pick q1?': 'O2', 'Ship it?': 'Reject' }, annotations: { 'Ship it?': { notes: 'not yet' } } } }
  }
  await start($)
  await $.tool.call({ tool: 'mcp__loft__yard_wait', submission: 'sub-7' })
  const recorded = w.state.runs.find(run => run.argv.includes('answer'))!
  expect(JSON.parse(recorded.stdin!)).toEqual([
    { block: 'q1', labels: ['O2'] },
    { block: 'ship', verdict: 'rejected', reason: 'not yet' },
  ])
  // What still waits is read for the agent without --mark-read, which leaves its last-read revision as it was.
  const reads = w.state.runs.filter(run => run.argv.includes('read')).at(-1)!
  expect(reads.argv).not.toContain('--mark-read')
})

test('a long question is asked over dialogs, three options and More options at a time', async ($, on) => {
  const w = world(on)
  w.state.read = sentDoc([pick('q1', 6)])
  const shown: string[][] = []
  w.state.terminal.AskUserQuestion = e => {
    const [question] = e.questions as { options: { label: string }[] }[]
    shown.push(question!.options.map(option => option.label))
    if (shown.length === 1) return { result: { questions: e.questions, answers: { 'Pick q1?': 'More options' } } }
    w.state.read = sentDoc([{ ...pick('q1', 6), answer: { labels: ['O5'] } }])
    w.state.page.resolve(answeredOnPage([]))
    return { result: { questions: e.questions, answers: { 'Pick q1?': 'O5' } } }
  }
  await start($)
  await $.tool.call({ tool: 'mcp__loft__yard_wait', submission: 'sub-7' })
  expect(shown).toEqual([
    ['O1', 'O2', 'O3', 'More options'],
    ['O4', 'O5', 'O6'],
  ])
  const recorded = w.state.runs.filter(run => run.argv.includes('answer'))
  expect(recorded.map(run => JSON.parse(run.stdin!))).toEqual([[{ block: 'q1', labels: ['O5'] }]])
})

test("the page's answer first closes the mod's own dialog, and nothing is recorded from it", async ($, on) => {
  const w = world(on)
  w.state.read = sentDoc([pick('q1', 2)])
  let isAsked = false
  w.state.terminal.AskUserQuestion = () => {
    isAsked = true
    w.state.page.resolve(answeredOnPage([]))
    return new Promise(() => {})
  }
  await start($)
  const ran = await $.tool.call({ tool: 'mcp__loft__yard_wait', submission: 'sub-7' })
  expect(isAsked).toBe(true)
  await w.clock.settle()
  expect(w.state.calls).toHaveLength(1)
  expect(JSON.parse(String(ran.result)).outcome).toBe('answered')
  expect(yardRuns(w).some(run => run.startsWith('answer'))).toBe(false)
})

test("a dialog for a subagent's submission names the subagent", async ($, on) => {
  const w = world(on)
  w.state.read = sentDoc([pick('q1', 2)], { session: SESSION, agent: 'a9', agentType: 'Explore' })
  const asked: string[] = []
  w.state.terminal.AskUserQuestion = e => {
    asked.push(...(e.questions as { question: string }[]).map(each => each.question))
    w.state.page.resolve(answeredOnPage([]))
    return new Promise(() => {})
  }
  await start($)
  const waited = $.tool.call({ tool: 'mcp__loft__yard_wait', submission: 'sub-7' })
  await w.clock.settle()
  expect(w.state.calls.map(call => call.tool)).toEqual(['AskUserQuestion'])
  expect(asked).toEqual(['Pick q1? (subagent Explore a9)'])
  await waited
})

test('a submission withdrawn while its dialog is open closes the dialog, records nothing and asks nothing more', async ($, on) => {
  const w = world(on)
  w.state.read = sentDoc([pick('q1', 2)])
  let asked = 0
  w.state.terminal.AskUserQuestion = () => {
    asked++
    w.state.page.resolve({ outcome: 'withdrawn', errors: [] })
    return new Promise(() => {})
  }
  await start($)
  const ran = await $.tool.call({ tool: 'mcp__loft__yard_wait', submission: 'sub-7' })
  await w.clock.settle()
  expect(JSON.parse(String(ran.result)).outcome).toBe('withdrawn')
  expect(asked).toBe(1)
  expect(yardRuns(w).some(run => run.startsWith('answer'))).toBe(false)
})

// The person interrupting the turn: a plugin above the mod's answers the call while it waits, which
// aborts what runs beneath, as Claude Code does on an interrupt.
const PERSON = {
  name: 'person',
  tier: 'prepend' as const,
  register(on: Parameters<typeof world>[0]) {
    on('tool.call', { tool: 'mcp__loft__yard_wait' }, async ($, e, next) => {
      const waited = next(e)
      waited.catch(() => {})
      // An inline plugin shares nothing with the test but the engine: the clock tells it when.
      await $.clock.sleep(60000)
      return { deny: 'Interrupted by the person.' }
    })
  },
}

test('an interrupt while yard_wait waits ends the running wait, closes the dialog, records nothing and leaves the submission open', { plugins: [PERSON] }, async ($, on) => {
  const w = world(on)
  w.state.read = sentDoc([pick('q1', 2)])
  const answer = deferred<Record<string, unknown>>()
  w.state.terminal.AskUserQuestion = () => answer.promise
  await start($)
  const ran = $.tool.call({ tool: 'mcp__loft__yard_wait', submission: 'sub-7' })
  await w.clock.settle()
  expect(w.state.calls.map(call => call.tool)).toEqual(['AskUserQuestion'])
  await w.clock.advance(60000)
  expect((await ran).deny).toBe('Interrupted by the person.')
  await w.clock.settle()
  answer.resolve({ result: { questions: [], answers: { 'Pick q1?': 'O1' } } })
  await w.clock.settle()
  expect(w.state.killed).toBe(1)
  expect(yardRuns(w).filter(run => run.startsWith('answer') || run.startsWith('withdraw'))).toEqual([])
  expect(w.state.calls).toHaveLength(1)
})

test("questions on a branch with no Yard Log open it, in a stage the mod opens, and Claude Code's dialog shows them as the agent wrote them", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ yardLog: { closedBy: 'open' }, stages: [] })
  w.state.terminal.AskUserQuestion = e => ({ result: { questions: e.questions, answers: { 'Which readers?': 'Visitors' }, annotations: { 'Which readers?': { notes: 'only them' } } } })
  await start($)
  await ask($)
  expect(writes(w).at(-1)!.map(each => each.kind)).toEqual(['stage', 'question'])
  expect(w.state.calls[0]!.questions).toEqual(QUESTIONS)
  const recorded = w.state.runs.find(run => run.argv.includes('answer'))!
  expect(JSON.parse(recorded.stdin!)).toEqual([{ block: `ask-${callIdOf(w, 'AskUserQuestion')}-1`, labels: ['Visitors'], notes: 'only them' }])
})

test('a plan from a file Yard Log holds is a new version of its block, in its stage, and an approval in the terminal is recorded approved', async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({
    stages: [{ ...stage('earlier', 'done', { session: SESSION }), blocks: [block('plan-shelf', 'plan', { title: 'Old' })] }, stage('shape', 'active', { session: SESSION })],
  })
  w.state.terminal.ExitPlanMode = () => ({ result: 'approved', text: 'approved' })
  await start($)
  const input = { plan: PLAN, planFilePath: '/p/shelf.md' }
  await $.tool.call({ tool: 'ExitPlanMode', ...input } as never)
  expect(writes(w).at(-1)!.map(each => [each.id, each.stage])).toEqual([['plan-shelf', 'earlier']])
  expect(w.state.calls.at(-1)).toMatchObject(input)
  const recorded = w.state.runs.find(run => run.argv.includes('answer'))!
  expect(JSON.parse(recorded.stdin!)).toEqual([{ block: 'plan-shelf', verdict: 'approved' }])
})

test("two subagents asking at one revision write to stages and blocks whose ids are made from each call's own id, not from what the mod read", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [] })
  w.state.terminal.AskUserQuestion = () => ({ result: { questions: QUESTIONS, answers: { 'Which readers?': 'Visitors' } }, text: 'ok' })
  await start($)
  await $.classic.SubagentStart({ agent_id: 'a9', agent_type: 'Explore' } as never)
  await $.classic.SubagentStart({ agent_id: 'b4', agent_type: 'Plan' } as never)
  await ask($, 'a9')
  await ask($, 'b4')
  const [first, second] = writes(w).slice(-2)
  const [a9, b4] = w.state.calls.filter(call => call.tool === 'AskUserQuestion').map(call => idOf(String(call.tool_use_id)))
  expect(a9).not.toBe(b4)
  expect(first!.map(each => each.id)).toEqual([`asked-${a9}`, `ask-${a9}-1`])
  expect(second!.map(each => each.id)).toEqual([`asked-${b4}`, `ask-${b4}-1`])
})

test("the mod's own reads, to send what the agent asks, leave the agent's last-read revision as it was", async ($, on) => {
  const w = world(on)
  w.state.read = readDoc({ stages: [stage('shape', 'active', { session: SESSION })] })
  w.state.terminal.AskUserQuestion = () => ({ result: { questions: QUESTIONS, answers: { 'Which readers?': 'Visitors' } }, text: 'ok' })
  await start($)
  const before = w.state.runs.length
  await ask($)
  const reads = w.state.runs.slice(before).filter(run => run.argv.includes('read'))
  expect(reads.length).toBeGreaterThan(0)
  for (const run of reads) expect(run.argv).not.toContain('--mark-read')
})
