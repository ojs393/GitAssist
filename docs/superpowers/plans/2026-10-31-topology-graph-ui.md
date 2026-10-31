# Topology Graph UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Overview cards and Repo Detail Work/Publish lists with an animated local→destination topology, dedicated local/destination detail pages, edge pause, and obvious multi-remote import — without losing TargetsModal capabilities or inventing auto-created targets.

**Architecture:** Custom SVG topology (computed layout, no drag). Destinations = existing `PublishTarget`s. Pause = `enabled: false`. Overview aggregates targets across tracked repos; working-copy graph shows that repo’s targets. New hash routes `#local` and `#destination/<repoKey>/<targetId>`. Small gateway addition for per-remote ahead/behind.

**Tech Stack:** Svelte 5 + Vite (app UI), existing `app.css` tokens/animations, Tauri commands, Rust `gitassist-gateway` (`work`, `targets`), Node `--test` for pure layout helpers, `cargo test` for gateway.

**Spec:** `docs/superpowers/specs/2026-10-31-topology-graph-ui-design.md`

## Global Constraints

- No auto-create of publish targets from remotes; import/add is explicit and multi-select.
- Create-new-GitHub-repo is out of scope (follow-up).
- Destination pause must not flip global `config.enabled`; global pause stays on local detail / tray.
- Preserve every TargetsModal field (identity, policy, filter, rules, checks) on Destination Detail.
- Push still uses target **label** via `api.jobStartPublish(path, label | '--all')`.
- Destination route id uses `targets::key_for(repo)` + `PublishTarget.id` (not label — labels change).
- No network crates; git subprocesses only for ahead/behind.
- Do not commit to `main`; work on a feature branch and open a PR when asked.
- Commit messages: GitAssist rules (terse, imperative, ≤500 chars total, no Co-authored-by / signatures).
- Never set `GITASSIST_BYPASS` or `--no-verify`.

---

## File Structure

```
app/ui/src/
  App.svelte                          # parse #local, #destination/<repoKey>/<targetId>
  lib/
    api.js                            # targetsListAll?, repoWorkRemote
    topology/
      layout.js                       # pure layout math (tested)
      TopologyGraph.svelte            # SVG graph
      EdgePausePopup.svelte           # pause/resume popup
      destinationId.js                # encode/decode route ids
    pages/
      Overview.svelte                 # host machine-wide graph (+ keep Setup/health strip if needed)
      LocalDetail.svelte              # NEW local commit / global pause
      DestinationDetail.svelte        # NEW full target config + actions
      RepoDetail.svelte               # working-copy graph instead of Work/Publish panels
      TargetsModal.svelte             # retire as primary UI; keep helpers or delete after parity
      Repos.svelte                    # may still open working-copy graph; not home
    app.css                           # topology / node / edge / flow styles
app/src-tauri/src/commands.rs         # register repo_work_remote, targets_list_all
crates/gateway/src/
  work.rs                             # RemoteSync + status_vs_remote
  targets.rs                          # list_all helper (optional thin wrapper)
app/ui/tests/
  topology-layout.test.js
  destination-id.test.js
crates/gateway/src/work.rs            # unit tests for per-remote counts
```

**Route / id helpers (lock names early):**

```js
// destinationId.js
export function encodeDestinationId(repoKey, targetId) {
  return `${repoKey}/${targetId}`;
}
export function parseDestinationHash(hash) {
  // hash like "destination/abc123/t1730..."
  const m = /^destination\/([^/]+)\/([^/]+)$/.exec(hash);
  return m ? { repoKey: m[1], targetId: m[2] } : null;
}
export function localHash() { return 'local'; }
```

```js
// layout.js — inputs/outputs Task 2 locks
export function layoutTopology(opts) {
  // opts: { width, height, destinations: [{ id, label, paused, ahead, behind, visibility }] }
  // returns: { local: {x,y,r}, nodes: [{ id, x, y, r, ... }], edges: [{ id, d, paused }] }
}
```

---

### Task 1: Per-remote ahead/behind API

**Files:**
- Modify: `crates/gateway/src/work.rs`
- Modify: `app/src-tauri/src/commands.rs` (add `repo_work_remote`)
- Modify: `app/ui/src/lib/api.js` (`repoWorkRemote`)
- Test: unit tests in `crates/gateway/src/work.rs` `mod tests`

