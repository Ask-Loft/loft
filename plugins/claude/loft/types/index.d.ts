// The Loft mod's state contract: what one read of a specification gives the Loft status pane,
// and the values the pane and Yard Client's hooks keep for the session in `$.state`.

/** A place in a file, as `loft` reports it. */
export type LoftSite = { path: string; line?: number; column?: number }

/** A node `loft status --base` lists as added, changed or removed. */
export type LoftNode = LoftSite & {
  key: string
  label: string | null
  kind: string
  name: string
}

/** A finding of `loft check` or `loft reconcile`, with the key of the node it is about. */
export type LoftFinding = {
  rule: string
  severity: string
  message: string
  key?: string
  label?: string | null
  path?: string
  line?: number
}

/** An error a `loft` command stopped on. */
export type LoftError = { command: string; message: string; path?: string; line?: number }

/** What `loft status --base` reports. */
export type LoftStatus = {
  lock: { publishSequence: number; matches: boolean; differs: string | null } | null
  branch: string | null
  modified: string[]
  base: { revision: string; commit: string; added: LoftNode[]; changed: LoftNode[]; removed: LoftNode[] } | null
}

/** One complete read: every command ran and printed its document. */
export type LoftRead = {
  status: LoftStatus
  canonical: boolean
  drift: string[]
  checkErrors: number
  checkFindings: LoftFinding[]
  traceFindings: LoftFinding[]
}

/** What the pane draws from. */
export type LoftSnapshot = {
  /** The directory that holds `spec/`, or null where the session has none. */
  root: string | null
  /** The last complete read, kept while a later one stops on an error. */
  read: LoftRead | null
  /** Why no read can run: `loftCommand` cannot be run or has no `status`. */
  notice: string | null
  /** The errors of the last read, empty when it was complete. */
  errors: LoftError[]
}

declare module 'claude-code' {
  interface PluginState {
    loft: {
      snapshot: LoftSnapshot | null
      /** The base `/loft-status <revision>` set for the session, or null for the setting. */
      base: string | null
      /** The file under `spec/` the agent last wrote, from the specification's directory. */
      workingOn: string | null
      /** The page's address for the branch's Yard Log, as the last `loft yard` command gave it. */
      yardPage: string | null
    }
  }
}
