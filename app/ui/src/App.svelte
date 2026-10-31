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

  let config = $state(null);
  let tabs = $state([]);
  let loadError = $state('');
  let saveState = $state('');
  let lastSaved = '';
  let timer;

  // A section is a top-level page or one rules tab (`gitassist-app --section commit`).
  let section = $state(window.__GITASSIST_SECTION__ || location.hash.slice(1) || 'overview');
  $effect(() => history.replaceState(null, '', '#' + section));

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
    } catch (e) {
      loadError = String(e);
    }
  }

  // Autosave shortly after each change; the indicator shows when it landed.
  $effect(() => {
    if (!config) return;
    const snapshot = JSON.stringify(config);
    if (snapshot === lastSaved) return;
    saveState = 'saving';
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        await api.save(JSON.parse(snapshot));
        lastSaved = snapshot;
        saveState = 'saved';
        setTimeout(() => saveState === 'saved' && (saveState = ''), 1800);
      } catch (e) {
        saveState = 'error:' + e;
      }
    }, 450);
  });

  const tabById = (id) => tabs.find((t) => t.id === id);
  const activeGroup = $derived(NAV.find((n) => n.id === section || n.children?.includes(section))?.id);
  const activeNav = $derived(NAV.find((n) => n.id === activeGroup));
  // Repo group uses the shared page-tab strip; rules tabs render their own siblings strip.
  const repoSiblings = $derived(activeGroup === 'repogroup' ? activeNav?.children ?? [] : []);

  function go(id) {
    section = id;
  }

  function goNav(item) {
    if (item.children?.includes(section)) return; // already in this group — keep sub-page
    go(item.target ?? item.id);
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
      {#if section === 'overview'}
        <Overview bind:config onnavigate={go} />
      {:else if section === 'ignore'}
        <Ignore bind:config />
      {:else if section === 'settings' || section === 'defaults'}
        <Settings bind:config />
      {:else if section === 'branches'}
        <Branches bind:config />
      {:else if section === 'repos'}
        <Repos />
      {:else if section === 'activity'}
        <Activity {tabs} />
      {:else if tabById(section)}
        {#key section}
          <Rules bind:config tab={tabById(section)} siblings={NAV.find((n) => n.children?.includes(section))?.children ?? []} {tabs} onselect={go} />
        {/key}
      {:else}
        <Overview bind:config />
      {/if}
    {/if}
  </main>
</div>
