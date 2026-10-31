<script>
  import { layoutTopology } from './layout.js';
  import EdgePausePopup from './EdgePausePopup.svelte';
  import Icon from '../ui/Icon.svelte';

  /** destinations: { id, label, paused, ahead?, behind?, visibility? }[] */
  let {
    destinations = [],
    localLabel = 'This machine',
    onlocalclick,
    onnodeclick,
    onedgepause,
  } = $props();

  let root = $state(null);
  let width = $state(0);
  let height = $state(0);
  /** @type {{ id: string, x: number, y: number } | null} */
  let popup = $state(null);

  const layout = $derived(layoutTopology({ width, height, destinations }));
  const popupDest = $derived(popup ? destinations.find((d) => d.id === popup.id) : null);

  $effect(() => {
    const el = root;
    if (!el) return;
    let prevW = 0;
    let prevH = 0;
    // Don't read width/height here — that resubscribes the effect and fights the observer.
    const apply = (w, h) => {
      const nextW = Math.round(w);
      const nextH = Math.round(h);
      if (nextW === prevW && nextH === prevH) return;
      const resized = prevW !== 0 && (Math.abs(nextW - prevW) > 1 || Math.abs(nextH - prevH) > 1);
      prevW = nextW;
      prevH = nextH;
      width = nextW;
      height = nextH;
      if (resized) popup = null;
    };
    apply(el.clientWidth, el.clientHeight);
    const ro = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect;
      if (rect) apply(rect.width, rect.height);
    });
    ro.observe(el);
    return () => ro.disconnect();
  });

  $effect(() => {
    if (popup && !destinations.some((d) => d.id === popup.id)) popup = null;
  });

  function clamp(n, lo, hi) {
    if (lo > hi) return (lo + hi) / 2;
    return Math.min(Math.max(n, lo), hi);
  }

  function visibilityIcon(visibility) {
    if (visibility === 'private') return 'lock';
    if (visibility === 'public') return 'eye';
    return '';
  }

  function syncText(node) {
    const parts = [];
    if (node.ahead) parts.push(`+${node.ahead}`);
    if (node.behind) parts.push(`−${node.behind}`);
    return parts.join(' ');
  }

  function syncTitle(node) {
    const parts = [];
    if (node.ahead) parts.push(`${node.ahead} ahead`);
    if (node.behind) parts.push(`${node.behind} behind`);
    return parts.join(', ');
  }

  function nodeName(node) {
    const bits = [node.label || 'Destination'];
    if (node.visibility === 'private') bits.push('private');
    else if (node.visibility === 'public') bits.push('public');
    if (node.paused) bits.push('paused');
    const sync = syncTitle(node);
    if (sync) bits.push(sync);
    return bits.join(', ');
  }

  function openEdge(edge, node, event) {
    if (!root) return;
    const rect = root.getBoundingClientRect();
    const pointer = event && (event.clientX || event.clientY);
    const rawX = pointer ? event.clientX - rect.left : (layout.local.x + (node?.x ?? layout.local.x)) / 2;
    const rawY = pointer ? event.clientY - rect.top : (layout.local.y + (node?.y ?? layout.local.y)) / 2;
    const half = Math.min(150, rect.width / 2);
    popup = {
      id: edge.id,
      x: clamp(rawX, half, rect.width - half),
      y: clamp(rawY, 12, Math.max(12, rect.height - 96)),
    };
  }

  function onEdgeKey(event, edge, node) {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    openEdge(edge, node, null);
  }

  function confirmPause() {
    if (!popup) return;
    const id = popup.id;
    onedgepause?.(id);
    popup = null;
  }
</script>

<div class="topology" bind:this={root} role="group" aria-label="Destinations">
  {#if width > 0 && height > 0}
    <button
      type="button"
      class="topo-node local"
      style="left: {layout.local.x}px; top: {layout.local.y}px;"
      aria-label={localLabel}
      onclick={() => onlocalclick?.()}
    >
      <span class="topo-rise">
        <span class="topo-node-face" style="width: {layout.local.r * 2}px; height: {layout.local.r * 2}px">
          <Icon name="layout" size={18} stroke={1.6} />
        </span>
        <span class="topo-label below" title={localLabel}>{localLabel}</span>
      </span>
    </button>

    {#each layout.nodes as node, i (node.id)}
      {@const vis = visibilityIcon(node.visibility)}
      {@const sync = syncText(node)}
      <button
        type="button"
        class="topo-node dest"
        class:paused={node.paused}
        style="left: {node.x}px; top: {node.y}px; --i: {i}"
        aria-label={nodeName(node)}
        title={node.label}
        onclick={() => onnodeclick?.(node.id)}
      >
        <span class="topo-rise">
          <span class="topo-node-face" style="width: {node.r * 2}px; height: {node.r * 2}px">
            <Icon name="folder" size={15} stroke={1.6} />
            {#if node.paused}
              <span class="topo-badge pause" title="Paused"><Icon name="pause" size={10} stroke={2.2} /></span>
            {/if}
            {#if vis}
              <span class="topo-badge vis" title={node.visibility === 'private' ? 'Private' : 'Public'}>
                <Icon name={vis} size={10} stroke={2} />
              </span>
            {/if}
          </span>
          {#if sync}<span class="topo-sync" title={syncTitle(node)}>{sync}</span>{/if}
          <span class="topo-label above" title={node.label}>{node.label}</span>
        </span>
      </button>
    {/each}

    <svg class="topology-svg" width={width} height={height} viewBox="0 0 {width} {height}">
      {#each layout.edges as edge, i (edge.id)}
        {@const node = layout.nodes[i]}
        <g
          class="topo-edge"
          class:flow={!edge.paused}
          class:dashed={edge.paused}
          class:hot={popup?.id === edge.id}
          style="--i: {i}"
        >
          <path class="topo-edge-line" pathLength="1" d={edge.d} />
          {#if !edge.paused}
            <path class="topo-edge-flow" pathLength="1" d={edge.d} />
          {/if}
          <path
            class="topo-edge-hit"
            d={edge.d}
            role="button"
            tabindex="0"
            aria-label="{node?.label ?? 'Destination'}: {edge.paused ? 'Resume commits to this repo' : 'Pause commits to this repo'}"
            aria-expanded={popup?.id === edge.id}
            onclick={(e) => openEdge(edge, node, e)}
            onkeydown={(e) => onEdgeKey(e, edge, node)}
          >
            <title>{edge.paused ? 'Resume commits to this repo' : 'Pause commits to this repo'}</title>
          </path>
        </g>
      {/each}
    </svg>
  {/if}

  <EdgePausePopup
    open={!!popup && !!popupDest}
    x={popup?.x ?? 0}
    y={popup?.y ?? 0}
    paused={!!popupDest?.paused}
    label={popupDest?.label ?? ''}
    onconfirm={confirmPause}
    onclose={() => (popup = null)}
  />
</div>
