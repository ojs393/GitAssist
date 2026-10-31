<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Card from '../ui/Card.svelte';
  import PageHead from '../ui/PageHead.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import RepoAvatar from '../ui/RepoAvatar.svelte';
  import RepoDetail from './RepoDetail.svelte';

  let { focusPath = null, focusTab = 'work', onclearFocus, globalConfig = $bindable(), tabs = [] } = $props();
  let repos = $state(null);
  let error = $state('');
  let query = $state('');
  let filter = $state('all');
  let folder = $state('');
  let scanned = $state(false);
  let selected = $state(null);
  let detailTab = $state('work');

  async function load() {
    try {
      repos = await api.repos();
      if (!repos.length && !scanned) await find();
      applyFocus();
    } catch (e) {
      error = String(e);
    }
  }
  async function find(root) {
    scanned = true;
    repos = await api.discoverRepos(root);
    applyFocus();
  }
  function applyFocus() {
    if (!focusPath || !repos) return;
    const hit = repos.find((r) => r.path === focusPath);
    if (hit) {
      selected = hit;
      detailTab = focusTab || 'work';
    }
  }
  $effect(() => {
    focusPath;
    focusTab;
    applyFocus();
  });
  onMount(load);

  const OWNER = { personal: 'Personal', organization: 'Organization', unknown: 'Local' };
  const VIS = { public: ['eye', 'Public'], private: ['lock', 'Private'], unknown: ['circle', ''] };
  const latest = (r) => Math.max(r.last_commit ?? 0, r.last_push ?? 0, r.last_sync ?? 0);
  const shown = $derived(
    (repos ?? []).filter((r) => {
      if (filter === 'github' && !r.slug) return false;
      if (filter === 'local' && r.slug) return false;
      if (filter === 'hooks' && !r.local_hooks_path) return false;
      const q = query.trim().toLowerCase();
      return !q || [r.name, r.slug, r.branch, ...r.remotes.map((x) => x.url)].join(' ').toLowerCase().includes(q);
    }),
  );
  const pushed = $derived((repos ?? []).map((r) => r.last_push ?? 0).reduce((a, b) => Math.max(a, b), 0));
</script>

{#if selected}
  <RepoDetail
    repo={selected}
    initialTab={detailTab}
    bind:globalConfig
    {tabs}
    onback={() => {
      selected = null;
      onclearFocus?.();
    }}
  />
{:else}

<PageHead title="Repos" icon="layers" info="Local repos GitAssist has seen, remotes, and publish targets. Find adds ones you haven't touched yet.">
  {#snippet actions()}
    <ActionButton run={() => find()} label="Find" icon="search" kind="ghost" size="sm" doneLabel="Done" />
  {/snippet}
</PageHead>

{#if error}<div class="card error-card"><Icon name="alert" /> {error}</div>{/if}

{#if !repos}
  <div class="loading"><span class="spinner"></span></div>
{:else}
  <div class="stat-row">
    <div class="stat emphasis"><span class="stat-label"><Icon name="layers" size={13} /> repos</span><span class="stat-num">{repos.length}</span></div>
    <div class="stat quiet"><span class="stat-label"><Icon name="globe" size={13} /> with remote</span><span class="stat-num">{repos.filter((r) => r.slug).length}</span></div>
    <div class="stat quiet"><span class="stat-label"><Icon name="upload" size={13} /> last push</span><span class="stat-num">{pushed ? relativeTime(pushed) : 'none yet'}</span></div>
  </div>

  <div class="toolbar">
    <Segmented size="sm" value={filter} onchange={(f) => (filter = f)}
      options={[{ id: 'all', label: 'All' }, { id: 'github', label: 'With a remote', icon: 'upload' }, { id: 'local', label: 'Local only', icon: 'folder' }, { id: 'hooks', label: 'Own hooks', icon: 'link' }]} />
    <div class="search"><Icon name="search" size={15} /><input bind:value={query} placeholder="Search name, remote or branch" /></div>
  </div>

  {#if !shown.length}
    <div class="sheet empty"><Icon name="folder" size={26} /><p>{repos.length ? 'Nothing matches.' : 'No repos found yet. Commit in a repo, or choose a folder below.'}</p></div>
  {:else}
    <div class="repo-grid fill">
      {#each shown as r (r.path)}
        {@const own = OWNER[r.owner] ?? OWNER.unknown}
        {@const vis = VIS[r.visibility] ?? VIS.unknown}
        <div class="repo-card-wrap">
          <button type="button" class="repo-card" onclick={() => (selected = r)}>
            <RepoAvatar login={r.avatar_login || ''} owner={r.owner || 'unknown'} size={36} />
            <span class="repo-card-body">
              <strong>{r.name}</strong>
              <span class="muted-2 small">{r.slug ? `${r.host === 'github.com' ? '' : r.host + ' · '}${r.slug}` : 'No remote'} · {own}</span>
              <span class="muted small">{r.branch || '—'} · {r.last_push ? relativeTime(r.last_push) : r.last_commit ? relativeTime(r.last_commit) : '—'}</span>
              {#if r.slug && vis[1]}
                <span class="repo-vis pill {r.visibility === 'public' ? 'warn' : r.visibility === 'private' ? 'ok' : ''}"><Icon name={vis[0]} size={11} /> {vis[1]}</span>
              {/if}
            </span>
            <span class="go"><Icon name="chevron-right" size={14} /></span>
          </button>
          <div class="repo-card-actions">
            {#if r.local_hooks_path && !r.connected}
              <ActionButton run={async () => { await api.connectRepo(r.path); repos = await api.repos(); }} label="Connect" size="sm" doneLabel="Ok" go={true} />
            {/if}
            <Info align="right">
              <p class="small"><strong>{own}</strong>{latest(r) ? ` · ${relativeTime(latest(r))}` : ''}</p>
              <p class="mono small break">{r.path}</p>
              {#each r.remotes as m}<p class="mono small break">{m.name}: {m.url}</p>{/each}
            </Info>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <Card title="Look in another folder" icon="search" info="Folders like Desktop and Documents are only searched when you choose them, because macOS asks first." quiet collapsible collapsed={true}>
    <div class="row">
      <input class="grow-input" bind:value={folder} placeholder="For example ~/Desktop or ~/Documents/code" />
      <ActionButton run={() => find(folder)} label="Search" kind="ghost" size="sm" disabled={!folder.trim()} doneLabel="Done" icon="search" />
    </div>
  </Card>
{/if}
{/if}
