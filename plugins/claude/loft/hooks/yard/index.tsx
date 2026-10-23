// Yard Client's hooks: once `loft yard protocol` shows that the `loft` the plugin runs speaks a
// protocol this mod speaks, they register Yard's tools, keep a page server running, give each
// session its context, send the agent's questions, plans and tasks to Yard Log and the session's
// events to the page server. Otherwise they leave Claude Code as it is and tell the person once
// (the Features "Yard's tools", "Keep the mod and loft in step" and "Start with the session").

import type { EngineInterface, On, PluginOptions } from 'claude-code'

import { QUIET, type Ran } from '../loft'
import { askQuestions, decideOnPage, ownDialog, presentPlan, waitOn } from './asking'
import { contextOf } from './context'
import { registerEvents } from './events'
import { flush } from './outbox'
import { callerArgs, failureOf, succeeded, yard } from './run'
import { beat, BEAT } from './server'
import { askerOf, callerOf, continued, pageNoted, PLUGIN, reset, run, state } from './state'
import { mirrorCreated, mirrorUpdated } from './tasks'
import { argumentsOf, commandOf, specOf, TOOL_NAMES, type ToolName } from './tools'

/** The plugin's version: the same as the `loft` it is released with (`tests/plugins.rs`). */
export const PLUGIN_VERSION = '2.2.1'
/** The oldest and the newest yard protocol this mod speaks. */
export const PROTOCOLS = { oldest: 1, newest: 1 } as const

/** Turns Yard Client on for the session, once, where the protocol check passes. */
export function ensureYard($: EngineInterface, cwd: string): Promise<boolean> {
  state.enabling ??= enable($, cwd)
  return state.enabling
}

async function enable($: EngineInterface, cwd: string): Promise<boolean> {
  const checked = await yard($, state.settings, cwd, ['protocol'], { beyond: false })
  const protocol = checked.doc?.protocol
  if (!succeeded(checked) || typeof protocol !== 'number') return off($, checked, cwd)
  if (protocol < PROTOCOLS.oldest || protocol > PROTOCOLS.newest) return off($, checked, cwd, protocol)
  await Promise.all(TOOL_NAMES.map(name => $.tool.register(specOf(name))))
  const home = (await $.env.get('HOME')) ?? (await $.env.get('USERPROFILE'))
  if (home) state.box.tokenPath = `${home}/.yard/token`
  state.isOn = true
  state.server.trees.add(cwd)
  void beat($, state.settings, state.server, state.box, await continued($), cwd)
  $.clock.every(BEAT, async () => {
    void beat($, state.settings, state.server, state.box, await continued($), await $.session.cwd())
    void flush($, state.box)
  })
  return true
}

/** Tells the person once why Yard Client is off; Claude Code's own dialogs then ask alone. */
async function off($: EngineInterface, checked: Ran, cwd: string, protocol?: number): Promise<false> {
  const command = `\`${state.settings.words.join(' ')}\``
  const speaks = `the yard protocol ${PROTOCOLS.oldest}${PROTOCOLS.newest === PROTOCOLS.oldest ? '' : `–${PROTOCOLS.newest}`}`
  let text: string
  if (checked.unrun !== undefined) {
    text = `Yard Client is off: ${command} cannot be run (${checked.unrun}). Set the loft plugin's loftCommand setting to a loft that can.`
  } else {
    const version = typeof checked.doc?.loft === 'string' ? checked.doc.loft : await versionOf($, cwd)
    const heard = protocol === undefined ? 'which has no yard commands' : `which speaks the yard protocol ${protocol}`
    const update = protocol !== undefined && protocol > PROTOCOLS.newest ? 'bring the loft plugin up to date' : 'bring loft up to date'
    text = `Yard Client is off: the loft plugin ${PLUGIN_VERSION} speaks ${speaks}, and ${command} runs loft ${version}, ${heard}. To use Yard Client, ${update}, or set the loft plugin's loftCommand setting.`
  }
  $.ui.toast(text, { timeoutMs: 15000 })
  return false
}

/** The version `loftCommand` reports, for the notice. */
async function versionOf($: EngineInterface, cwd: string): Promise<string> {
  try {
    const ran = await $.process.run([...state.settings.words, '--version'], { cwd, env: QUIET })
    return /\d+\.\d+\.\d+\S*/.exec(ran.stdout)?.[0] ?? 'of an unknown version'
  } catch {
    return 'of an unknown version'
  }
}

