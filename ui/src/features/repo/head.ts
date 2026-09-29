import type { HeadState } from "../../bindings/HeadState";

export function describeHead(head: HeadState): string {
  switch (head.kind) {
    case "branch":
      return head.unborn ? `${head.name} (no commits yet)` : head.name;
    case "detached":
      return `detached at ${head.oid.slice(0, 7)}`;
  }
}
