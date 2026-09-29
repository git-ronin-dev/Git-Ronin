import { Archive, Boxes, Cloud, GitBranch, Tag } from "lucide-react";

import { useRepoStore } from "../features/repo/store";
import { Section } from "../ui/Section";

const icon = "size-3.5";

/** Refs overview. Only the current branch is known until Phase 1 loads refs. */
export function Sidebar() {
  const repo = useRepoStore((s) => s.repo);
  if (!repo) return null;

  return (
    <nav aria-label="Repository" className="h-full overflow-y-auto py-2">
      <Section title="Local" icon={<GitBranch className={icon} />}>
        {repo.head.kind === "branch" && (
          <div className="flex h-7 items-center gap-2 px-4 pl-9 text-fg">
            <span className="size-1.5 rounded-full bg-accent" aria-label="current branch" />
            <span className="truncate">{repo.head.name}</span>
          </div>
        )}
      </Section>
      <Section title="Remote" icon={<Cloud className={icon} />} />
      <Section title="Tags" icon={<Tag className={icon} />} />
      <Section title="Stashes" icon={<Archive className={icon} />} />
      <Section title="Submodules" icon={<Boxes className={icon} />} />
    </nav>
  );
}
