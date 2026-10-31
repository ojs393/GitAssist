<script>
  import Field from '../ui/Field.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Icon from '../ui/Icon.svelte';
  import ChipList from '../ui/ChipList.svelte';

  // One publish target. `defaultTarget` is the repo-level label, not a field on the target.
  // `onenabled` lets the detail page persist Pause immediately; the modal edits a draft.
  let { target = $bindable(), defaultTarget = $bindable(null), onenabled = null } = $props();

  let tab = $state('identity');

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

  const filterText = $derived((target?.publish_filter ?? []).join('\n'));

  function setLabel(value) {
    if (defaultTarget === target.label) defaultTarget = value;
    target.label = value;
  }

  function setEnabled(v) {
    if (onenabled) onenabled(v);
    else target.enabled = v;
  }

  function setFilter(text) {
    target.publish_filter = text
      .split('\n')
      .map((l) => l.trimEnd())
      .filter((l) => l.length);
  }
</script>

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

{#if tab === 'identity'}
  <Field label="Label" info="What agents say: gitassist push personal">
    <input value={target.label} placeholder="personal" oninput={(e) => setLabel(e.currentTarget.value)} />
  </Field>
  <Field label="Role">
    <Segmented size="sm" value={target.role} onchange={(v) => (target.role = v)} options={ROLES} />
  </Field>
  <Field label="Git remote name"><input bind:value={target.remote_name} placeholder="origin" /></Field>
  <Field label="Remote URL"><input bind:value={target.url} placeholder="git@github.com:you/repo.git" /></Field>
  <Field label="Enabled"><Toggle checked={target.enabled} onchange={(v) => setEnabled(v)} /></Field>
{:else if tab === 'policy'}
  <Field label="When to push" info="After commit is best-effort and never fails the commit.">
    <Segmented size="sm" value={target.push_policy} onchange={(v) => (target.push_policy = v)} options={POLICIES} />
  </Field>
  <Field label="Include in push --all">
    <Toggle checked={target.include_in_push_all} onchange={(v) => (target.include_in_push_all = v)} />
  </Field>
  <Field label="Default target for bare gitassist push">
    <Toggle
      checked={defaultTarget === target.label}
      onchange={(v) => (defaultTarget = v ? target.label : null)}
    />
  </Field>
{:else if tab === 'filter'}
  <Field label="Publish filter" info="gitignore patterns never sent to this target. Uses a cleaned mirror under the hood.">
    <textarea rows="8" value={filterText} oninput={(e) => setFilter(e.currentTarget.value)} placeholder={'.env\n*.pem\n.local/'}></textarea>
  </Field>
  <Field label="Use mirror" info="On when a filter is set. Keeps your working tree unchanged.">
    <Toggle checked={target.use_mirror || !!(target.publish_filter ?? []).length} onchange={(v) => (target.use_mirror = v)} />
  </Field>
{:else if tab === 'rules'}
  <p class="muted small">Optional overrides for this target. Off = use global rules.</p>
  <Field label="Override branch rules">
    <Toggle
      checked={!!target.branches}
      onchange={(v) => {
        target.branches = v
          ? { enabled: true, base: 'main', protected: ['main', 'master'], protect_mode: 'warn', prefixes: [], naming_mode: 'warn', ideas: '' }
          : null;
      }}
    />
  </Field>
  {#if target.branches}
    <Field label="Base branch"><input bind:value={target.branches.base} /></Field>
    <Field label="Protected">
      <ChipList items={target.branches.protected} onchange={(v) => (target.branches.protected = v)} placeholder="main" />
    </Field>
    <Field label="Protect mode">
      <Segmented
        size="sm"
        value={target.branches.protect_mode}
        onchange={(v) => (target.branches.protect_mode = v)}
        options={[{ id: 'warn', label: 'Warn' }, { id: 'block', label: 'Block' }]}
      />
    </Field>
  {/if}
  <Field label="Override message tabs" info="When on, commit tab can set a total_max; other tabs stay global for now.">
    <Toggle
      checked={!!target.tabs}
      onchange={(v) => {
        target.tabs = v ? { commit: { total_max: { value: 500, mode: 'block' } } } : null;
      }}
    />
  </Field>
  {#if target.tabs?.commit?.total_max}
    <Field label="Commit total max">
      <input
        type="number"
        value={target.tabs.commit.total_max.value}
        oninput={(e) => (target.tabs.commit.total_max.value = Number(e.currentTarget.value) || 0)}
      />
    </Field>
  {/if}
{:else if tab === 'checks'}
  <Field label="Run checks before push">
    <Toggle checked={target.checks.enabled} onchange={(v) => (target.checks.enabled = v)} />
  </Field>
  <Field label="On failure">
    <Segmented
      size="sm"
      value={target.checks.on_fail}
      onchange={(v) => (target.checks.on_fail = v)}
      options={[{ id: 'block', label: 'Block' }, { id: 'warn', label: 'Warn' }]}
    />
  </Field>
  <Field label="Timeout (seconds)">
    <input type="number" bind:value={target.checks.timeout_secs} />
  </Field>
  <Field label="Commands" info="Local shell commands, cwd = repo. Example: cargo test. For GitHub Actions, add a gh run watch recipe.">
    {#each target.checks.commands as c, i}
      <div class="row" style="gap:0.5rem;margin-bottom:0.4rem">
        <input class="grow-input" placeholder="Name" bind:value={c.name} />
        <input class="grow-input" placeholder="shell command" bind:value={c.shell} />
        <button class="icon-btn" type="button" aria-label="Remove" onclick={() => target.checks.commands.splice(i, 1)}>
          <Icon name="x" size={14} />
        </button>
      </div>
    {/each}
    <button type="button" class="btn ghost sm" onclick={() => target.checks.commands.push({ name: '', shell: '', env: {} })}>
      <Icon name="plus" size={13} /> Add command
    </button>
  </Field>
{/if}
