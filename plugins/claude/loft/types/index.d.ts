// The Loft mod's state contract: the values Yard Client's hooks keep for the session in `$.state`.

/** The page's address for the branch's Yard Log, as the last `loft yard` command gave it. */
export type YardPage = string | null

declare module 'claude-code' {
  interface PluginState {
    loft: {
      yardPage: YardPage
    }
  }
}
