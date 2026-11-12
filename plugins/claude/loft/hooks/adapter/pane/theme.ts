// The pane's colours. No mod can tell whether a surface is drawn light or dark, and Claude Code
// Desktop may be light while the terminal is dark, so text, muted text and lines take Claude
// Code's own colours, which each surface resolves to its own appearance, and the Yard page's hues
// mark status alone, in shades that read on a light ground and on a dark one: action blue for
// what is in hand, rust for what waits on the person, rose for a fix the person asked for and teal
// for the option an agent recommends (the Feature "Keep the Yard page beside the session"). Pure.

import type { ThemeKey } from 'claude-code'

/** The colours the pane draws with: Claude Code's own for its neutrals, a fixed shade for each status. */
export type Palette = {
  ink: ThemeKey
  muted: ThemeKey
  line: ThemeKey
  danger: ThemeKey
  signal: string
  wait: string
  fix: string
  recommended: string
  /** Text on a ground filled with a status colour. */
  onButton: string
}

export const PALETTE: Palette = {
  ink: 'text',
  muted: 'inactive',
  line: 'subtle',
  danger: 'error',
  signal: '#468de5',
  wait: '#e0662a',
  fix: '#d9487c',
  recommended: '#169c8a',
  onButton: '#ffffff',
}

/** The shades that mark status, each of which reads on a light ground and on a dark one. */
export const STATUS_SHADES = ['signal', 'wait', 'fix', 'recommended'] as const
