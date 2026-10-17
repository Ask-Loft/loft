import { expect, test } from 'claude-code/testing'

import {
  ancestors,
  checks,
  commandWords,
  criteriaToVerify,
  isLoftSpecSkill,
  nextStep,
  nodesIn,
  parseDocument,
  readOf,
  runsLoftOrGit,
  statusLine,
  touchesSpecification,
  window,
} from '../hooks/model'
import { FEATURE, ROOT, type Variant, documents } from './fixtures'

const readWith = (variant: Variant = {}) => {
  const d = documents(variant)
  return readOf(d.status, d.fmt, d.check, d.reconcile)
}

const checkOf = (variant: Variant, id: string) => checks(readWith(variant)).find(check => check.id === id)!

test('the checks come in the order Format, Check, Spec committed, Trace and Lock', () => {
  expect(checks(readWith()).map(check => check.label)).toEqual(['Format', 'Check', 'Spec committed', 'Trace', 'Lock'])
})

test('Format passes when loft fmt --check reports the outcome canonical', () => {
  expect(checkOf({ canonical: true }, 'format').passes).toBe(true)
  expect(checkOf({ canonical: false }, 'format').passes).toBe(false)
})

test('Check passes when loft check reports no error and no finding at warning, and shows the count of each', () => {
  expect(checkOf({ checkErrors: 0, warnings: 0 }, 'check').passes).toBe(true)
  expect(checkOf({ checkErrors: 0, warnings: 1 }, 'check').passes).toBe(false)
  expect(checkOf({ checkErrors: 2, warnings: 0 }, 'check').passes).toBe(false)
  expect(checkOf({ checkErrors: 2, warnings: 1 }, 'check').detail).toBe('2 errors · 1 warning')
})

test('Spec committed passes when loft status lists no uncommitted path under spec', () => {
  expect(checkOf({ modified: [] }, 'committed').passes).toBe(true)
  expect(checkOf({ modified: [FEATURE] }, 'committed').passes).toBe(false)
})

test('Trace does not pass while a criterion the branch added or changed has an open finding', () => {
  expect(checkOf({ open: ['k-added'] }, 'trace').passes).toBe(false)
  expect(checkOf({ open: [] }, 'trace').passes).toBe(true)
  // A test changed after it was recorded asks for its review again.
  expect(checkOf({ open: [], changed: ['k-added'] }, 'trace').passes).toBe(false)
  // So does a recorded test that is no longer located.
  expect(checkOf({ open: [], lost: ['k-added'] }, 'trace').passes).toBe(false)
  // A finding on a node the branch did not touch is not this branch's work.
  expect(checkOf({ open: ['k-elsewhere'] }, 'trace').passes).toBe(true)
})

test('Trace does not pass while a TR-004, TR-005 or TR-010 finding stands anywhere in the trace', () => {
  expect(checkOf({ open: [], broken: true }, 'trace').passes).toBe(false)
  expect(checkOf({ open: [], unparsed: true }, 'trace').passes).toBe(false)
})

test('Lock passes when the tree matches the lock, and otherwise says the tree holds unpublished changes', () => {
  expect(checkOf({ matches: true }, 'lock').passes).toBe(true)
  const lock = checkOf({ matches: false }, 'lock')
  expect(lock.passes).toBe(false)
  expect(lock.detail).toBe('unpublished changes since publish 3')
})

test('the next step is the action of the first check that does not pass', () => {
  const clean: Variant = { canonical: true, warnings: 0, modified: [], open: [], matches: true }
  expect(nextStep(checks(readWith({ ...clean, canonical: false, warnings: 1 })))).toBe('run `loft fmt`')
  expect(nextStep(checks(readWith({ ...clean, warnings: 1 })))).toBe('fix what `loft check` reports')
  expect(nextStep(checks(readWith({ ...clean, modified: [FEATURE] })))).toBe('commit `spec/`')
  expect(nextStep(checks(readWith({ ...clean, open: ['k-added'] })))).toBe(
    'link and review the tests of the criteria to verify',
  )
  expect(nextStep(checks(readWith({ ...clean, matches: false })))).toBe(
    'merge the branch, then publish on the publish branch',
  )
  expect(nextStep(checks(readWith(clean)))).toBe('nothing left to do')
})

