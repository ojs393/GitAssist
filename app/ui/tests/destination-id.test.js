import { test } from 'node:test';
import assert from 'node:assert/strict';
import { encodeDestinationId, parseDestinationHash } from '../src/lib/topology/destinationId.js';

test('round-trip destination hash', () => {
  const id = encodeDestinationId('abc', 't1');
  assert.equal(id, 'abc/t1');
  assert.deepEqual(parseDestinationHash('destination/abc/t1'), { repoKey: 'abc', targetId: 't1' });
});

test('rejects bad hash', () => {
  assert.equal(parseDestinationHash('destination/only'), null);
});
