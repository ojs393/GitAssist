<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import RepoAvatar from '../ui/RepoAvatar.svelte';
  import RunsPanel from '../ui/RunsPanel.svelte';
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
  const AGENT = { current: ['ok', 'On'], stale: ['warn', 'Stale'], not_applied: ['idle', 'Off'], not_detected: ['muted', '—'] };

  const installed = $derived(report?.adapters.filter((a) => a.status !== 'not_detected') ?? []);
  const missing = $derived(report?.adapters.filter((a) => a.status === 'not_detected') ?? []);
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
        <ActionButton run={fixAll} label="Fix all" icon="wrench" size="sm" doneLabel="Queued" onresult={refresh} go={true} />
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

  <div class="home-split">
    <section class="sheet agents-pane">
      <div class="sheet-head">
        <div class="sheet-title"><Icon name="sparkle" size={14} /> Agents</div>
        <span class="muted-2 small">{installed.length}</span>
      </div>
      <ul class="agent-rows compact">
        {#each installed as a}
          {@const st = AGENT[a.status]}
          <li class="agent-row">
            <span class="agent-glyph"><Icon name={'agent-' + a.id} size={13} /></span>
            <span class="agent-name">{a.name}</span>
            <span class="info-slot">
              {#if a.manual.length}<Info align="right"><p><strong>{a.name}</strong></p>{#each a.manual as m}<p>{m}</p>{/each}</Info>{/if}
            </span>
            <span class="pill {st[0]}">{st[1]}</span>
            <span class="agent-action">
              {#if a.status === 'current'}
                <ActionButton run={() => api.removeAdapter(a.id)} label="Remove" kind="ghost" size="sm" doneLabel="Ok" onresult={refresh} />
              {:else}
                <ActionButton run={() => api.jobStartApply(a.id)} label="Apply" size="sm" doneLabel="Queued" onresult={refresh} go={true} />
              {/if}
            </span>
          </li>
        {:else}
          <li class="agent-row muted">None found</li>
        {/each}
      </ul>
      {#if missing.length}<p class="muted-2 small footnote">Auto: {missing.map((a) => a.name).join(', ')}</p>{/if}
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
          <button type="button" class="repo-card" onclick={() => onnavigate?.('repos')}>
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
