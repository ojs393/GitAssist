const MIN_GAP = 88;
const NODE_R = 28;
const LOCAL_R = 36;

export function layoutTopology({ width, height, destinations }) {
  const local = { x: width / 2, y: height - 72, r: LOCAL_R };
  const topY = 96;
  const maxPerRow = Math.max(1, Math.floor((width - 80) / MIN_GAP));
  const nodes = [];
  const edges = [];
  destinations.forEach((d, i) => {
    const row = Math.floor(i / maxPerRow);
    const idx = i % maxPerRow;
    const rowCount = Math.min(maxPerRow, destinations.length - row * maxPerRow);
    const rowWidth = (rowCount - 1) * MIN_GAP;
    const startX = width / 2 - rowWidth / 2;
    const x = startX + idx * MIN_GAP;
    const y = topY + row * 88;
    nodes.push({ ...d, x, y, r: NODE_R });
    edges.push({
      id: d.id,
      paused: !!d.paused,
      d: `M ${local.x} ${local.y - LOCAL_R} C ${local.x} ${(local.y + y) / 2}, ${x} ${(local.y + y) / 2}, ${x} ${y + NODE_R}`,
    });
  });
  return { local, nodes, edges };
}
