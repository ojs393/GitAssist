<script>
  import Modal from '../ui/Modal.svelte';
  import Field from '../ui/Field.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import Icon from '../ui/Icon.svelte';
  import ChipList from '../ui/ChipList.svelte';

  let {
    open = false,
    onclose,
    onsave,
    remotes = [],
    data = $bindable({ repo_path: '', default_target: null, targets: [] }),
  } = $props();

  let tab = $state('list');
  let editIdx = $state(-1);
  let draft = $state(null);
  let error = $state('');

  const ROLES = [
    { id: 'personal', label: 'Personal' },
    { id: 'public', label: 'Public' },
    { id: 'organization', label: 'Org' },
    { id: 'custom', label: 'Custom' },
  ];
  const POLICIES = [
    { id: 'manual', label: 'Manual' },
    { id: 'after_commit', label: 'After commit' },
    { id: 'with_all', label: 'With push-all' },
    { id: 'agent_only', label: 'Agent / CLI' },
  ];

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
    tab = 'identity';
    error = '';
  }

  function openEdit(i) {
    draft = structuredClone(data.targets[i]);
    if (!draft.checks) draft.checks = blank().checks;
    if (!Array.isArray(draft.publish_filter)) draft.publish_filter = [];
    editIdx = i;
    tab = 'identity';
    error = '';
  }

  function importRemote(r) {
    draft = blank();
    draft.label = r.name === 'origin' ? 'personal' : r.name;
    draft.remote_name = r.name;
    draft.url = r.url;
    draft.role = r.name === 'origin' ? 'personal' : 'custom';
    editIdx = -1;
    tab = 'identity';
  }

  function applyDraft() {
    if (!draft.label.trim() || !draft.remote_name.trim()) {
      error = 'Label and remote name are required.';
      return;
    }
    const next = structuredClone(data);
    if (editIdx >= 0) next.targets[editIdx] = draft;
    else next.targets.push(draft);
    data = next;
    draft = null;
    tab = 'list';
    error = '';
  }

  function removeAt(i) {
    const next = structuredClone(data);
    next.targets.splice(i, 1);
    data = next;
  }

  async function save() {
    error = '';
    await onsave?.(data);
  }

  const filterText = $derived((draft?.publish_filter ?? []).join('\n'));
  function setFilter(text) {
    draft.publish_filter = text.split('\n').map((l) => l.trimEnd()).filter((l) => l.length);
  }
</script>

