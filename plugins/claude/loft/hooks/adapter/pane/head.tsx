// The Yard pane's head, as the Yard page heads Yard Log: the tree above the Yard Log's title, set
// larger, a line that sums up its stages, a link to the page, and the count of what waits on the
// person in the waiting colour. Where Desktop's browser pane could not be reached, it says so beside
// the link (the Feature "Keep the Yard page beside the session").

import type { PaneView } from '../../../types'
import type { Drawing } from './kit'
import { kickerOf, linkOf, summaryOf } from './model'

/** The head; `isUnreached` where Desktop's browser pane could not be reached. */
export function head({ t, palette }: Drawing, view: PaneView, waiting: number, isUnreached: boolean) {
  const { Box, Text, Link } = t
  const link = view.page ? linkOf(view.page) : null
  return (
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
        <Box key="wait-count" backgroundColor={palette.wait} paddingX={1} alignSelf="flex-start">
          <Text color={palette.onButton} weight="semibold">{`${waiting} wait on you`}</Text>
        </Box>
      ) : null}
    </Box>
  )
}
