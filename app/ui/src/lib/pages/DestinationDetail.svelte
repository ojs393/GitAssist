<script>
  import { onDestroy } from 'svelte';
  import { api } from '../api.js';
  import Icon from '../ui/Icon.svelte';
  import PageHead from '../ui/PageHead.svelte';
  import ActionButton from '../ui/ActionButton.svelte';
  import TargetEditor from './TargetEditor.svelte';
  import {
    enqueueTargetsWriteForRepo,
    waitForTargetsWriteForRepo,
  } from '../topology/targetsWriteQueue.js';

  let { repoKey, targetId, onback } = $props();

  let doc = $state(null);
  let target = $state(null);
  let defaultTarget = $state(null);
  let loading = $state(true);
  let saving = $state(false);
  let error = $state('');
  let missing = $state('');
  let saveNote = $state('');
  let pushMsg = $state('');
  let sync = $state(null);
  let syncError = $state('');

  let ticket = 0;
  let syncTicket = 0;
  let saveTicket = 0;
  let alive = true;
  onDestroy(() => {
    alive = false;
  });

  const repoName = $derived((doc?.repo_path || '').split(/[\\/]/).filter(Boolean).pop() || '');
  const syncLine = $derived.by(() => {
    if (syncError) return syncError;
    if (sync) {
      const where = sync.ref_name || [sync.remote, sync.branch].filter(Boolean).join('/');
      const counts = `${sync.ahead ?? 0} ahead · ${sync.behind ?? 0} behind`;
      return where ? `${counts} · ${where}` : counts;
    }
    return 'No sync yet';
  });

  function pageLeft(myPage, key, id) {
    return !alive || myPage !== ticket || repoKey !== key || targetId !== id;
  }

  function message(e) {
    return String(e).replace(/^Error: /, '');
  }

  function normalize(t) {
    if (!t.checks) t.checks = { enabled: false, on_fail: 'block', timeout_secs: 600, commands: [] };
    if (!Array.isArray(t.checks.commands)) t.checks.commands = [];
    if (!Array.isArray(t.publish_filter)) t.publish_filter = [];
  }

  const roleIcon = (role) => (role === 'public' ? 'eye' : role === 'organization' ? 'building' : 'user');

  async function load(key, id) {
    const my = ++ticket;
    loading = true;
    error = '';
    missing = '';
    saveNote = '';
    pushMsg = '';
    sync = null;
    syncError = '';
    try {
      await waitForTargetsWriteForRepo({ key, path: doc?.repo_path });
      if (!alive || my !== ticket) return;
      const docIn = await api.targetsGetByKey(key);
      if (!alive || my !== ticket) return;
      if (!docIn) {
        doc = null;
        target = null;
        missing = 'No targets file for this repo.';
        return;
      }
      const found = (docIn.targets ?? []).find((t) => t.id === id);
      if (!found) {
        doc = docIn;
        target = null;
        missing = 'This destination is not in the repo.';
        return;
      }
      normalize(found);
      doc = docIn;
      target = structuredClone($state.snapshot(found));
      defaultTarget = docIn.default_target ?? null;
      try {
        await refreshSync();
      } catch {
        // refreshSync stores syncError for the hero line.
      }
    } catch (e) {
      if (!alive || my !== ticket) return;
      error = message(e);
      doc = null;
      target = null;
    } finally {
      if (alive && my === ticket) loading = false;
    }
  }

  $effect(() => {
    load(repoKey, targetId);
  });

  async function refreshSync() {
    const my = ++syncTicket;
    const myPage = ticket;
    const key = repoKey;
    const id = targetId;
    syncError = '';
    try {
      const path = doc?.repo_path;
      const remote = target?.remote_name?.trim() ?? '';
      if (!path) throw new Error('Repo path is missing.');
      if (!remote) throw new Error('Set a remote name to compare.');
      const work = await api.repoWork(path);
      if (pageLeft(myPage, key, id)) throw new Error('This destination changed before sync finished.');
      if (my !== syncTicket) return;
      const branch = (work?.branch || '').trim();
      if (!branch) throw new Error('No current branch to compare.');
      const row = await api.repoWorkRemote(path, remote, branch);
      if (pageLeft(myPage, key, id)) throw new Error('This destination changed before sync finished.');
      if (my !== syncTicket) return;
      sync = row;
    } catch (e) {
      if (pageLeft(myPage, key, id)) throw e;
      if (my !== syncTicket) return;
      sync = null;
      syncError = message(e);
      throw new Error(syncError);
    }
  }

  async function persist() {
    const myPage = ticket;
    const key = repoKey;
    const id = targetId;
    if (!doc || !target) throw new Error('Nothing to save.');
    if (saving) throw new Error('Save already in progress.');
    const label = target.label.trim();
    const remote = target.remote_name.trim();
    if (!label || !remote) {
      const msg = 'Label and remote name are required.';
      error = msg;
      throw new Error(msg);
    }
    const next = structuredClone($state.snapshot(doc));
    const i = next.targets.findIndex((t) => t.id === id);
    if (i < 0) throw new Error('This destination is not in the repo.');
    const draft = structuredClone($state.snapshot(target));
    draft.label = label;
    draft.remote_name = remote;
    next.targets[i] = draft;
    const picked = typeof defaultTarget === 'string' ? defaultTarget.trim() : '';
    next.default_target = picked || null;
    const mySave = ++saveTicket;
    saving = true;
    error = '';
    try {
      const notes = await enqueueTargetsWriteForRepo({ key, path: next.repo_path }, () =>
        api.targetsSave(next.repo_path, next),
      );
      if (pageLeft(myPage, key, id)) throw new Error('This destination changed before the save finished.');
      const fresh = await api.targetsGetByKey(key);
      if (pageLeft(myPage, key, id)) throw new Error('This destination changed before the save finished.');
      if (!fresh) throw new Error('Saved, but the targets file is missing.');
      const found = (fresh.targets ?? []).find((t) => t.id === id);
      if (!found) throw new Error('Saved, but this destination is missing.');
      normalize(found);
      doc = fresh;
      target = structuredClone($state.snapshot(found));
      defaultTarget = fresh.default_target ?? null;
      saveNote = notes?.length ? notes.join('; ') : 'Saved.';
      return true;
    } catch (e) {
      if (!pageLeft(myPage, key, id)) error = message(e);
      throw e;
    } finally {
      if (mySave === saveTicket) saving = false;
    }
  }

  async function removeDestination() {
    const myPage = ticket;
    const key = repoKey;
    const id = targetId;
    if (!doc || !target) throw new Error('Nothing to remove.');
    if (saving) throw new Error('Save already in progress.');
    const label = (target.label || '').trim() || 'this destination';
    if (!confirm(`Remove destination “${label}”? This cannot be undone.`)) {
      throw new Error('Cancelled');
    }
    const path = doc.repo_path;
    const saved = (doc.targets ?? []).find((t) => t.id === id);
    const next = structuredClone($state.snapshot(doc));
    next.targets = (next.targets ?? []).filter((t) => t.id !== id);
    const labels = new Set([saved?.label, target.label, label].filter(Boolean));
    if (next.default_target && labels.has(next.default_target)) {
      next.default_target = null;
    }
    const mySave = ++saveTicket;
    saving = true;
    error = '';
    try {
      await enqueueTargetsWriteForRepo({ key, path }, () => api.targetsSave(path, next));
      if (pageLeft(myPage, key, id)) return;
      onback?.();
    } catch (e) {
      if (!pageLeft(myPage, key, id)) error = message(e);
      throw e;
    } finally {
      if (mySave === saveTicket) saving = false;
    }
  }

  async function setEnabled(v) {
    if (!target || v === target.enabled) return;
    const prev = target.enabled;
    const myPage = ticket;
    const key = repoKey;
    const id = targetId;
    target.enabled = v;
    try {
      await persist();
    } catch (e) {
      if (pageLeft(myPage, key, id)) return;
      if (target) target.enabled = prev;
      error = message(e);
    }
  }

  async function push() {
    const myPage = ticket;
    const key = repoKey;
    const id = targetId;
    await persist();
    if (pageLeft(myPage, key, id) || !doc || !target) {
      throw new Error('This destination changed before the save finished.');
    }
    if (!target.enabled) throw new Error('Resume this destination before pushing.');
    const label = target.label.trim();
    const job = await api.jobStartPublish(doc.repo_path, label);
    if (pageLeft(myPage, key, id)) throw new Error('This destination changed before the push finished.');
    pushMsg = `Queued ${job.label}. Watch Overview → Runs.`;
  }
