// The events both parts of the mod act on. Claude Code takes one hook for each event without a
// matcher from a module, so the module's entry hooks each of these once and calls each part.

import type { Hook } from 'claude-code'

/** A `tool.call` event's input, as a hook receives it. */
export type ToolCallEvent = Parameters<Hook<'tool.call'>>[1]
/** What a `tool.call` hook's `next` gives, and what the hook answers. */
export type ToolCallAnswer = Awaited<ReturnType<Parameters<Hook<'tool.call'>>[2]>>
/** A `tool.call` hook's `next`. */
export type ToolCallNext = Parameters<Hook<'tool.call'>>[2]
/** A `turn.complete` event's input. */
export type TurnCompleteEvent = Parameters<Hook<'turn.complete'>>[1]

/** What runs the call on: the other part's hook, then Claude Code's own. */
export type Onward = (e: ToolCallEvent) => Promise<ToolCallAnswer>
