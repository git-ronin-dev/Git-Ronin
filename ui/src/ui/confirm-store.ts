import { create } from "zustand";

export interface ConfirmRequest {
  title: string;
  message: string;
  confirmLabel: string;
  /** Styles the confirm button as destructive. */
  danger?: boolean;
}

interface ConfirmState {
  request: (ConfirmRequest & { resolve: (ok: boolean) => void }) | null;
  settle: (ok: boolean) => void;
}

export const useConfirm = create<ConfirmState>()((set, get) => ({
  request: null,
  settle: (ok) => {
    get().request?.resolve(ok);
    set({ request: null });
  },
}));

/** Asks the user to confirm; resolves to false if they cancel. */
export function confirm(request: ConfirmRequest): Promise<boolean> {
  useConfirm.getState().request?.resolve(false);
  return new Promise((resolve) => useConfirm.setState({ request: { ...request, resolve } }));
}
