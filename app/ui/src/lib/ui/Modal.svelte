<script>
  import Icon from './Icon.svelte';
  let { open = false, onclose, title = '', wide = false, children, footer } = $props();
</script>

<svelte:window onkeydown={(e) => open && e.key === 'Escape' && onclose?.()} />

{#if open}
  <div class="modal-backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose?.()}>
    <div class="modal" class:wide role="dialog" aria-modal="true" aria-label={title}>
      <header class="modal-head">
        <h2>{title}</h2>
        {#if onclose}<button class="icon-btn" aria-label="Close" onclick={onclose}><Icon name="x" size={16} /></button>{/if}
      </header>
      <div class="modal-body">{@render children()}</div>
      {#if footer}<footer class="modal-foot">{@render footer()}</footer>{/if}
    </div>
  </div>
{/if}