test('the criteria to verify are the criteria the branch added or changed, in file order', () => {
  const toVerify = criteriaToVerify(readWith({ open: ['k-added'] }))
  expect(toVerify.map(each => each.node.key)).toEqual(['k-changed', 'k-added'])
  expect(toVerify[0]!.open).toBeUndefined()
  expect(toVerify[1]!.open?.message).toBe('no link names this node')
  // A criterion whose test changed after it was recorded is not reviewed.
  const changed = criteriaToVerify(readWith({ open: [], changed: ['k-changed'] }))
  expect(changed[0]!.open?.rule).toBe('TR-006')
  expect(changed[1]!.open).toBeUndefined()
  // Nor is one whose recorded test is no longer located.
  const lost = criteriaToVerify(readWith({ open: [], lost: ['k-changed'] }))
  expect(lost[0]!.open?.rule).toBe('TR-009')
})

test('the nodes a file holds are the added and changed nodes whose path it is', () => {
  const inFile = nodesIn(readWith(), FEATURE)
  expect(inFile.map(each => `${each.change} ${each.node.key}`)).toEqual([
    'added k-added',
    'changed k-rq',
    'changed k-changed',
  ])
  expect(nodesIn(readWith(), 'spec/elsewhere.yaml')).toEqual([])
})

test('the status line names the branch and gives the counts of the delta, the check and the trace', () => {
  expect(statusLine(readWith())).toBe('feature/basket · +1 ~2 −1 · check 0e/1w · trace 1/2')
})

test('a list longer than its room ends with a count of what it leaves out', () => {
  expect(window([1, 2, 3, 4, 5], 3)).toEqual({ shown: [1, 2], more: 3 })
  expect(window([1, 2], 3)).toEqual({ shown: [1, 2], more: 0 })
})

test('loftCommand is loft by default, and a setting of several words runs its first with the others', () => {
  expect(commandWords('')).toEqual(['loft'])
  expect(commandWords('pnpm exec loft')).toEqual(['pnpm', 'exec', 'loft'])
})

test('the directories looked in run from the working directory up to the root', () => {
  expect(ancestors('/work/shop/src')).toEqual(['/work/shop/src', '/work/shop', '/work', '/'])
})

test('a write of a file under spec or of the trace can change what the pane reads', () => {
  expect(touchesSpecification(ROOT, `${ROOT}/${FEATURE}`)).toBe(true)
  expect(touchesSpecification(ROOT, `${ROOT}/loft.trace.yaml`)).toBe(true)
  expect(touchesSpecification(ROOT, `${ROOT}/src/main.rs`)).toBe(false)
  expect(touchesSpecification(ROOT, `/elsewhere/${FEATURE}`)).toBe(false)
})

test('a shell command that runs loft or git starts a read', () => {
  for (const command of ['loft fmt', 'git commit -m x', 'cd a && loft check', 'npx --no-install loft fmt']) {
    expect(runsLoftOrGit(command)).toBe(true)
  }
  for (const command of ['cargo test', 'echo loftier', 'gitk']) expect(runsLoftOrGit(command)).toBe(false)
})

test('the loft-spec skill is known by any plugin prefix', () => {
  expect(isLoftSpecSkill('loft-spec')).toBe(true)
  expect(isLoftSpecSkill('loft:loft-spec')).toBe(true)
  expect(isLoftSpecSkill('opsx:apply')).toBe(false)
})

test('a document is one JSON object with an outcome', () => {
  expect(parseDocument('{"outcome":"canonical"}')).toEqual({ outcome: 'canonical' })
  expect(parseDocument('not json')).toBeUndefined()
  expect(parseDocument('[1]')).toBeUndefined()
})