<Modal {open} {onclose} title="Configure publish targets" wide={true}>
  {#if draft}
    <div class="row" style="margin-bottom:0.75rem">
      <Segmented
        size="sm"
        value={tab}
        onchange={(v) => (tab = v)}
        options={[
          { id: 'identity', label: 'Identity' },
          { id: 'policy', label: 'Push' },
          { id: 'filter', label: 'Filter' },
          { id: 'rules', label: 'Rules' },
          { id: 'checks', label: 'Checks' },
        ]}
      />
    </div>
    {#if error}<p class="muted small" style="color:var(--bad)">{error}</p>{/if}
    {#if tab === 'identity'}
      <Field label="Label" info="What agents say: gitassist push personal"><input bind:value={draft.label} placeholder="personal" /></Field>
      <Field label="Role">
        <Segmented size="sm" value={draft.role} onchange={(v) => (draft.role = v)} options={ROLES} />
      </Field>
      <Field label="Git remote name"><input bind:value={draft.remote_name} placeholder="origin" /></Field>
      <Field label="Remote URL"><input bind:value={draft.url} placeholder="git@github.com:you/repo.git" /></Field>
      <Field label="Enabled"><Toggle checked={draft.enabled} onchange={(v) => (draft.enabled = v)} /></Field>
    {:else if tab === 'policy'}
      <Field label="When to push" info="After commit is best-effort and never fails the commit.">
        <Segmented size="sm" value={draft.push_policy} onchange={(v) => (draft.push_policy = v)} options={POLICIES} />
      </Field>
      <Field label="Include in push --all"><Toggle checked={draft.include_in_push_all} onchange={(v) => (draft.include_in_push_all = v)} /></Field>
      <Field label="Default target for bare gitassist push">
        <Toggle
          checked={data.default_target === draft.label}
          onchange={(v) => (data.default_target = v ? draft.label : null)}
        />
      </Field>
    {:else if tab === 'filter'}
      <Field label="Publish filter" info="gitignore patterns never sent to this target. Uses a cleaned mirror under the hood.">
        <textarea rows="8" value={filterText} oninput={(e) => setFilter(e.currentTarget.value)} placeholder={'.env\n*.pem\n.local/'}></textarea>
      </Field>
      <Field label="Use mirror" info="On when a filter is set. Keeps your working tree unchanged.">
        <Toggle checked={draft.use_mirror || !!draft.publish_filter.length} onchange={(v) => (draft.use_mirror = v)} />
      </Field>
    {:else if tab === 'rules'}
      <p class="muted small">Optional overrides for this target only. Leave off to use your global GitAssist rules.</p>
      <Field label="Override branch rules">
        <Toggle
          checked={!!draft.branches}
          onchange={(v) => {
            draft.branches = v
              ? { enabled: true, base: 'main', protected: ['main', 'master'], protect_mode: 'warn', prefixes: [], naming_mode: 'warn', ideas: '' }
              : null;
          }}
        />
      </Field>
      {#if draft.branches}
        <Field label="Base branch"><input bind:value={draft.branches.base} /></Field>
        <Field label="Protected">
          <ChipList items={draft.branches.protected} onchange={(v) => (draft.branches.protected = v)} placeholder="main" />
        </Field>
        <Field label="Protect mode">
          <Segmented size="sm" value={draft.branches.protect_mode} onchange={(v) => (draft.branches.protect_mode = v)}
            options={[{ id: 'warn', label: 'Warn' }, { id: 'block', label: 'Block' }]} />
        </Field>
      {/if}
      <Field label="Override message tabs" info="When on, commit tab can set a total_max; other tabs stay global for now.">
        <Toggle
          checked={!!draft.tabs}
          onchange={(v) => {
            draft.tabs = v ? { commit: { total_max: { value: 500, mode: 'block' } } } : null;
          }}
        />
      </Field>
      {#if draft.tabs?.commit?.total_max}
        <Field label="Commit total max">
          <input type="number" value={draft.tabs.commit.total_max.value}
            oninput={(e) => (draft.tabs.commit.total_max.value = Number(e.currentTarget.value) || 0)} />
        </Field>
      {/if}
    {:else if tab === 'checks'}
      <Field label="Run checks before push"><Toggle checked={draft.checks.enabled} onchange={(v) => (draft.checks.enabled = v)} /></Field>
      <Field label="On failure">
        <Segmented size="sm" value={draft.checks.on_fail} onchange={(v) => (draft.checks.on_fail = v)}
          options={[{ id: 'block', label: 'Block' }, { id: 'warn', label: 'Warn' }]} />
      </Field>
      <Field label="Timeout (seconds)">
        <input type="number" bind:value={draft.checks.timeout_secs} />
      </Field>
      <Field label="Commands" info="Local shell commands, cwd = repo. Example: cargo test. For GitHub Actions, add a gh run watch recipe.">
        {#each draft.checks.commands as c, i}
          <div class="row" style="gap:0.5rem;margin-bottom:0.4rem">
            <input class="grow-input" placeholder="Name" bind:value={c.name} />
            <input class="grow-input" placeholder="shell command" bind:value={c.shell} />
            <button class="icon-btn" type="button" aria-label="Remove" onclick={() => draft.checks.commands.splice(i, 1)}><Icon name="x" size={14} /></button>
          </div>
        {/each}
        <button type="button" class="linkish" onclick={() => draft.checks.commands.push({ name: '', shell: '', env: {} })}>+ Add command</button>
      </Field>
    {/if}
    <div class="row" style="margin-top:1rem;gap:0.5rem">
      <ActionButton run={async () => applyDraft()} label="Apply target" kind="primary" size="sm" />
      <button type="button" class="ghost" onclick={() => { draft = null; tab = 'list'; }}>Back</button>
    </div>
  {:else}
    {#if remotes.length}
      <p class="muted small">Import from existing remotes:</p>
      <div class="row" style="flex-wrap:wrap;gap:0.4rem;margin-bottom:0.75rem">
        {#each remotes as r}
          <button type="button" class="pill" onclick={() => importRemote(r)}>{r.name}</button>
        {/each}
      </div>
    {/if}
    {#if !data.targets.length}
      <div class="empty"><Icon name="upload" size={22} /><p>No targets yet. Add personal, public, or org.</p></div>
    {:else}
      <ul class="repo-rows">
        {#each data.targets as t, i}
          <li class="repo-row">
            <span class="repo-name">
              <strong>{t.label}</strong>
              <span class="muted small">{t.role} · {t.remote_name} · {t.push_policy}</span>
            </span>
            <span class="repo-end">
              <button type="button" class="ghost" onclick={() => openEdit(i)}>Edit</button>
              <button type="button" class="ghost" onclick={() => removeAt(i)}>Remove</button>
            </span>
          </li>
        {/each}
      </ul>
    {/if}
    <div class="row" style="margin-top:1rem;gap:0.5rem">
      <ActionButton run={async () => openNew()} label="Add target" icon="plus" size="sm" />
      <ActionButton run={save} label="Save" kind="primary" size="sm" doneLabel="Saved" />
    </div>
  {/if}
</Modal>
