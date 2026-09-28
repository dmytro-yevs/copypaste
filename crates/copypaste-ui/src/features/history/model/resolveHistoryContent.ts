import type { ErrorKind } from "@/lib/errors";

export type HistoryContentKind =
  | "list" | "loading" | "key_unusable" | "key_locked" | "offline"
  | "not_ready" | "error" | "private" | "filtered" | "empty";

/** A failed poll or refresh never removes rows already available to read. */
export function resolveHistoryContent({ loading, hasItems, errorKind, privateMode, filtered }: {
  loading: boolean;
  hasItems: boolean;
  errorKind: ErrorKind | null;
  privateMode: boolean;
  filtered: boolean;
}): HistoryContentKind {
  if (hasItems) return "list";
  if (loading) return "loading";
  if (errorKind === "key_unusable" || errorKind === "key_locked" || errorKind === "offline" || errorKind === "not_ready") return errorKind;
  if (errorKind !== null) return "error";
  if (privateMode) return "private";
  if (filtered) return "filtered";
  return "empty";
}
