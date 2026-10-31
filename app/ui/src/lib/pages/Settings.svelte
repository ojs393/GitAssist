<script>
  import { onMount } from 'svelte';
  import { api, MODES, formatBytes } from '../api.js';
  import Icon from '../ui/Icon.svelte';
  import Card from '../ui/Card.svelte';
  import PageHead from '../ui/PageHead.svelte';
  import Field from '../ui/Field.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import ChipList from '../ui/ChipList.svelte';
  import StyleFields from '../ui/StyleFields.svelte';
  import Tabs from '../ui/Tabs.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import Setup from './Setup.svelte';

  let { config = $bindable() } = $props();
  const g = $derived(config.global);
  const DEFAULT_PATTERNS = ['^\\s*Co-authored-by:.*$', '^.*Generated with \\[?Claude Code\\]?.*$', '^.*Made with \\[?Cursor\\]?.*$', '^\\s*Made-with:\\s*Cursor.*$', '^\\s*\\u{1F916}.*$'];
  const idSet = $derived(!!g.identity.author_name.trim() && !!g.identity.author_email.trim());

  let section = $state('you');
  let usage = $state(null);
  let wizard = $state(false);
  let report = $state(null);
  const SECTIONS = [
    { id: 'you', label: 'You', icon: 'user' },
    { id: 'coauthor', label: 'Co-authors', icon: 'bot' },
    { id: 'safety', label: 'Safety', icon: 'lock' },
    { id: 'storage', label: 'Storage', icon: 'storage' },
    { id: 'install', label: 'Install', icon: 'wrench' },
  ];

  async function loadUsage() {
    usage = await api.storage().catch(() => null);
  }
  onMount(loadUsage);
  async function clear(what) {
    usage = await api.clearStorage(what);
  }
  async function openSetup() {
    report = await api.health(false);
    wizard = true;
  }
</script>

<Setup open={wizard} bind:config {report} onclose={() => (wizard = false)} ondone={() => (wizard = false)} />

<PageHead title="Settings" icon="gear" info="Identity, co-authors, safety, storage, install." />

<Tabs options={SECTIONS} value={section} onchange={(v) => (section = v)} />

