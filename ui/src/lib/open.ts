import { toast } from "../ui/toast-store";
import { ipc } from "./ipc";

/** Opens a web page in the default browser. */
export async function openUrl(url: string) {
  try {
    await ipc.openUrl(url);
  } catch (err) {
    toast.error("Could not open the browser", String(err));
  }
}
