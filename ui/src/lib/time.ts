const units: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 365 * 24 * 3600],
  ["month", 30 * 24 * 3600],
  ["week", 7 * 24 * 3600],
  ["day", 24 * 3600],
  ["hour", 3600],
  ["minute", 60],
];

const relative = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
const absolute = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

/** "3 days ago", "yesterday", "now"; `now` is injectable for tests. */
export function relativeTime(seconds: number, now = Date.now() / 1000): string {
  const diff = seconds - now;
  for (const [unit, size] of units) {
    if (Math.abs(diff) >= size) return relative.format(Math.round(diff / size), unit);
  }
  return relative.format(0, "second");
}

export function formatDate(seconds: number): string {
  return absolute.format(new Date(seconds * 1000));
}
