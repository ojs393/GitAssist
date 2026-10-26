<script>
  import Icon from './Icon.svelte';
  // A large selectable option card (presets).
  let { icon, title, note = '', selected = false, onclick } = $props();
</script>

<button type="button" class="choice" class:selected aria-pressed={selected} {onclick}>
  <span class="choice-icon"><Icon name={icon} size={18} /></span>
  <span class="choice-text"><strong>{title}</strong>{#if note}<span>{note}</span>{/if}</span>
  {#if selected}<span class="choice-check"><Icon name="check" size={14} /></span>{/if}
</button>
