const MIN_GAP = 88;
const NODE_R = 28;
const LOCAL_R = 36;
const DEFAULT_TOP_Y = 96;
const DEFAULT_ROW_PITCH = 88;
const MIN_ROW_PITCH = 48;
const ROW_CLEARANCE = NODE_R + 20;
const MIN_TOP_Y = NODE_R + 8;

export function layoutTopology({ width, height, destinations }) {
  const maxPerRow = Math.max(1, Math.floor((width - 80) / MIN_GAP));
  const n = destinations.length;
  const rowCount = Math.max(1, Math.ceil((n || 1) / maxPerRow));
  const maxLocalY = height - LOCAL_R - 4;

  let localY = Math.min(height - 72, maxLocalY);
  const lastCenterMaxFor = (ly) => ly - LOCAL_R - ROW_CLEARANCE;

  // Push local down when the canvas is too short for the preferred stack.
  while (lastCenterMaxFor(localY) < MIN_TOP_Y && localY < maxLocalY) {
    localY = Math.min(maxLocalY, localY + 8);
  }

  let lastCenterMax = lastCenterMaxFor(localY);
  let topY = DEFAULT_TOP_Y;
  let rowPitch = DEFAULT_ROW_PITCH;

  if (n === 0) {
    // Local alone.
  } else if (rowCount === 1) {
    topY = Math.min(DEFAULT_TOP_Y, Math.max(MIN_TOP_Y, lastCenterMax));
  } else {
    const naturalLast = DEFAULT_TOP_Y + (rowCount - 1) * DEFAULT_ROW_PITCH;
    if (naturalLast <= lastCenterMax) {
      topY = DEFAULT_TOP_Y;
      rowPitch = DEFAULT_ROW_PITCH;
    } else {
      // Fit into [MIN_TOP_Y, lastCenterMax], shrinking pitch past MIN_ROW_PITCH if needed.
      const topAtMinPitch = lastCenterMax - (rowCount - 1) * MIN_ROW_PITCH;
      if (topAtMinPitch >= DEFAULT_TOP_Y) {
        topY = DEFAULT_TOP_Y;
        rowPitch = (lastCenterMax - topY) / (rowCount - 1);
      } else if (topAtMinPitch >= MIN_TOP_Y) {
        topY = topAtMinPitch;
        rowPitch = MIN_ROW_PITCH;
      } else {
        topY = MIN_TOP_Y;
        const avail = lastCenterMax - topY;
        if (avail < 0 && localY < maxLocalY) {
          localY = Math.min(maxLocalY, MIN_TOP_Y + LOCAL_R + ROW_CLEARANCE);
          lastCenterMax = lastCenterMaxFor(localY);
        }
        rowPitch = Math.max(0, (lastCenterMaxFor(localY) - topY) / (rowCount - 1));
      }
    }
  }

  const local = { x: width / 2, y: localY, r: LOCAL_R };
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
