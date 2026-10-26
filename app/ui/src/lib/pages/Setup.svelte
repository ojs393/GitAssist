<script>
  import { api, LENGTH_PRESETS, BRANCH_PRESETS } from '../api.js';
  import Icon from '../ui/Icon.svelte';
  import Modal from '../ui/Modal.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Choice from '../ui/Choice.svelte';

  // First-run setup: one question per step; everything has a sensible default,
  // so pressing Continue all the way through is a complete setup.
  let { open = false, config = $bindable(), report, onclose, ondone } = $props();

  const SECRETS = ['.env', '.env.*', '!.env.example', '!.env.sample', '*.pem', '*.key', '*.p12', '*.pfx', 'id_rsa*', 'id_ed25519*', '*.keystore', 'credentials.json', 'secrets.*'];
  const JUNK = ['.DS_Store', '._*', 'Thumbs.db', 'desktop.ini', '*.swp'];
  const PERSONAL = ['CLAUDE.local.md', '.claude/settings.local.json', '.aider*', '*.local.md'];

  const STEPS = [
    { id: 'welcome', label: 'Welcome', icon: 'shield' },
    { id: 'account', label: 'Account', icon: 'user' },
    { id: 'commits', label: 'Commits', icon: 'commit' },
    { id: 'coauthor', label: 'Co-authors', icon: 'bot' },
    { id: 'branches', label: 'Branches', icon: 'branch' },
    { id: 'ignore', label: 'Ignore', icon: 'eye-off' },
    { id: 'agents', label: 'Agents', icon: 'sparkle' },
    { id: 'background', label: 'Background', icon: 'login' },
    { id: 'finish', label: 'Turn on', icon: 'power' },
  ];
  let step = $state(0);
  let busy = $state(false);
  let results = $state(null);
  let error = $state('');
  let commitPreset = $state('standard');
  let branchPreset = $state('team');
  let startAtLogin = $state(true);
  let detecting = $state(false);
  let agentsOn = $state({});
  let secrets = $state(true);
  let junk = $state(true);
  let personal = $state(true);

  const agents = $derived(report?.adapters.filter((a) => a.status !== 'not_detected') ?? []);
  $effect(() => {
    for (const a of agents) if (!(a.id in agentsOn)) agentsOn[a.id] = true;
  });

  async function detect() {
    detecting = true;
    const u = await api.detectGithubUser().catch(() => null);
    if (u) config.ignore.github_user = u;
    detecting = false;
  }
  $effect(() => {
    if (open && STEPS[step].id === 'account' && !config.ignore.github_user && !detecting) detect();
  });

  function applyCommitPreset(id) {
    commitPreset = id;
    const p = LENGTH_PRESETS.commit.find((x) => x.id === id);
    for (const tab of ['commit', 'docs_commit', 'merge']) {
      for (const [k, [value, mode]] of Object.entries(p.rules)) config.tabs[tab][k] = { value, mode };
    }
  }
  function applyBranchPreset(id) {
    branchPreset = id;
    config.branches = BRANCH_PRESETS.find((x) => x.id === id).apply(config.branches);
  }
  function applyIgnore() {
    config.ignore.public = secrets ? (config.ignore.public.length ? config.ignore.public : [...SECRETS]) : [];
    config.ignore.every_repo = junk ? (config.ignore.every_repo.length ? config.ignore.every_repo : [...JUNK]) : [];
    config.ignore.organization = personal ? (config.ignore.organization.length ? config.ignore.organization : [...PERSONAL]) : [];
  }

  async function finish() {
    busy = true;
    error = '';
    try {
      applyIgnore();
      await api.save(config);
      results = await api.turnOn();
      for (const a of agents) if (!agentsOn[a.id]) await api.removeAdapter(a.id);
      if (!startAtLogin) await api.disableAutostart();
      if (results.some((r) => !r.ok)) error = 'Some steps need another try.';
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  const last = STEPS.length - 1;
</script>

<Modal {open} title="Set up GitAssist" wide {onclose}>
  <ol class="wizard-steps">
    {#each STEPS as s, i}
      <li>
        <button class:active={i === step} class:done={i < step} onclick={() => (step = i)} disabled={busy}>
          <span class="ws-dot">{#if i < step}<Icon name="check" size={12} />{:else}{i + 1}{/if}</span>
          <span class="ws-label">{s.label}</span>
        </button>
      </li>
    {/each}
  </ol>

  <div class="wizard-body">
    {#if STEPS[step].id === 'welcome'}
      <div class="wz-hero"><span class="wz-icon"><Icon name="shield-check" size={34} stroke={1.5} /></span>
        <h3>Your rules for every commit, pull request and issue</h3>
        <p>GitAssist checks what you and your coding agents send to GitHub, and fixes or stops anything that breaks your rules. It works offline, in every repo.</p>
      </div>
      <ul class="wz-points">
        <li><Icon name="bot" /> Removes co-author lines agents add</li>
        <li><Icon name="ruler" /> Keeps messages short and in your style</li>
        <li><Icon name="branch" /> Keeps work off main and branches tidy</li>
        <li><Icon name="eye-off" /> Keeps secrets out of public repos</li>
      </ul>
    {:else if STEPS[step].id === 'account'}
      <h3>Your GitHub account</h3>
      <p class="muted">Repos you own count as personal; everything else counts as organization.</p>
      <div class="row wz-row">
        <input class="grow-input" value={config.ignore.github_user} placeholder="username" oninput={(e) => (config.ignore.github_user = e.currentTarget.value.trim())} />
        <button class="btn ghost" onclick={detect} disabled={detecting}>{#if detecting}<span class="spinner"></span>{:else}<Icon name="search" size={15} />{/if} Find it</button>
      </div>
      <p class="muted small">Found using the GitHub CLI if you're signed in. You can skip this.</p>
    {:else if STEPS[step].id === 'commits'}
      <h3>How long should commit messages be?</h3>
      <div class="choices">
        {#each LENGTH_PRESETS.commit as p}<Choice icon={p.icon} title={p.label} note={p.note} selected={commitPreset === p.id} onclick={() => applyCommitPreset(p.id)} />{/each}
      </div>
      <h4>Tone</h4>
      <Segmented value={config.global.style.tone} onchange={(v) => (config.global.style.tone = v)}
        options={[{ id: 'terse', label: 'Short and direct', icon: 'bolt' }, { id: 'neutral', label: 'Neutral', icon: 'minus' }, { id: 'friendly', label: 'Friendly', icon: 'smile' }, { id: 'formal', label: 'Formal', icon: 'briefcase' }]} />
    {:else if STEPS[step].id === 'coauthor'}
      <h3>Co-author lines</h3>
      <div class="wz-list">
        <div class="wz-item"><div><strong>Remove lines agents add</strong><p class="muted small">"Co-authored-by: Claude", "Made with Cursor" and similar.</p></div>
          <Toggle checked={config.global.trailers.strip_mode !== 'warn'} onchange={(v) => (config.global.trailers.strip_mode = v ? 'fix' : 'warn')} /></div>
        <div class="wz-item"><div><strong>Add my own line to commits</strong><p class="muted small">For example Signed-off-by or a co-author you choose.</p></div>
          <Toggle checked={config.global.trailers.add.length > 0} onchange={(v) => (config.global.trailers.add = v ? [`Signed-off-by: ${config.ignore.github_user || 'Your Name'} <you@example.com>`] : [])} /></div>
        {#if config.global.trailers.add.length}
          <input class="full mono" value={config.global.trailers.add[0]} oninput={(e) => (config.global.trailers.add = [e.currentTarget.value])} />
        {/if}
      </div>
    {:else if STEPS[step].id === 'branches'}
      <h3>How do you work with branches?</h3>
      <div class="choices">
        {#each BRANCH_PRESETS as p}<Choice icon={p.icon} title={p.label} note={p.note} selected={branchPreset === p.id} onclick={() => applyBranchPreset(p.id)} />{/each}
      </div>
      <div class="row wz-row"><span class="muted">Start new work from</span><input class="short" value={config.branches.base} oninput={(e) => (config.branches.base = e.currentTarget.value.trim())} /></div>
    {:else if STEPS[step].id === 'ignore'}
      <h3>Files that should never be committed</h3>
      <div class="wz-list">
        <div class="wz-item"><div><strong>Secrets in public repos</strong><p class="muted small">.env files, keys and certificates.</p></div><Toggle checked={secrets} onchange={(v) => (secrets = v)} /></div>
        <div class="wz-item"><div><strong>System junk everywhere</strong><p class="muted small">.DS_Store, Thumbs.db and editor swap files.</p></div><Toggle checked={junk} onchange={(v) => (junk = v)} /></div>
        <div class="wz-item"><div><strong>Personal agent files in work repos</strong><p class="muted small">Local agent notes and settings stay on your machine.</p></div><Toggle checked={personal} onchange={(v) => (personal = v)} /></div>
      </div>
    {:else if STEPS[step].id === 'agents'}
      <h3>Coding agents on this computer</h3>
      {#if agents.length}
        <div class="wz-list">
          {#each agents as a}
            <div class="wz-item"><div class="row"><span class="agent-glyph"><Icon name={'agent-' + a.id} size={17} /></span><strong>{a.name}</strong></div>
              <Toggle checked={agentsOn[a.id]} onchange={(v) => (agentsOn[a.id] = v)} /></div>
          {/each}
        </div>
        <p class="muted small">Agents you install later are set up automatically.</p>
      {:else}
        <p class="muted">None found yet. Agents you install later are set up automatically.</p>
      {/if}
    {:else if STEPS[step].id === 'background'}
      <h3>Keep it running</h3>
      <div class="wz-list">
        <div class="wz-item"><div><strong>Start at login</strong><p class="muted small">A small menu bar icon. Checks still run inside git when it's closed.</p></div><Toggle checked={startAtLogin} onchange={(v) => (startAtLogin = v)} /></div>
        <div class="wz-item"><div><strong>Fix itself</strong><p class="muted small">Puts things back if another tool changes them.</p></div><Toggle checked={config.auto_repair} onchange={(v) => (config.auto_repair = v)} /></div>
      </div>
    {:else}
      {#if !results}
        <h3>Ready to turn on</h3>
        <ul class="wz-summary">
          <li><Icon name="commit" /> Commits: {LENGTH_PRESETS.commit.find((p) => p.id === commitPreset).note}</li>
          <li><Icon name="bot" /> Co-author lines: {config.global.trailers.strip_mode === 'warn' ? 'kept' : 'removed'}</li>
          <li><Icon name="branch" /> Branches: {BRANCH_PRESETS.find((p) => p.id === branchPreset).note}</li>
          <li><Icon name="sparkle" /> Agents: {agents.filter((a) => agentsOn[a.id]).map((a) => a.name).join(', ') || 'none yet'}</li>
        </ul>
      {:else}
        <h3>{error ? 'Almost there' : 'GitAssist is on'}</h3>
        <ul class="wz-summary">
          {#each results as r}<li class:bad={!r.ok}><Icon name={r.ok ? 'check' : 'x'} /> {r.message}</li>{/each}
        </ul>
      {/if}
      {#if error}<p class="bad small">{error}</p>{/if}
    {/if}
  </div>

  {#snippet footer()}
    <button class="btn ghost" onclick={() => (step = Math.max(0, step - 1))} disabled={step === 0 || busy}><Icon name="chevron-left" size={15} /> Back</button>
    <span class="muted small">Step {step + 1} of {STEPS.length}</span>
    {#if step < last}
      <button class="btn primary" onclick={() => (step += 1)}>Continue <Icon name="chevron-right" size={15} /></button>
    {:else if !results || error}
      <button class="btn primary" onclick={finish} disabled={busy}>{#if busy}<span class="spinner"></span> Turning on{:else}<Icon name="power" size={15} /> {error ? 'Try again' : 'Turn on'}{/if}</button>
    {:else}
      <button class="btn primary" onclick={() => ondone?.()}><Icon name="check" size={15} /> Done</button>
    {/if}
  {/snippet}
</Modal>
