<script>
  import Icon from './Icon.svelte';

  // Shows working / done / failed so every click visibly lands; the error stays until the next try.
  let { run, label, icon = '', doneLabel = 'Done', kind = 'primary', size = 'sm', onresult, disabled = false, go = false } = $props();
  let state = $state('idle');
  let error = $state('');
  let timer;

  async function click() {
    clearTimeout(timer);
    state = 'working';
    error = '';
    try {
      const r = await run();
      state = 'done';
      onresult?.(r, null);
      timer = setTimeout(() => (state = 'idle'), 2200);
    } catch (e) {
      state = 'failed';
      error = String(e).replace(/^Error: /, '');
      onresult?.(null, error);
    }
  }
</script>

<span class="action">
  <button type="button" class="btn {kind} {size} {state}" disabled={disabled || state === 'working'} onclick={click}>
    {#if state === 'working'}
      <span class="spinner"></span>
    {:else if state === 'done'}
      <Icon name="check" size={15} />
    {:else if state === 'failed'}
      <Icon name="refresh" size={15} />
    {:else if icon}
      <Icon name={icon} size={15} />
    {/if}
    <span>{state === 'done' ? doneLabel : state === 'failed' ? 'Try again' : label}</span>
    {#if go && state === 'idle'}<span class="go"><Icon name="chevron-right" size={14} /></span>{/if}
  </button>
  {#if state === 'failed'}<span class="action-error">{error}</span>{/if}
</span>