/** Serves one of Yard's tools with the document of its `loft yard` command. */
async function callTool($: EngineInterface, name: ToolName, e: Record<string, unknown>, signal: AbortSignal) {
  if (!state.isOn) return { deny: 'Yard Client is off in this session.' }
  const checked = argumentsOf(name, e)
  if ('fault' in checked) return { deny: checked.fault }
  const cwd = await $.session.cwd()
  const caller = await callerOf($, typeof e.agentId === 'string' ? e.agentId : undefined)
  let ran: Ran
  if (name === 'yard_wait') {
    ran = await waitOn($, cwd, caller, String(checked.args.submission), signal)
  } else {
    const command = commandOf(name, checked.args, caller)
    ran = await run($, cwd, command.args, command.stdin)
  }
  if (!succeeded(ran)) return { deny: failureOf(ran) }
  return { result: JSON.stringify(ran.doc) }
}

/** The session's context for its branch's Yard Log, as one `loft yard read` gives it: the agent's own read. */
async function contextFor($: EngineInterface, cwd: string): Promise<string | undefined> {
  const caller = await callerOf($, undefined)
  const ran = await run($, cwd, ['read', '--mark-read', ...callerArgs(caller)])
  return succeeded(ran) ? contextOf(ran.doc, askerOf(caller)) : undefined
}

/** Registers Yard Client's own hooks; the entry hooks the events it shares with the pane. */
export function registerYard(on: On, options: PluginOptions) {
  reset(options)

  on('session.end', async ($, e, next) => {
    if (e.reason === 'clear' || e.reason === 'resume') state.continues = e.sessionId
    return next(e)
  })

  // The context, at the session's start and again after `/resume`, `/branch` and `/clear`.
  on('classic.SessionStart', async ($, e, next) => {
    const started = await next(e)
    if (!['startup', 'resume', 'clear', 'fork'].includes(e.source)) return started
    if (!(await ensureYard($, e.cwd))) return started
    const text = await contextFor($, e.cwd)
    return text === undefined ? started : { ...started, additionalContext: [...(started.additionalContext ?? []), text] }
  })

  // Each tool is served by a hook whose matcher names it literally.
  on('tool.call', { tool: 'mcp__loft__yard_read' }, ($, e, next) => callTool($, 'yard_read', e, next.signal))
  on('tool.call', { tool: 'mcp__loft__yard_write' }, ($, e, next) => callTool($, 'yard_write', e, next.signal))
  on('tool.call', { tool: 'mcp__loft__yard_wait' }, ($, e, next) => callTool($, 'yard_wait', e, next.signal))
  on('tool.call', { tool: 'mcp__loft__yard_withdraw' }, ($, e, next) => callTool($, 'yard_withdraw', e, next.signal))
  on('tool.call', { tool: 'mcp__loft__yard_close' }, ($, e, next) => callTool($, 'yard_close', e, next.signal))
  on('tool.call', { tool: 'mcp__loft__yard_published' }, ($, e, next) => callTool($, 'yard_published', e, next.signal))

  on('tool.call', { tool: 'AskUserQuestion' }, async ($, e, next) => {
    if (next.origin.plugin === PLUGIN) return ownDialog(e, next)
    if (!state.isOn) return next(e)
    return askQuestions($, e, next)
  })

  on('tool.call', { tool: 'ExitPlanMode' }, async ($, e, next) => {
    if (!state.isOn) return next(e)
    return presentPlan($, e as typeof e & { plan?: unknown; planFilePath?: unknown }, next)
  })

  on('classic.PermissionRequest', async ($, e, next) => {
    const presented = state.presenting
    if (e.tool_name !== 'ExitPlanMode' || !presented) return next(e)
    return decideOnPage($, presented, () => next(e), e.tool_input, next.signal)
  })

  // The agent's task call returns at once; its mirror follows.
  on('tool.call', { tool: 'TaskCreate' }, async ($, e, next) => {
    const ran = await next(e)
    if (state.isOn) mirrorCreated($, e, ran)
    return ran
  })

  on('tool.call', { tool: 'TaskUpdate' }, async ($, e, next) => {
    const ran = await next(e)
    if (state.isOn) mirrorUpdated($, e, ran)
    return ran
  })

  on('ui.render', { component: 'AskUserQuestion' }, async ($, e, next) => {
    const drawn = await next(e)
    const url = state.isOn ? await pageNoted($) : null
    if (!url) return drawn
    const { Box, Text } = $.ui.resolve(e)
    return (
      <Box flexDirection="column">
        <Text dimColor>Also on the Yard page: {url}</Text>
        {drawn}
      </Box>
    )
  })

  registerEvents(on)
}
