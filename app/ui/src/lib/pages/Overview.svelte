<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import Setup from './Setup.svelte';

  let { config = $bindable(), onnavigate } = $props();
  let report = $state(null);
  let repoList = $state([]);
  let error = $state('');
  let checkedAt = $state(null);
  let wizard = $state(false);

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
  const AGENT = { current: ['ok', 'Working'], stale: ['warn', 'Stale'], not_applied: ['idle', 'Not set'], not_detected: ['muted', 'Missing'] };

  const installed = $derived(report?.adapters.filter((a) => a.status !== 'not_detected') ?? []);
  const missing = $derived(report?.adapters.filter((a) => a.status === 'not_detected') ?? []);
  const problems = $derived(
    report ? report.items.filter((i) => i.state === 'attention').length + installed.filter((a) => a.status !== 'current').length + report.repos.filter((r) => !r.connected).length : 0,
  );
  const name = (p) => p.split(/[\\/]/).filter(Boolean).pop();
  const shownRepos = $derived(
    repoList.length
      ? repoList
      : (report?.repos ?? []).map((r) => ({ path: r.path, name: name(r.path), connected: r.connected, branch: null, slug: null, last_push: null, last_sync: null })),
  );

  async function fixAll() {
    const snap = report;
    if (!snap) return api.repair();
    const errors = [];
    for (const r of snap.repos.filter((x) => !x.connected)) {
      try {
        await api.connectRepo(r.path);
      } catch (e) {
        errors.push(`${name(r.path)}: ${e}`);
      }
    }
    for (const a of snap.adapters.filter((x) => x.status !== 'current' && x.status !== 'not_detected')) {
      try {
        await api.applyAdapter(a.id);
      } catch (e) {
        errors.push(`${a.name}: ${e}`);
      }
    }
    for (const i of snap.items.filter((x) => x.state === 'attention')) {
      try {
        await api.install(i.id);
      } catch (e) {
        errors.push(`${i.label}: ${e}`);
      }
    }
    await api.repair();
    if (errors.length) throw new Error(errors.join('; '));
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
        <button class="btn primary sm" onclick={() => (wizard = true)}><Icon name="rocket" size={13} /> Set up <span class="go"><Icon name="chevron-right" size={13} /></span></button>
      {:else if problems && config.enabled}
        <ActionButton run={fixAll} label="Fix all" icon="wrench" size="sm" doneLabel="Fixed" onresult={refresh} go={true} />
      {/if}
      <button class="icon-btn" title="Check again" aria-label="Check again" onclick={refresh}><Icon name="refresh" size={14} /></button>
    </div>
  </section>

  {#if report.turned_on}
    <div class="tiles">
      {#each report.items as i}
        {@const st = STATE[i.state]}
        <div class="tile">
          <div class="tile-top">
            <span class="tile-icon {st[0]}"><Icon name={ITEM_ICONS[i.id] ?? 'info'} size={13} /></span>
            <Info align="right"><p>{i.summary}</p>{#each i.details as d}<p class="small muted">{d}</p>{/each}</Info>
          </div>
          <div class="tile-label">{i.label}</div>
          <div class="tile-foot">
            <span class="pill {st[0]}">{st[1]}</span>
            {#if i.state === 'attention' || (i.state === 'off' && i.id !== 'repair')}
              <ActionButton run={() => api.install(i.id)} label={i.state === 'attention' ? 'Fix' : 'On'} size="sm" doneLabel="On" onresult={refresh} />
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <section class="sheet">
    <div class="sheet-head">
      <div class="sheet-title"><Icon name="sparkle" size={14} /> Agents</div>
      <span class="muted-2 small">{installed.length} found</span>
    </div>
    <ul class="agent-rows">
      {#each installed as a}
        {@const st = AGENT[a.status]}
        <li class="agent-row">
          <span class="agent-glyph"><Icon name={'agent-' + a.id} size={14} /></span>
          <span class="agent-name">{a.name}</span>
          <span class="info-slot">
            {#if a.manual.length}<Info align="right"><p><strong>One step in {a.name}</strong></p>{#each a.manual as m}<p>{m}</p>{/each}</Info>{/if}
          </span>
          <span class="pill {st[0]}">{st[1]}</span>
          <span class="agent-action">
            {#if a.status === 'current'}
              <ActionButton run={() => api.removeAdapter(a.id)} label="Remove" kind="ghost" size="sm" doneLabel="Removed" onresult={refresh} />
            {:else}
              <ActionButton run={() => api.applyAdapter(a.id)} label="Apply" size="sm" doneLabel="Applied" onresult={refresh} go={true} />
            {/if}
          </span>
        </li>
      {:else}
        <li class="agent-row muted">No coding agents found yet.</li>
      {/each}
    </ul>
    {#if missing.length}<p class="muted-2 small footnote">Auto-setup when installed: {missing.map((a) => a.name).join(', ')}</p>{/if}
  </section>

  {#if report.repos.some((r) => !r.connected)}
    <section class="sheet" style="background:var(--accent-softer)">
      <div class="sheet-head">
        <div class="sheet-title"><Icon name="link" size={14} /> Own hooks</div>
        <Info><p>Husky/lefthook repos — connect so GitAssist runs too.</p></Info>
      </div>
      <ul class="agent-rows">
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
      <div class="repo-rows">
        {#each shownRepos.slice(0, 8) as r}
          <div class="repo-row repo-row-click">
            <button type="button" class="repo-open" onclick={() => onnavigate?.('repos')}>
              <span class="agent-glyph"><Icon name="folder" size={13} /></span>
              <span class="repo-name">
                <strong>{r.name || name(r.path)}</strong>
                <span class="muted-2 small">{r.slug || (r.connected === false ? 'own hooks' : 'local')}</span>
              </span>
              <span class="mono small muted">{r.branch || '—'}</span>
              <span class="small muted">{r.last_push ? relativeTime(r.last_push) : r.last_sync ? relativeTime(r.last_sync) : '—'}</span>
              <span class="go"><Icon name="chevron-right" size={14} /></span>
            </button>
          </div>
        {/each}
      </div>
      {#if shownRepos.length > 8}
        <button type="button" class="btn ghost sm" style="margin-top:6px" onclick={() => onnavigate?.('repos')}>+{shownRepos.length - 8} more <span class="go"><Icon name="chevron-right" size={12} /></span></button>
      {/if}
    {/if}
  </section>

  {#if checkedAt}<p class="checked muted-2">Checked {checkedAt.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })} · <button class="link-text" onclick={() => (wizard = true)}>Setup again</button></p>{/if}
{/if}
