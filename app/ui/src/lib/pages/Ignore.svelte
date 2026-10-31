<script>
  import { api } from '../api.js';
  import Icon from '../ui/Icon.svelte';
  import Card from '../ui/Card.svelte';
  import PageHead from '../ui/PageHead.svelte';
  import Field from '../ui/Field.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import ChipList from '../ui/ChipList.svelte';
  import ActionButton from '../ui/ActionButton.svelte';

  let { config = $bindable() } = $props();
  const ig = $derived(config.ignore);

  const LISTS = [
    { key: 'every_repo', label: 'Every repo', icon: 'globe', info: 'Applies everywhere, including repos with no GitHub remote. Also used as git\'s global ignore file.' },
    { key: 'personal', label: 'Personal repos', icon: 'user', info: 'Repos owned by your GitHub account.' },
    { key: 'organization', label: 'Organization repos', icon: 'building', info: 'Repos owned by anyone else, such as your company.' },
    { key: 'public', label: 'Public repos', icon: 'eye', info: 'Repos anyone can see. Also used when visibility is unknown, as the safe choice.' },
    { key: 'private', label: 'Private repos', icon: 'lock', info: 'Repos only you or your team can see.' },
  ];

  let remote = $state('');
  let visibility = $state('public');
  let files = $state('.env\n.env.example\nsrc/main.rs\ncerts/dev.pem\n.DS_Store');
  let test = $state(null);
  let ttimer;

  $effect(() => {
    const snapshot = JSON.parse(JSON.stringify(config));
    const r = remote || `git@github.com:${ig.github_user || 'you'}/project.git`;
    const v = visibility;
    const list = files.split('\n');
    clearTimeout(ttimer);
    ttimer = setTimeout(async () => {
      test = await api.ignorePreview(snapshot, r, v, list).catch(() => null);
    }, 150);
  });
</script>

<PageHead title="Ignore" icon="eye-off" info="Paths that must never be committed. Scoped by owner and visibility. .gitignore syntax; ! allows.">
  {#snippet actions()}
    <Toggle checked={ig.enabled} onchange={(v) => (config.ignore.enabled = v)} label={ig.enabled ? 'On' : 'Off'} />
  {/snippet}
</PageHead>

<div class="split" class:dim={!ig.enabled}>
  <div class="col">
    <Card title="How it works" icon="sliders" raised>
      <Field label="When a listed file is staged" info="Fix leaves it out of the commit and adds it to the repo's local exclude list. At push time listed files always block (history can't be fixed silently).">
        <Segmented size="sm" value={ig.mode} onchange={(m) => (config.ignore.mode = m)}
          options={[{ id: 'fix', label: 'Leave out', icon: 'wrench' }, { id: 'block', label: 'Block', icon: 'x' }, { id: 'warn', label: 'Warn', icon: 'alert' }]} />
      </Field>
      <Field label="Your GitHub account" info="Repos owned by this account count as personal; everything else is organization.">
        <div class="limit">
          <input class="short" value={ig.github_user} placeholder="username" oninput={(e) => (config.ignore.github_user = e.currentTarget.value.trim())} />
          <ActionButton run={async () => { const u = await api.detectGithubUser(); if (!u) throw new Error('gh is not signed in'); config.ignore.github_user = u; }} label="Detect" kind="ghost" size="sm" doneLabel="Found" />
        </div>
      </Field>
      <Field label="Check public or private" info="Asks your own gh once per repo and remembers it for a week. Off: every repo with a remote is treated as public.">
        <Toggle checked={ig.lookup_visibility} onchange={(v) => (config.ignore.lookup_visibility = v)} />
      </Field>
    </Card>

    {#each LISTS as l, i}
      <Card title={l.label} icon={l.icon} info={l.info} quiet={i > 0} raised={i === 0}>
        <ChipList mono items={ig[l.key]} empty="Nothing listed" placeholder={'.env\n*.pem\n!.env.example'} onchange={(v) => (config.ignore[l.key] = v)} />
      </Card>
    {/each}
  </div>

  <div class="col">
    <Card title="Try it" icon="search" info="Pick a repo and see which files would be left out." emphasis>
      <div class="sticky-inner">
      <Field label="Repo" stacked>
        <input class="full" bind:value={remote} placeholder={`git@github.com:${ig.github_user || 'you'}/project.git`} />
      </Field>
      <Field label="Visibility">
        <Segmented size="sm" value={visibility} onchange={(v) => (visibility = v)}
          options={[{ id: 'public', label: 'Public', icon: 'eye' }, { id: 'private', label: 'Private', icon: 'lock' }]} />
      </Field>
      <Field label="Files" stacked>
        <textarea class="mono" rows="5" bind:value={files}></textarea>
      </Field>
      {#if test}
        <div class="scope-pill"><Icon name="tag" size={14} /> {test.scope}</div>
        <ul class="file-results">
          {#each test.files as [f, ignored]}
            <li class:ignored><Icon name={ignored ? 'eye-off' : 'check'} size={14} /><span class="mono">{f}</span><span class="small">{ignored ? 'left out' : 'committed'}</span></li>
          {/each}
        </ul>
      {/if}
      </div>
    </Card>
  </div>
</div>
