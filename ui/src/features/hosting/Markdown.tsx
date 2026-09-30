import { lazy, Suspense } from "react";

// The Markdown parser is only loaded once a pull request is opened.
const MarkdownView = lazy(() => import("./MarkdownView"));

/** Rendered Markdown; plain text until the renderer has loaded. */
export function Markdown({ text, base }: { text: string; base?: string }) {
  return (
    <Suspense fallback={<p className="break-words whitespace-pre-wrap">{text}</p>}>
      <MarkdownView text={text} base={base} />
    </Suspense>
  );
}
