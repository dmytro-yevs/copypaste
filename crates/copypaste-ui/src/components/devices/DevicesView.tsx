/**
 * The Devices screen: paired devices, pairing, unpairing, sync.
 *
 * Rules carried from manifest §3.2:
 *
 *  - **`online: false` means "not seen", never "unreachable"** — a device on a
 *    network without multicast is reachable by address and still reads as
 *    offline, so the dot is a hint and the row never renders as an error
 *    (5917.11 / SCRD-3: the two-state version lied).
 *  - **Peer-reported names are unverified** (INV-15) and are labelled as such,
 *    because `name` is whatever the other device called itself.
 *  - Unpairing is one-sided and needs a confirm that says so (§3.2.6).
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
import { ServiceOffline } from "@/components/shell/ServiceOffline";
import { usePeers, useSyncNow, useUnpair } from "@/hooks/useDevices";
import { cn } from "@/lib/cn";
import { classifyError, friendlyError } from "@/lib/errors";
import { longAge } from "@/lib/format";
import type { PeerInfo } from "@/lib/ipc";

export function DevicesView() {
  const peers = usePeers();
  const unpair = useUnpair();
  const sync = useSyncNow();

  const [creating, setCreating] = useState(false);
  const [accepting, setAccepting] = useState(false);
  const [confirmUnpair, setConfirmUnpair] = useState<PeerInfo | null>(null);

  const errorKind = peers.error ? classifyError(peers.error) : null;
  const list = peers.data ?? [];

  if (errorKind === "offline") return <ServiceOffline />;

  if (errorKind === "unavailable") {
    return (
      <EmptyState
        icon={Link2}
        title="Device sync isn't available in this build"
        body="Pairing runs in the background service, which this build doesn't reach yet. Everything else on this device works normally."
      />
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex shrink-0 flex-wrap items-center gap-s-2 border-b border-divider bg-panel px-s-3 py-s-2">
        <h1 className="mr-auto text-sm font-semibold">Devices</h1>

        <Button size="sm" onClick={() => setCreating(true)}>
          <KeyRound aria-hidden="true" />
          Pair a new device
        </Button>
        <Button size="sm" variant="outline" onClick={() => setAccepting(true)}>
          <Link2 aria-hidden="true" />
          Enter a code
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={sync.isPending || list.length === 0}
          onClick={() => sync.mutate(undefined)}
          title="Sync with every paired device now"
        >
          <RefreshCw
            aria-hidden="true"
            className={cn(sync.isPending && "animate-spin")}
          />
          {sync.isPending ? "Syncing…" : "Sync now"}
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-s-3">
        {peers.isPending ? (
          <EmptyState busy title="Loading…" body="Looking for paired devices." />
        ) : errorKind !== null ? (
          <EmptyState
            title="Couldn't load your devices"
            body={friendlyError(errorKind)}
            action={{ label: "Try again", onClick: () => void peers.refetch() }}
          />
        ) : list.length === 0 ? (
          <EmptyState
            icon={Laptop}
            title="No other devices paired"
            body="Pair your phone or another Mac to sync your clipboard — end-to-end encrypted, directly between your devices."
            action={{
              label: "Pair a new device",
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
                    title="Name reported by the device itself — not verified"
                  >
                    {peer.name}
                  </span>
                  <span className="truncate text-xs text-muted-foreground">
                    Last seen {longAge(peer.last_seen_ms)}
                    {peer.last_addr ? ` · ${peer.last_addr}` : ""}
                  </span>
                </div>

                <Badge variant={peer.online ? "ok" : "secondary"}>
                  {peer.online ? "On the network" : "Not seen"}
                </Badge>

                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={`Sync with ${peer.name} now`}
                  title="Sync with this device now"
                  disabled={sync.isPending}
                  onClick={() => sync.mutate(peer.pairing_id)}
                >
                  <RefreshCw aria-hidden="true" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={`Unpair ${peer.name}`}
                  title="Unpair this device"
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
              Unpair {confirmUnpair?.name ?? "this device"}?
            </AlertDialogTitle>
            <AlertDialogDescription>
              This device will stop syncing with it. Unpairing is one-sided — the
              other device keeps its half of the pairing until it unpairs too.
              Nothing already synced is deleted.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                if (confirmUnpair) unpair.mutate(confirmUnpair);
                setConfirmUnpair(null);
              }}
            >
              Unpair
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
