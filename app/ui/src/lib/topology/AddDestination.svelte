<script>
  // Multi-import sheet. Task 8 (Overview graph) and Task 9 (working-copy graph)
  // mount this with: open, repo, existingTargets, onsave, onclose.
  // onsave(created) receives only the new targets. The caller appends and saves.
  // Listing remotes does not create anything. Confirm does.
  import Modal from '../ui/Modal.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import Icon from '../ui/Icon.svelte';
  import {
    importableRemotes,
    planImports,
    buildImportedTargets,
    labelForRemote,
    roleForRemote,
  } from './importTargets.js';

  let {
    open = false,
    repo = null,
    existingTargets = [],
    onsave,
    onclose,
  } = $props();

  let picked = $state([]);
  let wasOpen = false;

  const candidates = $derived(importableRemotes(repo, existingTargets));
  const chosen = $derived(candidates.filter((r) => picked.includes(r.name)));
  const planned = $derived(planImports(repo, existingTargets, chosen));

  $effect(() => {
    if (open && !wasOpen) picked = [];
    wasOpen = open;
  });

  function toggle(name) {
    picked = picked.includes(name) ? picked.filter((n) => n !== name) : [...picked, name];
  }

  function selectAll() {
    picked = candidates.map((r) => r.name);
  }

  function rowLabel(remote) {
    const hit = planned.find((p) => p.remote_name === remote.name);
    return hit ? hit.label : labelForRemote(remote, repo);
  }

  function roleIcon(role) {
    if (role === 'public') return 'eye';
    if (role === 'personal') return 'user';
    if (role === 'organization') return 'building';
    return 'link';
  }

  async function confirm() {
    if (!chosen.length) throw new Error('Select at least one remote.');
    const created = buildImportedTargets(repo, existingTargets, chosen);
    await onsave?.(created);
    onclose?.();
  }
</script>

<Modal {open} {onclose} title="Add destinations">
  <p class="muted small">Pick git remotes that are not destinations yet. Nothing is added until you confirm.</p>
  {#if !candidates.length}
    <div class="empty">
      <Icon name="link" size={22} />
      <p>{repo?.remotes?.length ? 'Every remote is already a destination.' : 'No git remotes on this repo.'}</p>
    </div>
  {:else}
    <div class="import-list" role="group" aria-label="Remotes to import">
      {#each candidates as r (r.name)}
        {@const role = roleForRemote(r)}
        <button
          type="button"
          class="import-row"
          class:on={picked.includes(r.name)}
          aria-pressed={picked.includes(r.name)}
          onclick={() => toggle(r.name)}
        >
          <span class="agent-glyph"><Icon name={roleIcon(role)} size={15} /></span>
          <span class="repo-name">
            <strong>{r.name}</strong>
            <span class="muted-2 small">{rowLabel(r)} · {role}{r.url ? ` · ${r.url}` : ''}</span>
          </span>
          {#if picked.includes(r.name)}<Icon name="check" size={14} />{/if}
        </button>
      {/each}
    </div>
    <div class="row" style="margin-top:1rem;gap:0.5rem">
      <button type="button" class="btn ghost sm" onclick={selectAll}>Select all</button>
      <ActionButton
        run={confirm}
        label={chosen.length ? `Add ${chosen.length}` : 'Add'}
        kind="primary"
        size="sm"
        icon="check"
        disabled={!chosen.length}
        doneLabel="Added"
      />
    </div>
  {/if}
</Modal>

<style>
  .import-list { display: flex; flex-direction: column; gap: 2px; margin-top: 0.75rem; }
  .import-row {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 10px 6px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
    width: 100%;
  }
  .import-row:hover, .import-row.on { background: var(--accent-softer); }
  .import-row.on { color: var(--accent); }
</style>
