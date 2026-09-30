const rad = (deg: number) => (deg * Math.PI) / 180;
const point = (cx: number, cy: number, r: number, deg: number) =>
  `${+(cx + r * Math.cos(rad(deg))).toFixed(2)} ${+(cy + r * Math.sin(rad(deg))).toFixed(2)}`;

/**
 * An ensō: a circle drawn clockwise in one stroke, open by `gap` degrees at
 * the upper right. Used by the icons and the graph's commit dots.
 */
export function ensoPath(cx: number, cy: number, r: number, gap = 50) {
  const start = -40 + gap / 2;
  const end = start + 360 - gap;
  return `M${point(cx, cy, r, start)}A${r} ${r} 0 1 1 ${point(cx, cy, r, end)}`;
}

/**
 * A brush-drawn ensō as a filled outline: the stroke starts heavy, swells a
 * little and thins to a dry tail. `width` is the heaviest part.
 */
export function brushEnsoPath(cx: number, cy: number, r: number, width: number, gap = 40) {
  const steps = 72;
  // Pressure along the stroke (0..1): full at the start, a second swell a
  // third of the way round, then lifting off into a dry tail.
  const widthAt = (t: number) =>
    (0.35 + 0.65 * Math.sin(Math.PI * Math.min(1, t * 1.6 + 0.35)) ** 0.8) * (1 - 0.55 * t ** 3);
  const start = -40 + gap / 2;
  const sweep = 360 - gap;
  const outer: string[] = [];
  const inner: string[] = [];
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const w = width * widthAt(t);
    const deg = start + sweep * t;
    // The brush drifts slightly outwards as it goes round.
    const rr = r * (1 + 0.03 * Math.sin(t * Math.PI * 2));
    outer.push(point(cx, cy, rr + w / 2, deg));
    inner.push(point(cx, cy, rr - w / 2, deg));
  }
  // A round cap where the brush first touches down.
  const cap = (width * widthAt(0)) / 2;
  return `M${outer.join("L")}L${inner.reverse().join("L")}A${cap} ${cap} 0 0 1 ${outer[0]}Z`;
}
