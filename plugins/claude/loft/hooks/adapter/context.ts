// The context an agent starts with: its briefing, as `loft yard read --context --mark-read` gives
// it, followed only by what is particular to Claude Code (the Features "Start with the session"
// and "Read Yard Log").

import type { EngineInterface } from 'claude-code'

import { isObject } from '../model'
import { callerArgs, succeeded } from './run'
import { callerOf, run } from './state'

/** What the context adds to the briefing for Claude Code: the tool the agent reads with. */
export const FOOTER = 'Read Yard Log with `yard_read` before writing.'

/**
 * The calling agent's context in `cwd`, from the briefing its read marks as read; undefined
 * where Yard Log cannot be read.
 */
export async function contextFor($: EngineInterface, cwd: string, agentId: string | undefined): Promise<string | undefined> {
  const caller = await callerOf($, agentId)
  const ran = await run($, cwd, ['read', '--context', '--mark-read', ...callerArgs(caller)])
  const context = succeeded(ran) && isObject(ran.doc.context) ? ran.doc.context : undefined
  return typeof context?.text === 'string' ? `${context.text}\n${FOOTER}` : undefined
}
