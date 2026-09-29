import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app/App";
import { followSystemTheme } from "./app/theme";
import "./index.css";

const queryClient = new QueryClient({
  defaultOptions: {
    // Git failures are deterministic; retrying just delays the error. The fs
    // watcher (Phase 1) drives refreshes instead of window focus.
    queries: { retry: false, refetchOnWindowFocus: false },
  },
});

followSystemTheme();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </StrictMode>,
);
