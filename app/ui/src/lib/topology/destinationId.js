export function encodeDestinationId(repoKey, targetId) {
  return `${repoKey}/${targetId}`;
}

export function parseDestinationHash(hash) {
  const m = /^destination\/([^/]+)\/([^/]+)$/.exec(hash);
  return m ? { repoKey: m[1], targetId: m[2] } : null;
}
