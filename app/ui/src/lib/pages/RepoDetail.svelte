<script>
  import { api, NAV, TAB_ICONS } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Card from '../ui/Card.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import Rules from './Rules.svelte';
  import TargetsModal from './TargetsModal.svelte';

  let { repo, onback, initialTab = 'work', globalConfig = $bindable(), tabs = [] } = $props();

  let panel = $state('work');
  let targets = $state({ repo_path: '', default_target: null, targets: [] });
  let work = $state(null);
  let showDiff = $state(false);
  let error = $state('');
  let pushMsg = $state('');
  let modal = $state(false);
  let loading = $state(true);
  let overrideConfig = $state(null);
  let overridePath = $state('');
  let saveNote = $state('');

  const PANELS = [
    { id: 'work', label: 'Work', icon: 'activity' },
    { id: 'publish', label: 'Publish', icon: 'upload' },
    { id: 'commit', label: 'Commits', icon: 'commit' },
    { id: 'pr_create', label: 'Pull requests', icon: 'pr' },
  ];

  const commitSiblings = NAV.find((n) => n.id === 'commits')?.children ?? ['commit'];
  const prSiblings = NAV.find((n) => n.id === 'prs')?.children ?? ['pr_create'];

  async function load(path) {
    loading = true;
    error = '';
    try {
      const [t, w, ov] = await Promise.all([
        api.targetsGet(path),
        api.repoWork(path).catch(() => null),
        api.overridesGet(path).catch(() => ({ tabs: {} })),
      ]);
      targets = t?.targets ? t : { ...t, targets: [] };
      work = w;
      overridePath = path;
      // Clone global config; overlay this repo's tab customizations.
      const base = JSON.parse(JSON.stringify(globalConfig));
      for (const [id, partial] of Object.entries(ov.tabs || {})) {
        base.tabs[id] = { ...(base.tabs[id] || {}), ...partial };
      }
      overrideConfig = base;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (repo?.path) load(repo.path);
  });
  $effect(() => {
    panel = initialTab || 'work';
  });

  // Persist only the active rules tab overlay when editing Commits/PRs for this repo.
  let saveTimer;
  $effect(() => {
    if (!overrideConfig || !overridePath || !globalConfig) return;
    if (panel !== 'commit' && panel !== 'pr_create' && !commitSiblings.includes(panel) && !prSiblings.includes(panel)) return;
    const snap = JSON.stringify(overrideConfig);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      try {
        const ov = await api.overridesGet(overridePath);
        const tabsOut = { ...(ov.tabs || {}) };
        const ids = [...commitSiblings, ...prSiblings];
        for (const id of ids) {
          const local = overrideConfig.tabs[id] || {};
          const glob = globalConfig.tabs[id] || {};
          if (JSON.stringify(local) !== JSON.stringify(glob)) tabsOut[id] = local;
          else delete tabsOut[id];
        }
        await api.overridesSave(overridePath, { repo_path: overridePath, tabs: tabsOut });
        saveNote = 'Saved for this repo';
        setTimeout(() => (saveNote = ''), 1600);
      } catch (e) {
        error = String(e);
      }
    }, 500);
  });

  async function save(data) {
    const notes = await api.targetsSave(repo.path, data);
    targets = await api.targetsGet(repo.path);
    modal = false;
    pushMsg = notes?.length ? notes.join('; ') : 'Saved.';
  }

  async function push(which) {
    pushMsg = '';
    const job = await api.jobStartPublish(repo.path, which);
    pushMsg = `Queued ${job.label}. Watch Overview → Runs.`;
  }

  async function refreshWork() {
    work = await api.repoWork(repo.path);
  }

  async function resetOverride(tabId) {
    await api.overridesClearTab(repo.path, tabId);
    await load(repo.path);
  }

  const roleIcon = (role) => (role === 'public' ? 'eye' : role === 'organization' ? 'building' : 'user');
  const tabById = (id) => tabs.find((t) => t.id === id);
  const openWork = $derived(work && (work.ahead > 0 || work.dirty > 0 || work.staged > 0));
</script>

