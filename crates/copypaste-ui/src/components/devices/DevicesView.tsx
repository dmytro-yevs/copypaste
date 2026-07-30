/**
 * Rules carried from manifest §3.2:
 *
 *  - **`online: false` means "not seen", never "unreachable"** — a device on a
 *    network without multicast is reachable by address and still reads as
 *    offline, so the dot is a hint and the row never renders as an error
 *    (5917.11 / SCRD-3: the two-state version lied).
 *  - **Peer-reported names are unverified** (INV-15) and are labelled as such.
 *  - Unpairing is one-sided and needs a confirm that says so (§3.2.6).
 *  - **Unpair and revoke are two actions, not one with a flag.** Unpairing is
 *    recoverable — the pairing code still enrols the pairing — and revoking is
 *    not, so they carry different weight and `RevokeDialog` owns the heavier
 *    half.
 *  - Every load state is *visible*: a spinner, not a classless empty element
 *    (CopyPaste-8ebg.29, bdac.2).
 */
import { useState } from "react";
import {
  KeyRound,
  Laptop,
  Link2,
  LoaderCircle,
  RefreshCw,
  ShieldOff,
  Unlink,
} from "lucide-react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { EmptyState } from "@/components/EmptyState";
import { PairAcceptDialog } from "@/components/devices/PairAcceptDialog";
import { PairCreateDialog } from "@/components/devices/PairCreateDialog";
import { RevokeDialog } from "@/components/devices/RevokeDialog";
import { ServiceOffline } from "@/components/shell/ServiceOffline";
import { usePeers, useRevoke, useSyncNow, useUnpair } from "@/hooks/useDevices";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { classifyError, friendlyError } from "@/lib/errors";
import { longAge } from "@/lib/format";
import type { PeerInfo } from "@/lib/ipc";

export function DevicesView() {
  const { t } = useTranslation();
  const peers = usePeers();
  const unpair = useUnpair();
  const revoke = useRevoke();
  const sync = useSyncNow();

  const [creating, setCreating] = useState(false);
  const [accepting, setAccepting] = useState(false);
  const [confirmUnpair, setConfirmUnpair] = useState<PeerInfo | null>(null);
  const [confirmRevoke, setConfirmRevoke] = useState<PeerInfo | null>(null);

  const errorKind = peers.error ? classifyError(peers.error) : null;
  const list = peers.data ?? [];

  if (errorKind === "offline") return <ServiceOffline />;

  if (errorKind === "unavailable") {
    return (
      <EmptyState
        icon={Link2}
        title={t("devices.unavailable.title")}
        body={t("devices.unavailable.body")}
      />
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex shrink-0 flex-wrap items-center gap-s-2 border-b border-divider bg-panel px-s-3 py-s-2">
        <h1 className="mr-auto text-sm font-semibold">{t("devices.title")}</h1>

        <Button size="sm" onClick={() => setCreating(true)}>
          <KeyRound aria-hidden="true" />
          {t("devices.actions.pairNew")}
        </Button>
        <Button size="sm" variant="outline" onClick={() => setAccepting(true)}>
          <Link2 aria-hidden="true" />
          {t("devices.actions.add")}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={sync.isPending || list.length === 0}
          onClick={() => sync.mutate(undefined)}
          title={t("devices.actions.syncAllHint")}
        >
          <RefreshCw
            aria-hidden="true"
            className={cn(sync.isPending && "animate-spin")}
          />
          {t(
            sync.isPending ? "devices.actions.syncing" : "devices.actions.syncAll",
          )}
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-s-3">
        {peers.isPending ? (
          <EmptyState
            busy
            title={t("devices.loading.title")}
            body={t("devices.loading.body")}
          />
        ) : errorKind !== null ? (
          <EmptyState
            title={t("devices.failed.title")}
            body={friendlyError(errorKind)}
            action={{
              label: t("common.tryAgain"),
              onClick: () => void peers.refetch(),
            }}
          />
        ) : list.length === 0 ? (
          <EmptyState
            icon={Laptop}
            title={t("devices.none.title")}
            body={t("devices.none.body")}
            action={{
              label: t("devices.actions.pairNew"),
              onClick: () => setCreating(true),
              icon: KeyRound,
            }}
          />
        ) : (
          <ul className="mx-auto flex max-w-[var(--content-max-width)] flex-col gap-s-2">
            {list.map((peer) => (
              <li
                key={peer.pairing_id}
                className="flex flex-wrap items-center gap-s-3 rounded-xl border border-border bg-card px-s-3 py-s-3"
              >
                <span
                  aria-hidden="true"
                  className="flex size-[var(--sz-tile)] shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground"
                >
                  <Laptop size={18} />
                </span>

                <div className="flex min-w-0 flex-1 flex-col">
                  <span
                    className="truncate text-sm font-medium"
                    title={t("devices.peer.nameHint")}
                  >
                    {peer.name}
                  </span>
                  <span className="truncate text-xs text-muted-foreground">
                    {t("devices.peer.lastSeen", {
                      age: longAge(peer.last_seen_ms),
                    })}
                    {peer.last_addr ? ` · ${peer.last_addr}` : ""}
                  </span>
                </div>

                <Badge variant={peer.online ? "ok" : "secondary"}>
                  {t(peer.online ? "devices.peer.online" : "devices.peer.offline")}
                </Badge>

                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={t("devices.peer.syncOne", { name: peer.name })}
                  title={t("devices.peer.syncOneHint")}
                  disabled={sync.isPending}
                  onClick={() => sync.mutate(peer.pairing_id)}
                >
                  <RefreshCw aria-hidden="true" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={t("devices.peer.unpairOne", { name: peer.name })}
                  title={t("devices.peer.unpairHint")}
                  className="hover:text-err-strong"
                  disabled={unpair.isPending}
                  onClick={() => setConfirmUnpair(peer)}
                >
                  {unpair.isPending &&
                  unpair.variables?.pairing_id === peer.pairing_id ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" />
                  ) : (
                    <Unlink aria-hidden="true" />
                  )}
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={t("devices.peer.revokeOne", { name: peer.name })}
                  title={t("devices.peer.revokeHint")}
                  className="hover:text-err-strong"
                  disabled={revoke.isPending}
                  onClick={() => setConfirmRevoke(peer)}
                >
                  {revoke.isPending &&
                  revoke.variables?.pairing_id === peer.pairing_id ? (
                    <LoaderCircle aria-hidden="true" className="animate-spin" />
                  ) : (
                    <ShieldOff aria-hidden="true" />
                  )}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <PairCreateDialog open={creating} onOpenChange={setCreating} />
      <PairAcceptDialog open={accepting} onOpenChange={setAccepting} />

      <AlertDialog
        open={confirmUnpair !== null}
        onOpenChange={(open) => !open && setConfirmUnpair(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("devices.unpair.title", {
                name: confirmUnpair?.name ?? t("devices.peer.thisDevice"),
              })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("devices.unpair.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {/* Where a user who has lost a device finds the action that helps.
              They reach for "unpair" first, because that is the word they
              know. */}
          <p className="text-sm text-muted-foreground">
            {t("devices.unpair.lost")}
          </p>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                if (confirmUnpair) unpair.mutate(confirmUnpair);
                setConfirmUnpair(null);
              }}
            >
              {t("devices.unpair.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <RevokeDialog
        peer={confirmRevoke}
        onOpenChange={(open) => !open && setConfirmRevoke(null)}
        onConfirm={(peer) => {
          revoke.mutate(peer);
          setConfirmRevoke(null);
        }}
      />
    </div>
  );
}
