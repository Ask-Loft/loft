// The Loft status pane: a pane, a status-line entry and `/loft-status`, kept up to date while the
// agent works on a specification. Every value comes from `loft … --format json`; the hooks only
// ask for a read and note which file the agent wrote (the Feature "The Loft status pane").

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import type { LoftError, LoftRead, LoftSnapshot } from '../types'
import {
  ancestors,
  brokenLinks,
  checks,
  commandWords,
  criteriaToVerify,
  documentErrors,
  isLoftSpecSkill,
  labelOf,
  nextStep,
  nodesIn,
  parseDocument,
  readOf,
  relativeTo,
  runsLoftOrGit,
  statusLine,
  stopped,
  touchesSpecification,
  window,
} from './model'
import { kit } from './ui'

const PANE = 'loft-status'
const TITLE = 'Loft'
const TICK_MS = 400
const POLL_MS = 10000
/** A read is the pane's, not the person's: it sends no command event and asks for no update. */
const QUIET = { LOFT_TELEMETRY: 'off', LOFT_UPDATE_CHECK: 'off' }

const snapshot = atom({ plugin: 'loft', key: 'snapshot' } as const, null)
const baseOverride = atom({ plugin: 'loft', key: 'base' } as const, null)
const workingOn = atom({ plugin: 'loft', key: 'workingOn' } as const, null)

type Ran = { document?: Record<string, unknown>; exitCode: number; unrun?: string }

/** The words that run loft, and the base the pane follows by default: the plugin's settings. */
let words = ['loft']
let setting = 'develop'
/** The directory that holds `spec/`: undefined until looked for, null where there is none. */
let root: string | null | undefined
let isDirty = true
let isReading = false

function requestRead() {
  isDirty = true
}

async function findRoot($: EngineInterface, cwd: string): Promise<string | null> {
  for (const dir of ancestors(cwd)) {
    const manifest = `${dir === '/' ? '' : dir}/spec/loftspec.yaml`
    if (await $.fs.exists(manifest)) return dir
  }
  return null
}

async function run($: EngineInterface, cwd: string, args: string[]): Promise<Ran> {
  try {
    const ran = await $.process.run([...words, ...args, '--format', 'json'], { cwd, env: QUIET, timeoutMs: 20000 })
    return { document: parseDocument(ran.stdout), exitCode: ran.exitCode }
  } catch (error) {
    return { exitCode: -1, unrun: error instanceof Error ? error.message : String(error) }
  }
}

async function readOnce($: EngineInterface) {
  if (!root) {
    await update($, snapshot, () => ({ root: null, read: null, notice: null, errors: [] }))
    $.ui.status(undefined)
    return
  }
  const dir = root
  const base = (await read($, baseOverride)) ?? setting
  const [status, fmt, check, reconcile] = await Promise.all([
    run($, dir, ['status', '--base', base]),
    run($, dir, ['fmt', '--check']),
    run($, dir, ['check']),
    run($, dir, ['reconcile']),
  ])
  const previous = (await read($, snapshot))?.read ?? null
  const command = `\`${words.join(' ')}\``
  const notice =
    status.unrun !== undefined
      ? `${command} cannot be run (${status.unrun}); set the loft plugin's loftCommand setting.`
      : status.exitCode === 2 && !status.document
        ? `${command} has no \`status\` command: it is too old for the Loft status pane; update loft or set loftCommand.`
        : null
  if (notice) {
    await update($, snapshot, () => ({ root: dir, read: previous, notice, errors: [] }))
    $.ui.status(undefined)
    return
  }
  const errors: LoftError[] = []
  const documents = [
    ['status', status],
    ['fmt --check', fmt],
    ['check', check],
    ['reconcile', reconcile],
  ] as const
  for (const [name, ran] of documents) {
    if (!ran.document) {
      errors.push({ command: name, message: ran.unrun ?? `printed no JSON document (exit ${ran.exitCode})` })
    } else if (stopped(ran.document)) {
      errors.push(...documentErrors(name, ran.document))
    }
  }
  const next: LoftRead | null =
    errors.length === 0 && status.document && fmt.document && check.document && reconcile.document
      ? readOf(status.document, fmt.document, check.document, reconcile.document)
      : previous
  await update($, snapshot, () => ({ root: dir, read: next, notice: null, errors }))
  $.ui.status(next ? statusLine(next) : undefined)
}

