<script>
  import { api } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Card from '../ui/Card.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import TargetsModal from './TargetsModal.svelte';

  let { repo, onback } = $props();

  let targets = $state({ repo_path: repo.path, default_target: null, targets: [] });
  let error = $state('');
  let pushMsg = $state('');
  let modal = $state(false);
  let loading = $state(true);

  async function load() {
    loading = true;
    error = '';
    try {
      targets = await api.targetsGet(repo.path);
      if (!targets.targets) targets.targets = [];
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (repo?.path) load();
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
</script>

<header class="page-head">
  <div class="page-title">
    <button type="button" class="icon-btn" aria-label="Back" onclick={onback}><Icon name="chevron-left" size={18} /></button>
    <span class="page-icon"><Icon name="folder" size={20} /></span>
    <h1>{repo.name}</h1>
    <Info>
      <p>Configure named push targets (personal, public, org). Agents use <span class="mono">gitassist push &lt;label&gt;</span> or <span class="mono">--all</span>.</p>
    </Info>
  </div>
  <ActionButton run={async () => (modal = true)} label="Configure" icon="gear" size="sm" />
</header>

{#if error}<div class="card error-card"><Icon name="alert" /> {error}</div>{/if}

<div class="stat-row">
  <div class="stat"><span class="stat-num mono small">{repo.slug || 'local'}</span><span class="muted small">remote</span></div>
  <div class="stat"><span class="stat-num mono small">{repo.branch || '—'}</span><span class="muted small">branch</span></div>
  <div class="stat"><span class="stat-num">{repo.last_push ? relativeTime(repo.last_push) : '—'}</span><span class="muted small">last push</span></div>
</div>

<p class="mono small muted break">{repo.path}</p>

{#if loading}
  <div class="loading"><span class="spinner"></span></div>
{:else}
  <Card title="Publish targets" icon="upload" info="Each target can have its own filter, rules, and pre-push checks.">
    {#if !targets.targets.length}
      <div class="empty"><Icon name="upload" size={24} /><p>No targets yet. Click Configure to add personal and public remotes.</p></div>
    {:else}
      <ul class="repo-rows">
        {#each targets.targets as t}
          <li class="repo-row">
            <span class="agent-glyph"><Icon name={t.role === 'public' ? 'eye' : t.role === 'organization' ? 'building' : 'user'} size={16} /></span>
            <span class="repo-name">
              <strong>{t.label}</strong>
              <span class="muted small">{t.remote_name}{#if t.url} · {t.url}{/if}</span>
            </span>
            <span class="small muted">{t.push_policy}{#if t.include_in_push_all} · in --all{/if}</span>
            <span class="repo-end">
              {#if t.enabled}
                <ActionButton run={() => push(t.label)} label="Push" size="sm" doneLabel="Pushed" />
              {/if}
            </span>
          </li>
        {/each}
      </ul>
      <div class="row" style="margin-top:0.75rem">
        <ActionButton run={() => push('--all')} label="Push all" kind="primary" size="sm" icon="upload" doneLabel="Done" />
      </div>
    {/if}
  </Card>
{/if}

{#if pushMsg}<pre class="card mono small" style="white-space:pre-wrap;margin-top:0.75rem">{pushMsg}</pre>{/if}

{#if repo.remotes?.length}
  <Card title="Git remotes" icon="link" collapsible collapsed={true}>
    {#each repo.remotes as m}
      <p class="mono small break">{m.name}: {m.url}</p>
    {/each}
  </Card>
{/if}

<TargetsModal bind:data={targets} open={modal} onclose={() => (modal = false)} onsave={save} remotes={repo.remotes ?? []} />
