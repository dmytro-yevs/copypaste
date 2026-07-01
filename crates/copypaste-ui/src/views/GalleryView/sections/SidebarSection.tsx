import { Sidebar } from "../../../components/Sidebar";

// Task 6.7: "sidebar". Renders the REAL Sidebar component (including
// SyncStatusChip in its footer) inside a fixed-height frame so `.sb__foot`'s
// margin-top:auto has room to push the footer down, same as it does under
// `.app`'s flex layout in production. Clicking a NAV item here updates the
// store's `view` field, but that has no visible effect while the gallery
// branch is active — App.tsx's `galleryActive()` reads the URL, not the
// store — so this is safe to click without leaving the gallery.
export function SidebarSection() {
  return (
    <section id="gallery-sidebar">
      <h2>Sidebar</h2>
      <div className="gallery__sidebar-frame">
        <Sidebar />
      </div>
    </section>
  );
}
