import { useQuery } from "@tanstack/react-query";

import type { Token } from "./highlight";

/**
 * Highlights whole lines of a file in its language, or null while the
 * parser loads (or when the language is unknown). The highlighting code and
 * each language's parser are loaded on first use.
 */
export function useLineHighlighter(path: string): ((lines: string[]) => Token[][]) | null {
  return (
    useQuery({
      queryKey: ["lineHighlighter", path.split("/").pop()],
      queryFn: async () => {
        const { loadParser, highlightLines } = await import("./highlight");
        const parser = await loadParser(path);
        return parser ? { run: (lines: string[]) => highlightLines(parser, lines) } : null;
      },
      staleTime: Infinity,
    }).data?.run ?? null
  );
}
