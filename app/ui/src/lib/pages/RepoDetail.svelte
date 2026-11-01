<script>
  import { api, NAV } from '../api.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import Rules from './Rules.svelte';
  import TopologyGraph from '../topology/TopologyGraph.svelte';
  import AddDestination from '../topology/AddDestination.svelte';
  import { encodeDestinationId } from '../topology/destinationId.js';
  import { enqueueTargetsWriteForRepo } from '../topology/targetsWriteQueue.js';

  let { repo, onback, onnavigate, initialTab = 'work', globalConfig = $bindable(), tabs = [] } = $props();

  let panel = $state('work');
  let targets = $state({ repo_path: '', default_target: null, targets: [] });
  let repoKey = $state('');
  let destNodes = $state([]);
  let work = $state(null);
  let error = $state('');
  let pushMsg = $state('');
  let addOpen = $state(false);
  let loading = $state(true);
  let overrideConfig = $state(null);
  let overridePath = $state('');
  let saveNote = $state('');

  const inflightPause = new Set();
  let loadGen = 0;
  let pauseGen = 0;

  const PANELS = [
    { id: 'work', label: 'Destinations', icon: 'layers' },
    { id: 'commit', label: 'Commits', icon: 'commit' },
    { id: 'pr_create', label: 'Pull requests', icon: 'pr' },
  ];

  const commitSiblings = NAV.find((n) => n.id === 'commits')?.children ?? ['commit'];
  const prSiblings = NAV.find((n) => n.id === 'prs')?.children ?? ['pr_create'];

  function message(e) {
    return String(e).replace(/^Error: /, '');
  }

  function visibilityOf(role) {
    if (role === 'public') return 'public';
    if (role === 'personal' || role === 'organization') return 'private';
    return undefined;
  }

  function mapNodes(rows, path, key) {
    const prevById = new Map(destNodes.map((n) => [n.id, n]));
    return (rows ?? [])
      .filter((row) => row?.target?.id)
      .map((row) => {
        const t = row.target;
        const id = encodeDestinationId(key || row.repo_key, t.id);
        const prev = prevById.get(id);
        return {
          id,
          label: (t.label || t.remote_name || 'Destination').trim() || 'Destination',
          paused: inflightPause.has(id) ? !!prev?.paused : !t.enabled,
          visibility: visibilityOf(t.role),
          ahead: prev?.ahead,
          behind: prev?.behind,
          targetId: t.id,
          remote: t.remote_name || '',
          repoPath: path,
          repoKey: key || row.repo_key,
        };
      });
  }

  async function fillSync(gen, path, nodes) {
    let branch = String(work?.branch || repo?.branch || '').trim();
    if (!branch) {
      try {
        const w = await api.repoWork(path);
        branch = String(w?.branch || '').trim();
      } catch {
        return;
      }
    }
    if (!branch || gen !== loadGen) return;
    await Promise.all(
      nodes.map(async (n) => {
        if (!n.remote) return;
        try {
          const row = await api.repoWorkRemote(path, n.remote, branch);
          if (gen !== loadGen) return;
          destNodes = destNodes.map((d) =>
            d.id === n.id ? { ...d, ahead: row?.ahead ?? 0, behind: row?.behind ?? 0 } : d,
          );
        } catch {
          // Sync counts are best-effort.
        }
      }),
    );
  }

  async function load(path) {
    const gen = ++loadGen;
    const seenPause = pauseGen;
    loading = true;
    addOpen = false;
    error = '';
    try {
      const [t, w, ov, rows] = await Promise.all([
        api.targetsGet(path),
        api.repoWork(path).catch(() => null),
        api.overridesGet(path).catch(() => ({ tabs: {} })),
        api.targetsListAll([path]).catch(() => []),
      ]);
      if (gen !== loadGen) return;
      targets = t?.targets ? t : { ...t, targets: [] };
      work = w;
      overridePath = path;
      const key = rows?.[0]?.repo_key || '';
      repoKey = key;
      if (seenPause === pauseGen) {
        destNodes = mapNodes(rows, path, key);
        void fillSync(gen, path, destNodes);
      }
      const base = JSON.parse(JSON.stringify(globalConfig));
      for (const [id, partial] of Object.entries(ov.tabs || {})) {
        base.tabs[id] = { ...(base.tabs[id] || {}), ...partial };
      }
      overrideConfig = base;
    } catch (e) {
      if (gen !== loadGen) return;
      error = message(e);
    } finally {
      if (gen === loadGen) loading = false;
    }
  }

  $effect(() => {
    if (repo?.path) load(repo.path);
  });
  $effect(() => {
    const tab = initialTab || 'work';
    panel = tab === 'publish' ? 'work' : tab;
  });

  // Persist only the active rules tab overlay when editing Commits/PRs for this repo.
  let saveTimer;
  $effect(() => {
    if (!overrideConfig || !overridePath || !globalConfig) return;
    if (panel !== 'commit' && panel !== 'pr_create' && !commitSiblings.includes(panel) && !prSiblings.includes(panel)) return;
    const snap = JSON.stringify(overrideConfig);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      try {
        const ov = await api.overridesGet(overridePath);
        const tabsOut = { ...(ov.tabs || {}) };
        const ids = [...commitSiblings, ...prSiblings];
        for (const id of ids) {
          const local = overrideConfig.tabs[id] || {};
          const glob = globalConfig.tabs[id] || {};
          if (JSON.stringify(local) !== JSON.stringify(glob)) tabsOut[id] = local;
          else delete tabsOut[id];
        }
        await api.overridesSave(overridePath, { repo_path: overridePath, tabs: tabsOut });
        saveNote = 'Saved for this repo';
        setTimeout(() => (saveNote = ''), 1600);
      } catch (e) {
        error = message(e);
      }
    }, 500);
  });

  function openImport() {
    if (loading) return;
    addOpen = true;
  }

  async function importDestinations(created) {
    const path = repo.path;
    await enqueueTargetsWriteForRepo({ path, key: repoKey }, async () => {
      const fresh = await api.targetsGet(path);
      const next = {
        ...(fresh ?? {}),
        repo_path: path,
        default_target: fresh?.default_target ?? null,
        targets: [...(fresh?.targets ?? []), ...created],
      };
      const notes = await api.targetsSave(path, next);
      pushMsg = notes?.length ? notes.join('; ') : 'Saved.';
    });
    await load(path);
  }

  async function pushAll() {
    pushMsg = '';
    const job = await api.jobStartPublish(repo.path, '--all');
    pushMsg = `Queued ${job.label}. Watch Overview → Runs.`;
  }

  async function togglePause(id) {
    const node = destNodes.find((d) => d.id === id);
    if (!node?.targetId) return;
    const path = repo.path;
    const targetId = node.targetId;
    const nextPaused = !node.paused;
    inflightPause.add(id);
    destNodes = destNodes.map((n) => (n.id === id ? { ...n, paused: nextPaused } : n));
    try {
      await enqueueTargetsWriteForRepo({ path, key: repoKey || node.repoKey }, async () => {
        const doc = await api.targetsGet(path);
        const list = doc?.targets ?? [];
        if (!list.some((t) => t.id === targetId)) throw new Error('This destination is not in the repo.');
        const next = {
          ...(doc ?? {}),
          repo_path: path,
          targets: list.map((t) => (t.id === targetId ? { ...t, enabled: !nextPaused } : t)),
        };
        await api.targetsSave(path, next);
      });
      pauseGen += 1;
      destNodes = destNodes.map((n) => (n.id === id ? { ...n, paused: nextPaused } : n));
    } catch (e) {
      destNodes = destNodes.map((n) => (n.id === id && n.paused === nextPaused ? { ...n, paused: !nextPaused } : n));
      error = message(e);
    } finally {
      inflightPause.delete(id);
    }
  }

  async function resetOverride(tabId) {
    await api.overridesClearTab(repo.path, tabId);
    await load(repo.path);
  }

  const tabById = (id) => tabs.find((t) => t.id === id);
  const openWork = $derived(work && (work.ahead > 0 || work.dirty > 0 || work.staged > 0));
  const canPush = $derived(!!(openWork || (work?.ahead > 0)));