**Interfaces:**
- Consumes: existing `git()` helper in `work.rs`
- Produces:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSync {
    pub remote: String,
    pub branch: String,
    pub ref_name: Option<String>, // e.g. origin/main when known
    pub ahead: u32,
    pub behind: u32,
}

/// Compare HEAD to `remote/branch` if that ref exists; else ahead vs remote tips for that remote only.
pub fn status_vs_remote(repo: &Path, remote: &str, branch: &str) -> Result<RemoteSync>
```

```js
// api.js
repoWorkRemote: (path, remote, branch) =>
  invoke('repo_work_remote', { path, remote, branch }),
```

- [ ] **Step 1: Write failing Rust tests**

In `work.rs` tests (temp git repo with two remotes or one remote + branch):

```rust
#[test]
fn status_vs_remote_counts_ahead() {
    // init repo, commit, add remote pointing at bare, push, make local commit
    // assert status_vs_remote(&repo, "origin", "main").ahead >= 1
}
```

If fixture setup is heavy, mirror style of existing `clean_repo_summary` and keep one focused test that parses `rev-list` output via a small internal `parse_left_right` helper tested with string fixtures:

```rust
#[test]
fn parse_left_right_swapped_order() {
    assert_eq!(parse_left_right("3\t1\n"), Some((1, 3))); // ahead=1 behind=3 from "@{u}...HEAD" style
}
```

For remote-specific path, test that missing ref returns `ahead/behind` best-effort without panic:

```rust
#[test]
fn missing_remote_ref_is_zeroish() {
    // repo without that remote → RemoteSync { ahead: 0, behind: 0, ref_name: None, ... }
}
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `cargo test -p gitassist-gateway status_vs_remote parse_left_right -- --nocapture`  
Expected: FAIL (symbols missing) or compile error.

- [ ] **Step 3: Implement `status_vs_remote`**

```rust
pub fn status_vs_remote(repo: &Path, remote: &str, branch: &str) -> Result<RemoteSync> {
    let tip = format!("{remote}/{branch}");
    let has_tip = git(repo, &["rev-parse", "--verify", &tip]).is_some();
    let (ahead, behind) = if has_tip {
        git(repo, &["rev-list", "--left-right", "--count", &format!("{tip}...HEAD")])
            .and_then(|s| parse_left_right(&s))
            .unwrap_or((0, 0))
    } else {
        // commits not reachable from this remote's tips
        let ahead = git(
            repo,
            &["rev-list", "--count", "HEAD", "--not", "--remotes={remote}"],
        )
        // NOTE: pass format!("--remotes={remote}") as one arg
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
        (ahead, 0)
    };
    Ok(RemoteSync {
        remote: remote.to_string(),
        branch: branch.to_string(),
        ref_name: has_tip.then_some(tip),
        ahead,
        behind,
    })
}
```

Fix the `--remotes=` arg to a single owned `String` in the real code.

- [ ] **Step 4: Wire Tauri + api.js**

Add `repo_work_remote` in `commands.rs` mirroring `repo_work`. Export `api.repoWorkRemote`.

- [ ] **Step 5: Run tests — PASS**

Run: `cargo test -p gitassist-gateway parse_left_right status_vs_remote`  
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/gateway/src/work.rs app/src-tauri/src/commands.rs app/ui/src/lib/api.js
git commit -m "$(cat <<'EOF'
Add per-remote ahead/behind work status.

Destination nodes need sync vs a specific remote, not only @{u}.
EOF
)"
```

---

### Task 2: Destination id helpers + topology layout (pure JS)

**Files:**
- Create: `app/ui/src/lib/topology/destinationId.js`
- Create: `app/ui/src/lib/topology/layout.js`
- Create: `app/ui/tests/destination-id.test.js`
- Create: `app/ui/tests/topology-layout.test.js`
- Modify: `app/ui/package.json` test script to include new tests

**Interfaces:**
- Produces: `encodeDestinationId`, `parseDestinationHash`, `layoutTopology` as above
- Layout rules from spec §4: local bottom-center; nodes evenly spaced; wrap second row when `width` too tight

- [ ] **Step 1: Write failing tests**

`destination-id.test.js`:

```js
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
```

`topology-layout.test.js`:

```js
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
```

- [ ] **Step 2: Run — FAIL**

Run: `cd app/ui && node --test tests/destination-id.test.js tests/topology-layout.test.js`  
Expected: FAIL (MODULE_NOT_FOUND).

- [ ] **Step 3: Implement helpers**

`layout.js` sketch:

```js
const MIN_GAP = 88;
const NODE_R = 28;
const LOCAL_R = 36;

