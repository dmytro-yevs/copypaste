import { SyncStatusChip } from "../../../components/SyncStatusChip";

// Standalone SyncStatusChip example (also embedded inside SidebarSection's
// real <Sidebar/> footer — this section gives it its own deep-linkable id).
export function SyncStatusSection() {
  return (
    <section id="gallery-sync-status">
      <h2>Sync status chip</h2>
      <div className="gallery__row">
        <SyncStatusChip />
      </div>
    </section>
  );
}
