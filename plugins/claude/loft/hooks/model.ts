// The mod's pure helpers over what `loft` prints: a JSON document and its records, the words of
// the `loftCommand` setting, and a path within a tree. Nothing here runs a command or reads a file
// (the accepted decision "Yard Client is a mod over loft").

type Json = Record<string, unknown>

/** Whether a JSON value is an object, rather than an array, a scalar or null. */
export const isObject = (value: unknown): value is Json =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

/** The objects of a JSON array; none for anything else. */
export const records = (value: unknown): Json[] => (Array.isArray(value) ? value.filter(isObject) : [])

/** One document as `loft --format json` prints it, or undefined for anything else. */
export function parseDocument(stdout: string): Json | undefined {
  try {
    const value: unknown = JSON.parse(stdout)
    return isObject(value) && typeof value.outcome === 'string' ? value : undefined
  } catch {
    return undefined
  }
}

/** The words of the `loftCommand` setting: its first is the program, the rest its first arguments. */
export function commandWords(setting: string): string[] {
  const words = setting.trim().split(/\s+/).filter(word => word !== '')
  return words.length > 0 ? words : ['loft']
}

/** `path` from `root`, when it lies under it. */
export function relativeTo(root: string, path: string): string | undefined {
  const prefix = root.endsWith('/') ? root : `${root}/`
  return path.startsWith(prefix) ? path.slice(prefix.length) : undefined
}
