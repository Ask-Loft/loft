import { expect, test } from 'claude-code/testing'

import { askResultOf, feedbackIn, givenOf, planId, planTitle, questionBlocks, taskId, taskState } from '../hooks/yard/answers'
import { contextOf, LIMIT } from '../hooks/yard/context'
import { APPROVE, dialogOf, MORE, recordedOf, REJECT } from '../hooks/yard/dialog'
import { activeStage, lastUnplanned, waitingOf, whoOf, type Block } from '../hooks/yard/log'
import { loftSubcommands } from '../hooks/yard/shell'
import { argumentsOf, commandOf, specOf, TOOL_NAMES } from '../hooks/yard/tools'
import { block, readDoc, stage } from './yard-world'

const ASKED = [
  {
    question: 'Which readers does the change serve?',
    header: 'Readers',
    multiSelect: false,
    options: [
      { label: 'Members (Recommended)', description: 'Readers who hold a card' },
      { label: 'Visitors (Recommended)', description: '' },
    ],
  },
  {
    question: 'Which shelves?',
    header: 'Shelves',
    multiSelect: true,
    options: [
      { label: 'North (Recommended)', description: 'n' },
      { label: 'South (Recommended)', description: 's' },
      { label: 'East', description: 'e' },
    ],
  },
]

test('each question becomes a question block with its prompt, header, options and whether it takes more than one', () => {
  const blocks = questionBlocks(ASKED, ['ask-r1-1', 'ask-r1-2'], 'shape')
  expect(blocks[0]).toEqual({
    id: 'ask-r1-1',
    kind: 'question',
    stage: 'shape',
    prompt: 'Which readers does the change serve?',
    header: 'Readers',
    multiple: false,
    ownWords: true,
    options: [
      { label: 'Members', detail: 'Readers who hold a card', recommended: true },
      { label: 'Visitors', recommended: false },
    ],
  })
  expect(blocks[1]!.multiple).toBe(true)
})

test('a question that takes one option recommends only its first option marked recommended, and drops the words from every label', () => {
  const [one] = questionBlocks(ASKED, ['a', 'b'], 's')
  expect(one!.options.map(option => [option.label, option.recommended])).toEqual([
    ['Members', true],
    ['Visitors', false],
  ])
})

test('a question that takes more than one option recommends each option marked recommended', () => {
  const [, many] = questionBlocks(ASKED, ['a', 'b'], 's')
  expect(many!.options.map(option => option.recommended)).toEqual([true, true, false])
})

test('answers given on the page reach the agent by the labels it wrote, with words as written and the notes', () => {
  const answered = new Map<string, Block>([
    ['k1', { ...block('ask-r1-1', 'question', {}), key: 'k1', answer: { labels: ['Members'], notes: 'for now', channel: 'page' } }],
    ['k2', { ...block('ask-r1-2', 'question', {}), key: 'k2', answer: { labels: ['North', 'East'], words: 'and the attic', channel: 'page' } }],
  ])
  expect(askResultOf(ASKED, answered, ['ask-r1-1', 'ask-r1-2'])).toEqual({
    questions: ASKED,
    answers: {
      'Which readers does the change serve?': 'Members (Recommended)',
      'Which shelves?': 'North (Recommended), East, and the attic',
    },
    annotations: { 'Which readers does the change serve?': { notes: 'for now' } },
  })
})

test('answers given in the terminal are recorded by their labels without the recommendation, and anything else as the words typed', () => {
  const given = givenOf(ASKED, ['ask-r1-1', 'ask-r1-2'], {
    answers: { 'Which readers does the change serve?': 'Members (Recommended)', 'Which shelves?': 'Somewhere else' },
    annotations: { 'Which readers does the change serve?': { notes: ' later ' } },
  })
  expect(given).toEqual([
    { block: 'ask-r1-1', labels: ['Members'], notes: 'later' },
    { block: 'ask-r1-2', words: 'Somewhere else' },
  ])
})

test('a plan is titled by its first heading, or by its file where it has none, and its file names its block', () => {
  expect(planTitle('Intro\n\n## Build the shelf\nSteps', '/p/plans/shelf.md')).toBe('Build the shelf')
  expect(planTitle('No heading here', '/p/plans/groovy-plan.md')).toBe('groovy-plan')
  expect(planId('/p/plans/Groovy_Plan.md', 'plan-r3')).toBe('plan-groovy-plan')
  expect(planId(undefined, 'plan-r3')).toBe('plan-r3')
})

