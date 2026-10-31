<script>
  import Icon from './Icon.svelte';

  let { login = '', owner = 'unknown', size = 28 } = $props();
  let failed = $state(false);

  const src = $derived(
    login && !failed ? `https://github.com/${encodeURIComponent(login)}.png?size=${size * 2}` : '',
  );
  const glyph = $derived(owner === 'organization' ? 'building' : owner === 'personal' ? 'user' : 'folder');

  $effect(() => {
    login;
    failed = false;
  });
</script>

{#if src}
  <img class="repo-avatar" src={src} alt="" width={size} height={size} onerror={() => (failed = true)} />
{:else}
  <span class="repo-avatar fallback" style="width:{size}px;height:{size}px">
    <Icon name={glyph} size={Math.round(size * 0.5)} />
  </span>
{/if}

<style>
  .repo-avatar {
    border-radius: 8px;
    flex: none;
    object-fit: cover;
    background: var(--panel-3);
  }
  .fallback {
    display: grid;
    place-items: center;
    color: var(--muted);
  }
</style>
