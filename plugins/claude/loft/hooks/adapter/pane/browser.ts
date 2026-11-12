// The Yard page in Claude Code Desktop's browser pane, beside the chat: `/yard-pane` opens it there
// at the session's tree and branch, through Desktop's own browser server, and shows the tab it
// opened before rather than another. Where that server can't be reached, the pane links the page
// and says so (the Feature "Keep the Yard page beside the session").

import type { EngineInterface, McpToolResult } from 'claude-code'

import type { PaneBrowser } from '../../../types'

/** Desktop's built-in browser, as the session's MCP servers name it. */
const BROWSER = 'Claude_Browser'

const textOf = (result: McpToolResult) =>
  result.content.flatMap(block => (block.type === 'text' && typeof block.text === 'string' ? [block.text] : [])).join('\n')

/** The tab `preview_start` opened, from its structured result or its text. */
function tabOf(result: McpToolResult): string | null {
  const structured = result.structuredContent as { tabId?: unknown } | undefined
  if (typeof structured?.tabId === 'string' || typeof structured?.tabId === 'number') return String(structured.tabId)
  const named = /"?tabId"?\s*[:=]\s*"?([\w-]+)/.exec(textOf(result))
  return named ? named[1]! : null
}

/** Shows the tab `tab` at `url`; false where the browser no longer has it. */
async function shown($: EngineInterface, tab: string, url: string): Promise<boolean> {
  return !(await $.mcp.call(BROWSER, 'navigate', { tabId: tab, url })).isError
}

/**
 * Opens the Yard page at `url` in Desktop's browser pane, in the tab `held` where the browser still
 * has it; nothing where no surface of the session is Desktop. Says which tab shows it, or that the
 * browser pane could not be reached.
 */
export async function showPage($: EngineInterface, url: string, held: string | null): Promise<PaneBrowser | null> {
  if (!(await $.session.surfaces()).includes('desktop')) return null
  try {
    if (held !== null && (await shown($, held, url))) return { tab: held, isReached: true }
    const started = await $.mcp.call(BROWSER, 'preview_start', { url })
    if (started.isError) return { tab: null, isReached: false }
    return { tab: tabOf(started), isReached: true }
  } catch {
    return { tab: null, isReached: false }
  }
}