{#if section === 'you'}
  <div class="split even">
    <Card title="You" icon="user" raised>
      <Field label="Name"><input class="short" bind:value={config.global.identity.author_name} placeholder="Your name or handle" /></Field>
      <Field label="Email" info="Your GitHub no-reply address keeps your email private."><input class="short" bind:value={config.global.identity.author_email} placeholder="you@users.noreply.github.com" /></Field>
      <Field label="Always make me the author" info={idSet ? 'If an agent commits under another name, GitAssist changes it to you straight away.' : 'Fill in name and email first.'}>
        <Toggle checked={g.identity.enforce_author} disabled={!idSet} onchange={(v) => (config.global.identity.enforce_author = v)} />
      </Field>
    </Card>
    <Card title="Voice" icon="voice" info="Told to every agent. GitAssist never rewrites your words." quiet>
      <StyleFields bind:style={config.global.style} />
    </Card>
  </div>
{:else if section === 'coauthor'}
  <div class="split even">
    <Card title="Agent co-author lines" icon="bot" raised>
      <Field label="When an agent adds one" info="Fix removes it, Block makes the agent remove it, Warn only reports it.">
        <Segmented size="sm" options={MODES} value={g.trailers.strip_mode} onchange={(m) => (config.global.trailers.strip_mode = m)} />
      </Field>
      <Field label="Lines to remove" info="Patterns, one per line, matched without caring about capitals." stacked>
        <ChipList mono items={g.trailers.strip_patterns} onchange={(v) => (config.global.trailers.strip_patterns = v.length ? v : [...DEFAULT_PATTERNS])} />
      </Field>
    </Card>
    <Card title="Your own lines" icon="pen" quiet>
      <Field label="Add to every commit" info="For example your own Co-authored-by or Signed-off-by." stacked>
        <ChipList mono items={g.trailers.add} empty="Nothing added" placeholder={'Signed-off-by: You <email>'} onchange={(v) => (config.global.trailers.add = v)} />
      </Field>
      <Field label="Docs commits are" info="A commit counts as a docs commit when every changed file matches one of these." stacked>
        <ChipList mono items={g.docs.patterns} onchange={(v) => (config.global.docs.patterns = v)} />
      </Field>
    </Card>
  </div>
{:else if section === 'safety'}
  <Card title="Safety" icon="lock" raised>
    <Field label="Fix itself" info="Every 15 minutes and at login, puts back anything another tool changed.">
      <Toggle checked={config.auto_repair} onchange={(v) => (config.auto_repair = v)} />
    </Field>
    <Field label="Rules file in each repo" info="Cursor only reads rules stored inside a repo, so GitAssist adds .cursor/rules/gitassist.mdc when you clone or commit. It never gets committed.">
      <Toggle checked={config.repo_rule_files} onchange={(v) => (config.repo_rule_files = v)} />
    </Field>
    <Field label="Let repos add stricter rules" info="Reads a .gitassist.toml in a repo. A repo can never change your identity, co-author settings or turn GitAssist off.">
      <Toggle checked={config.allow_project_overrides} onchange={(v) => (config.allow_project_overrides = v)} />
    </Field>
    <Field label="Keep message text in Activity" info="Off: Activity only records which rules fired. Stays on this computer.">
      <Toggle checked={config.log_bodies} onchange={(v) => (config.log_bodies = v)} />
    </Field>
  </Card>
{:else if section === 'storage'}
  <Card title="Storage" icon="storage" info="Settings and setup are never touched by these buttons." quiet>
    {#snippet actions()}<button class="icon-btn" aria-label="Refresh" onclick={loadUsage}><Icon name="refresh" size={15} /></button>{/snippet}
    {#if usage}
      <Field label="Activity log" info="Capped at about 15 MB; older entries roll off automatically.">
        <div class="limit"><span class="muted small">{formatBytes(usage.logs)}</span><ActionButton run={() => clear('logs')} label="Clear" kind="ghost" size="sm" doneLabel="Cleared" /></div>
      </Field>
      <Field label="Backups" info="Copies of files taken before GitAssist edited them. The newest 20 are kept.">
        <div class="limit"><span class="muted small">{formatBytes(usage.backups)} · {usage.backup_sets} sets</span><ActionButton run={() => clear('backups')} label="Clear" kind="ghost" size="sm" doneLabel="Cleared" /></div>
      </Field>
      <Field label="Cache" info="Which repos are public or private, remembered for a week.">
        <div class="limit"><span class="muted small">{formatBytes(usage.cache)}</span><ActionButton run={() => clear('cache')} label="Clear" kind="ghost" size="sm" doneLabel="Cleared" /></div>
      </Field>
      <div class="row-end"><ActionButton run={() => clear('all')} label="Clear everything" kind="ghost" size="sm" icon="trash" doneLabel="Cleared" /></div>
    {:else}
      <div class="loading"><span class="spinner"></span></div>
    {/if}
  </Card>
{:else}
  <div class="split even">
    <Card title="Setup" icon="rocket" emphasis>
      <Field label="Walk through setup again">
        <button class="btn sm" onclick={openSetup}><Icon name="rocket" size={14} /> Open <span class="go"><Icon name="chevron-right" size={14} /></span></button>
      </Field>
      <Field label="Check and repair everything" info="Reinstalls anything that is missing or was changed.">
        <ActionButton run={() => api.repair()} label="Repair" size="sm" icon="wrench" doneLabel="Done" />
      </Field>
    </Card>
    <Card title="Uninstall" icon="trash" info="Puts git, your shell and every agent's files back exactly as they were before GitAssist." quiet>
      <Field label="Remove GitAssist from this computer">
        <ActionButton run={async () => { if (!confirm('Remove GitAssist from git, gh and every agent?')) throw new Error('Cancelled'); await api.uninstall(); }} label="Uninstall" kind="danger" size="sm" doneLabel="Removed" />
      </Field>
    </Card>
  </div>
{/if}
