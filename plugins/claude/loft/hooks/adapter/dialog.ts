// How a submission the agent sent and waits on is asked in Claude Code's own dialog: at most four
// of its blocks to a dialog, the options of a long question over several dialogs, and what an
// answered dialog records (the Feature "Answer a sent submission in either place"). Pure.

import type { Given } from './answers'
import type { Block } from './log'

/** The most questions a dialog asks, and the most options it shows of one question. */
const MOST = 4
/** The longest header Claude Code's dialog shows. */
const HEADER = 12
export const MORE = 'More options'
export const APPROVE = 'Approve'
export const REJECT = 'Reject'

/** How far each question asked over several dialogs has got: its next option and those chosen. */
export type Paging = Readonly<Record<string, { from: number; chosen: readonly string[] }>>

type Option = { label: string; detail?: string; recommended?: boolean }

/** A question as Claude Code's dialog asks it. */
export type DialogQuestion = {
  question: string
  header: string
  multiSelect: boolean
  options: { label: string; description: string }[]
}

/** One dialog: its questions, and for each the block it asks and what its labels stand for. */
export type Dialog = {
  questions: DialogQuestion[]
  asks: { block: Block; question: string; labels: Map<string, string>; from: number; more: boolean }[]
}

const text = (value: unknown) => (typeof value === 'string' ? value : '')

/** The next dialog for the blocks that still wait; null where none waits. */
export function dialogOf(waiting: readonly Block[], paging: Paging, who?: string): Dialog | null {
  const blocks = waiting.slice(0, MOST)
  if (blocks.length === 0) return null
  const used = new Set<string>()
  const asks: Dialog['asks'] = []
  const questions = blocks.map((block): DialogQuestion => {
    const body = block.version.body
    const by = who ? ` (${who})` : ''
    if (block.kind !== 'question') {
      const question = unique(`${text(body.title)}${by}`, used)
      asks.push({ block, question, labels: new Map([[APPROVE, APPROVE], [REJECT, REJECT]]), from: 0, more: false })
      return {
        question,
        header: block.kind === 'plan' ? 'Plan' : 'Approval',
        multiSelect: false,
        options: [
          { label: APPROVE, description: 'Record it approved.' },
          { label: REJECT, description: 'Record it rejected, with your notes as the reason.' },
        ],
      }
    }
    const options = (Array.isArray(body.options) ? body.options : []) as Option[]
    const from = paging[block.id]?.from ?? 0
    const left = options.slice(from)
    const more = left.length > MOST
    const shown = more ? left.slice(0, MOST - 1) : left
    const question = unique(`${text(body.prompt)}${by}`, used)
    const labelOf = (option: Option) => (option.recommended ? `${option.label} (Recommended)` : option.label)
    asks.push({ block, question, labels: new Map(shown.map(option => [labelOf(option), option.label])), from, more })
    return {
      question,
      header: (text(body.header) || 'Question').slice(0, HEADER),
      multiSelect: body.multiple === true,
      options: [
        ...shown.map(option => ({ label: labelOf(option), description: option.detail ?? '' })),
        ...(more ? [{ label: MORE, description: `Show the other ${left.length - shown.length} options.` }] : []),
      ],
    }
  })
  return { questions, asks }
}

/** What an answered dialog records, and how far its long questions have got. */
export function recordedOf(dialog: Dialog, result: unknown, paging: Paging): { given: Given[]; paging: Paging } {
  const answers = (result as { answers?: Record<string, unknown> } | undefined)?.answers ?? {}
  const annotations = (result as { annotations?: Record<string, { notes?: unknown }> } | undefined)?.annotations ?? {}
  const given: Given[] = []
  const next: Record<string, { from: number; chosen: readonly string[] }> = { ...paging }
  for (const ask of dialog.asks) {
    const value = answers[ask.question]
    if (typeof value !== 'string' || value === '') continue
    const typed = annotations[ask.question]?.notes
    const notes = typeof typed === 'string' && typed.trim() !== '' ? typed.trim() : undefined
    const id = ask.block.id
    if (ask.block.kind !== 'question') {
      // Words typed in place of an option reject it, with those words as the reason.
      const reason = value === REJECT ? notes : value === APPROVE ? undefined : value
      given.push(
        value === APPROVE
          ? { block: id, verdict: 'approved' }
          : { block: id, verdict: 'rejected', ...(reason ? { reason } : {}) },
      )
      continue
    }
    const before = paging[id]?.chosen ?? []
    const parts = ask.block.version.body.multiple === true ? value.split(', ') : [value]
    const known = parts.every(part => part === MORE || ask.labels.has(part))
    const picked = known ? parts.flatMap(part => (part === MORE ? [] : [ask.labels.get(part)!])) : []
    if (known && parts.includes(MORE)) {
      next[id] = { from: ask.from + MOST - 1, chosen: [...before, ...picked] }
      continue
    }
    delete next[id]
    const labels = [...before, ...picked]
    given.push({
      block: id,
      ...(labels.length > 0 ? { labels } : {}),
      ...(known ? {} : { words: value }),
      ...(notes ? { notes } : {}),
    })
  }
  return { given, paging: next }
}

// A dialog's answers are keyed by the question's text, so no two of its questions share one.
function unique(question: string, used: Set<string>): string {
  let n = 1
  let free = question
  while (used.has(free)) free = `${question} (${++n})`
  used.add(free)
  return free
}
