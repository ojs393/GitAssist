<script>
  import { api } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Card from '../ui/Card.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import TargetsModal from './TargetsModal.svelte';

  let { repo, onback } = $props();

  let targets = $state({ repo_path: '', default_target: null, targets: [] });
  let error = $state('');
  let pushMsg = $state('');
  let modal = $state(false);
  let loading = $state(true);

  async function load(path) {
    loading = true;
    error = '';
    try {
      targets = await api.targetsGet(path);
      if (!targets.targets) targets.targets = [];
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (repo?.path) load(repo.path);
  });

  async function save(data) {
    const notes = await api.targetsSave(repo.path, data);
    targets = await api.targetsGet(repo.path);
    modal = false;
    pushMsg = notes?.length ? notes.join('; ') : 'Saved.';
  }

  async function push(which) {
    pushMsg = '';
    const report = await api.targetsPush(repo.path, which);
    pushMsg = report.results.map((r) => `${r.ok ? 'ok' : 'fail'} ${r.label}: ${r.message}`).join('\n');
    if (report.drift?.length) pushMsg += `\ndrift: ${report.drift.join(', ')}`;
    if (!report.ok) throw new Error(pushMsg);
  }

  const roleIcon = (role) => (role === 'public' ? 'eye' : role === 'organization' ? 'building' : 'user');
</script>

<header class="page-head">
  <div class="page-title">
    <button type="button" class="icon-btn" aria-label="Back" onclick={onback}><Icon name="chevron-left" size={18} /></button>
    <span class="page-icon"><Icon name="folder" size={18} /></span>
    <h1>{repo.name}</h1>
    <Info>
      <p>Publish targets for this path only. Prefer <span class="mono">gitassist push &lt;label&gt;</span> or <span class="mono">--all</span>.</p>
    </Info>
  </div>
  <div class="page-actions">
    <ActionButton run={async () => (modal = true)} label="Configure" icon="sliders" size="sm" go={true} />
  </div>
</header>

{#if error}<div class="card error-card"><Icon name="alert" /> {error}</div>{/if}

<div class="stat-row">
  <div class="stat emphasis">
    <span class="stat-label"><Icon name="link" size={13} /> remote</span>
    <span class="stat-num mono small">{repo.slug || 'local'}</span>
  </div>
  <div class="stat quiet">
    <span class="stat-label"><Icon name="branch" size={13} /> branch</span>
    <span class="stat-num mono small">{repo.branch || '—'}</span>
  </div>
  <div class="stat quiet">
    <span class="stat-label"><Icon name="upload" size={13} /> last push</span>
    <span class="stat-num">{repo.last_push ? relativeTime(repo.last_push) : '—'}</span>
  </div>
</div>

<p class="mono small muted-2 break path-line">{repo.path}</p>

{#if loading}
  <div class="loading"><span class="spinner"></span></div>
{:else}
  <Card title="Publish targets" icon="upload" info="Each target can have its own filter, rules, and pre-push checks." raised>
    {#if !targets.targets.length}
      <div class="empty"><Icon name="upload" size={24} /><p>No targets yet. Configure to add personal and public remotes.</p>
        <ActionButton run={async () => (modal = true)} label="Configure" icon="sliders" size="sm" go={true} />
      </div>
    {:else}
      <ul class="target-list">
        {#each targets.targets as t}
          <li class="target-row" class:off={!t.enabled}>
            <span class="agent-glyph"><Icon name={roleIcon(t.role)} size={16} /></span>
            <span class="repo-name">
              <strong>{t.label}</strong>
              <span class="muted-2 small">{t.remote_name}{#if t.url} · {t.url}{/if}</span>
            </span>
            <span class="pill">{t.push_policy}{#if t.include_in_push_all} · --all{/if}</span>
            <span class="repo-end">
              {#if t.enabled}
                <ActionButton run={() => push(t.label)} label="Push" icon="upload" size="sm" doneLabel="Pushed" />
              {:else}
                <span class="muted-2 small">off</span>
              {/if}
            </span>
          </li>
        {/each}
      </ul>
      <div class="row" style="margin-top:12px">
        <ActionButton run={() => push('--all')} label="Push all" kind="primary" size="sm" icon="upload" doneLabel="Done" go={true} />
      </div>
    {/if}
  </Card>
{/if}

{#if pushMsg}<pre class="card quiet mono small push-out">{pushMsg}</pre>{/if}

{#if repo.remotes?.length}
  <Card title="Git remotes" icon="link" quiet collapsible collapsed={true}>
    {#each repo.remotes as m}
      <p class="mono small break muted">{m.name}: {m.url}</p>
    {/each}
  </Card>
{/if}

<TargetsModal bind:data={targets} open={modal} onclose={() => (modal = false)} onsave={save} remotes={repo.remotes ?? []} />

<style>
  .path-line { margin: -4px 0 16px; }
  .target-list { list-style: none; margin: 0; padding: 0; }
  .target-row {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    padding: 10px 6px;
    border-radius: 10px;
    min-height: 52px;
  }
  .target-row + .target-row { margin-top: 2px; }
  .target-row:hover { background: var(--accent-softer); }
  .target-row.off { opacity: 0.55; }
  .push-out { white-space: pre-wrap; margin-top: 12px; color: var(--fg-2); }
</style>
