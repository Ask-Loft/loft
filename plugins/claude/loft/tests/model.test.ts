import { expect, test } from 'claude-code/testing'

import { commandWords, parseDocument } from '../hooks/model'

test('loftCommand is loft by default, and a setting of several words runs its first with the others', () => {
  expect(commandWords('')).toEqual(['loft'])
  expect(commandWords('pnpm exec loft')).toEqual(['pnpm', 'exec', 'loft'])
})

test('a document is one JSON object with an outcome', () => {
  expect(parseDocument('{"outcome":"canonical"}')).toEqual({ outcome: 'canonical' })
  expect(parseDocument('not json')).toBeUndefined()
  expect(parseDocument('[1]')).toBeUndefined()
})
