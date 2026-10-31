import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  blankTarget,
  importableRemotes,
  buildImportedTargets,
} from '../src/lib/topology/importTargets.js';

const repo = {
  path: '/tmp/app',
  slug: 'octo/app',
  visibility: 'public',
  remotes: [
    { name: 'origin', url: 'git@github.com:octo/app.git' },
    { name: 'private', url: 'git@github.com:octo/app-private.git', visibility: 'private' },
  ],
};

test('blank target matches the editor shape', () => {
  const t = blankTarget();
  assert.equal(t.enabled, true);
  assert.equal(t.role, 'custom');
  assert.equal(t.push_policy, 'manual');
  assert.equal(t.include_in_push_all, true);
  assert.deepEqual(t.publish_filter, []);
  assert.equal(t.use_mirror, false);
  assert.equal(t.tabs, null);
  assert.equal(t.branches, null);
  assert.deepEqual(t.checks, { enabled: false, on_fail: 'block', timeout_secs: 600, commands: [] });
});

test('skips remotes that already have a target, including paused ones', () => {
  const existing = [
    { remote_name: 'origin', label: 'personal', enabled: false },
  ];
  const before = existing.length;
  const list = importableRemotes(repo, existing);
  assert.deepEqual(list.map((r) => r.name), ['private']);
  assert.equal(existing.length, before);
});

test('confirm builds one target per selected remote and does not touch the old list', () => {
  const existing = [{ id: 't0', remote_name: 'upstream', label: 'app', role: 'custom', enabled: true }];
  const made = buildImportedTargets(repo, existing, repo.remotes, 1000);
  assert.equal(existing.length, 1);
  assert.equal(made.length, 2);
  assert.equal(made[0].label, 'app-2');
  assert.equal(made[0].role, 'public');
  assert.equal(made[0].remote_name, 'origin');
  assert.equal(made[0].url, 'git@github.com:octo/app.git');
  assert.equal(made[0].enabled, true);
  assert.equal(made[1].label, 'private');
  assert.equal(made[1].role, 'personal');
  assert.equal(made[1].enabled, true);
  assert.notEqual(made[0].id, made[1].id);
  assert.equal(made[0].checks.timeout_secs, 600);
  assert.equal(made[0].push_policy, 'manual');
});

test('unknown visibility stays custom', () => {
  const local = { slug: null, visibility: 'unknown', remotes: [{ name: 'origin', url: '' }] };
  const made = buildImportedTargets(local, [], local.remotes, 5);
  assert.equal(made[0].label, 'origin');
  assert.equal(made[0].role, 'custom');
});
