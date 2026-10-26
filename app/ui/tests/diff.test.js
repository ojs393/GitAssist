import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lineDiff, relativeTime } from '../src/lib/diff.js';

test('removed attribution lines show as deletions', () => {
  const d = lineDiff('fix: x\n\nbody\n\nCo-authored-by: a <b>', 'fix: x\n\nbody');
  assert.deepEqual(d.filter((l) => l.kind === 'del').map((l) => l.text), ['', 'Co-authored-by: a <b>']);
  assert.equal(d.filter((l) => l.kind === 'add').length, 0);
});

test('wrapped lines show as one deletion and several additions', () => {
  const d = lineDiff('a b c d', 'a b\nc d');
  assert.deepEqual(d.map((l) => l.kind), ['del', 'add', 'add']);
});

test('identical text has no changes and ignores CRLF and trailing blanks', () => {
  const d = lineDiff('x\r\ny\n\n', 'x\ny');
  assert.ok(d.every((l) => l.kind === 'same'));
});

test('empty inputs', () => {
  assert.deepEqual(lineDiff('', ''), []);
  assert.deepEqual(lineDiff(undefined, 'a'), [{ kind: 'add', text: 'a' }]);
});

test('relative time', () => {
  assert.equal(relativeTime(1000, 1030), 'just now');
  assert.equal(relativeTime(1000, 1000 + 125), '2 min ago');
  assert.equal(relativeTime(1000, 1000 + 7300), '2 h ago');
  assert.equal(relativeTime(0, 90000), '1 day ago');
  assert.equal(relativeTime(0, 86400 * 3), '3 days ago');
});
