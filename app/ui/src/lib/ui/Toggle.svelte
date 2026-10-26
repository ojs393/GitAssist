<script>
  let { checked = false, onchange, label = '', disabled = false, small = false } = $props();
</script>

<label class="toggle" class:small class:disabled>
  <input type="checkbox" role="switch" {checked} {disabled} onchange={(e) => onchange?.(e.currentTarget.checked)} />
  <span class="track"><span class="thumb"></span></span>
  {#if label}<span class="toggle-label">{label}</span>{/if}
</label>
