<script>
  let { open = false, x = 0, y = 0, paused = false, label = '', onconfirm, onclose } = $props();

  let confirmBtn = $state(null);
  const text = $derived(paused ? 'Resume commits to this repo' : 'Pause commits to this repo');

  $effect(() => {
    if (open) confirmBtn?.focus();
  });
</script>

<svelte:window onkeydown={(e) => open && e.key === 'Escape' && onclose?.()} />

{#if open}
  <div class="topo-pop-backdrop" role="presentation" onclick={() => onclose?.()}></div>
  <div class="topo-pop" style="left: {x}px; top: {y}px;" role="dialog" aria-label={label || text}>
    {#if label}<p class="topo-pop-name">{label}</p>{/if}
    <button type="button" class="btn primary" bind:this={confirmBtn} onclick={() => onconfirm?.()}>{text}</button>
  </div>
{/if}
