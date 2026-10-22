// How the Yard hooks run `loft yard`: through the plugin's `loftCommand` setting, quietly, with
// `--format json`, the base branch and the calling agent, reading the one JSON document the
// command prints and nothing else (the Feature "The Loft plugin").

import type { EngineInterface, PluginOptions } from 'claude-code'

import { commandWords, parseDocument } from '../model'

/** A `loft yard` command sends no command event and asks for no update. */
export const QUIET = { LOFT_TELEMETRY: 'off', LOFT_UPDATE_CHECK: 'off' }

/** The page server's port unless the `yardPort` setting names another. */
export const DEFAULT_PORT = 7411

/** The plugin's settings, as the Yard hooks read them. */
export type Settings = { words: string[]; base: string; port: number }

export function settingsOf(options: PluginOptions): Settings {
  const command = typeof options.loftCommand === 'string' ? options.loftCommand : 'loft'
  const base =
    typeof options.baseBranch === 'string' && options.baseBranch.trim() !== '' ? options.baseBranch.trim() : 'develop'
  const port = Number(options.yardPort)
  return {
    words: commandWords(command),
    base,
    port: Number.isInteger(port) && port > 0 && port < 65536 ? port : DEFAULT_PORT,
  }
}

/** A JSON document a `loft` command printed. */
export type Doc = Record<string, unknown>

/** What one run gave: its document where it printed one, its exit code, and why it never ran. */
export type Ran = { doc?: Doc; exitCode: number; stderr: string; unrun?: string }

/** Whether the document reports a result, rather than a refusal or an error. */
export function succeeded(ran: Ran): ran is Ran & { doc: Doc } {
  return ran.doc !== undefined && ran.exitCode === 0 && ran.doc.outcome !== 'error' && ran.doc.outcome !== 'refused'
}

/** The command's own message for a run that did not succeed. */
export function failureOf(ran: Ran): string {
  if (ran.unrun !== undefined) return ran.unrun
  const errors = Array.isArray(ran.doc?.errors) ? (ran.doc.errors as { message?: unknown }[]) : []
  const messages = errors.map(error => String(error.message ?? '')).filter(message => message !== '')
  if (messages.length > 0) return messages.join('; ')
  return ran.stderr.trim() || `exit ${ran.exitCode}`
}

/** The calling agent: its session, and for a subagent its id and type. */
export type Caller = { session: string; agent?: { id: string; type: string }; continues?: string }

/** The flags that name the calling agent to a command that acts for one. */
export function callerArgs(caller: Caller): string[] {
  return [
    '--session',
    caller.session,
    ...(caller.agent ? ['--agent', caller.agent.id, '--agent-type', caller.agent.type] : []),
    ...(caller.continues ? ['--continues', caller.continues] : []),
  ]
}

/** The argument vector of `loft yard <args>`, with the settings' base and port where it takes them. */
export function argvOf(settings: Settings, args: readonly string[], beyond: boolean): string[] {
  const where = beyond ? ['--base', settings.base, '--port', String(settings.port)] : []
  return [...settings.words, 'yard', ...args, ...where, '--format', 'json']
}

/** Runs `loft yard <args>` in `cwd` and reads its document. */
export async function yard(
  $: EngineInterface,
  settings: Settings,
  cwd: string,
  args: readonly string[],
  init: { stdin?: string; beyond?: boolean; timeoutMs?: number } = {},
): Promise<Ran> {
  try {
    const ran = await $.process.run(argvOf(settings, args, init.beyond ?? true), {
      cwd,
      env: QUIET,
      timeoutMs: init.timeoutMs ?? 30000,
      ...(init.stdin !== undefined ? { stdin: init.stdin } : {}),
    })
    return { doc: parseDocument(ran.stdout), exitCode: ran.exitCode, stderr: ran.stderr }
  } catch (error) {
    return { exitCode: -1, stderr: '', unrun: error instanceof Error ? error.message : String(error) }
  }
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
          return { doc: parseDocument(stdout), exitCode: code, stderr }
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