test('the feedback typed with a rejection is read from what Claude Code gives the agent', () => {
  expect(feedbackIn("The user doesn't want to proceed. the user said: split it in two")).toBe('split it in two')
  expect(feedbackIn('Rejected.')).toBeUndefined()
})

test("a task's block id is made of the task's id and its agent, and its states follow the task's", () => {
  const main = whoOf({ session: '5F1C9A7E-0b2d-4c11-9e3a-77aa01c2d3e4' })
  expect(taskId('3', main)).toBe('task-5f1c9a7e-3')
  const sub = { session: '5f1c9a7e-0b2d', agent: { id: 'a9f2', type: 'Explore' } }
  expect(taskId('3', whoOf(sub))).toBe('task-a9f2-3')
  expect(taskId('1', whoOf({ session: 'b7d00e11-cleared' }))).not.toBe(taskId('1', main))
  expect(['pending', 'in_progress', 'completed', 'deleted'].map(taskState)).toEqual(['open', 'active', 'done', 'dropped'])
})

const question = (id: string, options: number) =>
  block(id, 'question', {
    prompt: `Pick for ${id}?`,
    header: 'A header longer than twelve',
    multiple: false,
    options: Array.from({ length: options }, (_, n) => ({ label: `O${n + 1}`, detail: `d${n + 1}`, recommended: n === 0 })),
  })

test('a dialog asks at most four blocks, in the order the submission holds them, with prompt, header and option details', () => {
  const dialog = dialogOf([1, 2, 3, 4, 5].map(n => question(`q${n}`, 2)), {})!
  expect(dialog.questions.map(each => each.question)).toEqual(['Pick for q1?', 'Pick for q2?', 'Pick for q3?', 'Pick for q4?'])
  expect(dialog.questions[0]).toEqual({
    question: 'Pick for q1?',
    header: 'A header lon',
    multiSelect: false,
    options: [
      { label: 'O1 (Recommended)', description: 'd1' },
      { label: 'O2', description: 'd2' },
    ],
  })
})

test('a question with more than four options left shows the next three and More options, and four or fewer all of them', () => {
  const long = question('q1', 6)
  const first = dialogOf([long], {})!
  expect(first.questions[0]!.options.map(option => option.label)).toEqual(['O1 (Recommended)', 'O2', 'O3', MORE])
  const { given, paging } = recordedOf(first, { answers: { 'Pick for q1?': MORE } }, {})
  expect(given).toEqual([])
  const second = dialogOf([long], paging)!
  expect(second.questions[0]!.options.map(option => option.label)).toEqual(['O4', 'O5', 'O6'])
})

test('a question that takes more than one option, asked over two dialogs, records the options chosen in each', () => {
  const many = { ...question('q1', 6), version: { body: { ...question('q1', 6).version.body, multiple: true } } }
  const first = dialogOf([many], {})!
  const after = recordedOf(first, { answers: { 'Pick for q1?': `O2, ${MORE}` } }, {})
  const second = dialogOf([many], after.paging)!
  expect(recordedOf(second, { answers: { 'Pick for q1?': 'O5' } }, after.paging).given).toEqual([{ block: 'q1', labels: ['O2', 'O5'] }])
})

test('an approval or a plan is asked by its title, with Approve and Reject, and the notes or words of a rejection are its reason', () => {
  const plan = block('plan-shelf', 'plan', { title: 'Build the shelf', markdown: '# Build' })
  const approval = block('ship', 'approval', { title: 'Ship it?', markdown: '' })
  const other = block('other', 'approval', { title: 'Other?', markdown: '' })
  const dialog = dialogOf([plan, approval, other], {})!
  expect(dialog.questions.map(each => [each.question, each.header, each.options.map(option => option.label)])).toEqual([
    ['Build the shelf', 'Plan', [APPROVE, REJECT]],
    ['Ship it?', 'Approval', [APPROVE, REJECT]],
    ['Other?', 'Approval', [APPROVE, REJECT]],
  ])
  const answered = recordedOf(
    dialog,
    { answers: { 'Build the shelf': APPROVE, 'Ship it?': REJECT, 'Other?': 'not this week' }, annotations: { 'Ship it?': { notes: 'too soon' } } },
    {},
  )
  expect(answered.given).toEqual([
    { block: 'plan-shelf', verdict: 'approved' },
    { block: 'ship', verdict: 'rejected', reason: 'too soon' },
    { block: 'other', verdict: 'rejected', reason: 'not this week' },
  ])
})

