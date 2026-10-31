<script>
  import Icon from './Icon.svelte';
  // Inner tabs: options [{ id, label, icon }]
  let { options, value, onchange } = $props();
</script>

<div class="tabs" role="tablist">
  {#each options as o}
    <button role="tab" aria-selected={value === o.id} class:active={value === o.id} onclick={() => onchange(o.id)}>
      {#if o.icon}<Icon name={o.icon} size={13} />{/if}<span>{o.label}</span>
    </button>
  {/each}
</div>
