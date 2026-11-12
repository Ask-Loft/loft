// The Loft mod's state contract: the values Yard Client's hooks keep for the session in `$.state`.

/** The page's address for the branch's Yard Log, as the last `loft yard` command gave it. */
export type YardPage = string | null

/** Who acted, as `loft yard read` names them: an agent by its host and session, or the person by name. */
export type PaneActor = Record<string, unknown>

/** When a block or a stage was listed, started and ended, where Yard Log records it. */
export type PaneTimes = { listed?: string; started?: string; ended?: string }

/** What a card's foot says of its block: who wrote its current version, its number, and when. */
export type PaneFoot = { by: PaneActor | null; number: number; created: string | null; updated: string | null }

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
}

/** A stage as the Yard pane keeps it: its own block, the agent that owns it and its blocks, in order. */
export type PaneStage = PaneBlock & { owner?: PaneActor | null; blocks: PaneBlock[] }

/** A submission the Yard pane knows of: its blocks, and when it was sent and settled. */
export type PaneSubmission = { key: string; blocks: string[]; sent: string | null; settled: string | null; isOpen: boolean }

/** What the Yard pane shows: which Yard Log, at which revision, and its stages. */
export type PaneView = {
  tree: string
  branch: string
  /** The key of the Yard Log shown; none where the branch has no Yard Log yet. */
  log: string | null
  revision: number
  page: string | null
  stages: PaneStage[]
  /** The blocks of the open submissions, by key, in each submission's order. */
  sent: string[]
  /** Every submission sent on the branch, oldest first. */
  submissions: PaneSubmission[]
}

/**
 * The Yard pane: its view once read, why the last read failed, if it did, and why the last answer
 * was not recorded, if it was not, until the next answer.
 */
export type YardPane = { view: PaneView | null; failure: string | null; notice: string | null }

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
