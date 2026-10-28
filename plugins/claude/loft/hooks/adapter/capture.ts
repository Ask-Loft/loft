// Captures from one caller run in hook receipt order, independently of other callers.
// Reserve before any asynchronous preparation; only capture belongs here, never a decision wait.
import { state } from './state'

export function checkCapture(signal: AbortSignal) {
  if (signal.aborted) throw new Error('Yard capture was interrupted.')
}

/**
 * Runs `operation` in its caller's lane. An interruption cancels it until it passes `keep` the
 * native call's result; from then on the capture returns that result, and only the mirror after
 * it is abandoned.
 */
export function capture<T>(agentId: string | undefined, signal: AbortSignal, operation: (keep: (ran: T) => T) => Promise<T>): Promise<T> {
  const lanes = state.captures
  const key = JSON.stringify([state.sessions[0], agentId ?? null])
  let kept: { ran: T } | undefined
  const keep = (ran: T) => {
    kept = { ran }
    return ran
  }
  let interrupt!: () => void
  const interrupted = new Promise<T>((resolve, reject) => {
    interrupt = () => kept ? resolve(kept.ran) : reject(new Error('Yard capture was interrupted.'))
    if (signal.aborted) interrupt()
    else signal.addEventListener('abort', interrupt, { once: true })
  })
  const job = (lanes.get(key) ?? Promise.resolve()).then(() => {
    checkCapture(signal)
    return Promise.race([operation(keep), interrupted])
  })
  // A cancelled queued job still follows its predecessor, but a cancelled active native
  // call need not finish before the lane is released; its continuation checks before writing.
  const tail = job.then(() => {}, () => {})
  lanes.set(key, tail)
  void tail.then(() => { if (lanes.get(key) === tail) lanes.delete(key) })
  return Promise.race([job, interrupted]).finally(() => signal.removeEventListener('abort', interrupt))
}
