// Blank publish target. Same fields as TargetsModal.blank().
// Ids are set here: the server stamps empty ids with the current millisecond,
// so two new targets in one save can collide.

export function blankTarget() {
  return {
    id: '',
    label: '',
    role: 'custom',
    remote_name: '',
    url: '',
    enabled: true,
    push_policy: 'manual',
    include_in_push_all: true,
    publish_filter: [],
    use_mirror: false,
    tabs: null,
    branches: null,
    checks: { enabled: false, on_fail: 'block', timeout_secs: 600, commands: [] },
  };
}

function clampLabel(raw) {
  let s = String(raw ?? '').trim().replace(/[^A-Za-z0-9._-]/g, '-');
  while (s.includes('..')) s = s.replaceAll('..', '.');
  s = s.replace(/^\.+|\.+$/g, '');
  if (!s) s = 'remote';
  return s.slice(0, 64);
}

function uniqueLabel(base, taken) {
  let label = base.slice(0, 64);
  let n = 2;
  while (taken.has(label.toLowerCase())) {
    const suffix = `-${n}`;
    label = `${base.slice(0, Math.max(1, 64 - suffix.length))}${suffix}`;
    n += 1;
  }
  taken.add(label.toLowerCase());
  return label;
}

/** Remote name, or the repo slug when the remote is origin or unnamed. */
export function labelForRemote(remote, repo) {
  const name = String(remote?.name ?? '').trim();
  if (name && name !== 'origin') return clampLabel(name);
  const slug = String(remote?.slug || repo?.slug || '')
    .split('/')
    .filter(Boolean)
    .pop();
  if (slug) return clampLabel(slug);
  return clampLabel(name || 'remote');
}

/** public → public, private → personal. Only this remote's visibility counts. */
export function roleForRemote(remote) {
  const vis = remote?.visibility;
  if (vis === 'public') return 'public';
  if (vis === 'private') return 'personal';
  return 'custom';
}

/** Remotes that do not already have a target, matched on remote_name. */
export function importableRemotes(repo, existingTargets) {
  const taken = new Set(
    (existingTargets ?? [])
      .map((t) => t?.remote_name)
      .filter((n) => typeof n === 'string' && n.length),
  );
  const seen = new Set();
  const out = [];
  for (const remote of repo?.remotes ?? []) {
    if (!remote?.name || taken.has(remote.name) || seen.has(remote.name)) continue;
    seen.add(remote.name);
    out.push(remote);
  }
  return out;
}

export function planImports(repo, existingTargets, remotes) {
  const taken = new Set(
    (existingTargets ?? []).map((t) => String(t?.label ?? '').toLowerCase()).filter(Boolean),
  );
  return (remotes ?? []).map((remote) => ({
    remote,
    remote_name: remote.name,
    url: remote.url || '',
    label: uniqueLabel(labelForRemote(remote, repo), taken),
    role: roleForRemote(remote),
  }));
}

export function buildImportedTargets(repo, existingTargets, remotes, now = Date.now()) {
  const usedIds = new Set((existingTargets ?? []).map((t) => t?.id).filter(Boolean));
  return planImports(repo, existingTargets, remotes).map((row, i) => {
    const target = blankTarget();
    let id = `t${now}-${i}`;
    while (usedIds.has(id)) id = `${id}x`;
    usedIds.add(id);
    target.id = id;
    target.label = row.label;
    target.role = row.role;
    target.remote_name = row.remote_name;
    target.url = row.url;
    target.enabled = true;
    return target;
  });
}
