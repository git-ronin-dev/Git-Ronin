/** Follows the OS light/dark preference. Replaced by a user setting in Phase 5. */
export function followSystemTheme(): () => void {
  const query = window.matchMedia("(prefers-color-scheme: light)");
  const apply = () => {
    document.documentElement.dataset.theme = query.matches ? "light" : "dark";
  };
  apply();
  query.addEventListener("change", apply);
  return () => query.removeEventListener("change", apply);
}
