import { LanguageDescription } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import type { Parser } from "@lezer/common";
import { classHighlighter, highlightTree } from "@lezer/highlight";

import type { DiffLine } from "../../bindings/DiffLine";

export interface Token {
  text: string;
  /** `tok-*` classes from lezer's class highlighter, or "" for plain text. */
  className: string;
}

/** Loads the parser for a file's language (code-split per language), if one is known. */
export async function loadParser(path: string): Promise<Parser | null> {
  const name = path.split("/").pop() ?? path;
  const language = LanguageDescription.matchFilename(languages, name);
  return language ? (await language.load()).language.parser : null;
}

/**
 * Highlights `lines` as a single document, so constructs spanning lines
 * (block comments, template strings) are coloured correctly, and returns
 * the tokens of each line.
 */
export function highlightLines(parser: Parser, lines: string[]): Token[][] {
  const doc = lines.join("\n");
  const ranges: [from: number, to: number, className: string][] = [];
  highlightTree(parser.parse(doc), classHighlighter, (from, to, className) =>
    ranges.push([from, to, className]),
  );

  const result: Token[][] = [];
  let r = 0;
  let lineStart = 0;
  for (const line of lines) {
    const lineEnd = lineStart + line.length;
    const tokens: Token[] = [];
    let pos = lineStart;
    while (r < ranges.length && ranges[r]![1] <= lineStart) r++;
    for (let i = r; i < ranges.length && ranges[i]![0] < lineEnd; i++) {
      const [from, to, className] = ranges[i]!;
      const start = Math.max(from, lineStart);
      const end = Math.min(to, lineEnd);
      if (start > pos) tokens.push({ text: doc.slice(pos, start), className: "" });
      if (end > start) tokens.push({ text: doc.slice(start, end), className });
      pos = Math.max(pos, end);
    }
    if (pos < lineEnd) tokens.push({ text: doc.slice(pos, lineEnd), className: "" });
    result.push(tokens);
    lineStart = lineEnd + 1;
  }
  return result;
}

/**
 * Tokens for each line of a hunk. The old side (context and removals) and
 * the new side (context and additions) are highlighted separately, since
 * each is valid code on its own and the mix is not.
 */
export function highlightHunk(parser: Parser, lines: DiffLine[]): Map<DiffLine, Token[]> {
  const oldSide = lines.filter((l) => l.kind !== "added");
  const newSide = lines.filter((l) => l.kind !== "removed");
  const tokens = new Map<DiffLine, Token[]>();
  const oldTokens = highlightLines(
    parser,
    oldSide.map((l) => l.text),
  );
  oldSide.forEach((l, i) => tokens.set(l, oldTokens[i]!));
  // Context lines take the new side's tokens.
  const newTokens = highlightLines(
    parser,
    newSide.map((l) => l.text),
  );
  newSide.forEach((l, i) => tokens.set(l, newTokens[i]!));
  return tokens;
}
