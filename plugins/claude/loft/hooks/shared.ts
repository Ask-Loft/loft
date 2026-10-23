// The events the mod's entry hooks once and hands to Yard Client: Claude Code takes one hook for
// each event without a matcher from a module.

import type { Hook } from 'claude-code'

/** A `tool.call` event's input, as a hook receives it. */
export type ToolCallEvent = Parameters<Hook<'tool.call'>>[1]
/** What a `tool.call` hook's `next` gives, and what the hook answers. */
export type ToolCallAnswer = Awaited<ReturnType<Parameters<Hook<'tool.call'>>[2]>>
/** A `tool.call` hook's `next`. */
export type ToolCallNext = Parameters<Hook<'tool.call'>>[2]
/** A `turn.complete` event's input. */
export type TurnCompleteEvent = Parameters<Hook<'turn.complete'>>[1]

/** What runs the call on: Claude Code's own handling of it. */
export type Onward = (e: ToolCallEvent) => Promise<ToolCallAnswer>
