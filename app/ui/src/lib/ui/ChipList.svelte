<script>
  import Icon from './Icon.svelte';

  // Shows a list as chips; Edit switches to a one-per-line editor.
  let { items = [], onchange, placeholder = 'One per line', empty = 'None', mono = false } = $props();
  let editing = $state(false);
  let draft = $state('');

  function edit() {
    draft = items.join('\n');
    editing = true;
  }
  function done() {
    onchange(draft.split('\n').map((x) => x.trim()).filter(Boolean));
    editing = false;
  }
  function remove(i) {
    onchange(items.filter((_, j) => j !== i));
  }
</script>

{#if editing}
  <div class="chip-editor">
    <!-- svelte-ignore a11y_autofocus -->
    <textarea class:mono rows={Math.min(10, Math.max(3, items.length + 1))} bind:value={draft} {placeholder} autofocus
      onkeydown={(e) => (e.metaKey || e.ctrlKey) && e.key === 'Enter' && done()}></textarea>
    <div class="row-end">
      <button class="btn ghost sm" type="button" onclick={() => (editing = false)}>Cancel</button>
      <button class="btn primary sm" type="button" onclick={done}><Icon name="check" size={14} /> Done</button>
    </div>
  </div>
{:else}
  <div class="chips">
    {#each items as item, i}
      <span class="chip" class:mono title={item}>
        <span class="chip-text">{item}</span>
        <button type="button" class="chip-x" aria-label="Remove {item}" onclick={() => remove(i)}><Icon name="x" size={12} /></button>
      </span>
    {:else}
      <span class="muted small">{empty}</span>
    {/each}
    <button class="btn ghost sm" type="button" onclick={edit}><Icon name={items.length ? 'edit' : 'plus'} size={14} /> {items.length ? 'Edit' : 'Add'}</button>
  </div>
{/if}
