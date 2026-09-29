import { toast } from "../ui/toast-store";

export async function copyText(text: string, what = "Copied") {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(what, text.length > 80 ? undefined : text);
  } catch (err) {
    toast.error("Could not copy to clipboard", String(err));
  }
}