test("a dialog for a subagent's submission names the subagent", () => {
  const dialog = dialogOf([question('q1', 2)], {}, 'subagent Explore a9')!
  expect(dialog.questions[0]!.question).toBe('Pick for q1? (subagent Explore a9)')
})

test("the active stage is the agent's own: a subagent's stage is not the main agent's", () => {
  const doc = readDoc({
    stages: [
      stage('shape', 'active', { session: 's1' }),
      stage('dig', 'active', { session: 's1', agent: 'a9', agentType: 'Explore' }),
      stage('later', 'planned', null),
    ],
  })
  expect(activeStage(doc, { sessions: ['s1'] })?.id).toBe('shape')
  expect(activeStage(doc, { sessions: ['s1'], agent: 'a9' })?.id).toBe('dig')
  expect(activeStage(doc, { sessions: ['s2'] })).toBeUndefined()
  expect(activeStage(doc, { sessions: ['s2', 's1'] })?.id).toBe('shape')
  expect(lastUnplanned(doc)?.id).toBe('dig')
})

test('the blocks of a submission that still wait are those not yet answered or decided', () => {
  const asked = { ...question('q1', 2), key: 'k1' }
  const answered = { ...question('q2', 2), key: 'k2', answer: { labels: ['O1'] } }
  const doc = readDoc({
    stages: [{ ...stage('shape', 'active', { session: 's1' }), blocks: [asked, answered] }],
    submissions: [{ agent: { session: 's1' }, open: [{ key: 'sub', blocks: ['k1', 'k2'], state: 'open' }] }],
  })
  expect(waitingOf(doc, 'sub').map(each => each.id)).toEqual(['q1'])
})

test('a shell command names the loft subcommands it runs by name, by path or through a variable it sets', () => {
  expect(loftSubcommands('loft check --fail-on warning && loft fmt --check')).toEqual(['check', 'fmt'])
  expect(loftSubcommands('./target/debug/loft reconcile --format json')).toEqual(['reconcile'])
  expect(loftSubcommands('L=/usr/local/bin/loft; "$L" status --base develop')).toEqual(['status'])
  expect(loftSubcommands('LOFT_TELEMETRY=off loft yard read')).toEqual(['yard'])
})

test('a subcommand is never taken from quoted text, a heredoc, or a word that is not one of loft', () => {
  expect(loftSubcommands('echo "loft check"')).toEqual([])
  expect(loftSubcommands("git commit -m 'loft publish'")).toEqual([])
  expect(loftSubcommands("cat <<'EOF'\nloft publish\nEOF\nloft verify")).toEqual(['verify'])
  expect(loftSubcommands('loft frobnicate')).toEqual([])
  expect(loftSubcommands('cargo run -q -p loft-cli -- check')).toEqual([])
})

test('the context gives the page, the active stage, the open submissions and what was answered since the last read', () => {
  const text = contextOf(
    readDoc({
      revision: 9,
      lastRead: 5,
      stages: [
        {
          ...stage('shape', 'active', { session: 's1' }),
          blocks: [
            { ...question('q1', 2), answer: { labels: ['O1'], act: { revision: 7 } } },
            { ...question('q0', 2), answer: { labels: ['O1'], act: { revision: 4 } } },
          ],
        },
      ],
      submissions: [{ agent: { session: 's1' }, open: [{ key: 'sub-7', blocks: ['k1'], state: 'open' }] }],
    }),
    { sessions: ['s1'] },
  )
  expect(text).toContain('http://127.0.0.1:7411/#tree=')
  expect(text).toContain('`shape`')
  expect(text).toContain('`sub-7`, 1 block, sent by you')
  expect(text).toContain('`q1` answered')
  expect(text).not.toContain('`q0`')
})

test('the context holds at most 2,000 characters and counts what it leaves out', () => {
  const blocks = Array.from({ length: 120 }, (_, n) => ({ ...question(`question-number-${n}`, 2), answer: { act: { revision: 8 } } }))
  const text = contextOf(readDoc({ revision: 9, lastRead: 1, stages: [{ ...stage('shape', 'active', { session: 's1' }), blocks }] }), { sessions: ['s1'] })
  expect(text.length).toBeLessThanOrEqual(LIMIT)
  expect(text).toMatch(/… and \d+ more/)
})

