import type { Theme } from "../bindings/Theme";

/** Applies a theme; "system" follows the OS until the returned cleanup runs. */
export function applyTheme(theme: Theme): () => void {
  const root = document.documentElement;
  if (theme !== "system") {
    root.dataset.theme = theme;
    return () => {};
  }
  const query = window.matchMedia("(prefers-color-scheme: light)");
  const apply = () => {
    root.dataset.theme = query.matches ? "light" : "dark";
  };
  apply();
  query.addEventListener("change", apply);
  return () => query.removeEventListener("change", apply);
}
