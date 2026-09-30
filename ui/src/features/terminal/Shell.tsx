import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Terminal, type ITheme } from "@xterm/xterm";
import { clsx } from "clsx";
import { useEffect, useRef } from "react";

import { copyText } from "../../lib/clipboard";
import { ipc } from "../../lib/ipc";
import { eventToShortcut, reachesAppFromTerminal } from "../commands/keys";

export interface ShellHandle {
  find: (text: string, backwards: boolean) => void;
  clearSearch: () => void;
  focus: () => void;
}

function tokens() {
  const css = getComputedStyle(document.documentElement);
  return (name: string) => css.getPropertyValue(`--rn-${name}`).trim();
}

/** Terminal colours from the design tokens, so it follows the theme. */
function theme(): ITheme {
  const v = tokens();
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

/** One shell in the repository folder. */
export function Shell({
  repo,
  fontSize,
  visible,
  onReady,
  onSearch,
  onExit,
}: {
  repo: string;
  fontSize: number;
  visible: boolean;
  onReady: (handle: ShellHandle | null) => void;
  /** Ctrl+Shift+F in the terminal. */
  onSearch: () => void;
  onExit: (code: number | null) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const terminal = useRef<{ term: Terminal; fit: () => void } | null>(null);
  // The latest callbacks, without restarting the shell when they change.
  const callbacks = useRef({ onReady, onSearch, onExit });
  useEffect(() => {
    callbacks.current = { onReady, onSearch, onExit };
  });

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const css = getComputedStyle(document.documentElement);
    const term = new Terminal({
      fontFamily: css.getPropertyValue("--rn-font-mono").trim() || "monospace",
      fontSize,
      cursorBlink: true,
      scrollback: 5000,
      allowProposedApi: true,
      theme: theme(),
    });
    const fitAddon = new FitAddon();
    const search = new SearchAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(search);
    term.open(el);
    const fit = () => {
      if (el.clientWidth > 0 && el.clientHeight > 0) fitAddon.fit();
    };
    terminal.current = { term, fit };
    fit();

    // Shell keys (Ctrl+W, Ctrl+R, …) stay with the shell; the app only
    // gets the shortcuts meant for it.
    term.attachCustomKeyEventHandler((ev) => {
      if (ev.type !== "keydown") return true;
      const shortcut = eventToShortcut(ev);
      if (shortcut === "Mod+Shift+F") {
        ev.preventDefault();
        ev.stopPropagation();
        callbacks.current.onSearch();
        return false;
      }
      if (shortcut === "Mod+Shift+C") {
        ev.preventDefault();
        const selection = term.getSelection();
        if (selection) void copyText(selection, "Copied");
        return false;
      }
      return !(shortcut && reachesAppFromTerminal(shortcut));
    });

    const v = tokens();
    const decorations = {
      matchBackground: v("hover"),
      matchOverviewRuler: v("warning"),
      activeMatchBackground: v("accent"),
      activeMatchColorOverviewRuler: v("accent"),
    };
    callbacks.current.onReady({
      find: (text, backwards) =>
        void (backwards
          ? search.findPrevious(text, { decorations })
          : search.findNext(text, { decorations })),
      clearSearch: () => search.clearDecorations(),
      focus: () => term.focus(),
    });

    let id: number | null = null;
    let closed = false;
    ipc
      .terminalOpen(repo, term.cols, term.rows, (event) => {
        if (event.kind === "data") term.write(event.data);
        else callbacks.current.onExit(event.code);
      })
      .then((opened) => {
        // Unmounted while starting: don't leave the shell behind.
        if (closed) void ipc.terminalClose(opened);
        else id = opened;
      })
      .catch((err: unknown) => {
        term.write(`\r\n\x1b[31m${String(err)}\x1b[0m\r\n`);
        callbacks.current.onExit(null);
      });

    const input = term.onData((data) => id !== null && void ipc.terminalWrite(id, data));
    const resize = term.onResize(
      ({ cols, rows }) => id !== null && void ipc.terminalResize(id, cols, rows).catch(() => {}),
    );
    const observer = new ResizeObserver(fit);
    observer.observe(el);
    const themeWatch = new MutationObserver(() => (term.options.theme = theme()));
    themeWatch.observe(document.documentElement, { attributeFilter: ["data-theme"] });

    return () => {
      closed = true;
      callbacks.current.onReady(null);
      observer.disconnect();
      themeWatch.disconnect();
      input.dispose();
      resize.dispose();
      if (id !== null) void ipc.terminalClose(id);
      term.dispose();
      terminal.current = null;
    };
    // The font size is applied below without restarting the shell.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repo]);

  useEffect(() => {
    const t = terminal.current;
    if (!t || t.term.options.fontSize === fontSize) return;
    t.term.options.fontSize = fontSize;
    t.fit();
  }, [fontSize]);

  useEffect(() => {
    if (!visible) return;
    terminal.current?.fit();
    terminal.current?.term.focus();
  }, [visible]);

  return (
    <div
      ref={host}
      data-terminal
      className={clsx("absolute inset-0 overflow-hidden pl-2", !visible && "invisible")}
    />
  );
}
