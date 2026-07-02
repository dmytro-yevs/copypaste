import { CheckCircle2, XCircle } from "lucide-react";

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
    <div
      role={entry.ok ? "status" : "alert"}
      className={entry.ok ? "banner banner--ok" : "banner banner--err"}
    >
      {entry.ok ? <CheckCircle2 aria-hidden="true" /> : <XCircle aria-hidden="true" />}
      <span className="banner__x">
        {entry.message}
      </span>
    </div>
  );
}
