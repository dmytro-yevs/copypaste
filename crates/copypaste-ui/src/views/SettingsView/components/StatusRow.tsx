// StatusRow.tsx
// Extracted from SettingsView.tsx (CopyPaste-g06m.14 split) — cut/paste only.
export function StatusRow({ label, ok }: { label: string; ok: boolean }) {
  return (
    <span className="statusrow">
      <span className={ok ? "dot-stat" : "dot-stat off"} aria-hidden="true" />
      <span>{label}</span>
      <span>{ok ? null : <span>—</span>}</span>
    </span>
  );
}
