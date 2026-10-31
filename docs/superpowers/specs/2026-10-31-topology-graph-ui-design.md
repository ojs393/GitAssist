# Topology graph UI — Design Spec

Date: 2026-10-31
Status: Draft — awaiting user review

Builds on `2026-10-31-multi-target-publish-design.md` (destinations =
publish targets). This spec redesigns how those destinations are shown and
configured: an animated local→repos topology instead of Overview cards and
Repo Detail Work/Publish lists.

## 1. Purpose

Give the user one clear picture of GitAssist on their machine:

- One **local machine** node (this computer runs GitAssist and local commits)
- **Destination repo nodes** above it (separate repos they publish to — e.g.
  public and private — not branches)
- Lines that show the relationship, support pause via click, and always
  reflow cleanly on resize
- Click local or a destination → a **dedicated detail page** with full
  configure + actions (no lost TargetsModal / Work / Publish capability)

## 2. Decisions (locked)

| Topic | Choice |
|-------|--------|
| Scope | Overview graph **and** per-working-copy graph (same visual language) |
| Node click | Navigate to a **dedicated detail page** (not drawer/modal) |
| Destination pause | Pause **publish and destination-scoped checks** for that target |
| Overview nodes | One upper node per **destination repo** (publish target / wired remote repo), not branches |
| Missing destinations | **Manual** add/import only — never auto-create targets from remotes |
| Create GitHub repo | **Out of scope** this pass; follow-up after graph works |
| Home UI | **Replace** Overview cards and Repo Detail Work/Publish lists with the graph + detail pages |
| Implementation | Custom **SVG topology** (no graph library); computed layout, no user drag |

## 3. Information architecture & routes

### 3.1 Home / Overview (`#` or current Overview hash)

- Full-viewport topology: local at bottom → lines → destination nodes.
- Primary home. Old Overview stats/cards are removed from this surface.
- Empty / partial setup: obvious **Add destination** control.
- Unwired git remotes do not appear as live destination nodes until the
  user imports/adds them. If nothing is configured, publish to that remote
  simply does not go through GitAssist destinations.

### 3.2 Local detail (`#local` or equivalent)

Opened by clicking the local machine node.

- Configure how **local commits** work (hooks / message rules entry points
  already in the app).
- Global pause/resume for hooks (`config.enabled`) — distinct from
  per-destination pause.
- At-a-glance local work status; links to destinations that need attention.

### 3.3 Destination detail (`#destination/<id>` or equivalent)

Opened by clicking a destination node.

Actions / sections (preserve today’s power):

| Action | Behavior |
|--------|----------|
| Pause / Resume | Destination pause (see §5) |
| Push | Publish to this destination only |
| Message / rules | Target `tabs` / overlays |
| Check commits | Status / run checks relevant to this destination |
| Up to date? | Ahead/behind vs **this remote** |
| Branch | Which branch, or create new (target `branches`) |
| Full configure | Identity, push policy, filter/mirror, checks — same fields as TargetsModal |

Pause control is also available from the node treatment on this page
(click node / dedicated Pause button).

### 3.4 Working-copy graph (Repo Detail)

When viewing one local working copy, the body is the same topology:
local-work node → that copy’s **configured destinations**. Click-through
uses the same destination detail routes. Replaces Work + Publish list
panels and the collapsed “Git remotes” card as the primary discovery UI.

### 3.5 Edge interaction

Click a connecting line → compact popup:

- If active: **Pause commits to this repo**
- If paused: **Resume commits to this repo**

Same semantics as Pause on the destination detail page.

### 3.6 Add destination

- Manual only. Multi-select **import from existing git remotes** and/or
  pick known repos so public + private can both be added in one step.
- Do not invent targets the user did not add.
- **Create new repo** on GitHub (personal/org, public/private) is a
  documented follow-up, not part of this implementation.

## 4. Layout & animation

### 4.1 Layout

- One composition filling the main viewport (not a card dashboard).
- Local node anchored bottom-center; destinations in a row (or gentle arc)
  above.
- Positions from container size: even horizontal spacing, stable vertical
  gap; labels truncate with full name on hover.
- One destination → centered above local. Many → equal gaps with min/max
  spacing; overflow wraps to a second upper row still feeding lines to
  local (organized, not free-form).
- SVG paths for lines; wide invisible stroke for hit-testing; thin visible
  stroke.
- Public vs private: small visibility cue on the node (pill or icon).
- No user drag of nodes. Recalculate on resize only.

