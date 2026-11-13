import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../lib/ipc";
import type { ApiError, Status } from "../lib/types";
import { useUi } from "./store";

export const keys = {
  status: ["status"] as const,
  snapshot: ["snapshot"] as const,
  item: (id: string) => ["item", id] as const,
  security: ["security"] as const,
  health: ["health"] as const,
  backups: ["backups"] as const,
};

export function useStatus() {
  return useQuery({ queryKey: keys.status, queryFn: api.status, staleTime: Infinity });
}

export function useSnapshot(enabled = true) {
  return useQuery({ queryKey: keys.snapshot, queryFn: api.snapshot, enabled, staleTime: Infinity });
}

export function useItem(id: string | null) {
  return useQuery({ queryKey: keys.item(id ?? ""), queryFn: () => api.itemGet(id!), enabled: !!id, staleTime: Infinity });
}

export function useApplyStatus() {
  const qc = useQueryClient();
  return (s: Status) => {
    qc.setQueryData(keys.status, s);
    if (s.stage !== "unlocked") {
      qc.removeQueries({ predicate: (q) => q.queryKey[0] !== "status" });
    }
  };
}

export function useVaultMutation<TArgs, TResult>(
  fn: (args: TArgs) => Promise<TResult>,
  opts: { success?: string | ((r: TResult) => string); invalidate?: readonly (readonly string[])[] } = {},
) {
  const qc = useQueryClient();
  const toast = useUi((s) => s.toast);
  return useMutation<TResult, ApiError, TArgs>({
    mutationFn: fn,
    onSuccess: (r) => {
      for (const k of opts.invalidate ?? [keys.snapshot, ["item"]]) qc.invalidateQueries({ queryKey: k });
      if (opts.success) toast(typeof opts.success === "function" ? opts.success(r) : opts.success, "ok");
    },
    onError: (e) => toast(e.message, "error"),
  });
}
