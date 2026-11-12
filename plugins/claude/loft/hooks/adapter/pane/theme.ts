// The pane's colours. No mod can tell whether a surface is drawn light or dark, and Claude Code
// Desktop may be light while the terminal is dark, so text, muted text and lines take Claude
// Code's own colours, which each surface resolves to its own appearance, and the Yard page's hues
// mark status alone, in shades that read on a light ground and on a dark one: action blue for
// what is in hand, teal for what is done, rust for what waits on the person and rose for a fix the
// person asked for (the Feature "See Yard Log in the pane as on the Yard page"). Pure.

import type { ThemeKey } from 'claude-code'

/** The colours the pane draws with: Claude Code's own for its neutrals, a fixed shade for each status. */
export type Palette = {
  ink: ThemeKey
  body: ThemeKey
  muted: ThemeKey
  line: ThemeKey
  /** A planned stage's mark. */
  rail: ThemeKey
  danger: ThemeKey
  signal: string
  done: string
  wait: string
  drop: string
  fix: string
  task: string
  plan: string
  spec: string
  recommended: string
  /** Text on a button filled with a status colour. */
  onButton: string
}

export const PALETTE: Palette = {
  ink: 'text',
  body: 'text',
  muted: 'inactive',
  line: 'subtle',
  rail: 'inactive',
  danger: 'error',
  signal: '#468de5',
  done: '#169c8a',
  wait: '#e0662a',
  drop: '#8a939c',
  fix: '#d9487c',
  task: '#468de5',
  plan: '#8b57be',
  spec: '#b8860b',
  recommended: '#169c8a',
  onButton: '#ffffff',
}

/** The shades that mark status, each of which reads on a light ground and on a dark one. */
export const STATUS_SHADES = ['signal', 'done', 'wait', 'drop', 'fix', 'task', 'plan', 'spec'] as const

/** The colour of a stage's or a block's status, as the page marks it. */
export function statusColour(palette: Palette, status: string): string {
  switch (status) {
    case 'active':
      return palette.signal
    case 'done':
      return palette.done
    case 'waiting':
      return palette.wait
    case 'dropped':
      return palette.drop
    default:
      return palette.rail
  }
}
