// The Loft status pane's tones: every Text the pane draws takes its colour and weight from this
// table by what it means, each colour a key of the person's theme, and none is drawn dim. A
// state is never told by colour alone: every check, change and criterion also has a glyph and
// a word. tests/pane.test.tsx holds every drawn Text to the table.

import type { ThemeKey } from 'claude-code'

export type Tone =
  | 'title'
  | 'heading'
  | 'body'
  | 'meta'
  | 'muted'
  | 'current'
  | 'done'
  | 'todo'
  | 'error'
  | 'accent'
  | 'rule'

export const TONES: Record<Tone, { color: ThemeKey; bold?: true }> = {
  title: { color: 'claude', bold: true },
  heading: { color: 'text', bold: true },
  body: { color: 'text' },
  meta: { color: 'subtle' },
  muted: { color: 'inactive' },
  current: { color: 'claude', bold: true },
  done: { color: 'success' },
  todo: { color: 'warning' },
  error: { color: 'error' },
  accent: { color: 'suggestion', bold: true },
  rule: { color: 'subtle' },
}

type Elements = { Box: any; Text: any }

/** The drawing kit of one render: the surface's elements, text drawn by tone only. */
export function kit({ Box, Text }: Elements, width: number) {
  const tone = (name: Tone, children: unknown, wrap: 'wrap' | 'truncate-end' = 'truncate-end') => (
    <Text color={TONES[name].color} bold={TONES[name].bold} wrap={wrap}>
      {children}
    </Text>
  )
  const rule = () => tone('rule', '─'.repeat(Math.max(4, Math.min(width, 48))))
  return { Box, tone, rule }
}