<header class="page-head">
  <div class="page-title">
    <button type="button" class="icon-btn" aria-label="Back" onclick={onback}><Icon name="chevron-left" size={18} /></button>
    <span class="page-icon"><Icon name="folder" size={18} /></span>
    <h1>{repo.name}</h1>
    <Info>
      <p>Work, publish targets, and per-repo rule overrides. Global Commits/PR pages set defaults for every project.</p>
    </Info>
  </div>
  <div class="page-actions">
    {#if saveNote}<span class="save-pill ok"><Icon name="check" size={13} />{saveNote}</span>{/if}
    <ActionButton run={async () => (modal = true)} label="Configure" icon="sliders" size="sm" go={true} />
  </div>
</header>

<div class="tab-bar">
  <div class="subtabs">
    {#each PANELS as p}
      <button type="button" class:active={panel === p.id || (p.id === 'commit' && commitSiblings.includes(panel)) || (p.id === 'pr_create' && prSiblings.includes(panel))} onclick={() => (panel = p.id)}>
        <Icon name={p.icon} size={13} /> {p.label}
      </button>
    {/each}
  </div>
</div>

{#if error}<div class="card error-card"><Icon name="alert" /> {error}</div>{/if}

{#if loading}
  <div class="loading"><span class="spinner"></span></div>
{:else if panel === 'work'}
  <div class="stat-row">
    <div class="stat emphasis">
      <span class="stat-label"><Icon name="upload" size={13} /> ahead</span>
      <span class="stat-num">{work?.ahead ?? 0}</span>
    </div>
    <div class="stat quiet">
      <span class="stat-label"><Icon name="edit" size={13} /> dirty</span>
      <span class="stat-num">{work?.dirty ?? 0}</span>
    </div>
    <div class="stat quiet">
      <span class="stat-label"><Icon name="activity" size={13} /> diff</span>
      <span class="stat-num"><span class="ok">+{work?.additions ?? 0}</span> <span class="bad">−{work?.deletions ?? 0}</span></span>
    </div>
  </div>
  <p class="mono small muted-2 break path-line">{repo.path}</p>
  <Card title="Local work" icon="activity" info="Unpushed commits and uncommitted changes on this machine." raised>
    <div class="row" style="margin-bottom:10px">
      <span class="muted small">{work?.branch || repo.branch || '—'}{#if work?.upstream} → {work.upstream}{/if}{#if work?.behind} · {work.behind} behind{/if}</span>
      <span class="grow"></span>
      <ActionButton run={refreshWork} label="Refresh" icon="refresh" kind="ghost" size="sm" doneLabel="Ok" />
      <button type="button" class="btn ghost sm" class:primary={showDiff} onclick={() => (showDiff = !showDiff)}>
        <Icon name="layout" size={13} /> {showDiff ? 'Hide changes' : 'View changes'}
      </button>
      <ActionButton run={() => push('--all')} label="Push" icon="upload" size="sm" doneLabel="Queued" go={true} disabled={!openWork && !(work?.ahead > 0)} />
    </div>
    {#if !openWork && !(work?.ahead > 0)}
      <p class="muted small">Clean — nothing waiting to push.</p>
    {:else}
      <p class="small">{work?.ahead ? `${work.ahead} commit${work.ahead === 1 ? '' : 's'} ahead of remote` : 'No unpushed commits'}{#if work?.dirty} · {work.dirty} changed file{work.dirty === 1 ? '' : 's'}{/if}</p>
    {/if}
    {#if showDiff && work?.files?.length}
      <ul class="diff-files">
        {#each work.files as f}
          <li>
            <span class="mono pill">{f.status}</span>
            <span class="mono grow break">{f.path}</span>
            <span class="ok small">+{f.additions}</span>
            <span class="bad small">−{f.deletions}</span>
          </li>
        {/each}
      </ul>
    {:else if showDiff}
      <p class="muted small">No file-level diff (untracked-only or binary).</p>
    {/if}
  </Card>
  {#if pushMsg}<pre class="card quiet mono small push-out">{pushMsg}</pre>{/if}

{:else if panel === 'publish'}
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
  <Card title="Publish targets" icon="upload" info="Each target can have its own filter, rules, and pre-push checks." raised>
    {#if !targets.targets.length}
      <div class="empty"><Icon name="upload" size={24} /><p>No targets yet. Configure to add remotes.</p>
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
                <ActionButton run={() => push(t.label)} label="Push" icon="upload" size="sm" doneLabel="Queued" />
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
  {#if pushMsg}<pre class="card quiet mono small push-out">{pushMsg}</pre>{/if}

{:else if overrideConfig && (panel === 'commit' || commitSiblings.includes(panel))}
  {@const tid = commitSiblings.includes(panel) ? panel : 'commit'}
  {@const tmeta = tabById(tid)}
  <p class="banner muted small">Defaults come from the header Commits pages. Changes here apply only to <strong>{repo.name}</strong>.
    <button type="button" class="link-text" onclick={() => resetOverride(tid)}>Reset this tab</button>
  </p>
  {#if tmeta}
    {#key tid}
      <Rules bind:config={overrideConfig} tab={tmeta} siblings={commitSiblings} {tabs} onselect={(id) => (panel = id)} embedded={true} />
    {/key}
  {/if}

{:else if overrideConfig && (panel === 'pr_create' || prSiblings.includes(panel))}
  {@const tid = prSiblings.includes(panel) ? panel : 'pr_create'}
  {@const tmeta = tabById(tid)}
  <p class="banner muted small">Defaults come from the header Pull requests pages. Changes here apply only to <strong>{repo.name}</strong>.
    <button type="button" class="link-text" onclick={() => resetOverride(tid)}>Reset this tab</button>
  </p>
  {#if tmeta}
    {#key tid}
      <Rules bind:config={overrideConfig} tab={tmeta} siblings={prSiblings} {tabs} onselect={(id) => (panel = id)} embedded={true} />
    {/key}
  {/if}
{/if}

{#if repo.remotes?.length && panel === 'publish'}
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
  .diff-files { list-style: none; margin: 10px 0 0; padding: 0; display: flex; flex-direction: column; gap: 4px; max-height: 280px; overflow: auto; }
  .diff-files li { display: flex; align-items: center; gap: 8px; padding: 4px 0; }
  .banner { margin: 0 0 10px; }
  .grow { flex: 1; }
</style>
