import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";

import { STATUS_POLL_MS } from "@/lib/scheduling";
import {
  getPairingProgress,
  hasBridge,
  type PairingState,
} from "@/lib/ipc";
import { useUi } from "@/store/ui";

const INBOUND_PAIRING_KEY = ["pairing", "inbound"] as const;

export function inboundPairingNeedsDevices(state: PairingState): boolean {
  return state === "handshaking" || state === "awaiting_confirmation";
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
    const state = progress.data?.state;
    if (state === undefined || view === "devices") return;
    if (inboundPairingNeedsDevices(state)) setView("devices");
  }, [progress.data?.state, setView, view]);
}
