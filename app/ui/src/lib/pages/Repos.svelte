<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Card from '../ui/Card.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import ActionButton from '../ui/ActionButton.svelte';

  import RepoDetail from './RepoDetail.svelte';

  let repos = $state(null);
  let error = $state('');
  let query = $state('');
  let filter = $state('all');
  let folder = $state('');
  let scanned = $state(false);
  let selected = $state(null);

  async function load() {
    try {
      repos = await api.repos();
      if (!repos.length && !scanned) await find();
    } catch (e) {
      error = String(e);
    }
  }
  async function find(root) {
    scanned = true;
    repos = await api.discoverRepos(root);
  }
  onMount(load);

  const OWNER = { personal: ['user', 'Personal'], organization: ['building', 'Organization'], unknown: ['folder', 'Local'] };
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
  <RepoDetail repo={selected} onback={() => (selected = null)} />
{:else}

<header class="page-head">
  <div class="page-title">
    <span class="page-icon"><Icon name="folder" size={20} /></span>
    <h1>Your repos</h1>
    <Info><p>Every repo on this computer GitAssist knows about, where it pushes, and when you last committed and pushed. It updates itself as you work; Find repos adds ones you haven't touched yet.</p></Info>
  </div>
  <ActionButton run={() => find()} label="Find repos" icon="search" kind="ghost" size="sm" doneLabel="Updated" />
</header>

{#if error}<div class="card error-card"><Icon name="alert" /> {error}</div>{/if}

{#if !repos}
  <div class="loading"><span class="spinner"></span></div>
{:else}
  <div class="stat-row">
    <div class="stat"><span class="stat-num">{repos.length}</span><span class="muted small">repos</span></div>
    <div class="stat"><span class="stat-num">{repos.filter((r) => r.slug).length}</span><span class="muted small">on GitHub or another host</span></div>
    <div class="stat"><span class="stat-num">{pushed ? relativeTime(pushed) : 'none yet'}</span><span class="muted small">last push</span></div>
  </div>

  <div class="toolbar">
    <Segmented size="sm" value={filter} onchange={(f) => (filter = f)}
      options={[{ id: 'all', label: 'All' }, { id: 'github', label: 'With a remote', icon: 'upload' }, { id: 'local', label: 'Local only', icon: 'folder' }, { id: 'hooks', label: 'Own hooks', icon: 'link' }]} />
    <div class="search"><Icon name="search" size={15} /><input bind:value={query} placeholder="Search name, remote or branch" /></div>
  </div>

  <Card>
    {#if !shown.length}
      <div class="empty"><Icon name="folder" size={26} /><p>{repos.length ? 'Nothing matches.' : 'No repos found yet. Commit in a repo, or choose a folder below.'}</p></div>
    {:else}
      <div class="repo-head"><span></span><span>Repo</span><span>Visibility</span><span>Branch</span><span>Committed</span><span>Pushed</span><span></span></div>
      <ul class="repo-rows">
        {#each shown as r (r.path)}
          {@const own = OWNER[r.owner] ?? OWNER.unknown}
          {@const vis = VIS[r.visibility] ?? VIS.unknown}
          <li class="repo-row repo-row-click" role="button" tabindex="0"
            onclick={() => (selected = r)}
            onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (selected = r)}>
            <span class="agent-glyph" title={own[1]}><Icon name={own[0]} size={16} /></span>
            <span class="repo-name">
              <strong>{r.name}</strong>
              <span class="muted small">{r.slug ? `${r.host === 'github.com' ? '' : r.host + ' · '}${r.slug}` : 'No remote'}</span>
            </span>
            <span>{#if r.slug}<span class="pill {r.visibility === 'public' ? 'warn' : r.visibility === 'private' ? 'ok' : ''}">{#if vis[1]}<Icon name={vis[0]} size={12} /> {vis[1]}{:else}Not checked{/if}</span>{/if}</span>
            <span class="mono small repo-branch" title={r.branch ?? ''}>{r.branch ?? '—'}</span>
            <span class="small muted">{r.last_commit ? relativeTime(r.last_commit) : '—'}</span>
            <span class="small muted" title={r.last_push_remote ? `to ${r.last_push_remote}` : r.last_sync ? 'last fetch or push' : ''}>{r.last_push ? relativeTime(r.last_push) : r.last_sync ? relativeTime(r.last_sync) : '—'}</span>
            <span class="repo-end" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
              {#if r.local_hooks_path && !r.connected}
                <ActionButton run={async () => { await api.connectRepo(r.path); repos = await api.repos(); }} label="Connect" size="sm" doneLabel="Connected" />
              {/if}
              <Info align="right">
                <p class="small"><strong>{own[1]} repo</strong>{latest(r) ? ` · active ${relativeTime(latest(r))}` : ''}</p>
                <p class="mono small break">{r.path}</p>
                {#each r.remotes as m}<p class="mono small break">{m.name}: {m.url}</p>{/each}
                {#if r.local_hooks_path}<p class="small">Uses its own hooks ({r.local_hooks_path}){r.connected ? ', connected to GitAssist' : '. Connect adds GitAssist to them.'}</p>{/if}
              </Info>
            </span>
          </li>
        {/each}
      </ul>
    {/if}
  </Card>

  <Card title="Look in another folder" icon="search" info="Folders like Desktop and Documents are only searched when you choose them, because macOS asks first." collapsible collapsed={true}>
    <div class="row">
      <input class="grow-input" bind:value={folder} placeholder="For example ~/Desktop or ~/Documents/code" />
      <ActionButton run={() => find(folder)} label="Search" kind="ghost" size="sm" disabled={!folder.trim()} doneLabel="Done" />
    </div>
  </Card>
{/if}
{/if}

<style>
  .repo-row-click { cursor: pointer; }
  .repo-row-click:hover { background: color-mix(in srgb, var(--fg, #111) 4%, transparent); }
</style>
