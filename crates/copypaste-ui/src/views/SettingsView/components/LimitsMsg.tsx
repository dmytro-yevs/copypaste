// bdac.106: branch on .ok (typed signal) — no string comparison.
// Extracted from GeneralTab/SyncTab/StorageTab (CopyPaste-crh3.46 — three identical copies).
export function LimitsMsg({
  field,
  limitsMsg,
}: {
  field: string;
  limitsMsg: Record<string, { ok: boolean; message: string } | null>;
}) {
  const entry = limitsMsg[field];
  if (!entry) return null;
  return (
    <span className={`text-[11px] ${entry.ok ? "text-ide-success" : "text-ide-danger"}`}>
      {entry.message}
    </span>
  );
}
