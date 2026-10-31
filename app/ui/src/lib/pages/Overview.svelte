<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import RepoAvatar from '../ui/RepoAvatar.svelte';
  import RunsPanel from '../ui/RunsPanel.svelte';
  import Setup from './Setup.svelte';

  let { config = $bindable(), onnavigate, onopenrepo } = $props();
  let report = $state(null);
  let repoList = $state([]);
  let error = $state('');
  let checkedAt = $state(null);
  let wizard = $state(false);
  let syncing = $state(false);

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
    } catch (e) {
      error = String(e);
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
  const shownRepos = $derived(repoList.length ? repoList : []);

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
      error = String(e);
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
          <div class="status-cell-top">
            <span class="status-icon {st[0]}"><Icon name={ITEM_ICONS[i.id] ?? 'info'} size={13} /></span>
            <div class="status-controls">
              {#if itemToggleable(i)}
                <Toggle small checked={itemOn(i)} label={itemOn(i) ? 'On' : 'Off'} onchange={(v) => setItem(i, v)} />
              {:else}
                <span class="status-state {st[0]}">{st[1]}</span>
              {/if}
              <Info align="right"><p>{i.summary}</p>{#each i.details as d}<p class="small muted">{d}</p>{/each}</Info>
            </div>
          </div>
          <div class="status-label">{i.label}</div>
          {#if i.state === 'attention'}
            <div class="status-cell-action">
              <ActionButton run={() => api.install(i.id)} label="Fix" size="sm" doneLabel="On" onresult={refresh} />
            </div>
          {/if}
        </div>
      {/each}
    </section>
  {/if}

  <div class="home-split">
    <section class="sheet agents-pane">
      <div class="sheet-head">
        <div class="sheet-title"><Icon name="sparkle" size={14} /> Agents</div>
        <div class="sheet-head-actions">
          <span class="muted-2 small">{installed.length}/{report.adapters.length}</span>
          <ActionButton run={syncAll} label={syncing ? 'Syncing' : 'Sync'} icon="refresh" size="sm" kind="ghost" doneLabel="Synced" onresult={refresh} disabled={syncing} />
        </div>
      </div>
      <ul class="agent-rows compact agents-fill">
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
      <p class="muted-2 small footnote agents-note">Sync repairs hooks, PATH, login, and every agent rule file, then re-checks shells.</p>
    </section>

    <div class="sheet runs-pane">
      <RunsPanel {onnavigate} />
    </div>
  </div>

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

  <section class="sheet home-repos">
    <div class="sheet-head">
      <div class="sheet-title"><Icon name="layers" size={14} /> Repos</div>
      <button type="button" class="btn ghost sm" onclick={() => onnavigate?.('repos')}>All <span class="go"><Icon name="chevron-right" size={12} /></span></button>
    </div>
    {#if !shownRepos.length}
      <p class="muted small">No repos yet. Commit somewhere or open Repos to find them.</p>
    {:else}
      <div class="repo-grid">
        {#each shownRepos as r}
          <button type="button" class="repo-card" onclick={() => (onopenrepo ? onopenrepo(r.path, 'work') : onnavigate?.('repos', { repo: r.path, tab: 'work' }))}>
            <RepoAvatar login={r.avatar_login || ''} owner={r.owner || 'unknown'} size={32} />
            <span class="repo-card-body">
              <strong>{r.name || name(r.path)}</strong>
              <span class="muted-2 small">{r.slug || 'local'}{#if r.owner === 'organization'} · org{:else if r.owner === 'personal'} · you{/if}</span>
              <span class="muted small">{r.branch || '—'} · {r.last_push ? relativeTime(r.last_push) : r.last_sync ? relativeTime(r.last_sync) : '—'}</span>
            </span>
            <span class="go"><Icon name="chevron-right" size={14} /></span>
          </button>
        {/each}
      </div>
    {/if}
  </section>

  {#if checkedAt}<p class="checked muted-2">Checked {checkedAt.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })} · <button class="link-text" onclick={() => (wizard = true)}>Setup again</button></p>{/if}
{/if}
