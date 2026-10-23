// How the agent's own questions, plans and tasks become blocks of Yard Log, and how the answers
// recorded in Yard Log come back to the agent as Claude Code's tools give them. Pure, so that
// each rule is tested on its own.

import { relativeTo } from '../model'
import { idOf, type Block } from './log'

/** A question as `AskUserQuestion` takes it. */
export type Asked = {
  question: string
  header: string
  multiSelect: boolean
  options: readonly { label: string; description: string }[]
}

/** The words after an option's label that make it the recommended one. */
const RECOMMENDED = /\s*\(Recommended\)\s*$/i

/** An option's label without "(Recommended)", and whether it carried it. */
export function recommendedIn(label: string): { label: string; isRecommended: boolean } {
  return RECOMMENDED.test(label)
    ? { label: label.replace(RECOMMENDED, ''), isRecommended: true }
    : { label, isRecommended: false }
}

/** The `question` blocks of a batch for the agent's questions, each with its id. */
export function questionBlocks(questions: readonly Asked[], ids: readonly string[], stage: string) {
  return questions.map((asked, n) => {
    let isTaken = false
    const options = asked.options.map(option => {
      const { label, isRecommended } = recommendedIn(option.label)
      // A question that takes one option has one recommended option: the first marked.
      const recommended = isRecommended && (asked.multiSelect || !isTaken)
      if (recommended) isTaken = true
      return { label, ...(option.description ? { detail: option.description } : {}), recommended }
    })
    return {
      id: ids[n]!,
      kind: 'question',
      stage,
      prompt: asked.question,
      header: asked.header || 'Question',
      multiple: asked.multiSelect,
      ownWords: true,
      options,
    }
  })
}

/** What `AskUserQuestion` gives the agent: the answers by question, and the notes typed. */
export type AskResult = {
  questions: readonly Asked[]
  answers: Record<string, string>
  annotations?: Record<string, { notes: string }>
}

/** The labels an answer chose, and the words typed in place of one. */
function chosen(answer: Record<string, unknown>): { labels: string[]; words?: string; notes?: string } {
  const labels = Array.isArray(answer.labels) ? answer.labels.map(String) : []
  const words = typeof answer.words === 'string' && answer.words !== '' ? answer.words : undefined
  const notes = typeof answer.notes === 'string' && answer.notes !== '' ? answer.notes : undefined
  return { labels, ...(words ? { words } : {}), ...(notes ? { notes } : {}) }
}

/**
 * The answers recorded on the page, as the agent's tool would have given them: each option by the
 * label the agent wrote, "(Recommended)" and all, and words in place of an option as written.
 */
export function askResultOf(questions: readonly Asked[], blocks: ReadonlyMap<string, Block>, ids: readonly string[]): AskResult {
  const answers: Record<string, string> = {}
  const annotations: Record<string, { notes: string }> = {}
  questions.forEach((asked, n) => {
    const answer = [...blocks.values()].find(block => block.id === ids[n])?.answer
    if (!answer) return
    const { labels, words, notes } = chosen(answer)
    const written = new Map(asked.options.map(option => [recommendedIn(option.label).label, option.label]))
    const parts = labels.map(label => written.get(label) ?? label)
    const value = [...parts, ...(words ? [words] : [])].join(', ')
    if (value !== '') answers[asked.question] = value
    if (notes) annotations[asked.question] = { notes }
  })
  return { questions, answers, ...(Object.keys(annotations).length > 0 ? { annotations } : {}) }
}

/** One answer or decision as `loft yard answer` takes it. */
export type Given = { block: string; labels?: string[]; words?: string; notes?: string } | { block: string; verdict: 'approved' | 'rejected'; reason?: string }

/**
 * The answers the person gave in the terminal's dialog, as `loft yard answer` takes them: an
 * option by its label without "(Recommended)", and anything else as the person's own words.
 */
export function givenOf(questions: readonly Asked[], ids: readonly string[], result: unknown): Given[] {
  const answers = (result as { answers?: Record<string, unknown> } | undefined)?.answers ?? {}
  const annotations = (result as { annotations?: Record<string, { notes?: unknown }> } | undefined)?.annotations ?? {}
  return questions.flatMap((asked, n) => {
    const value = answers[asked.question]
    if (typeof value !== 'string' || value === '') return []
    const labels = new Map(asked.options.map(option => [option.label, recommendedIn(option.label).label]))
    const parts = asked.multiSelect ? value.split(', ') : [value]
    const known = parts.every(part => labels.has(part))
    const notes = annotations[asked.question]?.notes
    return [
      {
        block: ids[n]!,
        ...(known ? { labels: parts.map(part => labels.get(part)!) } : { words: value }),
        ...(typeof notes === 'string' && notes.trim() !== '' ? { notes: notes.trim() } : {}),
      },
    ]
  })
}

/** A plan's title: its first heading, or its file's name where it has none. */
export function planTitle(markdown: string, path: string | undefined): string {
  const heading = /^#{1,6}\s+(.+?)\s*#*\s*$/m.exec(markdown)?.[1]?.trim()
  if (heading) return heading
  const name = path?.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '')
  return name && name !== '' ? name : 'Plan'
}

/** A plan's block id, from its file, so that the same file is a new version of the same block. */
export function planId(path: string | undefined, fallback: string): string {
  const name = path?.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '')
  return name ? idOf('plan', name) : fallback
}

/**
 * The path a plan block keeps of its file: relative to the tree, and none for a file outside it,
 * such as one of Claude Code's own plans, so that no path of the machine reaches Yard Log.
 */
export function planPath(tree: string | undefined, path: string | undefined): string | undefined {
  const relative = tree && path ? relativeTo(tree, path) : undefined
  return relative && !relative.split('/').includes('..') ? relative : undefined
}

// Claude Code gives the agent a rejection's feedback after these words.
const SAID = /the user said:\s*/i

/** The feedback typed with a rejection, as Claude Code gives it to the agent. */
export function feedbackIn(text: string): string | undefined {
  const said = SAID.exec(text)
  const rest = said ? text.slice(said.index + said[0].length).trim() : ''
  return rest === '' ? undefined : rest
}

/** A task's block id: from the task's id and who made it ([`whoOf`]), so that each update finds it. */
export function taskId(task: string, who: string): string {
  return idOf('task', who, task)
}

const TASK_STATES: Record<string, string> = {
  pending: 'open',
  in_progress: 'active',
  completed: 'done',
  deleted: 'dropped',
}

/** A task's state as Yard Log writes it. */
export function taskState(status: string | undefined): string | undefined {
  return status === undefined ? undefined : TASK_STATES[status]
}
