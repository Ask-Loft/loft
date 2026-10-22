// The context a session starts with: where the branch's Yard Log stands, from one `loft yard read`
// (the Feature "Start with the session"). Pure, and never longer than LIMIT characters.

import { activeStage, openSubmissions, pageOf, stagesOf, type Asker, type Block } from './log'
import type { Doc } from './run'

/** The most characters the context holds. */
export const LIMIT = 2000

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

/** The revision of a response's Act. */
function revisionOf(response: unknown): number {
  const act = isObject(response) && isObject(response.act) ? response.act : undefined
  return typeof act?.revision === 'number' ? act.revision : 0
}

/** What was answered or decided after the revision the agent last read, oldest first. */
function respondedSince(doc: Doc, since: number): { block: Block; revision: number }[] {
  return stagesOf(doc)
    .flatMap(stage => stage.blocks ?? [])
    .flatMap(block => {
      const revision = Math.max(revisionOf(block.answer), revisionOf(block.outcome))
      return revision > since ? [{ block, revision }] : []
    })
    .sort((a, b) => a.revision - b.revision)
}

const titleOf = (block: Block) => {
  const body = block.version.body
  return String(body.title ?? body.prompt ?? block.id)
}

/** A list of lines cut to fit `room` characters, saying how many it leaves out. */
function fitted(lines: readonly string[], room: number): string[] {
  const kept: string[] = []
  let used = 0
  for (const [n, line] of lines.entries()) {
    const left = lines.length - n
    const tail = `  … and ${left} more`
    if (used + line.length + 1 + (left > 1 ? tail.length + 1 : 0) > room) {
      kept.push(tail)
      return kept
    }
    kept.push(line)
    used += line.length + 1
  }
  return kept
}

/** The session's context for the branch's Yard Log, as `loft yard read` reported it. */
export function contextOf(doc: Doc, asker: Asker): string {
  const branch = String(doc.branch ?? 'this branch')
  const page = pageOf(doc)
  const head = [`Yard Log of \`${branch}\`.`, page ? `Its page: ${page}` : 'No page server serves its page yet.']
  const log = isObject(doc.yardLog) ? doc.yardLog : {}
  if (log.key === undefined) {
    head.push('The branch has no Yard Log yet: the first `yard_write` opens it.')
    return head.join('\n')
  }
  if (doc.ignored === true) head.push('git ignores `.yard/`, so Yard Log stays out of the commits.')
  if (log.closedBy !== undefined && log.closedBy !== 'open') {
    head.push('Yard Log is closed: it takes no more writes, so more work goes on a branch of its own.')
    return head.join('\n')
  }
  head.push(`Revision ${String(doc.revision)}.`)
  const stage = activeStage(doc, asker)
  head.push(stage ? `Your active stage: \`${stage.id}\` — ${titleOf(stage)}.` : 'You have no active stage.')
  const sent = openSubmissions(doc)
  const since = typeof doc.lastRead === 'number' ? doc.lastRead : 0
  const responded = respondedSince(doc, since)
  const sections: { title: string; lines: string[] }[] = [
    {
      title: sent.length === 0 ? 'No submission is open.' : `Open submissions (${sent.length}), each waited on with \`yard_wait\`:`,
      lines: sent.map(each => {
        const who = asker.sessions.includes(each.agent.session ?? '') && !each.agent.agent ? 'sent by you' : 'sent by another agent'
        return `- \`${each.key}\`, ${each.blocks.length} block${each.blocks.length === 1 ? '' : 's'}, ${who}`
      }),
    },
    {
      title:
        responded.length === 0
          ? `Nothing was answered or decided since revision ${since}, which you last read.`
          : `Answered or decided since revision ${since}, which you last read (${responded.length}):`,
      lines: responded.map(({ block }) => {
        const kind = block.outcome ? `${String(block.outcome.verdict ?? 'decided')}` : 'answered'
        return `- \`${block.id}\` ${kind}: ${titleOf(block)}`
      }),
    },
  ]
  let text = head.join('\n')
  for (const [n, section] of sections.entries()) {
    const later = sections.slice(n + 1).reduce((sum, each) => sum + each.title.length + 40, 0)
    const room = LIMIT - text.length - section.title.length - 2 - later
    text += `\n${section.title}`
    if (section.lines.length > 0) text += `\n${fitted(section.lines, Math.max(0, room)).join('\n')}`
  }
  text += '\nRead Yard Log with `yard_read` before writing.'
  return text.length <= LIMIT ? text : `${text.slice(0, LIMIT - 1)}…`
}