export function layoutTopology({ width, height, destinations }) {
  const local = { x: width / 2, y: height - 72, r: LOCAL_R };
  const topY = 96;
  const maxPerRow = Math.max(1, Math.floor((width - 80) / MIN_GAP));
  const nodes = [];
  const edges = [];
  destinations.forEach((d, i) => {
    const row = Math.floor(i / maxPerRow);
    const idx = i % maxPerRow;
    const rowCount = Math.min(maxPerRow, destinations.length - row * maxPerRow);
    const rowWidth = (rowCount - 1) * MIN_GAP;
    const startX = width / 2 - rowWidth / 2;
    const x = startX + idx * MIN_GAP;
    const y = topY + row * 88;
    nodes.push({ ...d, x, y, r: NODE_R });
    edges.push({
      id: d.id,
      paused: !!d.paused,
      d: `M ${local.x} ${local.y - LOCAL_R} C ${local.x} ${(local.y + y) / 2}, ${x} ${(local.y + y) / 2}, ${x} ${y + NODE_R}`,
    });
  });
  return { local, nodes, edges };
}
```

- [ ] **Step 4: Run — PASS**

Run: `cd app/ui && node --test tests/destination-id.test.js tests/topology-layout.test.js`  
Expected: PASS. Update `package.json` `"test"` to run all `tests/*.test.js`.

- [ ] **Step 5: Commit**

```bash
git add app/ui/src/lib/topology app/ui/tests app/ui/package.json
git commit -m "$(cat <<'EOF'
Add topology layout and destination route id helpers.

Pure functions so graph spacing and hash parsing stay tested.
EOF
)"
```

---

### Task 3: `TopologyGraph` + `EdgePausePopup` components

**Files:**
- Create: `app/ui/src/lib/topology/TopologyGraph.svelte`
- Create: `app/ui/src/lib/topology/EdgePausePopup.svelte`
- Modify: `app/ui/src/app.css` (`.topology`, `.topo-node`, `.topo-edge`, flow/dash, rise-in)

**Interfaces:**
- Consumes: `layoutTopology`
- Produces props/events:

```svelte
<!-- TopologyGraph.svelte -->
<script>
  /** destinations: { id, label, paused, ahead?, behind?, visibility? }[] */
  let {
    destinations = [],
    localLabel = 'This machine',
    onlocalclick,
    onnodeclick,   // (id) => void
    onedgepause,   // (id) => void  // request pause/resume for edge target
  } = $props();
</script>
```

```svelte
<!-- EdgePausePopup.svelte -->
let { open = false, x = 0, y = 0, paused = false, label = '', onconfirm, onclose } = $props();
// Button text: paused ? 'Resume commits to this repo' : 'Pause commits to this repo'
```

- [ ] **Step 1: Implement `TopologyGraph.svelte`**

- Measure container with `ResizeObserver`; call `layoutTopology`.
- SVG: edges (visible path + transparent wide hit path), local circle + icon/label, destination circles + labels + pause/visibility cues.
- CSS classes for enter animation (stagger nodes), `.flow` when `!paused`, `.dashed` when paused.
- Click local → `onlocalclick()`; click node → `onnodeclick(id)`; click edge hit → open popup coords + remember edge id; confirm → `onedgepause(id)`.

- [ ] **Step 2: Implement `EdgePausePopup.svelte`**

Positioned absolutely at click; backdrop click closes; primary button calls `onconfirm`.

- [ ] **Step 3: Add CSS**

Reuse existing CSS variables. Avoid purple-glow / card-dashboard look. Prefer thin lines, clear nodes, muted paused state. Add `@keyframes topo-draw` / `topo-rise` (2–3 motions per spec).

- [ ] **Step 4: Manual smoke (dev)**

Run UI (`cd app/ui && npm run dev` or existing Tauri dev). Temporarily mount graph in Overview with fake destinations; verify resize, clicks, popup. Remove fake mount before commit if not wired yet — or leave behind a prop-driven empty state.

- [ ] **Step 5: Commit**

```bash
git add app/ui/src/lib/topology app/ui/src/app.css
git commit -m "$(cat <<'EOF'
Add TopologyGraph and edge pause popup.

