<script>
  import Icon from './Icon.svelte';

  // Fixed popover so it stays above sheets/overflow parents (status bar, agent list).
  let { children, label = 'More info', align = 'left' } = $props();
  let open = $state(false);
  let root;
  let popStyle = $state('');

  function place() {
    if (!root) return;
    const r = root.getBoundingClientRect();
    const width = Math.min(300, window.innerWidth - 24);
    let left = align === 'right' ? r.right - width : r.left - 8;
    left = Math.max(12, Math.min(left, window.innerWidth - width - 12));
    let top = r.bottom + 6;
    if (top + 160 > window.innerHeight) top = Math.max(12, r.top - 6 - 160);
    popStyle = `position:fixed;top:${top}px;left:${left}px;width:${width}px;z-index:300;`;
  }

  function toggle() {
    open = !open;
    if (open) place();
  }

  function onWindowClick(e) {
    if (open && root && !root.contains(e.target)) open = false;
  }
</script>

<svelte:window
  onclick={onWindowClick}
  onkeydown={(e) => e.key === 'Escape' && (open = false)}
  onresize={() => open && place()}
  onscroll={() => open && place()}
/>

<span class="info" bind:this={root}>
  <button class="info-btn" class:on={open} type="button" aria-label={label} aria-expanded={open} onclick={toggle}>
    <Icon name="info" size={15} />
  </button>
  {#if open}
    <div class="pop fixed-pop {align}" role="dialog" style={popStyle}>{@render children()}</div>
  {/if}
</span>
