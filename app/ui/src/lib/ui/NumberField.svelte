<script>
  let { value, onchange, step = 1, min = 0, max = 100000, unit = '', disabled = false } = $props();

  function set(v) {
    const n = Math.max(min, Math.min(max, Number.isFinite(v) ? Math.round(v) : min));
    onchange(n);
  }
</script>

<div class="num" class:disabled>
  <button type="button" aria-label="Less" {disabled} onclick={() => set(value - step)}>−</button>
  <input type="number" inputmode="numeric" {min} {max} {disabled} value={value} onchange={(e) => set(parseInt(e.currentTarget.value || '0', 10))} />
  <button type="button" aria-label="More" {disabled} onclick={() => set(value + step)}>+</button>
  {#if unit}<span class="num-unit">{unit}</span>{/if}
</div>