</script>

<header class="page-head">
  <div class="page-title">
    <button type="button" class="icon-btn" aria-label="Back" onclick={onback}><Icon name="chevron-left" size={18} /></button>
    <span class="page-icon"><Icon name="folder" size={18} /></span>
    <h1>{repo.name}</h1>
    <Info>
      <p>Destinations for this working copy, and per-repo rule overrides. Global Commits/PR pages set defaults for every project.</p>
    </Info>
  </div>
  <div class="page-actions">
    {#if saveNote}<span class="save-pill ok"><Icon name="check" size={13} />{saveNote}</span>{/if}
    {#if panel === 'work'}
      <ActionButton run={openImport} label="Add" icon="plus" size="sm" disabled={loading} />
      <ActionButton run={pushAll} label="Push all" icon="upload" size="sm" doneLabel="Queued" go={true} disabled={!canPush} />
    {/if}
  </div>
</header>

<div class="tab-bar">
  <div class="subtabs">
    {#each PANELS as p}
      <button type="button" class:active={panel === p.id || (p.id === 'commit' && commitSiblings.includes(panel)) || (p.id === 'pr_create' && prSiblings.includes(panel))} onclick={() => (panel = p.id)}>
        <Icon name={p.icon} size={13} /> {p.label}
      </button>
    {/each}
  </div>
</div>

{#if error}<div class="card error-card"><Icon name="alert" /> {error}</div>{/if}

{#if loading}
  <div class="loading"><span class="spinner"></span></div>
{:else if panel === 'work'}
  <div class="work-strip">
    <span class="mono small muted-2 break">{repo.path}</span>
    <span class="grow"></span>
    <span class="muted small">
      {work?.branch || repo.branch || '—'}
      {#if work?.ahead} · {work.ahead} ahead{/if}
      {#if work?.dirty} · {work.dirty} dirty{/if}
      {#if !openWork && !(work?.ahead > 0)} · clean{/if}
    </span>
    <button type="button" class="btn ghost sm" onclick={() => onnavigate?.('local')}>
      Local <span class="go"><Icon name="chevron-right" size={12} /></span>
    </button>
  </div>

  <section class="sheet topo-stage" aria-label="Destinations">
    {#if !destNodes.length}
      <p class="muted small topo-hint">
        No destinations yet.
        <button type="button" class="link-text" onclick={openImport}>Add</button>
        from a git remote to publish to.
      </p>
    {/if}
    <div class="topo-canvas">
      <TopologyGraph
        destinations={destNodes}
        onlocalclick={() => onnavigate?.('local')}
        onnodeclick={(id) => onnavigate?.('destination/' + id)}
        onedgepause={togglePause}
      />
    </div>
  </section>
  {#if pushMsg}<pre class="card quiet mono small push-out">{pushMsg}</pre>{/if}

{:else if overrideConfig && (panel === 'commit' || commitSiblings.includes(panel))}
  {@const tid = commitSiblings.includes(panel) ? panel : 'commit'}
  {@const tmeta = tabById(tid)}
  <p class="banner muted small">Defaults come from the header Commits pages. Changes here apply only to <strong>{repo.name}</strong>.
    <button type="button" class="link-text" onclick={() => resetOverride(tid)}>Reset this tab</button>
  </p>
  {#if tmeta}
    {#key tid}
      <Rules bind:config={overrideConfig} tab={tmeta} siblings={commitSiblings} {tabs} onselect={(id) => (panel = id)} embedded={true} />
    {/key}
  {/if}

{:else if overrideConfig && (panel === 'pr_create' || prSiblings.includes(panel))}
  {@const tid = prSiblings.includes(panel) ? panel : 'pr_create'}
  {@const tmeta = tabById(tid)}
  <p class="banner muted small">Defaults come from the header Pull requests pages. Changes here apply only to <strong>{repo.name}</strong>.
    <button type="button" class="link-text" onclick={() => resetOverride(tid)}>Reset this tab</button>
  </p>
  {#if tmeta}
    {#key tid}
      <Rules bind:config={overrideConfig} tab={tmeta} siblings={prSiblings} {tabs} onselect={(id) => (panel = id)} embedded={true} />
    {/key}
  {/if}
{/if}

<AddDestination
  open={addOpen}
  {repo}
  existingTargets={targets.targets ?? []}
  onsave={importDestinations}
  onclose={() => (addOpen = false)}
/>

<style>
  .work-strip {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 0 10px;
    flex-wrap: wrap;
  }
  .topo-stage { display: flex; flex-direction: column; gap: 6px; }
  .topo-hint { margin: 0; }
  .topo-canvas {
    position: relative;
    height: clamp(420px, 58vh, 640px);
    min-height: 420px;
  }
  .push-out { white-space: pre-wrap; margin-top: 12px; color: var(--fg-2); }
  .banner { margin: 0 0 10px; }
  .grow { flex: 1; }
</style>
