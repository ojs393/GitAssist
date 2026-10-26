<script>
  import Info from './Info.svelte';
  // One setting: label (+ optional hidden details) on the left, control on the right.
  let { label, info = '', children, stacked = false } = $props();
</script>

<div class="field" class:stacked>
  <div class="field-label">
    <span>{label}</span>
    {#if info}<Info><p>{info}</p></Info>{/if}
  </div>
  <div class="field-control">{@render children()}</div>
</div>
