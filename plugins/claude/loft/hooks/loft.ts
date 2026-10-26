// How the mod runs `loft`: through the plugin's `loftCommand` setting, quietly, with
// `--format json`, reading the one JSON document the command prints and nothing else (the
// Feature "The Loft plugin").

import type { EngineInterface, PluginOptions } from 'claude-code'

import { commandWords, parseDocument } from './model'

/** A run is the mod's, not the person's: it sends no command event and asks for no update. */
export const QUIET = { LOFT_TELEMETRY: 'off', LOFT_UPDATE_CHECK: 'off' }

/** The page server's port unless the `yardPort` setting names another. */
export const DEFAULT_PORT = 7411

/** The plugin's settings: the words that run loft, the base branch and the page server's port. */
export type Settings = { words: string[]; base: string; port: number; protocol?: number }

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

/**
 * What one run gave: its document where it printed one, its exit code, and why it never ran; the
 * bytes it printed on standard output, and whether Claude Code cut them short.
 */
export type Ran = { doc?: Doc; exitCode: number; stderr: string; unrun?: string; printed?: number; truncated?: boolean }

/** The number of bytes `text` takes in UTF-8. */
export function bytesOf(text: string): number {
  return new TextEncoder().encode(text).length
}

/** The argument vector of `loft <args> --format json`. */
export function loftArgv(settings: Settings, args: readonly string[]): string[] {
  return [...settings.words, ...args, '--format', 'json']
}

/** Runs an argument vector of `loft` in `cwd`, quietly, and reads its document. */
export async function runLoft(
  $: EngineInterface,
  cwd: string,
  argv: string[],
  init: { stdin?: string; timeoutMs?: number } = {},
): Promise<Ran> {
  try {
    const ran = await $.process.run(argv, {
      cwd,
      env: QUIET,
      timeoutMs: init.timeoutMs ?? 30000,
      ...(init.stdin !== undefined ? { stdin: init.stdin } : {}),
    })
    // Output cut short is never a document, even where the bytes kept would read as one.
    const truncated = ran.isStdoutTruncated === true
    return {
      doc: truncated ? undefined : parseDocument(ran.stdout),
      exitCode: ran.exitCode,
      stderr: ran.stderr,
      printed: bytesOf(ran.stdout),
      truncated,
    }
  } catch (error) {
    return { exitCode: -1, stderr: '', unrun: error instanceof Error ? error.message : String(error) }
  }
}
