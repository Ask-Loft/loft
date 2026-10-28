import assert from 'node:assert/strict';
import { test } from 'node:test';
import { agentKey } from '../../yard-page/lib/format.js';

test('agents with identical native ids in different hosts have distinct page identities', () => {
  const claude = { host: 'claude-code', session: 'same', agent: 'sub' };
  const other = { ...claude, host: 'other-agent' };
  assert.equal(agentKey(claude), 'same/sub');
  assert.equal(agentKey(other), 'other-agent~same/sub');
  assert.notEqual(agentKey(claude), agentKey(other));
});
