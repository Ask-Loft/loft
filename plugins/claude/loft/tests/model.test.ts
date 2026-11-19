import { expect, test } from 'claude-code/testing'

import { commandWords, parseDocument } from '../hooks/model'
import { isListenAddress } from '../hooks/loft'

test('loftCommand is loft by default, and a setting of several words runs its first with the others', () => {
  expect(commandWords('')).toEqual(['loft'])
  expect(commandWords('pnpm exec loft')).toEqual(['pnpm', 'exec', 'loft'])
})

test('a document is one JSON object with an outcome', () => {
  expect(parseDocument('{"outcome":"canonical"}')).toEqual({ outcome: 'canonical' })
  expect(parseDocument('not json')).toBeUndefined()
  expect(parseDocument('[1]')).toBeUndefined()
})

test('an address is told from anything else as loft yard serve --listen reads one, the unspecified IPv6 address refused', () => {
  for (const address of ['100.64.0.1', '0.0.0.0', '::1', 'fd7a:115c:a1e0::1', '::ffff:192.168.1.5']) expect(isListenAddress(address)).toBe(true)
  for (const text of ['machine.example.ts.net', '999.1.1.1', '01.2.3.4', '1.2.3', '::1]/x[', 'fe80::1%en0', '', '::', '::0', '0:0:0:0:0:0:0:0', '::ffff:0.0.0.0']) expect(isListenAddress(text)).toBe(false)
})