test('a branch with no Yard Log is given only the page and that it has none yet', () => {
  const text = contextOf(readDoc({ yardLog: { branch: 'feat/shelf', closedBy: 'open' }, ignored: true }), { sessions: ['s1'] })
  expect(text).toContain('has no Yard Log yet')
  expect(text).toContain('http://127.0.0.1:7411/')
  expect(text).not.toContain('git ignores')
})

test('a closed Yard Log is said to be closed, and an ignored .yard/ to stay out of the commits', () => {
  expect(contextOf(readDoc({ yardLog: { key: 'y', closedBy: 'agent' } }), { sessions: ['s1'] })).toContain('Yard Log is closed')
  expect(contextOf(readDoc({ ignored: true }), { sessions: ['s1'] })).toContain('stays out of the commits')
})

test("each of Yard's six tools has a JSON schema of its input", () => {
  expect(TOOL_NAMES).toEqual(['yard_read', 'yard_write', 'yard_wait', 'yard_withdraw', 'yard_close', 'yard_published'])
  const schema = (name: (typeof TOOL_NAMES)[number]) => specOf(name).inputSchema as { properties: object; required: string[] }
  expect(Object.keys(schema('yard_read').properties)).toEqual(['since', 'reports'])
  expect(Object.keys(schema('yard_write').properties)).toEqual(['batch', 'basedOn', 'send'])
  expect(schema('yard_wait').required).toEqual(['submission'])
  expect(schema('yard_withdraw').required).toEqual(['submission'])
  expect(Object.keys(schema('yard_close').properties)).toEqual([])
  expect(Object.keys(schema('yard_published').properties)).toEqual(['branch', 'note'])
})

test('an input that does not match its schema names the field at fault', () => {
  expect(argumentsOf('yard_read', { since: -1 })).toEqual({ fault: '`yard_read`: the field `since` must be a whole number of 0 or more.' })
  expect(argumentsOf('yard_write', { batch: 'x' })).toEqual({ fault: '`yard_write`: the field `batch` must be a list of objects.' })
  expect(argumentsOf('yard_wait', {})).toEqual({ fault: '`yard_wait`: the field `submission` is required.' })
  expect(argumentsOf('yard_close', { force: true })).toEqual({ fault: '`yard_close` takes no field `force`.' })
  expect(argumentsOf('yard_read', { tool: 'mcp__loft__yard_read', tool_use_id: 't', since: 2 })).toEqual({ args: { since: 2 } })
})

test("each tool runs its loft yard command with the caller's session and, for a subagent, its id and type", () => {
  const main = { session: 's1' }
  const sub = { session: 's1', agent: { id: 'a9', type: 'Explore' } }
  expect(commandOf('yard_read', { since: 3 }, main).args).toEqual(['read', '--since', '3', '--mark-read', '--session', 's1'])
  expect(commandOf('yard_write', { batch: [{ id: 'x' }], send: true, basedOn: 4 }, sub)).toEqual({
    args: ['write', '--input', '-', '--send', '--based-on', '4', '--session', 's1', '--agent', 'a9', '--agent-type', 'Explore'],
    stdin: '[{"id":"x"}]',
  })
  expect(commandOf('yard_wait', { submission: 'k' }, main).args).toEqual(['wait', '--submission', 'k', '--timeout', '540', '--session', 's1'])
  expect(commandOf('yard_published', { branch: 'feat/x', note: 'n' }, main).args).toEqual(['published', '--branch', 'feat/x', '--note', 'n', '--session', 's1'])
})

test('after /clear the context counts the submissions the session before it sent as its own, and what was answered since it last read', () => {
  const text = contextOf(
    readDoc({
      revision: 9,
      lastRead: 6,
      stages: [{ ...stage('shape', 'active', { session: 's1' }), blocks: [{ ...question('q1', 2), answer: { labels: ['O1'], act: { revision: 8 } } }] }],
      submissions: [{ agent: { session: 's1' }, open: [{ key: 'sub-7', blocks: ['k1'], state: 'open' }] }],
    }),
    { sessions: ['s2', 's1'] },
  )
  expect(text).toContain('Your active stage: `shape`')
  expect(text).toContain('`sub-7`, 1 block, sent by you')
  expect(text).toContain('since revision 6, which you last read (1)')
})
