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

/** The JSON document that begins a browser answer, before the words it adds for an agent. */
function jsonOf(result: McpToolResult): Record<string, unknown> {
  if (result.structuredContent && typeof result.structuredContent === 'object') return result.structuredContent as Record<string, unknown>
  const text = textOf(result)
  const end = text.lastIndexOf('}')
  try {
    return text.startsWith('{') && end > 0 ? (JSON.parse(text.slice(0, end + 1)) as Record<string, unknown>) : {}
  } catch {
    return {}
  }
}

/** The tab a browser answer names. */
function tabOf(result: McpToolResult): string | null {
  const tab = jsonOf(result).tabId
  return typeof tab === 'string' || typeof tab === 'number' ? String(tab) : null
}

/** The tab to show the page in: the one held, where the browser still has it, else any at the page's origin. */
async function tabFor($: EngineInterface, url: string, held: string | null): Promise<string | null> {
  const context = await $.mcp.call(BROWSER, 'tabs_context', {})
  if (context.isError) return null
  const tabs = Array.isArray(jsonOf(context).tabs) ? (jsonOf(context).tabs as { tabId?: unknown; origin?: unknown }[]) : []
  const ids = tabs.map(tab => ({ id: String(tab.tabId), origin: tab.origin }))
  const origin = new URL(url).origin
  return ids.find(tab => tab.id === held)?.id ?? ids.find(tab => tab.origin === origin)?.id ?? null
}

/**
 * Opens the Yard page at `url` in Desktop's browser pane: in a tab that shows the page already,
 * the one `held` first, else in a new one; nothing where no surface of the session is Desktop.
 * Says which tab shows it, as the browser names it, or that the browser pane could not be reached.
 */
export async function showPage($: EngineInterface, url: string, held: string | null): Promise<PaneBrowser | null> {
  if (!(await $.session.surfaces()).includes('desktop')) return null
  try {
    const tab = await tabFor($, url, held)
    if (tab !== null) {
      const moved = await $.mcp.call(BROWSER, 'navigate', { tabId: tab, url })
      if (!moved.isError) return { tab, isReached: true }
    }
    const started = await $.mcp.call(BROWSER, 'preview_start', { url })
    if (started.isError) return { tab: null, isReached: false }
    return { tab: tabOf(started), isReached: true }
  } catch {
    return { tab: null, isReached: false }
  }
}