/** Runs the reads asked for since the last; never two at once. */
async function tick($: EngineInterface) {
  if (!isDirty || isReading) return
  isReading = true
  try {
    while (isDirty) {
      isDirty = false
      await readOnce($)
    }
  } finally {
    isReading = false
  }
}

export const register: Register = (on, options) => {
  words = commandWords(typeof options.loftCommand === 'string' ? options.loftCommand : 'loft')
  setting =
    typeof options.baseBranch === 'string' && options.baseBranch.trim() !== '' ? options.baseBranch.trim() : 'develop'
  root = undefined
  isDirty = true
  isReading = false

  on('session.start', async ($, e, next) => {
    await $.command.register({
      name: 'loft-status',
      description: 'Show the Loft status pane. Args: <revision> to compare with it, auto for the baseBranch setting',
    })
    root = await findRoot($, e.cwd)
    if (root) {
      void $.ui.open({ id: PANE, title: TITLE })
      $.clock.every(TICK_MS, () => void tick($))
      $.clock.every(POLL_MS, requestRead)
    }
    requestRead()
    await tick($)
    return next(e)
  })

  on('command.run', { command: 'loft-status' }, async ($, e) => {
    const arg = e.args.trim()
    if (arg === 'auto') await update($, baseOverride, () => null)
    else if (arg !== '') await update($, baseOverride, () => arg)
    await $.ui.open({ id: PANE, title: TITLE })
    requestRead()
    await tick($)
    if (!root) return { text: 'The Loft status pane found no Loft specification at or above this directory.' }
    const base = (await read($, baseOverride)) ?? setting
    return { text: `The Loft status pane compares the tree with \`${base}\`.` }
  })

  // The person running the `loft-spec` skill as a slash command.
  on('command.run', async ($, e, next) => {
    if (isLoftSpecSkill(e.command)) requestRead()
    return next(e)
  })

  on('tool.call', async ($, e, next) => {
    if (!root) return next(e)
    if (e.tool === 'Skill' && isLoftSpecSkill(e.skill)) {
      requestRead()
      return next(e)
    }
    if ((e.tool === 'Write' || e.tool === 'Edit') && touchesSpecification(root, e.file_path)) {
      const ran = await next(e)
      const relative = relativeTo(root, e.file_path)
      if (relative?.startsWith('spec/')) await update($, workingOn, () => relative)
      requestRead()
      return ran
    }
    if (e.tool === 'Bash' && runsLoftOrGit(e.command)) {
      const ran = await next(e)
      requestRead()
      return ran
    }
    return next(e)
  })

  on('turn.complete', async ($, e, next) => {
    requestRead()
    return next(e)
  })

  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const elements = $.ui.resolve(e)
    const width = Math.max(20, e.props.bodyColumns)
    const ui = kit(elements, width)
    const shown = await read($, snapshot)
    const file = await read($, workingOn)
    const base = (await read($, baseOverride)) ?? setting
    return (
      <elements.Box flexDirection="column">{drawPane(ui, shown, file, base, e.props.scroll.bodyRows)}</elements.Box>
    )
  })
}

type Kit = ReturnType<typeof kit>

const CHANGE_GLYPH = { added: '+', changed: '~', removed: '−' } as const

