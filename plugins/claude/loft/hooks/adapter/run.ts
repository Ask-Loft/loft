// How the Yard hooks run `loft yard`: as the mod runs `loft` (`../loft`), with the page server's
// port, the base branch for a write and the calling agent (the Feature "The Loft plugin").

import type { EngineInterface } from 'claude-code'

import { bytesOf, loftArgv, QUIET, runLoft, type Doc, type Ran, type Settings } from '../loft'
import { parseDocument } from '../model'
import { HOST } from './contract'

/** Whether the document reports a result, rather than a refusal or an error. */
export function succeeded(ran: Ran): ran is Ran & { doc: Doc } {
  return ran.doc !== undefined && ran.exitCode === 0 && ran.doc.outcome !== 'error' && ran.doc.outcome !== 'refused'
}

/**
 * The command's own message for a run that did not succeed; for output that does not read as a
 * document, the bytes printed, where Claude Code cut them short, and what the command said on
 * standard error.
 */
export function failureOf(ran: Ran): string {
  if (ran.unrun !== undefined) return ran.unrun
  const errors = Array.isArray(ran.doc?.errors) ? (ran.doc.errors as { message?: unknown }[]) : []
  const messages = errors.map(error => String(error.message ?? '')).filter(message => message !== '')
  if (messages.length > 0) return messages.join('; ')
  const said = ran.stderr.trim()
  if (ran.doc === undefined) {
    const printed = ran.printed ?? 0
    const cut = ran.truncated ? `, cut short by Claude Code after ${printed} bytes` : ''
    const unread = `loft printed ${printed} bytes on standard output that could not be read as a document${cut}`
    return said ? `${unread}: ${said}` : unread
  }
  return said || `exit ${ran.exitCode}`
}

/** The calling agent: its session, and for a subagent its id and type. */
export type Caller = { session: string; agent?: { id: string; type: string } }

/** The flags that name the calling agent to a command that acts for one. */
export function callerArgs(caller: Caller): string[] {
  return [
    '--session',
    caller.session,
    ...(caller.agent ? ['--agent', caller.agent.id, '--agent-type', caller.agent.type] : []),
  ]
}

/**
 * The argument vector of `loft yard <args>`, with the page server's port where it takes one, and
 * the default base branch for a write, which records it where it opens Yard Log on a branch git
 * names no other branch it was cut from.
 */
export function argvOf(settings: Settings, args: readonly string[], beyond: boolean): string[] {
  const base = args[0] === 'write' ? ['--default-base', settings.base] : []
  const where = beyond ? [...base, '--port', String(settings.port)] : []
  const host = (settings.protocol ?? 1) >= 2 && args.includes('--session') ? ['--host', HOST] : []
  return loftArgv(settings, ['yard', ...args, ...host, ...where])
}

/** Runs `loft yard <args>` in `cwd` and reads its document. */
export function yard(
  $: EngineInterface,
  settings: Settings,
  cwd: string,
  args: readonly string[],
  init: { stdin?: string; beyond?: boolean; timeoutMs?: number } = {},
): Promise<Ran> {
  return runLoft($, cwd, argvOf(settings, args, init.beyond ?? true), init)
}

/** A command that runs until its document is printed, ended early by leaving its loop. */
export type Running = { done: Promise<Ran>; end: () => void }

/** Starts `loft yard <args>` in `cwd`; `end()` kills it, and `done` then reads as never run. */
export function spawnYard($: EngineInterface, settings: Settings, cwd: string, args: readonly string[]): Running {
  const stream = $.process.spawn({ argv: argvOf(settings, args, true), cwd, env: QUIET })
  let ended = false
  const done = (async (): Promise<Ran> => {
    let stdout = ''
    let stderr = ''
    try {
      for (;;) {
        const piece = await stream.next()
        if (piece.done) {
          const code = piece.value?.code ?? 1
          return { doc: parseDocument(stdout), exitCode: code, stderr, printed: bytesOf(stdout) }
        }
        if (piece.value.stream === 'stdout') stdout += piece.value.text
        else stderr += piece.value.text
      }
    } catch (error) {
      return { exitCode: -1, stderr, unrun: ended ? 'ended' : error instanceof Error ? error.message : String(error) }
    }
  })()
  return {
    done,
    end: () => {
      ended = true
      stream.return(undefined as never).catch(() => {})
    },
  }
}
