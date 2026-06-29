// StorageTab.tsx
// Extracted from SettingsView.tsx renderStorage() (CopyPaste-g06m.14 split) — cut/paste only.
import { SectionHeader } from "../../../components/SectionHeader";
import { SettingsRow } from "../../../components/SettingsRow";
import { Panel } from "../../../components/Panel";
import { SliderRow } from "../../../components/SliderRow";
import {
  TEXT_SIZE_STEPS_BYTES, TEXT_SIZE_LABELS,
  IMAGE_SIZE_STEPS_BYTES, IMAGE_SIZE_LABELS,
  FILE_SIZE_STEPS_BYTES, FILE_SIZE_LABELS,
  QUOTA_STEPS_BYTES, QUOTA_LABELS,
  SENSITIVE_TTL_STEPS, SENSITIVE_TTL_LABELS,
  MAX_ITEMS_STEPS, MAX_ITEMS_LABELS,
  snapToNearest,
  DEFAULT_MAX_ITEMS,
} from "../lib/settingsSliders";
import type { UIPrefs } from "../../../store";
import { LimitsMsg } from "../components/LimitsMsg";

export type StorageTabProps = {
  offline: boolean;
  prefs: UIPrefs;
  setPrefs: (p: Partial<UIPrefs>) => void;
  maxTextBytes: number;
  setMaxTextBytes: (v: number) => void;
  maxImageBytes: number;
  setMaxImageBytes: (v: number) => void;
  maxFileBytes: number;
  setMaxFileBytes: (v: number) => void;
  quotaBytes: number;
  setQuotaBytes: (v: number) => void;
  sensitiveTtlSecs: number;
  setSensitiveTtlSecs: (v: number) => void;
  imageQuality: number;
  setImageQuality: (v: number) => void;
  exportInProgress: boolean;
  exportMsg: { text: string; isError: boolean } | null;
  exportIncludeSensitive: boolean;
  setExportIncludeSensitive: (v: boolean) => void;
  importInProgress: boolean;
  importMsg: { text: string; isError: boolean } | null;
  dbStats: { item_count: number; size_bytes: number } | null;
  vacuumBusy: boolean;
  vacuumMsg: { text: string; isError: boolean } | null;
  deleteMsg: { text: string; isError: boolean } | null;
  limitsMsg: Record<string, { ok: boolean; message: string } | null>;
  btnCls: string;
  btnStyle: React.CSSProperties;
  saveLimitsField: (field: string, patch: Record<string, unknown>, onRevert?: () => void) => Promise<void>;
  // bdac.106: ok param distinguishes success from error without string matching.
  showLimitsMsg: (field: string, msg: string | null, durationMs: number, ok?: boolean) => void;
  handleExport: () => void;
  handleImportFile: (e: React.ChangeEvent<HTMLInputElement>) => void;
  handleVacuum: () => void;
  setDeleteConfirm: (v: boolean) => void;
};

