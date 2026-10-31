<script>
  import { onMount } from 'svelte';
  import { api, NAV, TAB_ICONS, PAGES } from './lib/api.js';
  import Icon from './lib/ui/Icon.svelte';
  import logo from './assets/logo.png';
  import Toggle from './lib/ui/Toggle.svelte';
  import Overview from './lib/pages/Overview.svelte';
  import Rules from './lib/pages/Rules.svelte';
  import Ignore from './lib/pages/Ignore.svelte';
  import Settings from './lib/pages/Settings.svelte';
  import Branches from './lib/pages/Branches.svelte';
  import Repos from './lib/pages/Repos.svelte';
  import Activity from './lib/pages/Activity.svelte';
  import LocalDetail from './lib/pages/LocalDetail.svelte';
  import DestinationDetail from './lib/pages/DestinationDetail.svelte';
  import { parseDestinationHash } from './lib/topology/destinationId.js';

  let config = $state(null);
  let tabs = $state([]);
  let loadError = $state('');
  let saveState = $state('');
  let lastSaved = '';
  let timer;
  let flight = false;
  let again = false;
  let repoList = $state([]);
  let focusPath = $state(null);
  let focusTab = $state('work');

  // A section is a top-level page or one rules tab (`gitassist-app --section commit`).
  let section = $state(window.__GITASSIST_SECTION__ || location.hash.slice(1) || 'overview');
  $effect(() => history.replaceState(null, '', '#' + section));

  function routeOf(section) {
    if (section === 'local') return { kind: 'local' };
    const d = parseDestinationHash(section);
    if (d) return { kind: 'destination', ...d };
    return { kind: 'section', id: section || 'overview' };
  }

  const route = $derived(routeOf(section));

  onMount(() => {
    const onHash = () => (section = location.hash.slice(1) || 'overview');
    window.addEventListener('hashchange', onHash);
    load();
    return () => window.removeEventListener('hashchange', onHash);
  });

  async function load() {
    try {
      const s = await api.state();
      config = s.config;
      tabs = s.tabs;
      lastSaved = JSON.stringify(s.config);
      repoList = await api.repos().catch(() => []);
    } catch (e) {
      loadError = String(e);
    }
  }

  // One save at a time. A toggle back to the last saved value cancels the
  // debounce, and a write that lands after the user moved on is followed
  // by a write of the live config so the file cannot stay on the old value.
  async function flushSave() {
    if (!config) return;
    if (flight) {
      again = true;
      return;
    }
    const snapshot = JSON.stringify(config);
    if (snapshot === lastSaved) {
      again = false;
      if (saveState === 'saving') saveState = '';
      return;
    }
    flight = true;
    again = false;
    saveState = 'saving';
    try {
      await api.save(JSON.parse(snapshot));
      let written = snapshot;
      let now = JSON.stringify(config);
      if (now !== written) {
        await api.save(JSON.parse(now));
        written = now;
        now = JSON.stringify(config);
      }
      if (now === written) {
        lastSaved = written;
        saveState = 'saved';
        setTimeout(() => saveState === 'saved' && (saveState = ''), 1800);
      } else {
        again = true;
      }
    } catch (e) {
      const now = config ? JSON.stringify(config) : snapshot;
      if (now === snapshot) saveState = 'error:' + e;
      else again = true;
    } finally {
      flight = false;
      if (again) {
        again = false;
        await flushSave();
      }
    }
  }

  $effect(() => {
    if (!config) return;
    const snapshot = JSON.stringify(config);
    clearTimeout(timer);
    if (snapshot === lastSaved) {
      if (!flight && saveState === 'saving') saveState = '';
      return;
    }
    saveState = 'saving';
    timer = setTimeout(flushSave, 450);
  });

  const tabById = (id) => tabs.find((t) => t.id === id);
  const activeGroup = $derived(NAV.find((n) => n.id === section || n.children?.includes(section))?.id);
  const activeNav = $derived(NAV.find((n) => n.id === activeGroup));
  const repoSiblings = $derived(activeGroup === 'repogroup' ? activeNav?.children ?? [] : []);

  function go(id, opts = {}) {
    if (opts.repo) {
      focusPath = opts.repo;
      focusTab = opts.tab || 'work';
    } else if (id !== 'repos') {
      focusPath = null;
    }
    section = id;
  }

  function goNav(item) {
    if (item.children?.includes(section)) return;
    go(item.target ?? item.id);
  }

  function openRepo(path, tab = 'work') {
    go('repos', { repo: path, tab });
  }
