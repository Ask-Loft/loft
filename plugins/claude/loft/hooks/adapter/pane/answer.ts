// How the Yard pane records what the person answers or decides in it: with `loft yard answer` and
// the channel `terminal`, as Claude Code's own dialog does, in the tree the pane shows. A block
// answered elsewhere first keeps that answer, and loft's message, which names where, stays in the
// pane until the next answer (the Feature "Follow Yard Log in a pane").

import type { EngineInterface } from 'claude-code'

import type { Given } from '../answers'
import { record } from '../asking'
import { failureOf, succeeded } from '../run'
import { callerOf } from '../state'
import { paneOf, setDrafts } from './atoms'
import { readPane } from './follow'

/** The answer to a question: the labels chosen, and the person's own words where they typed some. */
export function questionGiven(key: string, labels: readonly string[], words: string | undefined): Given | undefined {
  const typed = words?.trim() ?? ''
  if (labels.length === 0 && typed === '') return undefined
  return { block: key, ...(labels.length > 0 ? { labels: [...labels] } : {}), ...(typed !== '' ? { words: typed } : {}) }
}

/** The decision of an approval or a plan: `rejected` with the words typed as its reason, where any. */
export function decisionGiven(key: string, isApproved: boolean, reason: string | undefined): Given {
  if (isApproved) return { block: key, verdict: 'approved' }
  const typed = reason?.trim() ?? ''
  return { block: key, verdict: 'rejected', ...(typed !== '' ? { reason: typed } : {}) }
}

/** Records `given` for the Yard Log the pane shows, forgets what was set for it, and reads what changed. */
export async function answerIn($: EngineInterface, given: Given) {
  const view = (await paneOf($)).view
  if (view === null) return
  const [caller, cwd] = await Promise.all([callerOf($, undefined), $.session.cwd()])
  const ran = await record($, cwd, { ...caller, tree: view.tree }, [given])
  // What the person set stays for another try unless loft took the answer, or kept another's first.
  const isSettled = succeeded(ran) || ran.doc?.outcome === 'answered elsewhere'
  if (isSettled) await setDrafts($, before => {
    const ticked = { ...before.ticked }
    const words = { ...before.words }
    delete ticked[given.block]
    delete words[given.block]
    return { ...before, ticked, words }
  })
  await readPane($, succeeded(ran) ? null : failureOf(ran))
}
