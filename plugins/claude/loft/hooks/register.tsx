// The Loft plugin's one hooks module: the Loft status pane and Yard Client's hooks, side by side.
// The events both act on are hooked here once, Yard's share around the pane's, and first, so that
// a tool call one of Yard's own hooks answers is still recorded; then each part registers its own
// hooks (the Feature "The Loft plugin").

import type { Register } from 'claude-code'

import { paneSessionStart, paneToolCall, paneTurnComplete, registerPane } from './pane'
import { eventsOfToolCall, eventsOfTurnEnd } from './yard/events'
import { ensureYard, registerYard } from './yard/index'

export const register: Register = (on, options) => {
  on('session.start', async ($, e, next) => {
    // Yard's tools are registered here, before the session's first turn.
    await ensureYard($, e.cwd)
    await paneSessionStart($, e.cwd)
    return next(e)
  })

  on('tool.call', ($, e, next) => eventsOfToolCall($, e, call => paneToolCall($, call, next), next))

  on('turn.complete', async ($, e, next) => {
    paneTurnComplete()
    const ended = await next(e)
    await eventsOfTurnEnd($, e)
    return ended
  })

  registerPane(on, options)
  registerYard(on, options)
}
