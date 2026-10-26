<script>
  import Icon from './Icon.svelte';

  // Details stay hidden until the (i) is clicked; Escape or a click elsewhere closes it.
  let { children, label = 'More info', align = 'left' } = $props();
  let open = $state(false);
  let root;

  function onWindowClick(e) {
    if (open && root && !root.contains(e.target)) open = false;
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={(e) => e.key === 'Escape' && (open = false)} />

<span class="info" bind:this={root}>
  <button class="info-btn" class:on={open} type="button" aria-label={label} aria-expanded={open} onclick={() => (open = !open)}>
    <Icon name="info" size={15} />
  </button>
  {#if open}
    <div class="pop {align}" role="dialog">{@render children()}</div>
  {/if}
</span>
