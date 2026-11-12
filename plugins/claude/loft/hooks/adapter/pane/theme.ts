// The Yard page's colours, as the pane wears them: the tokens of `yard.css`, light and dark, drawn
// again here since the mod shares no code with the page. Action blue marks what is in hand, teal
// what is done, rust what waits on the person, lilac plan mode and rose a fix the person asked for
// (the Feature "See Yard Log in the pane as on the Yard page"). Pure.

/** The tokens the pane draws with, by their names on the page. */
export type Palette = {
  canvas: string
  surface: string
  mist: string
  ink: string
  body: string
  muted: string
  line: string
  rail: string
  signal: string
  done: string
  wait: string
  waitSoft: string
  waitLine: string
  drop: string
  danger: string
  plan: string
  spec: string
  button: string
  onButton: string
  task: string
  taskPale: string
  fix: string
  fixPale: string
  waitBar: string
  recommended: string
  recommendedSoft: string
}

export const LIGHT: Palette = {
  canvas: '#ffffff',
  surface: '#f6f9fb',
  mist: '#e5f6ff',
  ink: '#0e141d',
  body: '#424b56',
  muted: '#5c646f',
  line: '#e4e9ee',
  rail: '#aab2ba',
  signal: '#14488f',
  done: '#0a756c',
  wait: '#a8480c',
  waitSoft: '#fbeee4',
  waitLine: '#c8642a',
  drop: '#5c646f',
  danger: '#b42318',
  plan: '#62359c',
  spec: '#7a5600',
  button: '#14488f',
  onButton: '#ffffff',
  task: '#468de5',
  taskPale: '#c4dbf7',
  fix: '#d23f74',
  fixPale: '#f6cddc',
  waitBar: '#eb6834',
  recommended: '#0a756c',
  recommendedSoft: '#e3f4f1',
}

export const DARK: Palette = {
  canvas: '#0c1117',
  surface: '#10161d',
  mist: '#132233',
  ink: '#e8eef4',
  body: '#b8c2cc',
  muted: '#95a0ab',
  line: '#222c37',
  rail: '#4a5561',
  signal: '#8ec5f2',
  done: '#5ed3c0',
  wait: '#f2a36e',
  waitSoft: '#33210f',
  waitLine: '#b8642e',
  drop: '#95a0ab',
  danger: '#ff8f84',
  plan: '#c3a2ec',
  spec: '#e8b64a',
  button: '#8ec5f2',
  onButton: '#0c1117',
  task: '#468de5',
  taskPale: '#24405f',
  fix: '#e2578a',
  fixPale: '#4d2033',
  waitBar: '#d95926',
  recommended: '#5ed3c0',
  recommendedSoft: '#0f2e2a',
}

/**
 * Whether Claude Code's `theme` setting is a dark one (`dark`, `dark-daltonized`, `dark-ansi`). A
 * theme that follows the system, or none, takes the page's own default, the light one.
 */
export function isDark(theme: unknown): boolean {
  return typeof theme === 'string' && theme.startsWith('dark')
}

export function paletteOf(dark: boolean): Palette {
  return dark ? DARK : LIGHT
}

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
