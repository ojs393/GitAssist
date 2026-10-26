<script>
  import { MODES, BRANCH_PRESETS } from '../api.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Card from '../ui/Card.svelte';
  import Field from '../ui/Field.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import ChipList from '../ui/ChipList.svelte';
  import Choice from '../ui/Choice.svelte';

  let { config = $bindable() } = $props();
  const b = $derived(config.branches);
  const modes = MODES.filter((m) => m.id !== 'fix');
  const active = $derived(
    !b.protected.length && !b.prefixes.length ? 'solo' : b.protect_mode === 'block' && b.naming_mode === 'block' && b.prefixes.length ? 'strict' : b.protected.length ? 'team' : '',
  );
  const example = $derived(`${b.prefixes[0] ?? 'feat'}/short-topic`);
</script>

<header class="page-head">
  <div class="page-title">
    <span class="page-icon"><Icon name="branch" size={20} /></span>
    <h1>Branches</h1>
    <Info><p>Which branch new work starts from, which branches never get direct commits, how branches are named, and what agents do with new ideas. Checked on every commit and push.</p></Info>
  </div>
  <Toggle checked={b.enabled} onchange={(v) => (config.branches.enabled = v)} label={b.enabled ? 'Checking' : 'Off'} />
</header>

<div class="split even" class:dim={!b.enabled}>
  <div class="col">
    <Card title="Quick setup" icon="sparkle">
      <div class="choices">
        {#each BRANCH_PRESETS as p}<Choice icon={p.icon} title={p.label} note={p.note} selected={active === p.id} onclick={() => (config.branches = p.apply(config.branches))} />{/each}
      </div>
    </Card>

    <Card title="Where work starts" icon="branch">
      <Field label="Start new work from" info="Agents are told to branch off this and pull it first.">
        <input class="short" value={b.base} placeholder="main" oninput={(e) => (config.branches.base = e.currentTarget.value.trim())} />
      </Field>
      <Field label="Protected branches" info="Direct commits and pushes to these are warned about or blocked. Merges are fine." stacked>
        <ChipList items={b.protected} empty="None" placeholder={'main\nmaster\nrelease'} onchange={(v) => (config.branches.protected = v)} />
      </Field>
      {#if b.protected.length}
        <Field label="Committing to them"><Segmented size="sm" options={modes} value={b.protect_mode === 'block' ? 'block' : 'warn'} onchange={(m) => (config.branches.protect_mode = m)} /></Field>
      {/if}
    </Card>
  </div>

  <div class="col">
    <Card title="Branch names" icon="tag" info="New branches must start with one of these words and a slash. Leave empty to allow any name.">
      <Field label="Allowed types" stacked>
        <ChipList items={b.prefixes} empty="Any name is fine" placeholder={'feat\nfix\ndocs\nchore'} onchange={(v) => (config.branches.prefixes = v)} />
      </Field>
      {#if b.prefixes.length}
        <Field label="Other names"><Segmented size="sm" options={modes} value={b.naming_mode === 'block' ? 'block' : 'warn'} onchange={(m) => (config.branches.naming_mode = m)} /></Field>
        <p class="muted small">Example: <code>{example}</code></p>
      {/if}
    </Card>

    <Card title="New ideas" icon="sparkle" info="Told to every agent, so ideas become issues before they become code.">
      <textarea rows="3" value={b.ideas} oninput={(e) => (config.branches.ideas = e.currentTarget.value)} placeholder="What should an agent do with a new idea?"></textarea>
    </Card>
  </div>
</div>
