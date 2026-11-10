import { expect, test } from 'claude-code/testing'

import { classOf, shellReads } from '../hooks/adapter/consult'
import { loftSubcommands } from '../hooks/adapter/shell'
import { CASES as cases } from './consult-cases'

test('each path of the shared cases takes its class', () => {
  for (const { path, dir, class: expected } of cases.paths) {
    expect([path, dir, classOf(path, dir, cases.trees)]).toEqual([path, dir, expected])
  }
})

test('each shell command of the shared cases reads its classes and names its loft subcommands', () => {
  for (const { command, dir, reads, loft } of cases.shell) {
    expect([command, shellReads(command, dir, cases.trees), loftSubcommands(command)]).toEqual([command, reads, loft])
  }
})
