// The Loft plugin's one hooks module: Yard Client. The events Yard records around Claude Code's own
// handling are hooked here, and first, so that a tool call one of Yard's own hooks answers is still
// recorded; then Yard registers its own hooks (the Feature "The Loft plugin").

import type { Register } from 'claude-code'

import { eventsOfToolCall, eventsOfTurnEnd } from './yard/events'
import { ensureYard, registerYard } from './yard/index'

export const register: Register = (on, options) => {
  on('session.start', async ($, e, next) => {
    // Yard's tools are registered here, before the session's first turn.
    await ensureYard($, e.cwd)
    return next(e)
  })

  on('tool.call', ($, e, next) => eventsOfToolCall($, e, next, next))

  on('turn.complete', async ($, e, next) => {
    const ended = await next(e)
    eventsOfTurnEnd($, e)
    return ended
  })

  registerYard(on, options)
}
