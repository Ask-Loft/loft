// The Yard page in Claude Code Desktop's browser pane, beside the chat: `/yard-pane` opens it there
// at the session's tree and branch, through Desktop's own browser server, and shows the tab it
// opened before rather than another. Where that server can't be reached, the pane links the page
// and says so (the Feature "Keep the Yard page beside the session").

import type { EngineInterface, McpToolResult } from 'claude-code'

import type { PaneBrowser } from '../../../types'
import { isObject, records } from '../../model'
import { linkOf } from './model'

/** Desktop's built-in browser, as the session's MCP servers name it. */
const BROWSER = 'Claude_Browser'

const resultText = (result: McpToolResult) =>
  result.content.flatMap(block => (block.type === 'text' && typeof block.text === 'string' ? [block.text] : [])).join('\n')

/** Where the JSON object that `text` begins with ends, matching its braces outside strings; -1 where none does. */
function objectEnd(text: string): number {
  let depth = 0
  let inString = false
  for (let at = 0; at < text.length; at++) {
    const char = text[at]
    if (inString) {
      if (char === '\\') at++
      else if (char === '"') inString = false
    } else if (char === '"') inString = true
    else if (char === '{') depth++
    else if (char === '}' && --depth === 0) return at
  }
  return -1
}

/** The first JSON object of a browser answer, among the words it adds for an agent; empty where there is none. */
function jsonOf(result: McpToolResult): Record<string, unknown> {
  if (isObject(result.structuredContent)) return result.structuredContent
  const whole = resultText(result)
  const text = whole.slice(Math.max(whole.indexOf('{'), 0))
  const end = text.startsWith('{') ? objectEnd(text) : -1
  try {
    const parsed: unknown = end < 0 ? undefined : JSON.parse(text.slice(0, end + 1))
    return isObject(parsed) ? parsed : {}
  } catch {
    return {}
  }
}

/** The tab a browser answer names. */
function tabOf(result: McpToolResult): string | null {
  const tab = jsonOf(result).tabId
  return typeof tab === 'string' || typeof tab === 'number' ? String(tab) : null
}

/** An address's origin, the page server's loopback named `localhost` as its link names it. */
const originOf = (address: string) => new URL(linkOf(address)).origin

/**
 * The tab to show the page in, among those at the page's origin: the one held, where it is still
 * there, else the first.
 */
async function tabFor($: EngineInterface, url: string, held: string | null): Promise<string | null> {
  const context = await $.mcp.call(BROWSER, 'tabs_context', {})
  if (context.isError) return null
  const origin = originOf(url)
  const ids = records(jsonOf(context).tabs).flatMap(tab => (typeof tab.origin === 'string' && originOf(tab.origin) === origin ? [String(tab.tabId)] : []))
  return ids.find(id => id === held) ?? ids[0] ?? null
}

/**
 * Opens the Yard page at `url` in Desktop's browser pane: in a tab that shows the page already,
 * the one `held` first, else in a new one; nothing where no surface of the session is Desktop.
 * Says which tab shows it, as the browser names it, or that the browser pane could not be reached,
 * keeping `held` for the next time.
 */
export async function showPage($: EngineInterface, url: string, held: string | null): Promise<PaneBrowser | null> {
  if (!(await $.session.surfaces()).includes('desktop')) return null
  try {
    const tab = await tabFor($, url, held)
    if (tab !== null) {
      const shown = await $.mcp.call(BROWSER, 'navigate', { tabId: tab, url })
      if (!shown.isError) return { tab: tabOf(shown) ?? tab, isReached: true }
    }
    const started = await $.mcp.call(BROWSER, 'preview_start', { url })
    if (started.isError) return { tab: held, isReached: false }
    return { tab: tabOf(started), isReached: true }
  } catch {
    return { tab: held, isReached: false }
  }
}
