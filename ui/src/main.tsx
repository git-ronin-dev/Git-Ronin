import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app/App";
import "./index.css";

const queryClient = new QueryClient({
  defaultOptions: {
    // Git failures are deterministic; retrying just delays the error. The
    // repository watcher drives refreshes instead of window focus.
    queries: { retry: false, refetchOnWindowFocus: false },
  },
});

// A desktop app has no use for the webview's Back/Reload/Inspect menu. Text
// fields keep theirs for copy and paste; dev builds keep it for the inspector.
if (import.meta.env.PROD) {
  document.addEventListener("contextmenu", (e) => {
    const target = e.target as HTMLElement;
    if (!target.closest("input, textarea, [contenteditable]")) e.preventDefault();
  });
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </StrictMode>,
);
