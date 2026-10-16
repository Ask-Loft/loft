// The documents `loft` prints, as the pane's tests answer its commands with them: a branch that
// added one criterion and changed another, with one finding of `loft check` at warning and one
// unreviewed criterion.

export const ROOT = '/work/shop'
export const FEATURE = 'spec/products/shop/basket/add-an-item.yaml'

const node = (key: string, label: string | null, kind: string, line: number, name: string) => ({
  key,
  label,
  kind,
  path: FEATURE,
  line,
  column: 14,
  name,
})

export const ADDED = node('k-added', null, 'criterion', 20, 'An added item shows in the basket.')
export const CHANGED = node('k-changed', 'AC-4', 'criterion', 12, 'An item added twice counts two.')
export const REQUIREMENT = node('k-rq', 'RQ-2', 'requirement', 8, 'The basket shall hold each item added.')

export type Variant = {
  modified?: string[]
  canonical?: boolean
  checkErrors?: number
  warnings?: number
  open?: string[]
  changed?: string[]
  broken?: boolean
  matches?: boolean
  added?: object[]
}

export const status = (variant: Variant = {}) => ({
  schema: 'loft-status/1',
  outcome: 'reported',
  lock: {
    publishSequence: 3,
    publishedAt: '2026-10-01T00:00:00Z',
    stateRoot: 'ab'.repeat(32),
    matches: variant.matches ?? false,
    differs: variant.matches ? null : 'contentRoot',
  },
  repository: { branch: 'feature/basket', modified: variant.modified ?? [], stopped: null },
  base: {
    revision: 'develop',
    commit: '1f9300b9695e4199854cf4962a89e94773ab9a47',
    added: variant.added ?? [ADDED],
    changed: [REQUIREMENT, CHANGED],
    removed: [node('k-removed', 'AC-9', 'criterion', 30, 'An empty basket says so.')],
  },
  errors: [],
})

export const fmt = (canonical = true) => ({
  schema: 'loft-fmt/1',
  outcome: canonical ? 'canonical' : 'drift',
  changed: canonical ? [] : [FEATURE],
  minted: 0,
  written: false,
  errors: [],
})

export const check = (errors = 0, warnings = 1) => ({
  schema: 'loft-check/1',
  outcome: errors > 0 || warnings > 0 ? 'failed' : 'passed',
  findings: [
    ...Array.from({ length: warnings }, () => ({
      rule: 'LQ-002',
      severity: 'warning',
      message: 'The text contains a vague quantifier.',
      path: FEATURE,
      line: 20,
      subjects: [{ key: 'k-added' }],
    })),
    { rule: 'LQ-005', severity: 'info', message: 'More than one statement.', subjects: [{ key: 'k-rq' }] },
  ],
  errors: Array.from({ length: errors }, () => ({
    kind: 'validation',
    message: 'a reference names nothing',
    path: FEATURE,
    line: 3,
    sites: [{ path: FEATURE, line: 3 }],
  })),
})

export const reconcile = (open: string[] = ['k-added'], broken = false, changed: string[] = []) => ({
  schema: 'loft-reconcile/2',
  outcome: 'passed',
  inScope: 10,
  linked: 9,
  accepted: [],
  findings: [
    ...open.map(key => ({
      rule: 'TR-001',
      severity: 'info',
      key,
      label: null,
      path: FEATURE,
      message: 'no link names this node',
    })),
    ...changed.map(key => ({
      rule: 'TR-006',
      severity: 'warning',
      key,
      label: null,
      path: FEATURE,
      message: 'the test changed after it was recorded',
    })),
    ...(broken
      ? [{ rule: 'TR-004', severity: 'warning', key: 'k-other', label: 'AC-1', path: 'spec/x.yaml', message: 'the test is missing' }]
      : []),
  ],
  tests: [],
  errors: [],
})

/** What every command prints for a read of `variant`. */
export const documents = (variant: Variant = {}) => ({
  status: status(variant),
  fmt: fmt(variant.canonical ?? true),
  check: check(variant.checkErrors ?? 0, variant.warnings ?? 1),
  reconcile: reconcile(variant.open ?? ['k-added'], variant.broken ?? false, variant.changed ?? []),
})
