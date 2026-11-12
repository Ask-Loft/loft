// The Yard pane's head, as the Yard page heads Yard Log: the repository above the Yard Log's
// title, set larger, a line that sums up its stages, a link to the page, the count of what waits
// on the person in the waiting colour, and beneath, the tabs of its views (the Feature "See Yard
// Log in the pane as on the Yard page").

import type { PaneShown, PaneView } from '../../../types'
import type { Drawing } from './kit'
import { kickerOf, linkOf, summaryOf } from './model'

const TABS: [PaneShown, string][] = [
  ['timeline', 'Timeline'],
  ['gantt', 'Gantt chart'],
]

/** The head; `isUnreached` where Desktop's browser pane could not be reached, which it then says beside the link. */
export function head({ t, palette, acts }: Drawing, view: PaneView, waiting: number, shown: PaneShown, isUnreached = false) {
  const { Box, Text, Button, Link } = t
  const link = view.page ? linkOf(view.page) : null
  return (
    <Box flexDirection="column">
      <Box flexDirection="row" justifyContent="space-between" gap={2}>
        <Box flexDirection="column" flexShrink={1}>
          <Box key="head-kicker" flexDirection="row" gap={1}>
            {kickerOf(view).map(part => (
              <Text color={palette.muted} size="caption">
                {part}
              </Text>
            ))}
          </Box>
          <Text color={palette.ink} size="title" weight="semibold">
            {view.title}
          </Text>
          <Text color={palette.muted} size="caption">
            {summaryOf(view)}
          </Text>
          {link ? <Link href={link} label="Open the Yard page" /> : null}
          {isUnreached ? (
            <Box key="browser-unreachable">
              <Text color={palette.muted} size="caption">
                Desktop's browser pane could not be reached.
              </Text>
            </Box>
          ) : null}
        </Box>
        {waiting > 0 ? (
          <Box flexDirection="column" alignItems="flex-end">
            <Text color={palette.muted} size="caption">
              Asked of you
            </Text>
            <Box key="wait-control" backgroundColor={palette.wait} paddingX={1}>
              <Button key="wait-count" plain onPress={() => acts.show('waiting')}>
                <Text color={palette.onButton} weight="semibold">{`${waiting} wait on you`}</Text>
              </Button>
            </Box>
          </Box>
        ) : null}
      </Box>
      <Box flexDirection="row" gap={1} marginTop={1}>
        {TABS.map(([view, label]) => (
          <Button key={`tab-${view}`} label={label} variant={shown === view ? 'primary' : 'secondary'} onPress={() => acts.show(view)} />
        ))}
      </Box>
    </Box>
  )
}
