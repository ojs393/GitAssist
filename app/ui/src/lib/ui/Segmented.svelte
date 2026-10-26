<script>
  import Icon from './Icon.svelte';
  let { options, value, onchange, disabled = false, size = 'md' } = $props();
</script>

<div class="seg {size}" class:disabled role="radiogroup">
  {#each options as o}
    <button type="button" role="radio" aria-checked={value === o.id} class:active={value === o.id} {disabled} title={o.title ?? ''} onclick={() => onchange(o.id)}>
      {#if o.icon}<Icon name={o.icon} size={14} />{/if}
      <span>{o.label}</span>
    </button>
  {/each}
</div>
