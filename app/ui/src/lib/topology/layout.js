const MIN_GAP = 88;
const NODE_R = 28;
const LOCAL_R = 36;
const DEFAULT_TOP_Y = 96;
const DEFAULT_ROW_PITCH = 88;
const MIN_ROW_PITCH = 48;
const ROW_CLEARANCE = NODE_R + 20;

export function layoutTopology({ width, height, destinations }) {
  const local = { x: width / 2, y: height - 72, r: LOCAL_R };
  const maxPerRow = Math.max(1, Math.floor((width - 80) / MIN_GAP));
  const rowCount = Math.max(1, Math.ceil((destinations.length || 0) / maxPerRow));
  // Keep the last destination row clear of the local node on short canvases.
  const localTop = local.y - LOCAL_R;
  const lastCenterMax = localTop - ROW_CLEARANCE;
  let topY = DEFAULT_TOP_Y;
  let rowPitch = DEFAULT_ROW_PITCH;
  if (rowCount > 1) {
    const usable = lastCenterMax - topY;
    rowPitch = Math.min(DEFAULT_ROW_PITCH, usable / (rowCount - 1));
    if (rowPitch < MIN_ROW_PITCH) {
      rowPitch = MIN_ROW_PITCH;
      topY = Math.max(40, lastCenterMax - (rowCount - 1) * rowPitch);
    } else if (topY + (rowCount - 1) * rowPitch > lastCenterMax) {
      topY = Math.max(40, lastCenterMax - (rowCount - 1) * rowPitch);
    }
  } else if (destinations.length && topY > lastCenterMax) {
    topY = Math.max(40, lastCenterMax);
  }

  const nodes = [];
  const edges = [];
  destinations.forEach((d, i) => {
    const row = Math.floor(i / maxPerRow);
    const idx = i % maxPerRow;
    const count = Math.min(maxPerRow, destinations.length - row * maxPerRow);
    const rowWidth = (count - 1) * MIN_GAP;
    const startX = width / 2 - rowWidth / 2;
    const x = startX + idx * MIN_GAP;
    const y = topY + row * rowPitch;
    nodes.push({ ...d, x, y, r: NODE_R });
    edges.push({
      id: d.id,
      paused: !!d.paused,
      d: `M ${local.x} ${local.y - LOCAL_R} C ${local.x} ${(local.y + y) / 2}, ${x} ${(local.y + y) / 2}, ${x} ${y + NODE_R}`,
    });
  });
  return { local, nodes, edges };
}
