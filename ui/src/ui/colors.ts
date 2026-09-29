/** Graph lane colour token for a lane (or any index to spread colours by). */
export function laneColor(index: number) {
  return `var(--rn-lane-${index % 8})`;
}
