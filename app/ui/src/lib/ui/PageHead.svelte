<script>
  import Icon from './Icon.svelte';
  import Info from './Info.svelte';

  let { title, icon = 'folder', info = '', actions } = $props();
</script>

<header class="page-head">
  <div class="page-title">
    <span class="page-icon"><Icon name={icon} size={15} /></span>
    <h1>{title}</h1>
    {#if info}<Info><p>{info}</p></Info>{/if}
  </div>
  {#if actions}<div class="page-actions">{@render actions()}</div>{/if}
</header>