</script>

<div class="dest">
  <PageHead title={target?.label || 'Destination'} icon="upload" info="Publish settings for this one destination. Pausing it leaves the rest of the machine on.">
    {#snippet actions()}
      <button type="button" class="btn ghost sm" onclick={() => onback?.()}>
        <Icon name="chevron-left" size={13} /> Overview
      </button>
    {/snippet}
  </PageHead>

  {#if error && !target}<p class="small bad">{error}</p>{/if}

  {#if loading}
    <div class="loading"><span class="spinner sm"></span></div>
  {:else if missing}
    <p class="muted">{missing}</p>
    <button type="button" class="btn ghost sm" onclick={() => onback?.()}>
      <Icon name="chevron-left" size={13} /> Back
    </button>
  {:else if target}
    <button
      type="button"
      class="hero status {target.enabled ? 'ok' : 'paused'}"
      aria-pressed={target.enabled}
      aria-label={target.enabled ? `Pause ${target.label}` : `Resume ${target.label}`}
      disabled={saving}
      onclick={() => setEnabled(!target.enabled)}
    >
      <span class="hero-icon face">
        <Icon name={roleIcon(target.role)} size={18} stroke={1.6} />
        {#if !target.enabled}
          <span class="pause-badge" title="Paused"><Icon name="pause" size={9} stroke={2.2} /></span>
        {/if}
      </span>
      <span class="hero-text">
        <h1>{target.enabled ? 'Publishing' : 'Paused'}</h1>
        <p>{target.label}{repoName ? ` · ${repoName}` : ''} · {syncLine}</p>
      </span>
    </button>

    <div class="action-bar sticky">
      <button
        type="button"
        class="btn sm {target.enabled ? 'ghost' : 'primary'}"
        disabled={saving}
        onclick={() => setEnabled(!target.enabled)}
      >
        <Icon name={target.enabled ? 'pause' : 'play'} size={13} />
        {target.enabled ? 'Pause' : 'Resume'}
      </button>
      <ActionButton
        run={push}
        label="Push"
        icon="upload"
        size="sm"
        doneLabel="Queued"
        disabled={!target.enabled || saving}
      />
      <ActionButton run={refreshSync} label="Sync" icon="refresh" kind="ghost" size="sm" doneLabel="Ok" disabled={saving} />
      <ActionButton run={persist} label="Save" icon="check" kind="primary" size="sm" doneLabel="Saved" disabled={saving} />
      <ActionButton
        run={removeDestination}
        label="Remove"
        icon="trash"
        kind="danger"
        size="sm"
        doneLabel="Removed"
        disabled={saving}
      />
      {#if saveNote}<span class="save-pill ok"><Icon name="check" size={13} />{saveNote}</span>{/if}
      <span class="grow"></span>
      <button type="button" class="btn ghost sm" onclick={() => onback?.()}>
        <Icon name="chevron-left" size={13} /> Back
      </button>
    </div>

    {#if error}<p class="small bad">{error}</p>{/if}
    {#if pushMsg}<p class="small muted">{pushMsg}</p>{/if}
    <p class="muted small">Pause saves this form. Push uses the label, then queues a publish job.</p>
    {#if doc?.repo_path}<p class="mono small muted-2 break">{doc.repo_path}</p>{/if}

    <div class="sheet" class:locked={saving}>
      <TargetEditor bind:target bind:defaultTarget onenabled={setEnabled} />
    </div>
  {/if}
</div>

<style>
  .status {
    width: 100%;
    text-align: left;
    cursor: pointer;
    border: 0;
    font: inherit;
    color: inherit;
  }
  .status:disabled { cursor: progress; }
  .face { position: relative; }
  .pause-badge {
    position: absolute;
    left: -3px;
    bottom: -3px;
    width: 16px;
    height: 16px;
    border-radius: 999px;
    display: grid;
    place-items: center;
    background: var(--panel);
    color: var(--muted);
    border: 1px solid var(--line-strong);
  }
  .action-bar {
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding: 8px 0;
    margin-bottom: 8px;
    background: var(--bg);
  }
  .locked { pointer-events: none; }
</style>
