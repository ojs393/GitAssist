<script>
  let { repoKey, targetId, onback } = $props();
</script>

<div class="page">
  <h1 class="page-title">Destination</h1>
  <p class="muted">{repoKey} / {targetId}</p>
  <button type="button" class="btn ghost sm" onclick={() => onback?.()}>Back</button>
</div>
