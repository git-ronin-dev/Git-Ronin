import { openUrl as open } from "@tauri-apps/plugin-opener";

import { toast } from "../ui/toast-store";

/** Opens a web page in the default browser. */
export async function openUrl(url: string) {
  try {
    await open(url);
  } catch (err) {
    toast.error("Could not open the browser", String(err));
  }
}
