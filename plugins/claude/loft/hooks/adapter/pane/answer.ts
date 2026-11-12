// How the Yard pane records what the person answers or decides in it: with `loft yard answer` and
// the channel `terminal`, as Claude Code's own dialog does, in the tree the pane shows. A block
// answered elsewhere first keeps that answer, and loft's message, which names where, stays in the
// pane until the next answer (the Feature "Follow Yard Log in a pane").

import type { EngineInterface } from 'claude-code'

import type { PaneBlock } from '../../../types'
import type { Given } from '../answers'
import { callerArgs, failureOf, succeeded } from '../run'
import { continued, run } from '../state'
import { paneOf, setDrafts, setPane } from './atoms'
import { readPane } from './follow'

/** The answer to a question: the labels chosen, and the person's own words where they typed some. */
export function questionGiven(block: PaneBlock, labels: readonly string[], words: string | undefined): Given | undefined {
  const typed = words?.trim() ?? ''
  if (labels.length === 0 && typed === '') return undefined
  return { block: block.key, ...(labels.length > 0 ? { labels: [...labels] } : {}), ...(typed !== '' ? { words: typed } : {}) }
}

/** The decision of an approval or a plan: `rejected` with the words typed as its reason, where any. */
export function decisionGiven(block: PaneBlock, isApproved: boolean, reason: string | undefined): Given {
  const typed = reason?.trim() ?? ''
  if (isApproved) return { block: block.key, verdict: 'approved' }
  return { block: block.key, verdict: 'rejected', ...(typed !== '' ? { reason: typed } : {}) }
}

/** Records `given` for the Yard Log the pane shows, then reads what changed. */
export async function record($: EngineInterface, given: Given) {
  const view = (await paneOf($)).view
  if (view === null) return
  const session = await continued($)
  const ran = await run($, await $.session.cwd(), ['answer', '--input', '-', ...callerArgs({ session, tree: view.tree })], JSON.stringify([given]))
  const notice = succeeded(ran) ? null : failureOf(ran)
  await setPane($, before => ({ ...before, notice }))
  await setDrafts($, before => {
    const ticked = { ...before.ticked }
    const words = { ...before.words }
    delete ticked[given.block]
    delete words[given.block]
    return { ...before, ticked, words }
  })
  await readPane($)
}