### 4.2 Motion

1. Enter: local appears → lines draw upward → destination nodes fade/rise.
2. Idle: subtle flow along lines when destination is active; paused =
   muted dashed line, no flow.
3. Interaction: node press scale; edge highlight before popup; short settle
   into detail pages (no flashy transitions).

### 4.3 Status cues

- Paused: dimmed node + dashed line + pause glyph.
- Sync: small ahead/behind (or equivalent) on the node; full detail on the
  destination page.

## 5. Pause semantics

### 5.1 Destination pause

Maps primarily to `PublishTarget.enabled = false` (already excludes the
target from publish selection, after-commit publish, and target-scoped
overlays / pre-push path).

While paused:

- GitAssist does **not** publish to that destination.
- Destination-scoped checks / rule overlays for that remote do **not** run.

Does **not**:

- Turn off global commit hooks (`config.enabled`) — that is local/global
  pause on the local detail page and existing tray/Overview pause.

Commits are local until publish; destination pause means “don’t enforce
or send for this repo.”

### 5.2 Global / local pause

Unchanged product meaning: `config.enabled` gates hook enforcement
machine-wide. Surfaced on the local detail page (and existing tray).

## 6. Data & API

### 6.1 Reuse

- `targets_get` / `targets_save` / `targets_push` (and job publish path)
- `PublishTarget` fields from multi-target publish design
- `repo_work` / work status (extend if needed for per-remote ahead/behind)
- Rules / overrides / `effective_config` for target overlays
- Global `config.enabled` for local pause
- Repo registry remotes list for **import** UI only

### 6.2 Gaps to close carefully

1. **Per-remote sync status** if current work status is only generic
   upstream — destination detail and node cues need ahead/behind vs that
   remote.
2. **Pause UX** on top of `enabled` with clear copy (pause ≠ delete).
3. **Import UX** that can add multiple remotes in one step (fixes “only
   see one” when two remotes exist but one target).
4. **Migrate every TargetsModal capability** onto Destination Detail so
   nothing is lost when the modal ceases to be the primary path.

### 6.3 Non-goals (this pass)

- Auto-creating publish targets from remotes
- In-app GitHub “create repository” (personal/org, visibility)
- Graph library / free-form node dragging
- Changing multi-target publish pipeline semantics beyond pause/UI wiring

## 7. Components

### Add

| Piece | Role |
|-------|------|
| `TopologyGraph.svelte` | SVG layout, nodes, edges, resize, motion, edge hits |
| `EdgePausePopup.svelte` | Pause/resume for one destination |
| `LocalDetail.svelte` | Local commit config + global pause |
| `DestinationDetail.svelte` | Full destination actions + config |
| Overview host | Renders machine-wide topology |
| Repo Detail host | Renders working-copy topology |

### Remove as primary UI

- Overview home cards/stats composition
- Repo Detail Work + Publish list panels
- Collapsed “Git remotes” as the way users discover destinations
- TargetsModal as the only configure path (retire or keep only if a thin
  shared editor is still useful internally)

### Keep

- Backend publish/targets/hooks/jobs
- Tray global pause
- Discovery / repos index (feeds import + status)

## 8. “Only seeing one remote” fix

Root cause today: Publish UI shows **targets**, not all git remotes; second
remote often sits in a collapsed remotes list until imported.

Fix without auto-create:

1. Graph only shows configured destinations (honest).
2. **Add destination** makes every git remote for the working copy easy to
   import in one multi-select step.
3. Working-copy graph and import copy make dual remotes (public + private
   repos) obvious.

## 9. Risks

- Field parity when leaving TargetsModal — checklist against multi-target
  publish §3 fields.
- Readability at 1 vs 10+ destinations (wrap row + spacing rules).
- Pause copy must not imply global hooks are off when only a destination
  is paused.
- Per-remote ahead/behind must be correct or omitted until accurate.

## 10. Success criteria

- Overview is the topology; local and each destination open detail pages.
- Public and private destinations can both be added and shown as separate
  nodes.
- Edge click pauses/resumes that destination; detail page has pause, push,
  message, checks, sync, branch, full config.
- No TargetsModal-only capability left unreachable.
- Layout reflows cleanly on resize; enter/idle/interact motions present.
- Create-on-GitHub still absent, with a clear follow-up path.

## 11. Follow-ups

1. Create new GitHub repo from Add destination (personal vs org, public vs
   private, then wire as target).
2. Optional: richer activity along edges (live publish job progress).
`}