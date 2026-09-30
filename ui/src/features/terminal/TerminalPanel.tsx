import "@xterm/xterm/css/xterm.css";

import { FitAddon } from "@xterm/addon-fit";
import { Terminal, type ITheme } from "@xterm/xterm";
import { RotateCcw, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { ipc } from "../../lib/ipc";
import { Tooltip } from "../../ui/Tooltip";
import { updateView } from "../workspace/view";

/** Terminal colours from the design tokens, so it follows the theme. */
function theme(): ITheme {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string) => css.getPropertyValue(`--rn-${name}`).trim();
  return {
    background: v("surface"),
    foreground: v("fg"),
    cursor: v("fg"),
    selectionBackground: v("hover"),
    black: v("raised"),
    brightBlack: v("fg-faint"),
    red: v("danger"),
    brightRed: v("danger"),
    green: v("success"),
    brightGreen: v("success"),
    yellow: v("warning"),
    brightYellow: v("warning"),
    blue: v("accent"),
    brightBlue: v("accent"),
    magenta: v("lane-3"),
    brightMagenta: v("lane-3"),
    cyan: v("lane-4"),
    brightCyan: v("lane-4"),
    white: v("fg-muted"),
    brightWhite: v("fg"),
  };
}

/**
 * A shell in the repository folder. It keeps running while the panel is
 * hidden and ends when the tab closes.
 */
export function TerminalPanel({ repo, visible }: { repo: string; visible: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | null>(null);
  // Bumped to start a new shell after the old one exited.
  const [session, setSession] = useState(0);
  const [exited, setExited] = useState<number | null | undefined>(undefined);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const css = getComputedStyle(document.documentElement);
    const term = new Terminal({
      fontFamily: css.getPropertyValue("--rn-font-mono").trim() || "monospace",
      fontSize: 13,
      cursorBlink: true,
      scrollback: 5000,
      theme: theme(),
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    terminal.current = term;
    const refit = () => {
      if (el.clientWidth > 0 && el.clientHeight > 0) fit.fit();
    };
    refit();

    let id: number | null = null;
    let closed = false;
    setExited(undefined);
    ipc
      .terminalOpen(repo, term.cols, term.rows, (event) => {
        if (event.kind === "data") term.write(event.data);
        else setExited(event.code);
      })
      .then((opened) => {
        // Unmounted while starting: don't leave the shell behind.
        if (closed) void ipc.terminalClose(opened);
        else id = opened;
      })
      .catch((err: unknown) => {
        term.write(`\r\n\x1b[31m${String(err)}\x1b[0m\r\n`);
        setExited(null);
      });

    const input = term.onData((data) => id !== null && void ipc.terminalWrite(id, data));
    const resize = term.onResize(
      ({ cols, rows }) => id !== null && void ipc.terminalResize(id, cols, rows).catch(() => {}),
    );
    const observer = new ResizeObserver(refit);
    observer.observe(el);
    // Follow theme switches.
    const themeWatch = new MutationObserver(() => (term.options.theme = theme()));
    themeWatch.observe(document.documentElement, { attributeFilter: ["data-theme"] });

    return () => {
      closed = true;
      observer.disconnect();
      themeWatch.disconnect();
      input.dispose();
      resize.dispose();
      if (id !== null) void ipc.terminalClose(id);
      term.dispose();
      terminal.current = null;
    };
  }, [repo, session]);

  useEffect(() => {
    if (visible) terminal.current?.focus();
  }, [visible, session]);

  return (
    <section aria-label="Terminal" className="flex h-full flex-col bg-surface">
      <header className="flex h-7 shrink-0 items-center gap-2 border-b border-line px-2 text-xs text-fg-muted">
        <span className="font-semibold tracking-wide uppercase">Terminal</span>
        {exited !== undefined && (
          <span role="status" className="text-fg-faint">
            {exited === null ? "stopped" : `exited with code ${exited}`}
          </span>
        )}
        <div className="ml-auto flex gap-1">
          <Tooltip content="New shell">
            <button
              type="button"
              aria-label="New shell"
              onClick={() => setSession((s) => s + 1)}
              className="rounded-sm p-0.5 hover:bg-hover hover:text-fg"
            >
              <RotateCcw className="size-3.5" />
            </button>
          </Tooltip>
          <Tooltip content="Hide terminal">
            <button
              type="button"
              aria-label="Hide terminal"
              onClick={() => updateView(repo, { terminal: false })}
              className="rounded-sm p-0.5 hover:bg-hover hover:text-fg"
            >
              <X className="size-3.5" />
            </button>
          </Tooltip>
        </div>
      </header>
      <div ref={host} className="min-h-0 flex-1 overflow-hidden pl-2" />
    </section>
  );
}