</script>

<div class="app">
  <header class="topbar">
    <div class="brand"><img src={logo} alt="" width="22" height="22" /><span>GitAssist</span></div>
    <nav class="nav" aria-label="Main">
      {#each NAV as item}
        <button
          type="button"
          class="nav-btn"
          class:active={activeGroup === item.id}
          onclick={() => goNav(item)}
        >
          <Icon name={item.icon} size={16} /> {item.label}
        </button>
      {/each}
    </nav>
    <div class="top-right">
      {#if saveState === 'saving'}<span class="save-pill"><span class="spinner sm"></span>Saving</span>
      {:else if saveState === 'saved'}<span class="save-pill ok"><Icon name="check" size={13} />Saved</span>
      {:else if saveState.startsWith('error:')}<span class="save-pill bad" title={saveState.slice(6)}><Icon name="alert" size={13} />Not saved</span>{/if}
      {#if config}
        <Toggle checked={config.enabled} onchange={(v) => (config.enabled = v)} label={config.enabled ? 'On' : 'Paused'} />
      {/if}
      <button class="icon-btn gear" class:active={section === 'settings'} title="Settings" aria-label="Settings" onclick={() => go('settings')}><Icon name="gear" size={18} /></button>
    </div>
  </header>

  <main class="content">
    {#if loadError}
      <div class="card error-card"><Icon name="alert" /> Settings could not be loaded: {loadError}</div>
    {:else if !config}
      <div class="loading"><span class="spinner"></span></div>
    {:else}
      {#if repoSiblings.length > 1}
        <div class="tab-bar">
          <div class="subtabs">
            {#each repoSiblings as id}
              {@const t = PAGES[id]}
              {#if t}
                <button type="button" class:active={section === id} onclick={() => go(id)}>
                  <Icon name={t.icon} size={13} /> {t.label}
                </button>
              {/if}
            {/each}
          </div>
        </div>
      {/if}
      {#if route.kind === 'local'}
        <LocalDetail bind:config {tabs} onnavigate={go} onback={() => go('overview')} />
      {:else if route.kind === 'destination'}
        <DestinationDetail repoKey={route.repoKey} targetId={route.targetId} onback={() => go('overview')} />
      {:else if section === 'overview'}
        <Overview bind:config onnavigate={go} onopenrepo={openRepo} />
      {:else if section === 'ignore'}
        <Ignore bind:config />
      {:else if section === 'settings' || section === 'defaults'}
        <Settings bind:config />
      {:else if section === 'branches'}
        <Branches bind:config />
      {:else if section === 'repos'}
        <Repos
          {focusPath}
          {focusTab}
          bind:globalConfig={config}
          {tabs}
          onclearFocus={() => (focusPath = null)}
        />
      {:else if section === 'activity'}
        <Activity {tabs} />
      {:else if tabById(section)}
        {#key section}
          <Rules
            bind:config
            tab={tabById(section)}
            siblings={NAV.find((n) => n.children?.includes(section))?.children ?? []}
            {tabs}
            onselect={go}
            repoOptions={repoList}
            appliesPath=""
            onapplies={(path) => {
              if (!path) return;
              const tab = NAV.find((n) => n.id === 'commits')?.children?.includes(section)
                ? 'commit'
                : NAV.find((n) => n.id === 'prs')?.children?.includes(section)
                  ? 'pr_create'
                  : 'work';
              openRepo(path, section && (section.startsWith('pr') || section === 'review' || section === 'issue') ? (section === 'pr_update' || section === 'pr_comment' || section === 'review' || section === 'issue' ? section : 'pr_create') : (['commit', 'docs_commit', 'merge'].includes(section) ? section : tab));
            }}
          />
        {/key}
      {:else}
        <Overview bind:config onnavigate={go} onopenrepo={openRepo} />
      {/if}
    {/if}
  </main>
</div>
