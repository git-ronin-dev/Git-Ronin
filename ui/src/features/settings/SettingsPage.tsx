import type { ReactNode } from "react";

/** One settings section: a heading and its fields. */
export function SettingsPage({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="space-y-4 pb-4">
      <header className="space-y-1">
        <h2 className="text-sm font-semibold">{title}</h2>
        {description && <p className="text-fg-muted">{description}</p>}
      </header>
      {children}
    </section>
  );
}

/** A titled group inside a settings section. */
export function SettingsGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="space-y-3 border-t border-line pt-4">
      <h3 className="text-xs font-semibold tracking-wide text-fg-muted uppercase">{title}</h3>
      {children}
    </div>
  );
}
