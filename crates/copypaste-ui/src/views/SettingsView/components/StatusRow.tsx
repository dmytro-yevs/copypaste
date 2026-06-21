// StatusRow.tsx
// Extracted from SettingsView.tsx (CopyPaste-g06m.14 split) — cut/paste only.
import { Check } from "lucide-react";

export function StatusRow({ label, ok }: { label: string; ok: boolean }) {
  return (
    <div className="flex items-center gap-2 text-[13px] text-ide-dim">
      <span className="w-[140px] shrink-0">{label}</span>
      {/* §6.6: replaced ✓/— text chars with Lucide icons (size 14, semantic tint) */}
      <span className={ok ? "text-ide-success" : "text-ide-faint"}>
        {ok ? <Check size={14} /> : <span className="text-[13px]">—</span>}
      </span>
    </div>
  );
}
