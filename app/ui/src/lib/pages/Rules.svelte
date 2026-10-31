<script>
  import { api, MODES, SAMPLES, SAMPLE_TITLES, TEMPLATES, TAB_ICONS, LENGTH_PRESETS } from '../api.js';
  import { lineDiff } from '../diff.js';
  import Icon from '../ui/Icon.svelte';
  import Info from '../ui/Info.svelte';
  import Card from '../ui/Card.svelte';
  import PageHead from '../ui/PageHead.svelte';
  import Field from '../ui/Field.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import NumberField from '../ui/NumberField.svelte';
  import ChipList from '../ui/ChipList.svelte';
  import StyleFields from '../ui/StyleFields.svelte';
  import Tabs from '../ui/Tabs.svelte';

  let { config = $bindable(), tab, siblings = [], tabs = [], onselect } = $props();

  const ABOUT = {
    commit: 'Every git commit on this machine, typed by you or run by any agent.',
    docs_commit: 'Commits where every changed file matches your docs patterns (Defaults → Docs commits).',
    merge: 'Merge and squash commits, plus the subject and body of gh pr merge.',
    pr_create: 'Title and description of new pull requests (gh pr create).',
    pr_update: 'Edits to an existing pull request (gh pr edit).',
    pr_comment: 'PR comments and review replies (gh pr comment, gh pr review --comment).',
    review: 'Approve and request-changes text (gh pr review).',
    issue: 'Issue titles, descriptions and comments (gh issue create / edit / comment).',
  };

  const tc = $derived(config.tabs[tab.id]);
  const d = $derived(tab.defaults);
  const isPr = $derived(tab.id === 'pr_create' || tab.id === 'pr_update');
  const hasConventional = $derived(!['pr_comment', 'review', 'issue'].includes(tab.id));
  const presets = $derived(tab.commit_message ? TEMPLATES.commit : TEMPLATES.markdown);
  const titleWord = $derived(tab.commit_message ? 'Subject' : 'Title');
  const enabled = $derived(tc.enabled ?? true);
  const conventional = $derived(tc.conventional ?? d.conventional);
  const banned = $derived(tc.banned_phrases ?? d.banned_phrases);
  const inheritedAdd = $derived(tab.commit_message ? config.global.trailers.add : []);
  const showTitle = $derived(tab.has_title && !tab.commit_message);

  const rule = (field) => tc[field] ?? d[field] ?? { value: 0, mode: 'warn' };
  const set = (field, patch) => (config.tabs[tab.id][field] = { ...rule(field), ...patch });
  const reset = (field) => delete config.tabs[tab.id][field];
  const modes = (allowFix) => MODES.filter((m) => allowFix || m.id !== 'fix');

  // svelte-ignore state_referenced_locally
  let sampleBody = $state(SAMPLES[tab.id] ?? '');
  // svelte-ignore state_referenced_locally
  let sampleTitle = $state(SAMPLE_TITLES[tab.id] ?? '');
  let result = $state(null);
  let previewError = $state('');
  let editTemplate = $state(false);
  let section = $state('quick');
  let previewSize = $state('normal');
  const SECTIONS = [
    { id: 'quick', label: 'Quick', icon: 'sparkle' },
    { id: 'length', label: 'Length', icon: 'ruler' },
    { id: 'layout', label: 'Layout', icon: 'layout' },
    { id: 'words', label: 'Words', icon: 'tag' },
    { id: 'coauthor', label: 'Co-authors', icon: 'user' },
    { id: 'voice', label: 'Voice', icon: 'voice' },
  ];
  const presetsLen = $derived(tab.commit_message ? LENGTH_PRESETS.commit : LENGTH_PRESETS.markdown);
  const activePreset = $derived(presetsLen.find((p) => Object.entries(p.rules).every(([k, [v]]) => (rule(k).value ?? 0) === v))?.id);
  function applyPreset(p) {
    for (const [k, [value, mode]] of Object.entries(p.rules)) config.tabs[tab.id][k] = { value, mode };
  }
  function resetTab() {
    config.tabs[tab.id] = {};
  }
  let ptimer;

  $effect(() => {
    const snapshot = JSON.parse(JSON.stringify(config));
    const body = sampleBody;
    const title = sampleTitle;
    clearTimeout(ptimer);
    ptimer = setTimeout(async () => {
      try {
        result = await api.preview(snapshot, tab.id, showTitle ? title : null, body);
        previewError = '';
      } catch (e) {
        previewError = String(e);
      }
    }, 180);
  });

  const diff = $derived(result ? lineDiff(sampleBody, result.body ?? '') : []);
  const verdict = $derived(
    !result ? null
      : result.findings.some((f) => f.mode === 'block') ? ['blocked', 'x', 'Blocked']
      : result.findings.some((f) => f.mode === 'warn') ? ['warned', 'alert', 'Allowed with a warning']
      : result.changed ? ['fixed', 'wrench', 'Fixed']
      : ['ok', 'check', 'Unchanged'],
  );
  const clean = (m) => m.replace(/^GitAssist \[[a-z_]+\] /, '');