SVG layout with resize, hit targets, and pause/resume popup.
EOF
)"
```

---

### Task 4: Aggregate destinations + App hash routes

**Files:**
- Modify: `crates/gateway/src/targets.rs` (add `list_all_for_paths` or load loop helper)
- Modify: `app/src-tauri/src/commands.rs` (`targets_list_all`)
- Modify: `app/ui/src/lib/api.js`
- Modify: `app/ui/src/App.svelte`
- Create: `app/ui/src/lib/pages/LocalDetail.svelte` (stub ok)
- Create: `app/ui/src/lib/pages/DestinationDetail.svelte` (stub ok)

**Interfaces:**

```rust
// returns Vec<{ repo_path, repo_key, targets: RepoTargets }> or flat DestinationView
pub fn list_all(paths: &Paths, repo_paths: &[PathBuf]) -> Result<Vec<DestinationRow>>

pub struct DestinationRow {
    pub repo_path: String,
    pub repo_key: String,
    pub target: PublishTarget,
}
```

```js
targetsListAll: (paths) => invoke('targets_list_all', { pathsIn: paths }),
```

Routing in `App.svelte`:

```js
function routeOf(section) {
  if (section === 'local') return { kind: 'local' };
  const d = parseDestinationHash(section);
  if (d) return { kind: 'destination', ...d };
  return { kind: 'section', id: section || 'overview' };
}
```

Mount:

```svelte
{#if route.kind === 'local'}
  <LocalDetail bind:config />
{:else if route.kind === 'destination'}
  <DestinationDetail repoKey={route.repoKey} targetId={route.targetId} onback={() => go('overview')} />
{:else if section === 'overview'}
  <Overview ... />
<!-- existing branches -->
```

`go('local')` / `go('destination/' + encodeDestinationId(key, id))`.

- [ ] **Step 1: Add `targets_list_all` + api**

Load each repo’s targets file via existing `load`; skip missing; flatten enabled+disabled (graph shows paused too).

- [ ] **Step 2: Wire App routing + stub pages**

Stubs:

```svelte
<!-- LocalDetail.svelte -->
<script>
  let { config = $bindable() } = $props();
</script>
<div class="page">
  <h1 class="page-title">Local machine</h1>
  <p class="muted">Configure local commits (full UI in next task).</p>
</div>
```

Similar stub for DestinationDetail showing `repoKey` / `targetId`.

- [ ] **Step 3: Manual — hash navigation**

Set `location.hash = '#local'` and `#destination/test/t1'`; confirm mounts. Unknown hashes still fall through sensibly.

- [ ] **Step 4: Commit**

```bash
git add crates/gateway/src/targets.rs app/src-tauri/src/commands.rs app/ui/src/lib/api.js app/ui/src/App.svelte app/ui/src/lib/pages/LocalDetail.svelte app/ui/src/lib/pages/DestinationDetail.svelte
git commit -m "$(cat <<'EOF'
Add destination list API and local/destination routes.

App hash can open #local and #destination/<repoKey>/<targetId>.
EOF
)"
```

---

### Task 5: LocalDetail page (full)

**Files:**
- Modify: `app/ui/src/lib/pages/LocalDetail.svelte`
- May reuse patterns from `Overview.svelte` Pause/Start and `Rules.svelte` entry via `onnavigate`

**Interfaces:**
- Bind `config`; `api.save(config)` on pause toggle (`config.enabled`)
- Links/buttons to commit/docs/merge rules sections via existing `go` / `onnavigate('commit')` etc.
- Show local work summaries via `api.reposWorkSummaries` (optional compact list)

- [ ] **Step 1: Implement LocalDetail**

- Header with back to overview
- Pause/Start checking (same meaning as Overview hero / tray)
- Cards or sections linking to message rules tabs (do not duplicate entire Rules editor — navigate)
- Short explanation: destination pause ≠ this global pause

- [ ] **Step 2: Manual check**

Toggle pause; confirm topbar/tray state still consistent after `api.save`.

- [ ] **Step 3: Commit**

```bash
git add app/ui/src/lib/pages/LocalDetail.svelte
git commit -m "$(cat <<'EOF'
Add LocalDetail for machine commit settings and global pause.
EOF
)"
```

---

### Task 6: DestinationDetail page (migrate TargetsModal)

**Files:**
- Modify: `app/ui/src/lib/pages/DestinationDetail.svelte`
- Modify: `app/ui/src/lib/pages/TargetsModal.svelte` (extract shared field editors **or** inline migrate then delete modal usage)
- Prefer: extract `TargetEditor.svelte` shared by detail page (and modal if still referenced temporarily)

**Parity checklist (must all exist on the page):**

1. Identity — label, role, remote_name, url, enabled (Pause maps here)
2. Push policy — push_policy, include_in_push_all, default_target
3. Filter — publish_filter, use_mirror
4. Rules — branches + tabs.commit.total_max (as today’s modal)
5. Checks — enabled, on_fail, timeout_secs, commands[]
6. Actions — Pause/Resume, Push (`jobStartPublish(path, label)`), refresh sync (`repoWorkRemote`), show ahead/behind

**Load path:**

1. Resolve `repoKey` → repo path: either return path from `targets_list_all` or add `targets_get_by_key`; simplest: pass `repo_path` in list rows and look up by key from `api.repos()` + `key_for` exposed, **or** include `repo_path` in destination route payload via list cache.
2. Practical approach: `DestinationDetail` receives `repoKey`+`targetId`; calls `api.targetsListAll(paths)` or new `targetsGetByKey(repoKey)`; find target.

Add if needed:

```rust
pub fn load_by_key(paths: &Paths, key: &str) -> Result<Option<RepoTargets>>
```

(scan `targets/*.toml` or map key→path from registry).

- [ ] **Step 1: Ensure load-by-key works**

Implement `targets_get_by_key` command if lookup is awkward; test with save/load round trip key.

- [ ] **Step 2: Build DestinationDetail UI**

Segmented sections matching modal tabs; sticky action bar: Pause, Push, Back.

Node-style pause control at top (clickable status).

- [ ] **Step 3: Parity walkthrough**

Open old TargetsModal on a throwaway branch or compare field list; check every control exists and saves via `api.targetsSave`.

- [ ] **Step 4: Commit**

```bash
git add app/ui/src/lib/pages/DestinationDetail.svelte app/ui/src/lib/pages/TargetEditor.svelte crates/gateway/src/targets.rs app/src-tauri/src/commands.rs app/ui/src/lib/api.js
git commit -m "$(cat <<'EOF'
Add DestinationDetail with full target config and actions.

Migrates TargetsModal fields; pause, push, and per-remote sync.
EOF
)"
```

---

### Task 7: Multi-import Add destination UI

**Files:**
- Create: `app/ui/src/lib/topology/AddDestination.svelte` (sheet/modal)
- Used from Overview graph empty/+ and working-copy graph

**Behavior:**
- Given a repo path (working-copy) or pick-repo then remotes: list `repo.remotes` from registry / `api.repos`.
- Multi-select remotes **not already** represented by a target (`remote_name` match).
- One confirm creates N targets with sensible defaults (label from remote name or slug; role guess public/personal from visibility if available; `enabled: true`).
- No auto-run on discover — only on confirm.

- [ ] **Step 1: Implement AddDestination**

Reuse blank target shape from TargetsModal (`blank()`).

- [ ] **Step 2: Wire + buttons on both graphs**

- [ ] **Step 3: Manual — two remotes**

Repo with origin + second remote: both appear in import list; select both; both nodes show after save.

- [ ] **Step 4: Commit**

```bash
git add app/ui/src/lib/topology/AddDestination.svelte app/ui/src/lib/pages/Overview.svelte app/ui/src/lib/pages/RepoDetail.svelte
git commit -m "$(cat <<'EOF'
Add multi-select import for publish destinations.

Makes public + private remotes addable in one step without auto-create.
EOF
)"
```

---

### Task 8: Overview hosts machine-wide topology

**Files:**
- Modify: `app/ui/src/lib/pages/Overview.svelte`

**Replace as primary surface:** `home-repos` grid and treat graph as the main composition. Keep Setup wizard + critical health/error. Agents/Runs: either move under a secondary disclosure or keep a compact strip below the graph — **prefer compact Runs strip below graph** so publish jobs remain visible; remove repo card grid.

Wire:

```svelte
<TopologyGraph
  destinations={destNodes}
  onlocalclick={() => onnavigate('local')}
  onnodeclick={(id) => onnavigate('destination/' + id)}
  onedgepause={togglePause}
/>
```

`togglePause`: load targets for that repo, flip `enabled`, `targetsSave`.

Load `api.repos` → paths → `targetsListAll` → map to graph nodes (`id: encodeDestinationId(repoKey, target.id)`, `paused: !enabled`, sync via `repoWorkRemote` best-effort).

- [ ] **Step 1: Implement data load + graph host**
- [ ] **Step 2: Remove repo-grid as home**
- [ ] **Step 3: Manual — 0, 1, many destinations; pause from edge**
- [ ] **Step 4: Commit**

```bash
git add app/ui/src/lib/pages/Overview.svelte
git commit -m "$(cat <<'EOF'
Make Overview a machine-wide destination topology.

Local node and destinations replace the home repo card grid.
EOF
)"
```

---

### Task 9: RepoDetail working-copy topology

**Files:**
- Modify: `app/ui/src/lib/pages/RepoDetail.svelte`
- Remove primary Work/Publish list panels; keep commit/PR rule overlays as subtabs or links if still needed
- Remove TargetsModal as the configure entry (detail pages instead)

**Behavior:**
- Body = `TopologyGraph` for this repo’s targets only
- Local node click → `#local` or a small local-work sheet for **this** repo (ahead/dirty) — prefer navigate `#local` for global settings **and** show this-repo work stats in a slim bar above the graph so functionality isn’t lost
- Destination click → `#destination/...`
- Edge pause → flip that target’s `enabled`
- Add destination → Task 7 sheet scoped to this repo
- Push all control can live as a graph toolbar button calling `jobStartPublish(path, '--all')`

Commit/PR panels: keep as secondary tabs `commit` / `pr_create` (existing) so per-repo rules aren’t lost.

- [ ] **Step 1: Replace Work/Publish UI with graph + slim work strip**
- [ ] **Step 2: Delete dead TargetsModal open sites; remove modal if unused**
- [ ] **Step 3: Manual — push, pause, open detail, rules tabs still work**
- [ ] **Step 4: Commit**

```bash
git add app/ui/src/lib/pages/RepoDetail.svelte app/ui/src/lib/pages/TargetsModal.svelte
git commit -m "$(cat <<'EOF'
Replace RepoDetail Work/Publish with destination topology.

Same graph language as Overview; keep commit/PR rule tabs.
EOF
)"
```

---

### Task 10: Polish, parity audit, verification

**Files:** CSS polish; any leftover Overview Pause duplication; README only if user-facing behavior needs a sentence (skip unless necessary)

- [ ] **Step 1: Spec coverage checklist**

Walk `2026-10-31-topology-graph-ui-design.md` §§3–10; tick each requirement against UI.

- [ ] **Step 2: TargetsModal field parity**

For each field in Task 6 checklist, confirm save/load round-trip from DestinationDetail.

- [ ] **Step 3: Automated tests**

Run:

```bash
cargo test -p gitassist-gateway parse_left_right status_vs_remote
cd app/ui && npm test
```

Expected: PASS.

- [ ] **Step 4: Manual script**

1. Repo with two remotes, zero targets → graph empty + Add → import both → two nodes  
2. Edge pause → dashed + `enabled: false` → no publish to that label  
3. Destination detail push queues job  
4. Local detail toggles global `config.enabled`  
5. Resize window — layout reflows, no overlap at 1 and 10+ nodes  
6. Commit/PR overlays still reachable from RepoDetail  

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
Polish topology UI and verify destination parity.

Finish pause/sync cues and drop leftover list UI.
EOF
)"
```

---

## Self-review (plan vs spec)

| Spec requirement | Task |
|------------------|------|
| Overview + working-copy graphs | 8, 9 |
| Dedicated local / destination pages | 4, 5, 6 |
| Edge pause popup | 3, 8, 9 |
| Destination pause = publish + destination checks (`enabled`) | 6, 8, 9 |
| Manual multi-import; no auto-create | 7 |
| Create GitHub repo out of scope | noted; no task |
| SVG layout scales / wraps | 2, 3 |
| Per-remote up-to-date | 1, 6 |
| TargetsModal parity | 6, 10 |
| Motion 2–3 | 3, 10 |
| Remove Overview cards / Work+Publish lists | 8, 9 |

No TBD placeholders. Types: `RemoteSync`, `layoutTopology`, `encodeDestinationId`, `targetsListAll`, `repoWorkRemote` used consistently.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-10-31-topology-graph-ui.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration  
2. **Inline Execution** — run tasks in this session with executing-plans and checkpoints  

Which approach?
