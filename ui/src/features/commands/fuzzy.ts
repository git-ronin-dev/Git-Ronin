/**
 * Scores how well `query` matches `text` as an in-order subsequence, or
 * returns null if it doesn't. Matches at word starts and runs of adjacent
 * letters score higher, so "cob" prefers "Check out branch" to "Clone
 * repository… (Ctrl+O) b".
 */
export function fuzzyScore(query: string, text: string): number | null {
  const q = query.toLowerCase().replace(/\s+/g, "");
  if (!q) return 0;
  const t = text.toLowerCase();
  let score = 0;
  let at = 0;
  let previous = -2;
  for (const ch of q) {
    const found = t.indexOf(ch, at);
    if (found < 0) return null;
    const wordStart = found === 0 || /[\s/:._-]/.test(t[found - 1]!);
    score += 1 + (wordStart ? 3 : 0) + (found === previous + 1 ? 2 : 0);
    // Skipped letters cost a little.
    score -= Math.min(found - at, 3) * 0.1;
    previous = found;
    at = found + 1;
  }
  return score;
}

/** `items` matching `query`, best first; ties keep their order. */
export function fuzzyFilter<T>(items: T[], query: string, text: (item: T) => string): T[] {
  if (!query.trim()) return items;
  return items
    .map((item, index) => ({ item, index, score: fuzzyScore(query, text(item)) }))
    .filter((m): m is { item: T; index: number; score: number } => m.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((m) => m.item);
}
