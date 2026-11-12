// The Loft mod's state contract: the values Yard Client's hooks keep for the session in `$.state`.

/** The page's address for the branch's Yard Log, as the last `loft yard` command gave it. */
export type YardPage = string | null

/** Who acted, as `loft yard read` names them: an agent by its host and session, or the person by name. */
export type PaneActor = Record<string, unknown>

/** When a block or a stage was listed, started and ended, where Yard Log records it. */
export type PaneTimes = { listed?: string; started?: string; ended?: string }

/** What a card's foot says of its block: who wrote its current version, its number, and when. */
export type PaneFoot = { by: PaneActor | null; number: number; created: string | null; updated: string | null }

/** A node a `spec-change` report names: how it changed, its kind, its label where it has one, and its words. */
export type PaneDiffNode = { change: string; kind: string; label: string | null; before: string | null; after: string | null }

/** What the pane keeps of a `spec-change` report: its first nodes, and how many more it names. */
export type PaneDiff = { nodes: PaneDiffNode[]; more: number }

/** A block as the Yard pane keeps it: only what the pane draws. */
export type PaneBlock = {
  id: string
  key: string
  kind: string
  version: { body: Record<string, unknown> }
  answer?: Record<string, unknown> | null
  outcome?: Record<string, unknown> | null
  foot?: PaneFoot
  times?: PaneTimes
  /** A `spec-change` block's report, as the pane draws it. */
  diff?: PaneDiff
}

/** A stage as the Yard pane keeps it: its own block, the agent that owns it and its blocks, in order. */
export type PaneStage = PaneBlock & { owner?: PaneActor | null; blocks: PaneBlock[] }

/** A submission the Yard pane knows of: its blocks, and when it was sent and settled. */
export type PaneSubmission = { key: string; blocks: string[]; sent: string | null; settled: string | null; isOpen: boolean }

/** What the Yard pane shows: which Yard Log, at which revision, and its stages. */
export type PaneView = {
  tree: string
  /** The repository's main worktree, where the read names git's common directory beside it. */
  repository: string | null
  branch: string
  /** The Yard Log's title: the branch's name unless it was given another. */
  title: string
  /** The key of the Yard Log shown; none where the branch has no Yard Log yet. */
  log: string | null
  revision: number
  page: string | null
  stages: PaneStage[]
  /** The blocks of the open submissions, by key, the oldest submission first, each in its own order. */
  sent: string[]
  /** The open submissions, by key, the oldest first. */
  asked: string[]
  /** Every submission sent on the branch, oldest first. */
  submissions: PaneSubmission[]
}

/**
 * The Yard pane: its view once read, why the last read failed, if it did, and why the last answer
 * was not recorded, if it was not, until the next answer.
 */
/** Claude Code Desktop's browser pane, as the pane last left it: the tab showing the Yard page, and whether it was reached. */
export type PaneBrowser = { tab: string | null; isReached: boolean }

export type YardPane = { view: PaneView | null; failure: string | null; notice: string | null; browser: PaneBrowser | null }

/** The views of the Yard pane: the page's timeline and Gantt chart, and what waits on the person. */
export type PaneShown = 'timeline' | 'gantt' | 'waiting'

/** What the person has set in the Yard pane and not sent: the stages unfolded, options ticked, words typed. */
export type YardPaneDrafts = {
  /** The view shown, and the one to show again once nothing waits. */
  shown: PaneShown
  previous: PaneShown
  /** The block what waits is shown at, by key, where the person asked for one. */
  at: string | null
  /** The open submissions the pane has shown, by key, so that only a new one shows what waits by itself. */
  seen: string[]
  /** The stage the person chose, by key; none until they choose one. */
  chosen: string | null
  /** The stages whose row the person opened or folded, against its default: open for the stage chosen. */
  toggled: string[]
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
