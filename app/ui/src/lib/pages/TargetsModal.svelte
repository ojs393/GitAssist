<script>
  import Modal from '../ui/Modal.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import Icon from '../ui/Icon.svelte';
  import TargetEditor from './TargetEditor.svelte';

  let {
    open = false,
    onclose,
    onsave,
    remotes = [],
    data = $bindable({ repo_path: '', default_target: null, targets: [] }),
  } = $props();

  let editIdx = $state(-1);
  let draft = $state(null);
  let error = $state('');

  function blank() {
    return {
      id: '',
      label: '',
      role: 'custom',
      remote_name: '',
      url: '',
      enabled: true,
      push_policy: 'manual',
      include_in_push_all: true,
      publish_filter: [],
      use_mirror: false,
      tabs: null,
      branches: null,
      checks: { enabled: false, on_fail: 'block', timeout_secs: 600, commands: [] },
    };
  }

  function openNew() {
    draft = blank();
    editIdx = -1;
    error = '';
  }

  function openEdit(i) {
    draft = structuredClone($state.snapshot(data.targets[i]));
    if (!draft.checks) draft.checks = blank().checks;
    if (!Array.isArray(draft.publish_filter)) draft.publish_filter = [];
    editIdx = i;
    error = '';
  }

  function importRemote(r) {
    draft = blank();
    draft.label = r.name === 'origin' ? 'personal' : r.name;
    draft.remote_name = r.name;
    draft.url = r.url;
    draft.role = r.name === 'origin' ? 'personal' : 'custom';
    editIdx = -1;
  }

  function applyDraft() {
    if (!draft.label.trim() || !draft.remote_name.trim()) {
      error = 'Label and remote name are required.';
      return;
    }
    const next = structuredClone($state.snapshot(data));
    if (editIdx >= 0) next.targets[editIdx] = $state.snapshot(draft);
    else next.targets.push($state.snapshot(draft));
    data = next;
    draft = null;
    error = '';
  }

  function removeAt(i) {
    const next = structuredClone($state.snapshot(data));
    next.targets.splice(i, 1);
    data = next;
  }

  async function save() {
    error = '';
    await onsave?.(data);
  }
</script>

<Modal {open} {onclose} title="Configure publish targets" wide={true}>
  {#if draft}
    {#if error}<p class="muted small" style="color:var(--bad)">{error}</p>{/if}
    <TargetEditor bind:target={draft} bind:defaultTarget={data.default_target} />
    <div class="row" style="margin-top:1rem;gap:0.5rem">
      <ActionButton run={async () => applyDraft()} label="Apply target" kind="primary" size="sm" icon="check" />
      <button type="button" class="btn ghost sm" onclick={() => { draft = null; }}><Icon name="chevron-left" size={14} /> Back</button>
    </div>
  {:else}
    {#if remotes.length}
      <p class="muted-2 small">Import from existing remotes</p>
      <div class="row" style="flex-wrap:wrap;gap:0.4rem;margin-bottom:0.75rem">
        {#each remotes as r}
          <button type="button" class="btn ghost sm" onclick={() => importRemote(r)}><Icon name="link" size={13} /> {r.name} <span class="go"><Icon name="chevron-right" size={13} /></span></button>
        {/each}
      </div>
    {/if}
    {#if !data.targets.length}
      <div class="empty"><Icon name="upload" size={22} /><p>No targets yet. Add personal, public, or org.</p></div>
    {:else}
      <ul class="target-edit-list">
        {#each data.targets as t, i}
          <li class="target-edit-row">
            <span class="agent-glyph"><Icon name={t.role === 'public' ? 'eye' : t.role === 'organization' ? 'building' : 'user'} size={15} /></span>
            <span class="repo-name">
              <strong>{t.label}</strong>
              <span class="muted-2 small">{t.role} · {t.remote_name} · {t.push_policy}</span>
            </span>
            <span class="repo-end">
              <button type="button" class="btn ghost sm" onclick={() => openEdit(i)}>Edit <span class="go"><Icon name="chevron-right" size={13} /></span></button>
              <button type="button" class="btn danger sm" onclick={() => removeAt(i)}><Icon name="trash" size={13} /></button>
            </span>
          </li>
        {/each}
      </ul>
    {/if}
    <div class="row" style="margin-top:1rem;gap:0.5rem">
      <ActionButton run={async () => openNew()} label="Add target" icon="plus" size="sm" go={true} />
      <ActionButton run={save} label="Save" kind="primary" size="sm" icon="check" doneLabel="Saved" />
    </div>
  {/if}
</Modal>

<style>
  .target-edit-list { list-style: none; margin: 0; padding: 0; }
  .target-edit-row {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 10px 6px;
    border-radius: 10px;
  }
  .target-edit-row:hover { background: var(--accent-softer); }
  .target-edit-row + .target-edit-row { margin-top: 2px; }
</style>
