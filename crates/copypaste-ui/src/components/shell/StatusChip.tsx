/**
 * The accessible name carries the same detail as the visible tooltip. Raw
 * backend errors never enter either surface (INV-12).
 */
import { useId } from "react";
import { useIsMutating } from "@tanstack/react-query";

import {
  peerIsStalled,
  peerLastSyncAt,
} from "@/components/devices/peerState";
import { SYNC_KEY, usePeers } from "@/hooks/useDevices";
import { useStatus } from "@/hooks/useHistory";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { classifyError } from "@/lib/errors";
import { longAge } from "@/lib/format";
import { CURRENT_PROTOCOL_VERSION, type StatusData } from "@/lib/ipc";
import { isAndroidPlatform } from "@/lib/platform";
import { useUi } from "@/store/ui";

type ChipState =
  | "synced"
  | "syncing"
  | "idle"
  | "offline"
  | "error";

const DOT: Record<ChipState, string> = {
  synced: "bg-ok",
  syncing: "bg-ok",
  idle: "bg-muted-foreground",
  offline: "bg-err",
  error: "bg-err",
};

const STATE_KEY = {
  synced: "shell.status.synced",
  syncing: "shell.status.syncing",
  idle: "shell.status.idle",
  offline: "shell.status.offline",
  error: "shell.status.error",
} as const satisfies Record<ChipState, string>;

const chipStatus = (data: StatusData) => ({
  protocol_version: data.protocol_version,
  capture_running: data.capture_running,
});

export function StatusChip({ attentionOnly = false }: { attentionOnly?: boolean }) {
  const { t } = useTranslation();
  const tooltipId = useId();
  const setView = useUi((state) => state.setView);
  const setSettingsTab = useUi((state) => state.setSettingsTab);
  const status = useStatus(chipStatus);
  const peers = usePeers();
  const syncing = useIsMutating({ mutationKey: SYNC_KEY }) > 0;
  const statusErrorKind = status.error ? classifyError(status.error) : null;
  const list = peers.data ?? [];
  const stalled = list.filter((peer) => peerIsStalled(peer));
  const lastSyncAt = list.reduce<number | null>((latest, peer) => {
    const at = peerLastSyncAt(peer);
    return at !== null && (latest === null || at > latest) ? at : latest;
  }, null);
  const mismatch =
    status.data !== undefined &&
    status.data.protocol_version !== CURRENT_PROTOCOL_VERSION;

  let state: ChipState;
  if (statusErrorKind !== null) {
    state =
      statusErrorKind === "offline"
        ? "offline"
        : statusErrorKind === "not_ready"
          ? "idle"
          : "error";
  } else if (status.data === undefined) {
    state = "idle";
  } else if (mismatch) {
    state = "error";
  } else if (peers.error) {
    state =
      classifyError(peers.error) === "unavailable" ? "idle" : "error";
  } else if (syncing) {
    state = "syncing";
  } else if (stalled.length > 0) {
    state = "error";
  } else {
    state = list.length > 0 ? "synced" : "idle";
  }

  const detail: string[] = [];
  if (statusErrorKind !== null) {
    detail.push(
      t(
        statusErrorKind === "not_ready"
          ? "shell.status.detail.serviceConnecting"
          : "shell.status.detail.serviceOffline",
      ),
    );
  } else if (status.data === undefined) {
    detail.push(t("shell.status.detail.serviceConnecting"));
  } else if (mismatch) {
    detail.push(t("shell.status.detail.protocolMismatch"));
  } else {
    detail.push(t("shell.status.detail.serviceConnected"));
  }

  if (!status.error) {
    if (peers.isPending) {
      detail.push(t("shell.status.detail.peersChecking"));
    } else if (peers.error) {
      detail.push(t("shell.status.detail.peersUnavailable"));
    } else if (list.length === 0) {
      detail.push(t("shell.status.detail.peersNone"));
    } else {
      detail.push(t("shell.status.detail.peers", { count: list.length }));
      detail.push(
        lastSyncAt === null
          ? t("shell.status.detail.peerNeverSynced")
          : t("shell.status.detail.peerLastSync", { age: longAge(lastSyncAt) }),
      );
      if (stalled.length > 0) {
        detail.push(
          t("shell.status.detail.stalled", { count: stalled.length }),
        );
      }
    }
  }
  const label = t(STATE_KEY[state]);
  const tooltip = detail.join(" · ");
  const serviceNeedsAttention =
    statusErrorKind === "offline" ||
    mismatch ||
    status.data?.capture_running === false;

  if (attentionOnly && !serviceNeedsAttention) return null;

  const openRecovery = () => {
    if (isAndroidPlatform() && status.data?.capture_running === false) {
      setView("capture");
      return;
    }
    setSettingsTab("service");
    setView("settings");
  };

  const actionLabel = serviceNeedsAttention
    ? t("shell.status.openService")
    : undefined;

  return (
    <div
      role="status"
      aria-describedby={tooltipId}
      aria-label={t("shell.status.label", { state: label, detail: tooltip })}
      title={tooltip}
      className={cn(
        "group relative",
        attentionOnly && "absolute bottom-[calc(100%+var(--s-2))] left-1/2 -translate-x-1/2",
      )}
    >
      <button
        type="button"
        onClick={serviceNeedsAttention ? openRecovery : undefined}
        disabled={!serviceNeedsAttention}
        aria-label={
          actionLabel === undefined
            ? t("shell.status.label", { state: label, detail: tooltip })
            : `${t("shell.status.label", { state: label, detail: tooltip })} ${actionLabel}`
        }
        className={cn(
          "flex items-center gap-s-2 rounded-full border border-transparent px-s-2 py-s-1 text-xs text-muted-foreground outline-none transition-colors duration-[var(--dur-fast)] focus-visible:ring-[3px] focus-visible:ring-ring",
          serviceNeedsAttention
            ? "border-warn/20 bg-warn/15 text-warn-strong hover:bg-warn/15"
            : "disabled:cursor-default",
          attentionOnly && "shadow-sm",
        )}
      >
        <span
          aria-hidden="true"
          className={cn(
            "size-[var(--sz-badge-dot)] shrink-0 rounded-full",
            DOT[state],
          )}
        />
        <span className="max-w-44 truncate">{label}</span>
      </button>
      <span
        id={tooltipId}
        role="tooltip"
        className="pointer-events-none invisible absolute bottom-full left-0 z-[var(--z-popover)] mb-s-2 w-80 max-w-[calc(100vw-var(--s-8))] rounded-md border border-border-strong bg-popover px-s-3 py-s-2 text-left text-xs text-popover-foreground opacity-0 shadow-2 transition-opacity duration-[var(--dur-fast)] group-hover:visible group-hover:opacity-100 group-focus-visible:visible group-focus-visible:opacity-100 motion-reduce:transition-none"
      >
        {tooltip}
      </span>
    </div>
  );
}
