// The Loft mod's state contract: the values Yard Client's hooks keep for the session in `$.state`.

/** The page's address for the branch's Yard Log, as the last `loft yard` command gave it. */
export type YardPage = string | null

/** A block as the Yard pane keeps it: only what the pane draws. */
export type PaneBlock = {
  id: string
  key: string
  kind: string
  version: { body: Record<string, unknown> }
  answer?: Record<string, unknown> | null
  outcome?: Record<string, unknown> | null
}

/** A stage as the Yard pane keeps it: its own block and its blocks, in order. */
export type PaneStage = PaneBlock & { blocks: PaneBlock[] }

/** What the Yard pane shows: which Yard Log, at which revision, and its stages. */
export type PaneView = {
  tree: string
  branch: string
  /** The key of the Yard Log shown; none where the branch has no Yard Log yet. */
  log: string | null
  hasLog: boolean
  revision: number
  page: string | null
  stages: PaneStage[]
  /** The blocks of the open submissions, by key, in each submission's order. */
  sent: string[]
}

/** The Yard pane: its view once read, and why the last read or answer failed, if one did. */
export type YardPane = { view: PaneView | null; failure: string | null }

/** What the person has set in the Yard pane and not sent: the stages unfolded, options ticked, words typed. */
export type YardPaneDrafts = {
  unfolded: string[]
  ticked: Record<string, string[]>
  words: Record<string, string>
}

declare module 'claude-code' {
  interface PluginState {
    loft: {
      yardPage: YardPage
      yardPane: Shaped<YardPane>
      yardPaneDrafts: Shaped<YardPaneDrafts>
    }
  }
}
