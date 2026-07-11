// StorageTab.tsx
// Extracted from SettingsView.tsx renderStorage() (CopyPaste-g06m.14 split) — cut/paste only.
import { useState } from "react";
import { SectionHeader } from "../../../components/SectionHeader";
import { Download, Sparkles, Trash2, Upload } from "lucide-react";
import { SettingsRow } from "../../../components/SettingsRow";
import { Panel } from "../../../components/Panel";
import { SliderRow } from "../../../components/SliderRow";
import { ConfirmModal } from "../../../components/ConfirmModal";
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
import { InfoPopover } from "../components/InfoPopover";

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
  btnCls: _btnCls,
  btnStyle: _btnStyle,
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
    info,
    field,
    steps,
    labels,
    value,
    onChange,
    onRelease,
  }: {
    label: string;
    info?: React.ReactNode;
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
      <SettingsRow title={label} info={info} fullWidth>
        <div className="ctl ctl--grow">
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

  // #12: Export now opens a confirmation modal instead of an inline
  // checkbox + warning + button cluster.
  const [exportModalOpen, setExportModalOpen] = useState(false);

  return (
    <div>
      {/* Design-reference parity: this group of size/quota/ttl/display-count
          sliders is labelled "Limits" in the reference markup. */}
      <SectionHeader label="Limits" />
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
          info={<InfoPopover text="Files over ~8 MB are kept locally but won't sync over P2P/cloud — they're skipped with a warning." />}
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
        {/* bdac.88: History display limit — UI-only display filter (localStorage / UIPrefs).
            No daemon IPC: the daemon stores items until the byte quota is reached.
            This slider filters how many items the UI renders — it does NOT delete items.
            Sentinel 100000 → "Unlimited". */}
        <LimitSliderRow
          label="History display limit"
          info={<InfoPopover text="Display filter only — does not delete stored items. The daemon stores more and prunes by the byte quota above." />}
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
      </Panel>

      {/* 85n9: Backup / Restore panel */}
      <SectionHeader
        label="Backup & restore"
        hint="Export your clipboard history as a JSON file, or restore it from a previous backup."
      />
      <Panel>
        {/* Export row — Q3: clicking opens a confirmation modal (see below) */}
        <SettingsRow title="Export backup" fullWidth>
          <div className="ctl">
            {exportMsg !== null && (
              <span className={`field-note `}>
                {exportMsg.text}
              </span>
            )}
            <button
              type="button"
              className="btn btn--secondary sm"
              disabled={offline || exportInProgress}
              onClick={() => setExportModalOpen(true)}
              data-testid="export-button"
            >
              <Download aria-hidden="true" />{exportInProgress ? "Exporting…" : "Export…"}
            </button>
          </div>
        </SettingsRow>

        {/* Import row — bdac.73: renamed "Restore backup" → "Import history" for parity with Android */}
        <SettingsRow title="Import history" fullWidth>
          <div className="ctl">
            {importMsg !== null && (
              <span className={`field-note `}>
                {importMsg.text}
              </span>
            )}
            {/* Invisible file input driven by the visible button below.
                accept="application/json" limits the picker to .json files.
                The file is read entirely in-browser via FileReader (no fs
                Tauri plugin needed). */}
            <label className="btn btn--secondary sm" style={{ cursor: offline || importInProgress ? "not-allowed" : "pointer" }}>
              <Upload aria-hidden="true" />{importInProgress ? "Importing…" : "Import…"}
              <input
                type="file"
                accept="application/json"
                disabled={offline || importInProgress}
                onChange={(e) => void handleImportFile(e)}
                data-testid="import-file-input"
                hidden
              />
            </label>
          </div>
        </SettingsRow>
      </Panel>

      <SectionHeader label="Data" />
      <Panel>
        {/* gq51: Database stats — shown when the daemon reports them.
            Falls back gracefully when db_stats is not available (older daemon). */}
        {dbStats !== null && (
          <SettingsRow title="Database">
            <span className="field-note field-note--dim">
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
        <SettingsRow title="Compact database" fullWidth>
          <div className="ctl">
            {vacuumMsg !== null && (
              <span className={`field-note `}>
                {vacuumMsg.text}
              </span>
            )}
            <button
              type="button"
              className="btn btn--secondary sm"
              disabled={offline || vacuumBusy}
              onClick={() => void handleVacuum()}
            >
              <Sparkles aria-hidden="true" />{vacuumBusy ? "Vacuuming…" : "Vacuum"}
            </button>
          </div>
        </SettingsRow>
        <SettingsRow title="Clear clipboard history" fullWidth>
          <div className="ctl">
            {deleteMsg !== null && (
              <span className={`field-note `}>
                {deleteMsg.text}
              </span>
            )}
            {/* w6xc: replaced misclick-prone inline Yes/No with a proper modal */}
            <button
              type="button"
              className="btn btn--danger sm"
              disabled={offline}
              onClick={() => setDeleteConfirm(true)}
            ><Trash2 aria-hidden="true" />Clear history…</button>
          </div>
        </SettingsRow>
      </Panel>

      {/* #12: Export confirmation modal — hosts the include-sensitive checkbox
          and plaintext warning that used to live inline in the row. */}
      <ConfirmModal
        open={exportModalOpen}
        title="Export backup"
        body={
          <>
            <label className="check-label">
              <input
                type="checkbox"
                checked={exportIncludeSensitive}
                onChange={(e) => setExportIncludeSensitive(e.target.checked)}
                disabled={offline || exportInProgress}
              />
              Include sensitive items
            </label>
            {exportIncludeSensitive && (
              <span className="field-note field-note--warn">
                Sensitive items will be exported as plaintext. Keep the file secure and delete it when done.
              </span>
            )}
          </>
        }
        confirmLabel="Export"
        danger={false}
        onConfirm={() => { setExportModalOpen(false); void handleExport(); }}
        onCancel={() => setExportModalOpen(false)}
      />
    </div>
  );
}
