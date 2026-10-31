import { test } from 'node:test';
import assert from 'node:assert/strict';
import { layoutTopology } from '../src/lib/topology/layout.js';

test('one destination centers above local', () => {
  const L = layoutTopology({
    width: 800, height: 500,
    destinations: [{ id: 'a', label: 'pub', paused: false }],
  });
  assert.ok(Math.abs(L.local.x - 400) < 1);
  assert.ok(Math.abs(L.nodes[0].x - 400) < 1);
  assert.ok(L.nodes[0].y < L.local.y);
  assert.equal(L.edges.length, 1);
});

test('many destinations wrap to second row', () => {
  const dests = Array.from({ length: 12 }, (_, i) => ({ id: String(i), label: `r${i}`, paused: false }));
  const L = layoutTopology({ width: 400, height: 600, destinations: dests });
  const ys = new Set(L.nodes.map((n) => Math.round(n.y)));
  assert.ok(ys.size >= 2);
});
