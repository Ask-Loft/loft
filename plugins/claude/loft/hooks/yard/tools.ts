// Yard's tools: what each takes, how its input is checked, and the `loft yard` command it runs
// (the Feature "Yard's tools"). Pure.

import type { ToolSpec } from 'claude-code'

import { callerArgs, type Caller } from './run'

export type ToolName = 'yard_read' | 'yard_write' | 'yard_wait' | 'yard_withdraw' | 'yard_close' | 'yard_published'

type Field = { type: 'integer' | 'boolean' | 'string' | 'array'; description: string; isRequired?: boolean }

const FIELDS: Record<ToolName, Record<string, Field>> = {
  yard_read: { since: { type: 'integer', description: 'Return only what changed after this revision.' } },
  yard_write: {
    batch: {
      type: 'array',
      description: 'The blocks to write, each an object with its id and kind, as the yard skill describes.',
      isRequired: true,
    },
    basedOn: { type: 'integer', description: 'The revision last read: a block changed since then is refused.' },
    send: { type: 'boolean', description: "Send the batch's questions, approvals and plans to the person." },
  },
  yard_wait: { submission: { type: 'string', description: "The submission's key, as `yard_write` gave it.", isRequired: true } },
  yard_withdraw: { submission: { type: 'string', description: "The submission's key.", isRequired: true } },
  yard_close: {},
  yard_published: {
    branch: { type: 'string', description: 'The branch whose Yard Log the merge brought into the base branch.', isRequired: true },
    note: { type: 'string', description: 'A note on the publish, in Markdown.' },
  },
}

const DESCRIPTIONS: Record<ToolName, string> = {
  yard_read: "Read the branch's Yard Log: all of it, or what changed since a revision. Gives the revision and the page's address.",
  yard_write:
    "Write a batch of blocks to the branch's Yard Log, opening it on the first write; with `send`, send its questions, approvals and plans to the person.",
  yard_wait:
    'Wait until the person answers a submission on the page or in the dialog, or it is withdrawn or Yard Log closed. Holds the turn until then.',
  yard_withdraw: 'Withdraw an open submission.',
  yard_close: "Close the branch's Yard Log once its work is done; it then takes no more writes.",
  yard_published: 'On the base branch, once `loft publish` is committed, record the publish in the Yard Log the merge brought in.',
}

export const TOOL_NAMES = Object.keys(FIELDS) as ToolName[]

/** The tool as `$.tool.register` takes it. */
export function specOf(name: ToolName): ToolSpec {
  const fields = FIELDS[name]
  return {
    name,
    description: DESCRIPTIONS[name],
    isDeferred: false,
    inputSchema: {
      type: 'object',
      properties: Object.fromEntries(
        Object.entries(fields).map(([field, { type, description }]) => [
          field,
          type === 'array' ? { type, description, items: { type: 'object' } } : { type, description },
        ]),
      ),
      required: Object.entries(fields)
        .filter(([, field]) => field.isRequired)
        .map(([field]) => field),
      additionalProperties: false,
    },
  }
}

/** The keys a tool call carries beside the tool's own arguments. */
const RESERVED = new Set(['tool', 'tool_use_id', 'agentId', 'requestMeta', 'consent'])

const fits = (type: Field['type'], value: unknown) =>
  type === 'integer'
    ? Number.isInteger(value) && (value as number) >= 0
    : type === 'array'
      ? Array.isArray(value) && value.every(each => typeof each === 'object' && each !== null && !Array.isArray(each))
      : typeof value === type

/** The input's arguments, or the fault that names the field at fault. */
export function argumentsOf(name: ToolName, e: Record<string, unknown>): { args: Record<string, unknown> } | { fault: string } {
  const fields = FIELDS[name]
  const args: Record<string, unknown> = {}
  for (const [field, value] of Object.entries(e)) {
    if (RESERVED.has(field) || value === undefined) continue
    const expected = fields[field]
    if (!expected) return { fault: `\`${name}\` takes no field \`${field}\`.` }
    if (!fits(expected.type, value)) return { fault: `\`${name}\`: the field \`${field}\` must be ${article(expected.type)}.` }
    args[field] = value
  }
  for (const [field, { isRequired }] of Object.entries(fields)) {
    if (isRequired && args[field] === undefined) return { fault: `\`${name}\`: the field \`${field}\` is required.` }
  }
  return { args }
}

function article(type: Field['type']): string {
  return type === 'integer' ? 'a whole number of 0 or more' : type === 'array' ? 'a list of objects' : `a ${type}`
}

/** The `loft yard` command a call runs: its arguments, and what it reads on standard input. */
export function commandOf(name: ToolName, args: Record<string, unknown>, caller: Caller): { args: string[]; stdin?: string } {
  const who = callerArgs(caller)
  switch (name) {
    case 'yard_read':
      // The agent's own read records the revision it read.
      return { args: ['read', ...(args.since !== undefined ? ['--since', String(args.since)] : []), '--mark-read', ...who] }
    case 'yard_write':
      return {
        args: [
          'write',
          '--input',
          '-',
          ...(args.send === true ? ['--send'] : []),
          ...(args.basedOn !== undefined ? ['--based-on', String(args.basedOn)] : []),
          ...who,
        ],
        stdin: JSON.stringify(args.batch),
      }
    case 'yard_wait':
      return { args: ['wait', '--submission', String(args.submission), '--timeout', '540', ...who] }
    case 'yard_withdraw':
      return { args: ['withdraw', '--submission', String(args.submission), ...who] }
    case 'yard_close':
      return { args: ['close', ...who] }
    case 'yard_published':
      return {
        args: ['published', '--branch', String(args.branch), ...(args.note !== undefined ? ['--note', String(args.note)] : []), ...who],
      }
  }
}
