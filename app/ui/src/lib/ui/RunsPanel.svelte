<script>
  import { onMount } from 'svelte';
  import { api } from '../api.js';
  import { relativeTime } from '../diff.js';
  import Icon from './Icon.svelte';

  let { onnavigate } = $props();
  let jobs = $state([]);
  let activity = $state([]);
  let openId = $state('');
  let error = $state('');

  const running = $derived(jobs.some((j) => j.status === 'running'));

  async function load() {
    try {
      const [j, a] = await Promise.all([api.jobsList(), api.activity(30).catch(() => [])]);
      jobs = j ?? [];
      activity = a ?? [];
      error = '';
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    load();
    // Poll fast only while a job runs; otherwise every 8s when the tab is visible.
    let n = 0;
    const t = setInterval(() => {
      if (document.visibilityState !== 'visible') return;
      n += 1;
      if (running || n % 5 === 0) load();
    }, 1600);
    return () => clearInterval(t);
  });

  const STATUS = {
    running: ['ok', 'Running'],
    paused: ['warn', 'Paused'],
    done: ['ok', 'Done'],
    failed: ['bad', 'Failed'],
    cancelled: ['idle', 'Cancelled'],
  };
  const STEP = {
    pending: 'muted',
    running: 'ok',
    done: 'ok',
    failed: 'bad',
    skipped: 'idle',
  };

  function kindIcon(k) {
    if (k === 'publish') return 'upload';
    if (k === 'fix_all') return 'wrench';
    return 'sparkle';
  }
</script>

<section class="runs-panel">
  <div class="sheet-head">
    <div class="sheet-title"><Icon name="activity" size={14} /> Runs</div>
    <div class="row">
      {#if running}<span class="pill ok">live</span>{/if}
      <button type="button" class="btn ghost sm" onclick={() => onnavigate?.('activity')}>Log <span class="go"><Icon name="chevron-right" size={12} /></span></button>
    </div>
  </div>

  {#if error}<p class="bad small">{error}</p>{/if}

  <div class="runs-scroll">
    {#if !jobs.length && !activity.length}
      <p class="muted small">No runs yet. Fix all, Apply, or Push shows up here.</p>
    {/if}

    {#each jobs as j (j.id)}
      {@const st = STATUS[j.status] ?? ['muted', j.status]}
      <div class="run-item" class:open={openId === j.id}>
        <button type="button" class="run-row" onclick={() => (openId = openId === j.id ? '' : j.id)}>
          <span class="run-icon"><Icon name={kindIcon(j.kind)} size={13} /></span>
          <span class="run-main">
            <span class="run-label">{j.label}</span>
            <span class="muted-2 small">{j.repo ? j.repo.split(/[\\/]/).pop() : j.kind} · {relativeTime(j.updated_at)}</span>
          </span>
          <span class="pill {st[0]}">{st[1]}</span>
          <span class="go"><Icon name={openId === j.id ? 'chevron-up' : 'chevron'} size={12} /></span>
        </button>
        {#if openId === j.id}
          <div class="run-detail">
            <ul class="run-steps">
              {#each j.steps as s}
                <li class={STEP[s.status] ?? 'muted'}>
                  <Icon name={s.status === 'done' ? 'check' : s.status === 'failed' ? 'x' : s.status === 'running' ? 'refresh' : 'circle'} size={12} />
                  <span class="grow">{s.label}</span>
                  <span class="small muted">{s.message || s.status}</span>
                </li>
              {/each}
            </ul>
            <div class="row">
              {#if j.status === 'running'}
                <button type="button" class="btn danger sm" onclick={() => api.jobsCancel(j.id).then(load)}>Cancel</button>
              {:else if (j.status === 'paused' || j.status === 'cancelled' || j.status === 'failed') && j.kind === 'publish' && j.steps.some((s) => s.status !== 'done')}
                <button type="button" class="btn primary sm" onclick={() => api.jobsResume(j.id).then(load)}>Resume <span class="go"><Icon name="chevron-right" size={12} /></span></button>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    {/each}

    {#if activity.length}
      <p class="runs-sub muted-2">Recent activity</p>
      {#each activity.slice(0, 12) as e, i}
        <button type="button" class="run-row activity-row" onclick={() => onnavigate?.('activity')}>
          <span class="run-icon"><Icon name={e.outcome === 'blocked' || e.outcome === 'error' ? 'x' : e.outcome === 'fixed' ? 'wrench' : 'check'} size={12} /></span>
          <span class="run-main">
            <span class="run-label">{e.messages?.[0]?.replace(/^GitAssist \[[a-z_]+\] /, '') || e.outcome}</span>
            <span class="muted-2 small">{[e.source, e.repo?.split(/[\\/]/).pop()].filter(Boolean).join(' · ')}</span>
          </span>
          <span class="small muted">{relativeTime(e.ts)}</span>
        </button>
      {/each}
    {/if}
  </div>
</section>
