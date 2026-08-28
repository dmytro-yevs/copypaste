import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";

import { STATUS_POLL_MS } from "@/lib/scheduling";
import {
  getPairingProgress,
  hasBridge,
  type PairingCeremony,
} from "@/lib/ipc";
import { useUi } from "@/store/ui";

const INBOUND_PAIRING_KEY = ["pairing", "inbound"] as const;

export function inboundPairingNeedsDevices(
  ceremony: PairingCeremony | undefined,
): boolean {
  return ceremony?.semantics.needs_devices ?? false;
}

export function useInboundPairingNav() {
  const view = useUi((state) => state.view);
  const setView = useUi((state) => state.setView);
  const bridge = hasBridge();
  const progress = useQuery({
    queryKey: INBOUND_PAIRING_KEY,
    queryFn: getPairingProgress,
    enabled: bridge && view !== "devices",
    retry: false,
    refetchInterval: STATUS_POLL_MS,
  });

  useEffect(() => {
    if (view === "devices") return;
    if (inboundPairingNeedsDevices(progress.data)) setView("devices");
  }, [progress.data, setView, view]);
}
