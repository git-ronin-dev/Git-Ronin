import { useQuery } from "@tanstack/react-query";

import { ipc } from "../../lib/ipc";
import { keys } from "../workspace/queries";

export function useCommitDetail(repo: string, oid: string | null) {
  return useQuery({
    queryKey: keys.commit(repo, oid ?? ""),
    queryFn: () => ipc.commitDetail(repo, oid!),
    enabled: oid !== null,
    staleTime: Infinity,
  });
}
