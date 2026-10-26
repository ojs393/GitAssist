<script>
  import Icon from './Icon.svelte';
  import Info from './Info.svelte';
  // Collapsible cards let you hide what you don't need; the state lasts for the session.
  let { title = '', icon = '', info = '', actions, children, tight = false, collapsible = false, collapsed = $bindable(false) } = $props();
</script>

<section class="card" class:tight class:collapsed>
  {#if title}
    <header class="card-head">
      <div class="card-title">
        {#if icon}<span class="card-icon"><Icon name={icon} size={16} /></span>{/if}
        <h2>{title}</h2>
        {#if info}<Info><p>{info}</p></Info>{/if}
      </div>
      <div class="card-actions">
        {#if actions}{@render actions()}{/if}
        {#if collapsible}
          <button class="icon-btn" aria-label={collapsed ? 'Show' : 'Hide'} aria-expanded={!collapsed} onclick={() => (collapsed = !collapsed)}>
            <Icon name={collapsed ? 'chevron' : 'chevron-up'} size={15} />
          </button>
        {/if}
      </div>
    </header>
  {/if}
  {#if !collapsed}<div class="card-body">{@render children()}</div>{/if}
</section>
