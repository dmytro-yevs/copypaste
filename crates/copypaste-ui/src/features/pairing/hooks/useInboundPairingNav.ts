import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";

import type { ChangePayload } from "@/generated/ipc";
import { POLL_PUSH_BACKSTOP_MS } from "@/lib/scheduling";
import {
  getPairingProgress,
  hasBridge,
  type PairingCeremony,
} from "@/lib/ipc";
import { subscribeNativeEvent } from "@/lib/tauriEventRegistry";
import { EVENT_CHANGED } from "@/lib/tauriEvents";
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
    enabled: bridge,
    retry: false,
    // Pairing changes normally arrive through the shared peer-change stream.
    // Keep the same slow bounded backstop as other push consumers: an event
    // subscription can disappear while the app is backgrounded.
    refetchInterval: POLL_PUSH_BACKSTOP_MS,
    refetchIntervalInBackground: false,
    refetchOnWindowFocus: false,
  });

  useEffect(() => {
    if (!bridge) return;
    return subscribeNativeEvent<ChangePayload>(EVENT_CHANGED, (event) => {
      if (event.payload.topic === "peers") void progress.refetch();
    });
  }, [bridge, progress.refetch]);

  useEffect(() => {
    if (!bridge) return;
    const refresh = () => {
      if (document.visibilityState === "visible") void progress.refetch();
    };
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [bridge, progress.refetch]);

  useEffect(() => {
    if (view === "devices") return;
    if (inboundPairingNeedsDevices(progress.data)) setView("devices");
  }, [progress.data, setView, view]);
}