export function StorageTab({
  offline,
  prefs,
  setPrefs,
  maxTextBytes,
  setMaxTextBytes,
  maxImageBytes,
  setMaxImageBytes,
  maxFileBytes,
  setMaxFileBytes,
  quotaBytes,
  setQuotaBytes,
  sensitiveTtlSecs,
  setSensitiveTtlSecs,
  imageQuality,
  setImageQuality,
  exportInProgress,
  exportMsg,
  exportIncludeSensitive,
  setExportIncludeSensitive,
  importInProgress,
  importMsg,
  dbStats,
  vacuumBusy,
  vacuumMsg,
  deleteMsg,
  limitsMsg,
  btnCls,
  btnStyle,
  saveLimitsField,
  showLimitsMsg,
  handleExport,
  handleImportFile,
  handleVacuum,
  setDeleteConfirm,
}: StorageTabProps) {
  // Helper: render a stepped slider row with inline feedback badge.
  // M9: LimitSliderRow now uses the unified SliderRow (index-based 0…steps.length-1).
  // onRelease fires only on mouse-up/touch-end to avoid hammering the IPC on drag.
  function LimitSliderRow<T extends number>({
    label,
    description,
    field,
    steps,
    labels,
    value,
    onChange,
    onRelease,
  }: {
    label: string;
    description?: string;
    field: string;
    steps: readonly T[];
    labels: readonly string[];
    value: T;
    onChange: (v: T) => void;
    onRelease: (v: T) => void;
  }) {
    const maxIdx = steps.length - 1;
    const idx = steps.indexOf(value);
    const safeIdx = idx < 0 ? 0 : idx;
    return (
      <SettingsRow title={label} description={description}>
        <div className="flex items-center gap-2">
          <SliderRow
            min={0}
            max={maxIdx}
            step={1}
            value={safeIdx}
            disabled={offline}
            // §6.5: pass step count so SliderRow renders a datalist for tick marks
            tickStepCount={steps.length}
            onChange={(i) => onChange(steps[Math.min(Math.max(i, 0), maxIdx)] as T)}
            onRelease={(i) => onRelease(steps[Math.min(Math.max(i, 0), maxIdx)] as T)}
            formatValue={(i) => labels[Math.min(Math.max(i, 0), maxIdx)] ?? String(i)}
          />
          <LimitsMsg field={field} limitsMsg={limitsMsg} />
        </div>
      </SettingsRow>
    );
  }

  const maxItems = snapToNearest(
    MAX_ITEMS_STEPS as unknown as readonly number[],
    prefs.historyDisplayLimit ?? DEFAULT_MAX_ITEMS
  );

  return (
    <div className="space-y-2">
      <Panel>
        <LimitSliderRow
          label="Max clip text size"
          field="max_text_size_bytes"
          steps={TEXT_SIZE_STEPS_BYTES as unknown as readonly number[]}
          labels={TEXT_SIZE_LABELS}
          value={maxTextBytes}
          onChange={(v) => setMaxTextBytes(v)}
          onRelease={(v) => {
            // P1 fix: capture prev before optimistic update (onChange already fired);
            // revert only this field on error, not the full reload.
            const prev = maxTextBytes;
            setMaxTextBytes(v);
            void saveLimitsField("max_text_size_bytes", { max_text_size_bytes: v }, () => setMaxTextBytes(prev));
          }}
        />
        <LimitSliderRow
          label="Max clip image size"
          field="max_image_size_bytes"
          steps={IMAGE_SIZE_STEPS_BYTES as unknown as readonly number[]}
          labels={IMAGE_SIZE_LABELS}
          value={maxImageBytes}
          onChange={(v) => setMaxImageBytes(v)}
          onRelease={(v) => {
            const prev = maxImageBytes;
            setMaxImageBytes(v);
            void saveLimitsField("max_image_size_bytes", { max_image_size_bytes: v }, () => setMaxImageBytes(prev));
          }}
        />
        <LimitSliderRow
          label="Max clip file size"
          field="max_file_size_bytes"
          steps={FILE_SIZE_STEPS_BYTES as unknown as readonly number[]}
          labels={FILE_SIZE_LABELS}
          value={maxFileBytes}
          onChange={(v) => setMaxFileBytes(v)}
          onRelease={(v) => {
            const prev = maxFileBytes;
            setMaxFileBytes(v);
            void saveLimitsField("max_file_size_bytes", { max_file_size_bytes: v }, () => setMaxFileBytes(prev));
          }}
        />
        <div className="border-b border-ide-divider/70 px-3 pb-2 text-[11px] text-ide-faint">
          Files over ~8&nbsp;MB are kept locally but won&apos;t sync over P2P/cloud — they&apos;re skipped with a warning.
        </div>
        <LimitSliderRow
          label="Local storage limit"
          field="storage_quota_bytes"
          steps={QUOTA_STEPS_BYTES as unknown as readonly number[]}
          labels={QUOTA_LABELS}
          value={quotaBytes}
          onChange={(v) => setQuotaBytes(v)}
          onRelease={(v) => {
            const prev = quotaBytes;
            setQuotaBytes(v);
            void saveLimitsField("storage_quota_bytes", { storage_quota_bytes: v }, () => setQuotaBytes(prev));
          }}
        />
        <LimitSliderRow
          label="Sensitive auto-wipe"
          field="sensitive_ttl_secs"
          steps={SENSITIVE_TTL_STEPS as unknown as readonly number[]}
          labels={SENSITIVE_TTL_LABELS}
          value={sensitiveTtlSecs}
          onChange={(v) => setSensitiveTtlSecs(v)}
          onRelease={(v) => {
            const prev = sensitiveTtlSecs;
            setSensitiveTtlSecs(v);
            void saveLimitsField("sensitive_ttl_secs", { sensitive_ttl_secs: v }, () => setSensitiveTtlSecs(prev));
          }}
        />
        {/* ctmv: image_quality is persisted to daemon config via set_config on release.
            The capture encode path reads this value: quality < 100 → JPEG, 100 → PNG lossless.
            bdac.68: removed "(1–100)" from label — range shown by the slider control itself */}
        <SettingsRow
          title="Image quality"
          description="Saved to daemon. Values below 100 use JPEG encoding (smaller files); 100 uses lossless PNG."
        >
          <div className="flex items-center gap-2">
            <SliderRow
              min={1}
              max={100}
              step={1}
              value={imageQuality}
              onChange={(v) => setImageQuality(v)}
              onRelease={(v) => {
                // Autosave on commit (mouse-up / touch-end / key-up), matching the
                // neighbouring limit sliders — no dedicated Save button.
                const prev = imageQuality;
                void saveLimitsField("image_quality", { image_quality: v }, () => setImageQuality(prev));
              }}
              formatValue={(v) => String(v)}
            />
            <LimitsMsg field="image_quality" limitsMsg={limitsMsg} />
          </div>
        </SettingsRow>
        {/* bdac.88: History display limit — UI-only display filter (localStorage / UIPrefs).
            No daemon IPC: the daemon stores items until the byte quota is reached.
            This slider filters how many items the UI renders — it does NOT delete items.
            Sentinel 100000 → "Unlimited". */}
        <LimitSliderRow
          label="History display limit"
          description="Display filter only — does not delete stored items. Daemon stores until byte quota."
          field="max_items"
          steps={MAX_ITEMS_STEPS as unknown as readonly number[]}
          labels={MAX_ITEMS_LABELS}
          value={maxItems}
          onChange={(v) => {
            // Persist live (on every drag tick) so the HistoryView cap updates in real time.
            setPrefs({ historyDisplayLimit: v });
          }}
          onRelease={(v) => {
            // Persist on commit (mouse-up / key-up) and show inline feedback.
            setPrefs({ historyDisplayLimit: v });
            showLimitsMsg("max_items", "Saved", 1500, true);
          }}
        />
        <div className="border-b border-ide-divider/70 px-3 pb-2 text-[11px] text-ide-faint">
          Limits items shown in the UI only — the daemon stores more and prunes by the byte quota above.
        </div>
      </Panel>

      {/* 85n9: Backup / Restore panel */}
      <SectionHeader
        label="Backup & restore"
        hint="Export your clipboard history as a JSON file, or restore it from a previous backup."
      />
      <Panel>
        {/* Export row */}
        <div className="border-b border-ide-divider/70 px-3 py-2 last:border-b-0">
          <div className="flex items-center justify-between gap-3">
            <span className="min-w-[160px] shrink-0 text-[13px] text-ide-text">Export backup</span>
            <div className="flex flex-col items-end gap-1.5">
              {/* Include-sensitive checkbox with plaintext warning */}
              <label className="flex cursor-pointer items-center gap-1.5 text-[12px] text-ide-dim select-none">
                <input
                  type="checkbox"
                  checked={exportIncludeSensitive}
                  onChange={(e) => setExportIncludeSensitive(e.target.checked)}
                  disabled={offline || exportInProgress}
                  className="h-3.5 w-3.5 accent-ide-accent disabled:cursor-not-allowed disabled:opacity-40"
                />
                Include sensitive items
              </label>
              {exportIncludeSensitive && (
                <span className="text-[11px] text-ide-warning">
                  Warning: sensitive items will be exported as plaintext. Keep the file secure and delete it when done.
                </span>
              )}
              <div className="flex items-center gap-2">
                {exportMsg !== null && (
                  <span className={`text-[12px] ${exportMsg.isError ? "text-ide-danger" : "text-ide-success"}`}>
                    {exportMsg.text}
                  </span>
                )}
                <button
                  type="button"
                  disabled={offline || exportInProgress}
                  onClick={() => void handleExport()}
                  data-testid="export-button"
                  className={[
                    "border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
                    "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
                  ].join(" ")}
                  style={{ borderRadius: "var(--r-ctl)" }}
                >
                  {exportInProgress ? "Exporting…" : "Export…"}
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* Import row — bdac.73: renamed "Restore backup" → "Import history" for parity with Android */}
        <div className="px-3 py-2">
          <div className="flex items-center justify-between gap-3">
            <span className="min-w-[160px] shrink-0 text-[13px] text-ide-text">Import history</span>
            <div className="flex flex-col items-end gap-1">
              {importMsg !== null && (
                <span className={`text-[12px] ${importMsg.isError ? "text-ide-danger" : "text-ide-success"}`}>
                  {importMsg.text}
                </span>
              )}
              {/* Invisible file input driven by the visible button below.
                  accept="application/json" limits the picker to .json files.
                  The file is read entirely in-browser via FileReader (no fs
                  Tauri plugin needed). */}
              <label
                className={[
                  "cursor-pointer border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-text",
                  "hover:bg-ide-hover",
                  (offline || importInProgress) ? "pointer-events-none opacity-40" : "",
                ].join(" ")}
                style={{ borderRadius: "var(--r-ctl)" }}
              >
                {importInProgress ? "Importing…" : "Import…"}
                <input
                  type="file"
                  accept="application/json"
                  disabled={offline || importInProgress}
                  onChange={(e) => void handleImportFile(e)}
                  data-testid="import-file-input"
                  className="sr-only"
                />
              </label>
            </div>
          </div>
        </div>
      </Panel>

      <SectionHeader label="Data" />
      <Panel>
        {/* gq51: Database stats — shown when the daemon reports them.
            Falls back gracefully when db_stats is not available (older daemon). */}
        {dbStats !== null && (
          <SettingsRow title="Database">
            <span className="text-[13px] text-ide-dim tabular-nums">
              {dbStats.item_count} item{dbStats.item_count === 1 ? "" : "s"}
              {" — "}
              {dbStats.size_bytes < 1024
                ? `${dbStats.size_bytes} B`
                : dbStats.size_bytes < 1024 * 1024
                ? `${(dbStats.size_bytes / 1024).toFixed(1)} KB`
                : `${(dbStats.size_bytes / (1024 * 1024)).toFixed(1)} MB`}
            </span>
          </SettingsRow>
        )}
        {/* gq51: Vacuum button — compacts the SQLite WAL to reclaim disk space */}
        <SettingsRow title="Compact database">
          <div className="flex items-center gap-3">
            {vacuumMsg !== null && (
              <span
                className={[
                  "text-[13px]",
                  vacuumMsg.isError ? "text-ide-danger" : "text-ide-success",
                ].join(" ")}
              >
                {vacuumMsg.text}
              </span>
            )}
            <button
              type="button"
              disabled={offline || vacuumBusy}
              onClick={() => void handleVacuum()}
              className={btnCls}
              style={btnStyle}
            >
              {vacuumBusy ? "Vacuuming…" : "Vacuum"}
            </button>
          </div>
        </SettingsRow>
        <SettingsRow title="Clear clipboard history">
          <div className="flex items-center gap-3">
            {deleteMsg !== null && (
              <span
                className={[
                  "text-[13px]",
                  deleteMsg.isError ? "text-ide-danger" : "text-ide-dim",
                ].join(" ")}
              >
                {deleteMsg.text}
              </span>
            )}
            {/* w6xc: replaced misclick-prone inline Yes/No with a proper modal */}
            <button
              type="button"
              disabled={offline}
              onClick={() => setDeleteConfirm(true)}
              className={[
                "border border-ide-border bg-ide-elevated px-3 py-1.5 text-[13px] text-ide-danger",
                "hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40",
              ].join(" ")}
              style={{ borderRadius: "var(--r-ctl)" }}
            >
              Clear history…
            </button>
          </div>
        </SettingsRow>
      </Panel>
    </div>
  );
}
