import { useState } from "react";
import { ActionButton } from "../../components/ActionButton";
import { Toggle } from "../../components/Toggle";
import { ConfirmModal } from "../../components/ConfirmModal";
import { DeviceBadge } from "../../components/DeviceBadge";
import { FIXTURE_OWN_DEVICE_ID } from "../../lib/fixtures";
import {
  ACCENT_VALUES,
  THEME_VALUES,
  type AccentValue,
  type ThemeValue,
} from "../../lib/theme/prefsSchema";
import { IconButtonsSection } from "./sections/IconButtonsSection";
import { SegmentedSection } from "./sections/SegmentedSection";
import { TilesSection } from "./sections/TilesSection";
import { ForcedStateSection } from "./sections/ForcedStateSection";
import { HistoryRowsSection } from "./sections/HistoryRowsSection";
import { DeviceRowsSection } from "./sections/DeviceRowsSection";
import { BannersSection } from "./sections/BannersSection";
import { EmptyStatesSection } from "./sections/EmptyStatesSection";
import { SidebarSection } from "./sections/SidebarSection";
import { SyncStatusSection } from "./sections/SyncStatusSection";
import { SettingsSection } from "./sections/SettingsSection";
import { PopupSection } from "./sections/PopupSection";
import { FileChipSection } from "./sections/FileChipSection";
import { MatrixSection } from "./sections/MatrixSection";
import { LongTextSection } from "./sections/LongTextSection";
import "../../styles/gallery.css";

// ---------------------------------------------------------------------------
// GalleryView — DEV+MOCK-only component gallery (design.md Decision 6/7).
//
// This module is dynamic-imported behind an `import.meta.env.DEV` gate in
// App.tsx, so it (and gallery.css) is tree-shaken out of production builds and
// lives in its own chunk. It renders the REAL shared components against local
// state, inside a scoped `.theme-scope[data-theme][data-accent][data-translucency]`
// wrapper — NEVER mutating <html> or the user's persisted prefs (task 6.9).
// Slice 2 seeds the primitive stories; later slices ADD their sections here.
// ---------------------------------------------------------------------------

// CopyPaste-8ebg.64: GalleryView is entered from Sidebar.tsx's
// navigateToGallery() via a hard `window.location.href` navigation that sets
// `?view=gallery` — App.tsx reads that URL param fresh on every render
// (galleryActive(), not store state, per design.md Decision 6), so there was
// no in-app way back short of manually editing the URL or closing the
// window. Sidebar.tsx (the entry point) is owned by another cluster/out of
// scope here, so this mirrors its navigation approach in reverse, entirely
// within GalleryView: drop `view` from the URL and do the same kind of full
// navigation, landing back on the default view (history).
function navigateBack(): void {
  const url = new URL(window.location.href);
  url.searchParams.delete("view");
  window.location.href = url.toString();
}

