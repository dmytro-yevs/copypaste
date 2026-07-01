// StatusRow.tsx
// Extracted from SettingsView.tsx (CopyPaste-g06m.14 split) — cut/paste only.
export function StatusRow({ label, ok }: { label: string; ok: boolean }) {
  return (
    <div>
      <span>{label}</span>
      <span>{ok ? null : <span>—</span>}</span>
    </div>
  );
}