</script>

{#snippet limit(field, label, info, allowFix, unit = 'chars')}
  {@const r = rule(field)}
  <Field {label} {info}>
    <div class="limit">
      <Toggle small checked={r.value > 0} onchange={(on) => set(field, { value: on ? (d[field]?.value || 72) : 0 })} />
      {#if r.value > 0}
        <NumberField value={r.value} step={field === 'wrap' ? 2 : 10} min={1} {unit} onchange={(v) => set(field, { value: v })} />
        <Segmented size="sm" options={modes(allowFix)} value={!allowFix && r.mode === 'fix' ? 'block' : r.mode} onchange={(m) => set(field, { mode: m })} />
      {:else}
        <span class="muted small">Off</span>
      {/if}
      {#if tc[field] != null}<button class="link" title="Back to default" onclick={() => reset(field)}><Icon name="refresh" size={13} /></button>{/if}
    </div>
  </Field>
{/snippet}

<PageHead title={tab.label} icon={TAB_ICONS[tab.id]} info={ABOUT[tab.id]}>
  {#snippet actions()}
    <Toggle checked={enabled} onchange={(v) => (config.tabs[tab.id].enabled = v)} label={enabled ? 'On' : 'Off'} />
  {/snippet}
</PageHead>

{#if siblings.length > 1}
  <div class="tab-bar">
    <div class="subtabs">
      {#each siblings as id}
        {@const t = tabs.find((x) => x.id === id)}
        <button class:active={id === tab.id} onclick={() => onselect(id)}><Icon name={TAB_ICONS[id]} size={13} /> {t?.label}</button>
      {/each}
    </div>
  </div>
{/if}

<div class="tab-bar">
  <Tabs options={SECTIONS} value={section} onchange={(v) => (section = v)} />
</div>

<div class="split {previewSize}" class:dim={!enabled}>
  <div class="col">
    {#if section === 'quick'}
    <Card title="Quick setup" icon="sparkle" info="Length presets. Fine-tune in Length.">
      <div class="preset-row">
        {#each presetsLen as p}
          <button type="button" class="preset-chip" class:on={activePreset === p.id} onclick={() => applyPreset(p)}>
            {p.label}<span class="note">{p.note}</span>
          </button>
        {/each}
      </div>
      <Field label="Remove agent co-author lines"><Toggle checked={tc.strip_trailers ?? true} onchange={(v) => (config.tabs[tab.id].strip_trailers = v)} /></Field>
      {#if hasConventional}
        <Field label="Require feat: / fix: prefix"><Toggle checked={conventional.value === 'required'} onchange={(v) => (config.tabs[tab.id].conventional = { value: v ? 'required' : 'off', mode: 'block' })} /></Field>
      {/if}
      {#if isPr}<Field label="Auto change table"><Toggle checked={tc.managed_table ?? false} onchange={(v) => (config.tabs[tab.id].managed_table = v)} /></Field>{/if}
      <div class="row-end"><button class="btn ghost sm" onclick={resetTab}><Icon name="refresh" size={12} /> Reset defaults</button></div>
    </Card>
    {/if}
    {#if section === 'length'}
    <Card title="Length" icon="ruler">
      {#if tab.has_title}{@render limit('subject_max', `${titleWord} max`, 'Longest allowed first line. Warn or Block only: shortening a title needs rewriting.', false)}{/if}
      {@render limit('total_max', 'Whole message max', 'Title and body together, including any lines GitAssist adds.', true)}
      {@render limit('body_max', 'Body max', 'Fix cuts the body and adds …; Block makes the agent condense it.', true)}
      {#if tab.commit_message}{@render limit('wrap', 'Wrap lines at', 'Only long prose lines are split. Tables, code, links and trailers stay as they are.', true, 'cols')}{/if}
      {#if tab.id === 'pr_update'}{@render limit('growth_max', 'Growth per update', 'Most characters an edit may add to the description, so it does not balloon with every push.', false)}{/if}
    </Card>
    {/if}
    {#if section === 'layout'}
    <Card title="Layout" icon="layout">
      {#if hasConventional}
        <Field label="Prefix like feat: / fix:" info="Conventional commits: type, optional (scope), colon, summary.">
          <div class="limit">
            <Segmented size="sm" options={[{ id: 'off', label: 'Any' }, { id: 'required', label: 'Required' }, { id: 'forbidden', label: 'Never' }]}
              value={conventional.value} onchange={(v) => (config.tabs[tab.id].conventional = { ...conventional, value: v })} />
            {#if conventional.value !== 'off'}
              <Segmented size="sm" options={modes(false)} value={conventional.mode === 'fix' ? 'block' : conventional.mode} onchange={(m) => (config.tabs[tab.id].conventional = { ...conventional, mode: m })} />
            {/if}
          </div>
        </Field>
      {/if}
      <Field label="Template" info={tab.commit_message ? 'Agents are told to follow it. Label: lines become required sections (git drops lines starting with #).' : 'Agents are told to follow it. ## headings become required sections.'} stacked>
        {#if editTemplate}
          <textarea class="mono" rows="6" value={tc.template ?? ''} oninput={(e) => (config.tabs[tab.id].template = e.currentTarget.value)} placeholder="Free form"></textarea>
          <div class="row-end"><button class="btn primary sm" onclick={() => (editTemplate = false)}><Icon name="check" size={14} /> Done</button></div>
        {:else}
          <div class="chips">
            {#each presets as p}
              <button class="chip pick" class:on={tc.template === p.text} onclick={() => (config.tabs[tab.id].template = p.text)}>{p.name}</button>
            {/each}
            <button class="chip pick" class:on={!tc.template} onclick={() => { delete config.tabs[tab.id].template; delete config.tabs[tab.id].sections_mode; }}>Free form</button>
            <button class="btn ghost sm" onclick={() => (editTemplate = true)}><Icon name="edit" size={14} /> Edit</button>
          </div>
          {#if tc.template}<pre class="tpl">{tc.template}</pre>{/if}
        {/if}
      </Field>
      {#if tc.template}
        <Field label="Missing section" info="What happens when a required section is left out.">
          <Segmented size="sm" options={modes(false)} value={tc.sections_mode === 'block' ? 'block' : 'warn'} onchange={(m) => (config.tabs[tab.id].sections_mode = m)} />
        </Field>
      {/if}
      {#if isPr}
        <Field label="Change table" info="Keeps a table of changed areas (files, +/− lines) at the bottom of the description, refreshed on every update from local git data.">
          <Toggle checked={tc.managed_table ?? false} onchange={(v) => (config.tabs[tab.id].managed_table = v)} />
        </Field>
      {/if}
    </Card>
    {/if}
    {#if section === 'words'}
    <Card title="Words" icon="tag">
      <Field label="Banned phrases" info="Case-insensitive. Block makes the agent reword." stacked>
        <ChipList items={banned.value} empty="None" placeholder={'As an AI\nThis PR\ncomprehensive'} onchange={(v) => (config.tabs[tab.id].banned_phrases = { ...banned, value: v })} />
      </Field>
      {#if banned.value.length}
        <Field label="When found"><Segmented size="sm" options={modes(false)} value={banned.mode === 'block' ? 'block' : 'warn'} onchange={(m) => (config.tabs[tab.id].banned_phrases = { ...banned, mode: m })} /></Field>
      {/if}
    </Card>
    {/if}
    {#if section === 'coauthor'}
    <Card title="Co-author lines" icon="user">
      <Field label="Remove agent attribution" info="Co-authored-by, Made-with and 'Generated with' lines, using the patterns in Defaults.">
        <Toggle checked={tc.strip_trailers ?? true} onchange={(v) => (config.tabs[tab.id].strip_trailers = v)} />
      </Field>
      <Field label="Own lines for this tab" info={tab.commit_message ? 'Off: uses the lines from Defaults.' : 'Lines from Defaults only go on commits; add lines here for this tab.'}>
        <Toggle checked={tc.add_trailers != null} onchange={(v) => { if (v) config.tabs[tab.id].add_trailers = [...inheritedAdd]; else delete config.tabs[tab.id].add_trailers; }} />
      </Field>
      {#if tc.add_trailers != null}
        <ChipList mono items={tc.add_trailers} empty="Nothing added" placeholder={'Co-authored-by: Name <email>\nSigned-off-by: You <email>'} onchange={(v) => (config.tabs[tab.id].add_trailers = v)} />
      {:else if inheritedAdd.length}
        <div class="chips">{#each inheritedAdd as t}<span class="chip mono">{t}</span>{/each}</div>
      {/if}
    </Card>
    {/if}
    {#if section === 'voice'}
    <Card title="Voice" icon="voice">
      <Field label="Own style for this tab" info="Off: uses the style from Defaults. Style is given to agents as instructions.">
        <Toggle checked={tc.style != null} onchange={(v) => { if (v) config.tabs[tab.id].style = { ...config.global.style }; else delete config.tabs[tab.id].style; }} />
      </Field>
      {#if tc.style}<StyleFields bind:style={config.tabs[tab.id].style} />{/if}
    </Card>
    {/if}
  </div>

  <div class="col">
    <section class="card preview sticky">
      <header class="card-head">
        <div class="card-title"><span class="card-icon"><Icon name="eye" size={16} /></span><h2>Preview</h2>
          <Info><p>Edit the sample to see exactly what GitAssist does with it. Uses your unsaved changes.</p></Info>
        </div>
        <div class="card-actions">
          <button class="icon-btn" title="Reset sample" aria-label="Reset sample" onclick={() => { sampleBody = SAMPLES[tab.id] ?? ''; sampleTitle = SAMPLE_TITLES[tab.id] ?? ''; }}><Icon name="refresh" size={15} /></button>
          <button class="icon-btn" title={previewSize === 'max' ? 'Make smaller' : 'Make bigger'} aria-label="Resize preview" onclick={() => (previewSize = previewSize === 'max' ? 'normal' : 'max')}><Icon name={previewSize === 'max' ? 'minimize' : 'maximize'} size={15} /></button>
          <button class="icon-btn" title={previewSize === 'min' ? 'Show preview' : 'Hide preview'} aria-label="Hide preview" onclick={() => (previewSize = previewSize === 'min' ? 'normal' : 'min')}><Icon name={previewSize === 'min' ? 'eye' : 'eye-off'} size={15} /></button>
        </div>
      </header>
      {#if previewSize !== 'min'}
      {#if showTitle}<input class="full" bind:value={sampleTitle} placeholder="Title" />{/if}
      <textarea class="mono sample" rows="7" bind:value={sampleBody}></textarea>
      {#if previewError}
        <p class="bad small">{previewError}</p>
      {:else if result && verdict}
        <div class="verdict {verdict[0]}"><Icon name={verdict[1]} size={16} /> {verdict[2]}</div>
        {#if result.findings.length}
          <ul class="findings">
            {#each result.findings as f}<li class={f.mode}><Icon name={f.mode === 'fix' ? 'wrench' : f.mode === 'block' ? 'x' : 'alert'} size={13} /> {clean(f.message)}</li>{/each}
          </ul>
        {/if}
        {#if showTitle && result.title}<div class="out-title">{result.title}</div>{/if}
        <div class="diff">
          {#each diff as l}<div class="dl {l.kind}"><span class="dm">{l.kind === 'del' ? '−' : l.kind === 'add' ? '+' : ''}</span><span class="dt">{l.text || ' '}</span></div>{/each}
        </div>
      {/if}
      {/if}
    </section>
  </div>
</div>
