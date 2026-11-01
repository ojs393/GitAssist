// One chain per id. Callers pass path and/or repo key; we resolve the missing
// side so Overview (path), RepoDetail, and DestinationDetail (key) share a chain.

import { api } from '../api.js';

const chains = new Map();

function normalizeIds(idOrIds) {
  return [...new Set((Array.isArray(idOrIds) ? idOrIds : [idOrIds]).filter(Boolean))];
}

/** Fill in missing path or key from the targets APIs. */
export async function resolveTargetsIds({ path, key } = {}) {
  let p = (path || '').trim();
  let k = (key || '').trim();
  if (p && k) return { path: p, key: k };
  if (p && !k) {
    try {
      const rows = await api.targetsListAll([p]);
      k = rows?.[0]?.repo_key || '';
    } catch {
      // Keep path-only if list fails (e.g. first create, no file yet).
    }
  } else if (k && !p) {
    try {
      const doc = await api.targetsGetByKey(k);
      p = doc?.repo_path || '';
    } catch {
      // Keep key-only if lookup fails.
    }
  }
  return { path: p, key: k };
}

/** @param {string | (string | null | undefined)[]} idOrIds */
export function enqueueTargetsWrite(idOrIds, fn) {
  const ids = normalizeIds(idOrIds);
  if (!ids.length) return Promise.resolve().then(fn);
  const prev = Promise.all(ids.map((id) => chains.get(id) ?? Promise.resolve()));
  const run = prev.then(
    () => fn(),
    () => fn(),
  );
  const settled = run.then(
    () => {},
    () => {},
  );
  for (const id of ids) chains.set(id, settled);
  settled.finally(() => {
    for (const id of ids) {
      if (chains.get(id) === settled) chains.delete(id);
    }
  });
  return run;
}

/** Resolve path+key when needed, then enqueue under both. */
export async function enqueueTargetsWriteForRepo({ path, key } = {}, fn) {
  const ids = await resolveTargetsIds({ path, key });
  return enqueueTargetsWrite([ids.path, ids.key], fn);
}

/** Wait until no write is pending for any of the given ids. */
export async function waitForTargetsWrite(idOrIds) {
  const ids = normalizeIds(idOrIds);
  if (!ids.length) return;
  for (;;) {
    const pending = ids.map((id) => chains.get(id)).filter(Boolean);
    if (!pending.length) return;
    await Promise.all(pending);
  }
}

/** Resolve path+key when needed, then wait on both. */
export async function waitForTargetsWriteForRepo({ path, key } = {}) {
  const ids = await resolveTargetsIds({ path, key });
  return waitForTargetsWrite([ids.path, ids.key]);
}
