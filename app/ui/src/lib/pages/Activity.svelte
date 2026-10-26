<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import ActionButton from '../ui/ActionButton.svelte';

  let { tabs = [] } = $props();
  let entries = $state([]);
  let error = $state('');
  let filter = $state('all');
  let query = $state('');
  let open = $state(-1);

  async function load() {
    try {
      entries = await api.activity(500);
      error = '';
    } catch (e) {
      error = String(e);
    }
  }
  onMount(() => {
    load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  });

  const SOURCES = {
    'hook:commit-msg': ['commit', 'Commit'],
    'hook:prepare-commit-msg': ['commit', 'Commit'],
    'hook:pre-commit': ['eye-off', 'Ignore'],
    'hook:post-commit': ['user', 'Author'],
    'hook:pre-push': ['login', 'Push'],
    gh: ['terminal', 'gh'],
    setup: ['wrench', 'Setup'],
  };
  const OUTCOME = { fixed: ['ok', 'wrench'], clean: ['ok', 'check'], warned: ['warn', 'alert'], blocked: ['bad', 'x'], error: ['bad', 'alert'], bypass: ['warn', 'shield-off'], passthrough: ['muted', 'check'] };
  const tabLabel = (id) => (id === 'ignore' ? 'Ignore' : tabs.find((t) => t.id === id)?.label ?? '');
  const repoName = (p) => (p ? p.split(/[\\/]/).filter(Boolean).pop() : '');
  const clean = (m) => m.replace(/^GitAssist \[[a-z_]+\] /, '');

  const shown = $derived(
    entries.filter((e) => {
      if (filter === 'setup' && e.source !== 'setup') return false;
      if (!['all', 'setup'].includes(filter) && e.outcome !== filter) return false;
      const q = query.trim().toLowerCase();
      return !q || [e.repo, e.tab, e.source, ...e.messages].join(' ').toLowerCase().includes(q);
    }),
  );
</script>

<header class="page-head">
  <div class="page-title">
    <span class="page-icon"><Icon name="activity" size={20} /></span>
    <h1>Activity</h1>
    <Info><p>Every check GitAssist ran. Stored only on this machine and capped at about 15 MB.</p></Info>
  </div>
  <div class="row">
    <button class="btn ghost sm" onclick={load}><Icon name="refresh" size={14} /> Refresh</button>
    <ActionButton run={async () => { await api.clearActivity(); await load(); }} label="Clear" kind="ghost" size="sm" icon="trash" doneLabel="Cleared" />
  </div>
</header>

<div class="toolbar">
  <Segmented size="sm" value={filter} onchange={(f) => (filter = f)}
    options={[{ id: 'all', label: 'All' }, { id: 'fixed', label: 'Fixed' }, { id: 'blocked', label: 'Blocked' }, { id: 'warned', label: 'Warned' }, { id: 'error', label: 'Errors' }, { id: 'setup', label: 'Setup' }]} />
  <div class="search"><Icon name="search" size={15} /><input bind:value={query} placeholder="Search repo or message" /></div>
</div>

{#if error}<div class="card error-card"><Icon name="alert" /> {error}</div>{/if}

{#if !shown.length}
  <div class="empty"><Icon name="activity" size={28} /><p>{entries.length ? 'Nothing matches.' : 'Nothing yet. Make a commit and it shows up here.'}</p></div>
{:else}
  <ul class="log">
    {#each shown as e, i}
      {@const src = SOURCES[e.source] ?? ['info', e.source]}
      {@const oc = OUTCOME[e.outcome] ?? ['muted', 'info']}
      <li class:open={open === i}>
        <button class="log-row" onclick={() => (open = open === i ? -1 : i)}>
          <span class="log-icon {oc[0]}"><Icon name={oc[1]} size={15} /></span>
          <span class="log-src"><Icon name={src[0]} size={14} /> {src[1]}</span>
          <span class="log-main">
            <span class="log-msg">{e.messages.length ? clean(e.messages[0]) : e.outcome === 'clean' ? 'Passed all rules' : e.outcome}</span>
            <span class="log-meta">{[repoName(e.repo), tabLabel(e.tab)].filter(Boolean).join(' · ')}</span>
          </span>
          <span class="log-time" title={new Date(e.ts * 1000).toLocaleString()}>{relativeTime(e.ts)}</span>
        </button>
        {#if open === i}
          <div class="log-detail">
            {#each e.messages.slice(1) as m}<p>{clean(m)}</p>{/each}
            {#if e.repo}<p class="mono small muted break">{e.repo}</p>{/if}
            {#if e.body}<pre class="tpl">{e.body}</pre>{/if}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}
