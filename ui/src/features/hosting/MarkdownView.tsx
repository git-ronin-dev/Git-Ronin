import type { MouseEvent } from "react";
import ReactMarkdown, { defaultUrlTransform, type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

import { openUrl } from "../../lib/open";

const external = (href: string | undefined) => !!href && /^https?:\/\//i.test(href);

/** Links go to the browser; the webview itself never navigates. */
const components: Components = {
  a: ({ href, children }) => (
    <a
      href={href}
      title={href}
      onClick={(e: MouseEvent) => {
        e.preventDefault();
        if (external(href)) void openUrl(href!);
      }}
    >
      {children}
    </a>
  ),
  img: ({ src, alt, title }) =>
    typeof src === "string" && external(src) ? (
      <img src={src} alt={alt ?? ""} title={title} loading="lazy" />
    ) : (
      <span className="text-fg-faint">[{alt || "image"}]</span>
    ),
};

/**
 * GitHub-flavoured Markdown from a hosting service (descriptions, comments).
 * Raw HTML is left out, never run; relative links resolve against
 * `base`, the page the text comes from.
 */
export default function MarkdownView({ text, base }: { text: string; base?: string }) {
  return (
    <div className="rn-markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={components}
        urlTransform={(url) => {
          const safe = defaultUrlTransform(url);
          if (!safe || !base || /^[a-z][a-z0-9+.-]*:/i.test(safe) || safe.startsWith("#"))
            return safe;
          try {
            return new URL(safe, base).href;
          } catch {
            return safe;
          }
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