/** The pane's body for what the last read showed. */
function drawPane(ui: Kit, shown: LoftSnapshot | null, file: string | null, base: string, rows: number) {
  const { Box, tone, rule } = ui
  if (shown === null) return tone('meta', 'Reading the specification…')
  if (shown.root === null) {
    return (
      <Box flexDirection="column">
        {tone('title', TITLE)}
        {tone('meta', 'No Loft specification here: no spec/loftspec.yaml at or above this directory.', 'wrap')}
      </Box>
    )
  }
  const errors = shown.errors.map(error => {
    const at = error.path ? ` (${error.path}${error.line ? `:${error.line}` : ''})` : ''
    return tone('error', `✗ ${error.command}: ${error.message}${at}`, 'wrap')
  })
  if (shown.notice !== null || shown.read === null) {
    return (
      <Box flexDirection="column">
        {tone('title', TITLE)}
        {shown.notice !== null && tone('todo', shown.notice, 'wrap')}
        {errors}
      </Box>
    )
  }
  const loft = shown.read
  const all = checks(loft)
  const toVerify = criteriaToVerify(loft)
  const reviewed = toVerify.filter(each => !each.open).length
  const delta = loft.status.base
  const changes = delta
    ? [
        ...delta.added.map(node => ({ change: 'added' as const, node })),
        ...delta.changed.map(node => ({ change: 'changed' as const, node })),
        ...delta.removed.map(node => ({ change: 'removed' as const, node })),
      ]
    : []
  const inFile = file ? nodesIn(loft, file) : []
  const fixed = 16 + all.length + errors.length + inFile.length
  const room = Math.max(3, Math.floor((Math.max(rows, 24) - fixed) / 2))
  const listed = window(changes, room)
  const verifying = window(toVerify, room)
  const current = toVerify.findIndex(each => each.open)
  const broken = brokenLinks(loft).length

  return (
    <Box flexDirection="column">
      {tone('title', `${TITLE} · ${loft.status.branch ?? 'detached HEAD'}`)}
      {tone('meta', delta ? `Base ${base} at ${delta.commit.slice(0, 7)}` : `Base ${base}`)}
      {rule()}
      {all.map(check =>
        tone(
          check.passes ? 'done' : 'todo',
          `${check.passes ? '✓' : '○'} ${check.label.padEnd(15)}${check.passes ? 'done' : 'to do'} · ${check.detail}`,
        ),
      )}
      {rule()}
      {tone('body', `Next: ${nextStep(all)}`, 'wrap')}
      {rule()}
      {tone('heading', delta ? `This branch  +${delta.added.length} ~${delta.changed.length} −${delta.removed.length}` : 'This branch')}
      {changes.length === 0 && tone('meta', '  no node added, changed or removed')}
      {listed.shown.map(({ change, node }) =>
        tone(change === 'removed' ? 'muted' : 'body', `  ${CHANGE_GLYPH[change]} ${change.padEnd(8)}${labelOf(node).padEnd(9)}${node.name}`),
      )}
      {listed.more > 0 && tone('meta', `  … ${listed.more} more`)}
      {rule()}
      {tone('heading', `Criteria to verify  ${reviewed}/${toVerify.length} reviewed`)}
      {toVerify.length === 0 && tone('meta', '  no criterion added or changed')}
      {verifying.shown.map((each, index) => {
        const isCurrent = index === current
        const mark = !each.open ? '✓ reviewed' : isCurrent ? '▶ current ' : '○ open    '
        const why = each.open ? ` — ${each.open.message}` : ''
        return tone(
          !each.open ? 'done' : isCurrent ? 'current' : 'body',
          `  ${mark} ${labelOf(each.node).padEnd(9)}${each.node.name}${why}`,
        )
      })}
      {verifying.more > 0 && tone('meta', `  … ${verifying.more} more`)}
      {broken > 0 && tone('error', `  ✗ ${broken} trace link${broken === 1 ? '' : 's'} no longer true`)}
      {rule()}
      {tone('heading', 'Working on')}
      {file === null ? tone('meta', '  no file under spec/ written yet') : tone('current', `  ▸ ${file}`)}
      {inFile.map(({ change, node }) => tone('body', `    ${CHANGE_GLYPH[change]} ${labelOf(node).padEnd(9)}${node.name}`))}
      {errors.length > 0 && rule()}
      {errors}
    </Box>
  )
}
