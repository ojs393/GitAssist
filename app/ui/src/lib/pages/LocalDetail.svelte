<script>
  import { onMount } from 'svelte';
  import { api, TAB_ICONS } from '../api.js';
  import Icon from '../ui/Icon.svelte';
  import PageHead from '../ui/PageHead.svelte';

  let { config = $bindable(), tabs = [], onnavigate, onback } = $props();

  // Commit-message sections only. The Rules editor stays on its own pages.
  const RULES = [
    { id: 'commit', about: 'Every git commit on this machine, typed by you or run by any agent.' },
    { id: 'docs_commit', about: 'Commits where every changed file matches your docs patterns.' },
    { id: 'merge', about: 'Merge and squash commits, plus the subject and body of gh pr merge.' },
  ];
  const FALLBACK = {
    commit: 'Commit messages',
    docs_commit: 'Docs commits',
    merge: 'Merge & squash commits',
  };

  let summaries = $state(null);
  let workError = $state('');

  const rules = $derived(
    RULES.map((rule) => ({
      ...rule,
      label: tabs.find((t) => t.id === rule.id)?.label ?? FALLBACK[rule.id],
      enabled: config.tabs?.[rule.id]?.enabled ?? true,
    })),
  );

  const nameOf = (p) => p.split(/[\\/]/).filter(Boolean).pop() || p;
  const needs = (row) => (row.ahead || 0) + (row.behind || 0) + (row.dirty || 0) + (row.staged || 0) > 0;
  const ordered = $derived(
    (summaries ?? []).slice().sort((a, b) => Number(needs(b)) - Number(needs(a)) || a.name.localeCompare(b.name)),
  );

  function workLine(row) {
    const bits = [];
    if (row.ahead) bits.push(`${row.ahead} ahead`);
    if (row.behind) bits.push(`${row.behind} behind`);
    if (row.dirty) bits.push(`${row.dirty} dirty`);
    if (row.staged) bits.push(`${row.staged} staged`);
    const status = bits.length ? bits.join(' · ') : 'Clean';
    return row.branch ? `${row.branch} · ${status}` : status;
  }

  async function loadWork() {
    workError = '';
    try {
      const repos = (await api.repos()) ?? [];
      const paths = repos.map((r) => r.path).filter(Boolean);
      if (!paths.length) {
        summaries = [];
        return;
      }
      const rows = (await api.reposWorkSummaries(paths)) ?? [];
      summaries = rows.map((row) => {
        const path = Array.isArray(row) ? row[0] : row.path;
        const status = (Array.isArray(row) ? row[1] : row.status) ?? {};
        const repo = repos.find((r) => r.path === path);
        return { path, name: repo?.name || nameOf(path), ...status };
      });
    } catch (e) {
      workError = String(e);
      summaries = summaries ?? [];
    }
  }

  onMount(loadWork);
</script>

<PageHead title="Local machine" icon="layout" info="Local commit rules and the machine-wide pause.">
  {#snippet actions()}
    <button type="button" class="btn ghost sm" onclick={() => onback?.()}>
      <Icon name="chevron-left" size={13} /> Overview
    </button>
  {/snippet}
</PageHead>

<section class="hero {config.enabled ? 'ok' : 'paused'}">
  <div class="hero-icon"><Icon name={config.enabled ? 'shield-check' : 'shield-off'} size={20} stroke={1.6} /></div>
  <div class="hero-text">
    <h1>{config.enabled ? 'Checking' : 'Paused'}</h1>
    <p>{config.enabled ? 'Commits, merges, and docs messages are checked on this machine.' : 'Unchecked until you turn it back on.'}</p>
  </div>
  <div class="hero-actions">
    <button
      type="button"
      class="btn sm {config.enabled ? 'ghost' : 'primary'}"
      onclick={() => (config.enabled = !config.enabled)}
      title={config.enabled ? 'Pause checking' : 'Turn checking back on'}
    >
      <Icon name={config.enabled ? 'pause' : 'play'} size={13} />
      {config.enabled ? 'Pause' : 'Start'}
    </button>
  </div>
</section>

<section class="sheet">
  <div class="sheet-title"><Icon name="info" size={14} /> Two different pauses</div>
  <p class="footnote">This pause stops commit checking on this whole machine. It is the same switch as the top bar.</p>
  <p class="footnote">A destination pause stops publish and checks for that one repo. The machine stays on.</p>
</section>

<h2 class="section-title"><Icon name="commit" size={13} /> Message rules</h2>
<div class="col" style="margin-bottom: 8px">
  {#each rules as rule (rule.id)}
    <button type="button" class="link-card" onclick={() => onnavigate?.(rule.id)}>
      <Icon name={TAB_ICONS[rule.id]} size={16} />
      <span class="repo-name grow">
        <strong>{rule.label}</strong>
        <span class="muted small">{rule.about}</span>
      </span>
      <span class="pill {rule.enabled ? 'ok' : 'idle'}">{rule.enabled ? 'On' : 'Off'}</span>
      <span class="go"><Icon name="chevron-right" size={14} /></span>
    </button>
  {/each}
</div>

<div class="sheet-head">
  <h2 class="section-title" style="margin: 0"><Icon name="activity" size={13} /> Local work</h2>
  <button type="button" class="icon-btn" title="Refresh" aria-label="Refresh local work" onclick={loadWork}><Icon name="refresh" size={14} /></button>
</div>
{#if workError}<p class="small bad">{workError}</p>{/if}
{#if summaries === null}
  <div class="loading"><span class="spinner sm"></span></div>
{:else if ordered.length}
  <div class="col">
    {#each ordered as row (row.path)}
      <button type="button" class="link-card" title={row.path} onclick={() => onnavigate?.('repos', { repo: row.path, tab: 'work' })}>
        <Icon name="folder" size={15} />
        <span class="repo-name grow">
          <strong>{row.name}</strong>
          <span class="muted small">{workLine(row)}</span>
        </span>
        {#if needs(row)}<span class="pill warn">Open</span>{/if}
        <span class="go"><Icon name="chevron-right" size={14} /></span>
      </button>
    {/each}
  </div>
{:else if !workError}
  <p class="muted small">No repos yet. Commit somewhere or open Repos to find them.</p>
{/if}
