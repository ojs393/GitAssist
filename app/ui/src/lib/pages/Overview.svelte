<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import Card from '../ui/Card.svelte';
  import Setup from './Setup.svelte';

  let { config = $bindable(), onnavigate } = $props();
  let report = $state(null);
  let error = $state('');
  let checkedAt = $state(null);
  let wizard = $state(false);

  // Quick status first, then the slower shell probe fills in.
  async function refresh() {
    try {
      if (!report) report = await api.health(false);
      report = await api.health(true);
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
  const STATE = { on: ['ok', 'On'], off: ['idle', 'Off'], attention: ['bad', 'Fix needed'], not_needed: ['muted', 'Not needed'] };
  const AGENT = { current: ['ok', 'Working'], stale: ['warn', 'Out of date'], not_applied: ['idle', 'Not set up'], not_detected: ['muted', 'Not installed'] };

  const installed = $derived(report?.adapters.filter((a) => a.status !== 'not_detected') ?? []);
  const missing = $derived(report?.adapters.filter((a) => a.status === 'not_detected') ?? []);
  const problems = $derived(
    report ? report.items.filter((i) => i.state === 'attention').length + installed.filter((a) => a.status !== 'current').length + report.repos.filter((r) => !r.connected).length : 0,
  );
  const name = (p) => p.split(/[\\/]/).filter(Boolean).pop();

  // Fix everything the Overview counts as a problem, then run repair for hooks/shim/login.
  // Connecting repos here matters: older builds of repair() skipped never-connected ones.
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
    <div class="hero-icon"><Icon name={!report.turned_on || !config.enabled ? 'shield-off' : problems ? 'shield-alert' : 'shield-check'} size={36} stroke={1.5} /></div>
    <div class="hero-text">
      <h1>{!report.turned_on ? 'GitAssist is off' : !config.enabled ? 'Paused' : problems ? `${problems} thing${problems > 1 ? 's' : ''} to fix` : 'Protected'}</h1>
      <p>{!report.turned_on ? 'Set it up once and it works in every repo.' : !config.enabled ? 'Everything goes through unchecked until you switch it back on.' : problems ? 'One click puts everything back.' : `Commits, pushes, pull requests and issues are checked${installed.length ? ` for you and ${installed.length} agent${installed.length > 1 ? 's' : ''}` : ''}.`}</p>
    </div>
    <div class="hero-actions">
      {#if !report.turned_on}
        <button class="btn primary lg" onclick={() => (wizard = true)}><Icon name="rocket" size={16} /> Set up <span class="go"><Icon name="chevron-right" size={16} /></span></button>
      {:else if problems && config.enabled}
        <ActionButton run={fixAll} label="Fix all" icon="wrench" doneLabel="Fixed" onresult={refresh} go={true} />
      {/if}
      <button class="icon-btn" title="Check again" aria-label="Check again" onclick={refresh}><Icon name="refresh" size={16} /></button>
    </div>
  </section>

  {#if report.turned_on}
    <div class="tiles">
      {#each report.items as i}
        {@const st = STATE[i.state]}
        <div class="tile">
          <div class="tile-top">
            <span class="tile-icon {st[0]}"><Icon name={ITEM_ICONS[i.id] ?? 'info'} size={18} /></span>
            <Info align="right"><p>{i.summary}</p>{#each i.details as d}<p class="small muted">{d}</p>{/each}</Info>
          </div>
          <div class="tile-label">{i.label}</div>
          <div class="tile-foot">
            <span class="pill {st[0]}">{st[1]}</span>
            {#if i.state === 'attention' || (i.state === 'off' && i.id !== 'repair')}
              <ActionButton run={() => api.install(i.id)} label={i.state === 'attention' ? 'Fix' : 'Turn on'} size="sm" doneLabel="On" onresult={refresh} />
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <Card title="Agents" icon="sparkle" info="GitAssist gives each agent your rules and switches off its co-author line. Commits from any agent are cleaned either way. Cloud agents run on remote servers and only follow the rules they are given." raised>
    {#snippet actions()}<span class="muted small">{installed.length} found</span>{/snippet}
    <ul class="agent-rows">
      {#each installed as a}
        {@const st = AGENT[a.status]}
        <li class="agent-row">
          <span class="agent-glyph"><Icon name={'agent-' + a.id} size={17} /></span>
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
    {#if missing.length}<p class="muted small footnote">Set up automatically when installed: {missing.map((a) => a.name).join(', ')}</p>{/if}
  </Card>

  {#if report.repos.some((r) => !r.connected)}
    <Card title="Repos with their own hooks" icon="link" info="These repos use Husky or lefthook, so git runs their hooks instead of GitAssist's until you connect them." emphasis>
      <ul class="agent-rows">
        {#each report.repos.filter((r) => !r.connected) as r}
          <li class="agent-row">
            <span class="agent-glyph"><Icon name="folder" size={16} /></span>
            <span class="agent-name" title={r.path}>{name(r.path)}</span>
            <span class="info-slot"></span>
            <span class="pill bad">Not connected</span>
            <span class="agent-action"><ActionButton run={() => api.connectRepo(r.path)} label="Connect" size="sm" doneLabel="Connected" onresult={refresh} /></span>
          </li>
        {/each}
      </ul>
    </Card>
  {/if}

  <button class="card link-card" type="button" onclick={() => onnavigate?.('repos')}>
    <span class="agent-glyph"><Icon name="layers" size={16} /></span>
    <span class="grow"><strong>Repos</strong><span class="muted-2 small"> · remotes & publish</span></span>
    <span class="go"><Icon name="chevron-right" size={16} /></span>
  </button>

  {#if checkedAt}<p class="checked muted small">Checked at {checkedAt.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })} · <button class="link-text" onclick={() => (wizard = true)}>Run setup again</button></p>{/if}
{/if}
