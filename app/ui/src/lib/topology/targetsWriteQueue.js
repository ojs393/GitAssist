// One chain per id. Callers may pass path and/or repo key so Overview
// pause, RepoDetail import, and DestinationDetail save serialize together.

const chains = new Map();

/** @param {string | (string | null | undefined)[]} idOrIds */
export function enqueueTargetsWrite(idOrIds, fn) {
  const ids = [
    ...new Set((Array.isArray(idOrIds) ? idOrIds : [idOrIds]).filter(Boolean)),
  ];
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

export async function waitForTargetsWrite(id) {
  if (!id) return;
  for (;;) {
    const pending = chains.get(id);
    if (!pending) return;
    await pending;
  }
}
