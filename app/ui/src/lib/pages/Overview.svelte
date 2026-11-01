<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import RunsPanel from '../ui/RunsPanel.svelte';
  import Modal from '../ui/Modal.svelte';
  import Setup from './Setup.svelte';
  import TopologyGraph from '../topology/TopologyGraph.svelte';
  import AddDestination from '../topology/AddDestination.svelte';
  import { encodeDestinationId } from '../topology/destinationId.js';
  import { enqueueTargetsWriteForRepo } from '../topology/targetsWriteQueue.js';

  let { config = $bindable(), onnavigate, onopenrepo } = $props();
  let report = $state(null);
  let repoList = $state([]);
  let destNodes = $state([]);
  let error = $state('');
  let topoError = $state('');
  let checkedAt = $state(null);
  let wizard = $state(false);
  let syncing = $state(false);
  let picking = $state(false);
  let addOpen = $state(false);
  let addRepo = $state(null);
  let addExisting = $state([]);

  const inflightPause = new Set();
  let loadGen = 0;
  // Bumped after a pause save so a targets list that started earlier cannot
  // paint the pre-save enabled flag back onto the graph.
  let pauseGen = 0;

  function message(e) {
    return String(e).replace(/^Error: /, '');
  }

  function visibilityOf(role) {
    if (role === 'public') return 'public';
    if (role === 'personal' || role === 'organization') return 'private';
    return undefined;
  }

  async function loadDestinations(repos) {
    const gen = ++loadGen;
    const seenPause = pauseGen;
    const paths = (repos ?? []).map((r) => r.path).filter(Boolean);
    if (!paths.length) {
      destNodes = [];
      topoError = '';
      return;
    }
    let rows = [];
    try {
      rows = (await api.targetsListAll(paths)) ?? [];
      if (gen !== loadGen || seenPause !== pauseGen) return;
      topoError = '';
    } catch (e) {
      if (gen !== loadGen) return;
      topoError = message(e);
      return;
    }
    const prevById = new Map(destNodes.map((n) => [n.id, n]));
    const repoPaths = new Set(rows.map((row) => row.repo_path).filter(Boolean));
    const multi = repoPaths.size > 1;
    destNodes = rows
      .filter((row) => row?.repo_key && row.target?.id)
      .map((row) => {
        const t = row.target;
        const id = encodeDestinationId(row.repo_key, t.id);
        const prev = prevById.get(id);
        const label = (t.label || t.remote_name || 'Destination').trim() || 'Destination';
        const repo = (repos ?? []).find((r) => r.path === row.repo_path);
        const who = repo?.name || name(row.repo_path);
        return {
          id,
          label: multi && who ? `${who} · ${label}` : label,
          paused: inflightPause.has(id) ? !!prev?.paused : t.enabled === false,
          visibility: visibilityOf(t.role),
          ahead: prev?.ahead,
          behind: prev?.behind,
          repoPath: row.repo_path,
          repoKey: row.repo_key,
          targetId: t.id,
          remote: t.remote_name || '',
        };
      });
    void fillSync(gen, repos, destNodes);
  }

  async function fillSync(gen, repos, nodes) {
    const branchByPath = new Map();
    for (const r of repos ?? []) {
      const branch = String(r?.branch || '').trim();
      if (r?.path && branch) branchByPath.set(r.path, branch);
    }
    const missing = [...new Set(nodes.map((n) => n.repoPath).filter((p) => p && !branchByPath.get(p)))];
    await Promise.all(
      missing.map(async (path) => {
        try {
          const work = await api.repoWork(path);
          const branch = String(work?.branch || '').trim();
          if (branch) branchByPath.set(path, branch);
        } catch {
          // Sync counts are best-effort. The node still draws.
        }
      }),
    );
    if (gen !== loadGen) return;
    await Promise.all(
      nodes.map(async (node) => {
        const remote = String(node.remote || '').trim();
        const branch = branchByPath.get(node.repoPath) || '';
        if (!remote || !branch) return;
        try {
          const sync = await api.repoWorkRemote(node.repoPath, remote, branch);
          if (gen !== loadGen) return;
          const ahead = Number(sync?.ahead) || 0;
          const behind = Number(sync?.behind) || 0;
          destNodes = destNodes.map((n) => (n.id === node.id ? { ...n, ahead, behind } : n));
        } catch {
          // A missing remote tip should not blank the graph.
        }
      }),
    );
  }

  async function refresh() {
    try {
      if (!report) report = await api.health(false);
      const [h, list] = await Promise.all([api.health(true), api.repos().catch(() => [])]);
      report = h;
      repoList = list ?? [];
      checkedAt = new Date();
      error = '';
      if (!report.turned_on && !sessionStorage.getItem('setup-shown')) {
        sessionStorage.setItem('setup-shown', '1');
        wizard = true;
      }
      await loadDestinations(repoList);
    } catch (e) {
      error = message(e);
    }
  }
  onMount(refresh);

  const ITEM_ICONS = { hooks: 'commit', shim: 'terminal', autostart: 'login', repair: 'wrench', config: 'file' };
  const STATE = { on: ['ok', 'On'], off: ['idle', 'Off'], attention: ['bad', 'Fix'], not_needed: ['muted', '—'] };
  const AGENT = { current: ['ok', 'On'], stale: ['warn', 'Stale'], not_applied: ['idle', 'Off'], not_detected: ['muted', '—'] };

  const installed = $derived(report?.adapters.filter((a) => a.status !== 'not_detected') ?? []);
  const problems = $derived(
    report ? report.items.filter((i) => i.state === 'attention').length + installed.filter((a) => a.status !== 'current').length + report.repos.filter((r) => !r.connected).length : 0,
  );
  const name = (p) => p.split(/[\\/]/).filter(Boolean).pop();

  async function fixAll() {
    const snap = report;
    if (!snap) return;
    const plan = {
      connect: snap.repos.filter((x) => !x.connected).map((r) => r.path),
      adapters: snap.adapters.filter((x) => x.status !== 'current' && x.status !== 'not_detected').map((a) => a.id),
      installs: snap.items.filter((x) => x.state === 'attention').map((i) => i.id),
    };
    await api.jobStartFixAll(plan);
  }

  const itemOn = (i) => i.state === 'on';
  const itemToggleable = (i) => i.id !== 'config' && i.state !== 'not_needed';

  async function setItem(i, on) {
    // Optimistic UI so the switch doesn't snap back while the install runs.
    if (report) {
      report = {
        ...report,
        items: report.items.map((x) => (x.id === i.id ? { ...x, state: on ? 'on' : 'off' } : x)),
      };
    }
    try {
      if (i.id === 'repair') {
        config.auto_repair = on;
      } else if (on) {
        await api.install(i.id);
      } else if (i.id === 'autostart') {
        await api.disableAutostart();
      } else {
        await api.uninstallStep(i.id);
      }
    } catch (e) {
      error = message(e);
    }
    await refresh();
  }

  /** Full check: repair hooks/shim/login, re-apply drifted agents, probe every shell. */
  async function syncAll() {
    syncing = true;
    try {
      await api.repair();
      const h = await api.health(true);
      report = h;
      const stale = (h.adapters ?? []).filter((a) => a.status === 'stale' || a.status === 'not_applied');
      for (const a of stale) {
        await api.jobStartApply(a.id);
      }
      const broken = (h.items ?? []).filter((i) => i.state === 'attention').map((i) => i.id);
      for (const id of broken) {
        if (id === 'config') continue;
        await api.install(id).catch(() => {});
      }
      await refresh();
    } finally {
      syncing = false;
    }
  }

  async function togglePause(id) {
    const node = destNodes.find((d) => d.id === id);
    if (!node?.repoPath || !node.targetId) return;
    const path = node.repoPath;
    const targetId = node.targetId;
    const nextPaused = !node.paused;
    inflightPause.add(id);
    destNodes = destNodes.map((n) => (n.id === id ? { ...n, paused: nextPaused } : n));
    try {
      await enqueueTargetsWriteForRepo({ path, key: node.repoKey }, async () => {
        const doc = await api.targetsGet(path);
        const targets = doc?.targets ?? [];
        if (!targets.some((t) => t.id === targetId)) throw new Error('This destination is not in the repo.');
        const next = {
          ...(doc ?? {}),
          repo_path: path,
          targets: targets.map((t) => (t.id === targetId ? { ...t, enabled: !nextPaused } : t)),
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

  function startAdd() {
    if (!repoList.length) return;
    if (repoList.length === 1) openAdd(repoList[0]);
    else picking = true;
  }

  async function openAdd(repo) {
    picking = false;
    try {
      const doc = await api.targetsGet(repo.path);
      addRepo = repo;
      addExisting = doc?.targets ?? [];
      addOpen = true;
    } catch (e) {
      error = message(e);
    }
  }

  async function saveAdded(created) {
    const repo = addRepo;
    if (!repo?.path) throw new Error('Pick a repo first.');
    const knownKey = destNodes.find((n) => n.repoPath === repo.path)?.repoKey;
    await enqueueTargetsWriteForRepo({ path: repo.path, key: knownKey }, async () => {
      const fresh = await api.targetsGet(repo.path);
      const next = {
        ...(fresh ?? {}),
        repo_path: repo.path,
        default_target: fresh?.default_target ?? null,
        targets: [...(fresh?.targets ?? []), ...created],
      };
      await api.targetsSave(repo.path, next);
    });
    await loadDestinations(repoList);
  }
</script>

<Setup open={wizard} bind:config {report} onclose={() => (wizard = false)} ondone={() => { wizard = false; refresh(); }} />

{#if error}<div class="card error-card"><Icon name="alert" /> {error}</div>{/if}

{#if !report}
  <div class="loading"><span class="spinner"></span></div>
{:else}
  <section class="hero {!report.turned_on ? 'off' : !config.enabled ? 'paused' : problems ? 'warn' : 'ok'}">
    <div class="hero-icon"><Icon name={!report.turned_on || !config.enabled ? 'shield-off' : problems ? 'shield-alert' : 'shield-check'} size={20} stroke={1.6} /></div>
    <div class="hero-text">
      <h1>{!report.turned_on ? 'GitAssist is off' : !config.enabled ? 'Paused' : problems ? `${problems} to fix` : 'Protected'}</h1>
      <p>{!report.turned_on ? 'Set up once — works in every repo.' : !config.enabled ? 'Unchecked until you turn it back on.' : problems ? 'One click puts everything back.' : `Commits, pushes, PRs and issues checked${installed.length ? ` · ${installed.length} agents` : ''}.`}</p>
    </div>
    <div class="hero-actions">
      {#if !report.turned_on}
        <button type="button" class="btn primary sm" onclick={() => (wizard = true)}><Icon name="rocket" size={13} /> Set up <span class="go"><Icon name="chevron-right" size={13} /></span></button>
      {:else}
        <button
          type="button"
          class="btn sm {config.enabled ? 'ghost' : 'primary'}"
          onclick={() => (config.enabled = !config.enabled)}
          title={config.enabled ? 'Pause checking' : 'Turn checking back on'}
        >
          <Icon name={config.enabled ? 'pause' : 'play'} size={13} />
          {config.enabled ? 'Pause' : 'Start'}
        </button>
        {#if problems && config.enabled}
          <ActionButton run={fixAll} label="Fix all" icon="wrench" size="sm" doneLabel="Queued" onresult={refresh} go={true} />
        {/if}
      {/if}
      <button type="button" class="icon-btn" title="Check again" aria-label="Check again" onclick={refresh}><Icon name="refresh" size={14} /></button>
    </div>
  </section>

  {#if report.turned_on}
    <section class="status-bar sheet">
      {#each report.items as i}
        {@const st = STATE[i.state]}
        <div class="status-cell">
          <span class="status-icon {st[0]}"><Icon name={ITEM_ICONS[i.id] ?? 'info'} size={13} /></span>
          <div class="status-label">{i.label}</div>
          <div class="status-controls">
            {#if i.state === 'attention'}
              <ActionButton run={() => api.install(i.id)} label="Fix" size="sm" doneLabel="On" onresult={refresh} />
            {/if}
            {#if itemToggleable(i)}
              <Toggle small checked={itemOn(i)} label={itemOn(i) ? 'On' : 'Off'} onchange={(v) => setItem(i, v)} />
            {:else}
              <span class="status-state {st[0]}">{st[1]}</span>
            {/if}
            <Info align="right"><p>{i.summary}</p>{#each i.details as d}<p class="small muted">{d}</p>{/each}</Info>
          </div>
        </div>
      {/each}
    </section>
  {/if}

  {#if report.repos.some((r) => !r.connected)}
    <section class="sheet" style="background:var(--accent-softer)">
      <div class="sheet-head">
        <div class="sheet-title"><Icon name="link" size={14} /> Own hooks</div>
      </div>
      <ul class="agent-rows compact">
        {#each report.repos.filter((r) => !r.connected) as r}
          <li class="agent-row">
            <span class="agent-glyph"><Icon name="folder" size={13} /></span>
            <span class="agent-name" title={r.path}>{name(r.path)}</span>
            <span class="info-slot"></span>
            <span class="pill bad">Open</span>
            <span class="agent-action"><ActionButton run={() => api.connectRepo(r.path)} label="Connect" size="sm" doneLabel="Ok" onresult={refresh} go={true} /></span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <section class="sheet topo-stage" aria-label="Destinations">
    <div class="sheet-head">
      <div class="sheet-title"><Icon name="layers" size={14} /> Destinations</div>
      <div class="sheet-head-actions">
        {#if topoError}<span class="bad small">{topoError}</span>{/if}
        <button type="button" class="btn ghost sm" onclick={startAdd} disabled={!repoList.length}>
          <Icon name="plus" size={13} /> Add
        </button>
      </div>
    </div>
    {#if !repoList.length}
      <p class="muted small topo-hint">No repos yet. Commit somewhere or open Repos to find them.</p>
    {:else if !destNodes.length}
      <p class="muted small topo-hint">No destinations yet. Add a git remote to publish to.</p>
    {/if}
    <div class="topo-canvas">
      <TopologyGraph
        destinations={destNodes}
        onlocalclick={() => onnavigate('local')}
        onnodeclick={(id) => onnavigate('destination/' + id)}
        onedgepause={togglePause}
      />
    </div>
  </section>

  <div class="sheet runs-strip">
    <RunsPanel {onnavigate} />
  </div>

  <details class="sheet agents-fold">
    <summary class="sheet-head">
      <div class="sheet-title"><Icon name="sparkle" size={14} /> Agents</div>
      <span class="muted-2 small">{installed.length}/{report.adapters.length}</span>
    </summary>
    <div class="agents-sync">
      <ActionButton run={syncAll} label={syncing ? 'Syncing' : 'Sync'} icon="refresh" size="sm" kind="ghost" doneLabel="Synced" onresult={refresh} disabled={syncing} />
    </div>
    <ul class="agent-rows compact">
      {#each report.adapters as a}
        {@const st = AGENT[a.status]}
        <li class="agent-row" class:dim={a.status === 'not_detected'}>
          <span class="agent-glyph"><Icon name={'agent-' + a.id} size={13} /></span>
          <span class="agent-name">{a.name}</span>
          <span class="info-slot">
            {#if a.manual.length}<Info align="right"><p><strong>{a.name}</strong></p>{#each a.manual as m}<p>{m}</p>{/each}</Info>{/if}
          </span>
          <span class="pill {st[0]}">{st[1]}</span>
          <span class="agent-action">
            {#if a.status === 'current'}
              <ActionButton run={() => api.removeAdapter(a.id)} label="Remove" kind="ghost" size="sm" doneLabel="Ok" onresult={refresh} />
            {:else if a.status === 'not_detected'}
              <span class="muted-2 small">Auto</span>
            {:else}
              <ActionButton run={() => api.jobStartApply(a.id)} label="Apply" size="sm" doneLabel="Queued" onresult={refresh} go={true} />
            {/if}
          </span>
        </li>
      {:else}
        <li class="agent-row muted">None found</li>
      {/each}
    </ul>
    <p class="muted-2 small footnote">Sync repairs hooks, PATH, login, and every agent rule file, then re-checks shells.</p>
  </details>

  {#if checkedAt}<p class="checked muted-2">Checked {checkedAt.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })} · <button class="link-text" onclick={() => (wizard = true)}>Setup again</button></p>{/if}
{/if}

<Modal open={picking} onclose={() => (picking = false)} title="Which repo?">
  <p class="muted small">Pick a repo, then choose remotes that are not destinations yet.</p>
  <div class="pick-list">
    {#each repoList as r (r.path)}
      <button type="button" class="pick-row" onclick={() => openAdd(r)}>
        <strong>{r.name || name(r.path)}</strong>
        <span class="muted-2 small">{r.slug || r.path}</span>
      </button>
    {/each}
  </div>
</Modal>

<AddDestination
  open={addOpen}
  repo={addRepo}
  existingTargets={addExisting}
  onsave={saveAdded}
  onclose={() => (addOpen = false)}
/>

<style>
  .topo-stage { display: flex; flex-direction: column; gap: 6px; }
  .topo-hint { margin: 0; }
  .topo-canvas {
    position: relative;
    height: clamp(420px, 58vh, 640px);
    min-height: 420px;
  }
  .runs-strip {
    padding: 0;
    overflow: hidden;
    max-height: 220px;
    margin-bottom: 8px;
  }
  .runs-strip :global(.runs-panel) {
    display: flex;
    flex-direction: column;
    max-height: 220px;
    min-height: 0;
    padding: 10px 12px;
  }
  .runs-strip :global(.runs-scroll) {
    min-height: 0;
    overflow-y: auto;
  }
  .agents-fold > summary { cursor: pointer; list-style: none; }
  .agents-fold > summary::-webkit-details-marker { display: none; }
  .agents-sync { display: flex; justify-content: flex-end; margin-bottom: 6px; }
  .pick-list { display: flex; flex-direction: column; gap: 2px; margin-top: 0.75rem; }
  .pick-row {
    display: flex;
    flex-direction: column;
    gap: 1px;
    width: 100%;
    padding: 10px 6px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
    font: inherit;
  }
  .pick-row:hover { background: var(--accent-softer); }
  .pick-row strong { font-size: 12.5px; }
</style>