export function GalleryView() {
  const [theme, setTheme] = useState<ThemeValue>("dark");
  const [accent, setAccent] = useState<AccentValue>("indigo");
  const [translucency, setTranslucency] = useState(true);
  const [toggleOn, setToggleOn] = useState(true);
  const [confirmOpen, setConfirmOpen] = useState(false);

  return (
    <div
      className="gallery theme-scope"
      data-theme={theme}
      data-accent={accent}
      data-translucency={translucency ? "on" : "off"}
    >
      <div
        className="gallery__title"
        style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}
      >
        <span>Component gallery (DEV)</span>
        {/* CopyPaste-8ebg.64: back action — see navigateBack() above. */}
        <ActionButton variant="ghost" size="sm" onClick={navigateBack}>
          ← Back
        </ActionButton>
      </div>

      {/* Local theme/accent/translucency switcher — component state only, never
          setPrefs, so leaving the gallery leaves the real prefs untouched. */}
      <div className="gallery__switcher">
        <div className="seg" role="group" aria-label="Theme">
          {THEME_VALUES.map((t) => (
            <button
              key={t}
              type="button"
              className={t === theme ? "on" : undefined}
              onClick={() => setTheme(t)}
            >
              {t}
            </button>
          ))}
        </div>
        <div className="gallery__swatches" role="group" aria-label="Accent">
          {ACCENT_VALUES.map((a) => (
            // Each swatch is its own nested theme-scope so var(--accent) resolves
            // to that swatch's accent value (theme-aware) — not the wrapper's.
            <span key={a} className="theme-scope" data-theme={theme} data-accent={a}>
              <button
                type="button"
                className="gallery__swatch"
                aria-label={a}
                aria-pressed={a === accent}
                style={{ background: "var(--accent)" }}
                onClick={() => setAccent(a)}
              />
            </span>
          ))}
        </div>
        <label className="gallery__row">
          <Toggle
            checked={translucency}
            onChange={setTranslucency}
            aria-label="Translucency"
          />
          Translucency
        </label>
      </div>

      <section id="gallery-buttons">
        <h2>Buttons</h2>
        <div className="gallery__row">
          <ActionButton variant="primary">Primary</ActionButton>
          <ActionButton variant="secondary">Secondary</ActionButton>
          <ActionButton variant="ghost">Ghost</ActionButton>
          <ActionButton variant="danger">Danger</ActionButton>
          <ActionButton variant="danger-solid">Danger solid</ActionButton>
          <ActionButton variant="secondary" size="sm">
            Small
          </ActionButton>
          <ActionButton variant="secondary" disabled>
            Disabled
          </ActionButton>
          <ActionButton variant="primary" pending pendingLabel="Working…">
            Pending
          </ActionButton>
        </div>
      </section>

      <section id="gallery-toggle">
        <h2>Toggle</h2>
        <div className="gallery__row">
          <Toggle checked={toggleOn} onChange={setToggleOn} aria-label="Demo toggle" />
          <Toggle checked={false} onChange={() => {}} aria-label="Off toggle" />
        </div>
      </section>

      <section id="gallery-field">
        <h2>Field</h2>
        <div className="field" style={{ maxWidth: "var(--modal-w)" }}>
          <input placeholder="Search clipboard…" aria-label="Search demo" />
        </div>
      </section>

      <section id="gallery-chips">
        <h2>Chips · badges · pills</h2>
        <div className="gallery__row">
          <span className="chip">Chip</span>
          <span className="chip on">Chip (on)</span>
          <span className="tpill tpill--p2p">P2P</span>
          <span className="tpill tpill--cloud">Cloud</span>
          <span className="tpill tpill--this">This</span>
          <span className="badge badge--verified">
            <span className="d" />
            Verified
          </span>
          <span className="badge badge--count">12</span>
          <span className="spinner" aria-label="Loading" />
          <span className="kbd">⌘</span>
          <span className="kbd">1</span>
          {/* DeviceBadge — own vs. remote origin-device chip (not currently
              mounted anywhere in production; ClipMetadata uses its exported
              deviceLabel() helper directly, so the gallery is this
              component's only live example). */}
          <DeviceBadge originId={FIXTURE_OWN_DEVICE_ID} ownId={FIXTURE_OWN_DEVICE_ID} />
          <DeviceBadge originId="ccddeeff-bbbb-cccc-dddd-eeff00112233" ownId={FIXTURE_OWN_DEVICE_ID} originName="iPhone 16 Pro" />
        </div>
      </section>

      <IconButtonsSection />
      <SegmentedSection />
      <TilesSection />
      <ForcedStateSection />

      <section id="gallery-dialog">
        <h2>Dialog</h2>
        <div className="gallery__row">
          <ActionButton variant="primary" onClick={() => setConfirmOpen(true)}>
            Open confirm dialog
          </ActionButton>
        </div>
        <ConfirmModal
          open={confirmOpen}
          title="Delete everything?"
          body="This is a gallery demo of the shared Dialog primitive."
          confirmLabel="Delete"
          onConfirm={() => setConfirmOpen(false)}
          onCancel={() => setConfirmOpen(false)}
        />
      </section>

      <HistoryRowsSection />
      <DeviceRowsSection />
      <FileChipSection />
      <BannersSection />
      <EmptyStatesSection />
      <SidebarSection />
      <SyncStatusSection />
      <SettingsSection />
      <PopupSection />
      <MatrixSection />
      <LongTextSection />
    </div>
  );
}

export default GalleryView;
