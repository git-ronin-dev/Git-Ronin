import { clsx } from "clsx";
import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import type { Search as SearchState } from "../workspace/view";

interface SearchBarProps {
  search: SearchState;
  onChange: (search: SearchState | null) => void;
  matches: number[] | undefined;
  loading: boolean;
  position: number;
  onStep: (delta: 1 | -1) => void;
}

export function SearchBar({
  search,
  onChange,
  matches,
  loading,
  position,
  onStep,
}: SearchBarProps) {
  const [text, setText] = useState(search.query);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => input.current?.focus(), []);
  // Searching loads the whole history, so wait until typing pauses.
  useEffect(() => {
    if (text === search.query) return;
    const timer = setTimeout(() => onChange({ ...search, query: text }), 250);
    return () => clearTimeout(timer);
  }, [text, search, onChange]);

  const status = !search.query
    ? ""
    : loading
      ? "Searching…"
      : matches?.length
        ? `${position + 1} of ${matches.length}`
        : "No results";

  return (
    <div className="flex h-10 shrink-0 items-center gap-2 border-b border-line bg-surface px-3">
      <Search className="size-4 text-fg-muted" />
      <input
        ref={input}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape") onChange(null);
          if (e.key === "Enter") onStep(e.shiftKey ? -1 : 1);
        }}
        placeholder={search.byPath ? "Commits touching path…" : "Message, author or SHA…"}
        aria-label="Search commits"
        className="h-7 min-w-0 flex-1 bg-transparent outline-none placeholder:text-fg-faint"
      />
      <span className="text-xs whitespace-nowrap text-fg-muted">{status}</span>
      <div className="flex rounded-md border border-line text-xs">
        {[false, true].map((byPath) => (
          <button
            key={String(byPath)}
            type="button"
            onClick={() => onChange({ ...search, byPath })}
            className={clsx(
              "h-6 px-2 first:rounded-l-md last:rounded-r-md",
              search.byPath === byPath ? "bg-raised text-fg" : "text-fg-muted hover:text-fg",
            )}
          >
            {byPath ? "Path" : "Commit"}
          </button>
        ))}
      </div>
      <IconButton label="Previous result" onClick={() => onStep(-1)} icon={ChevronUp} />
      <IconButton label="Next result" onClick={() => onStep(1)} icon={ChevronDown} />
      <IconButton label="Close search" onClick={() => onChange(null)} icon={X} />
    </div>
  );
}

function IconButton({
  label,
  onClick,
  icon: Icon,
}: {
  label: string;
  onClick: () => void;
  icon: typeof X;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className="flex size-6 items-center justify-center rounded-sm text-fg-muted hover:bg-hover hover:text-fg"
    >
      <Icon className="size-4" />
    </button>
  );
}
