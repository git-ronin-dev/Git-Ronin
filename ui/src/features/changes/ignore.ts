import type { IgnoreScope } from "../../bindings/IgnoreScope";
import { splitPath } from "../commit/lines";

/** The "ignore" menu entries that make sense for a file. */
export function ignoreChoices(path: string): { label: string; scope: IgnoreScope }[] {
  const [dir, name] = splitPath(path);
  const dot = name.lastIndexOf(".");
  const choices: { label: string; scope: IgnoreScope }[] = [
    { label: "Add to .gitignore", scope: "file" },
  ];
  if (dot > 0 && dot < name.length - 1) {
    choices.push({ label: `Ignore all *${name.slice(dot)} files`, scope: "extension" });
  }
  if (dir) choices.push({ label: `Ignore folder ${dir}`, scope: "folder" });
  return choices;
}
