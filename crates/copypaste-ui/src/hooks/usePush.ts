/**
 * **Push accelerates the poll; it does not replace it.** A subscription can die
 * in ways neither end notices promptly, and a UI that stopped polling because
 * it believed it was subscribed shows stale history for as long as the window
 * is open. `live` slows the poll rather than stopping it — manifest 05 §5.4
 * calls this "the only item that can silently reintroduce data loss".
 */
import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";

import { invalidateHistoryQueries, STATUS_KEY } from "@/hooks/useHistory";
import { PEERS_KEY } from "@/hooks/useDevices";
import { CONFIG_KEY, PRIVATE_MODE_KEY } from "@/hooks/useServiceConfig";
import { OPEN_AT_LOGIN_KEY } from "@/hooks/useOpenAtLogin";
import { hasBridge, type PrivateModeData } from "@/lib/ipc";
import {
  EVENT_AUTOSTART_CHANGED,
  EVENT_CHANGED,
  EVENT_PRIVATE_MODE_CHANGED,
  EVENT_PUSH_STATE,
} from "@/lib/tauriEvents";

export {
  EVENT_AUTOSTART_CHANGED,
  EVENT_CHANGED,
  EVENT_PRIVATE_MODE_CHANGED,
  EVENT_PUSH_STATE,
} from "@/lib/tauriEvents";

/** What the bridge sends on a change. Carries no clipboard content: a
 *  subscriber re-reads through the ordinary commands. */
export interface ChangePayload {
  readonly topic: "items" | "peers";
  readonly item_count: number;
}

export interface PushStatePayload {
  readonly live: boolean;
}

/** Mounted once, at the app root — not per screen. A second subscriber would
 *  invalidate the same queries twice for one change. */
export function usePush(): boolean {
  const qc = useQueryClient();
  const [live, setLive] = useState(false);

  useEffect(() => {
    if (!hasBridge()) return;

    // `listen` resolves to an unlisten function. Both are awaited in a
    // cancelled-aware way: an effect that unmounts before the promise settles
    // must still detach, or a fast remount stacks two listeners — which is how
    // v1 accumulated matchMedia handlers (CopyPaste-g27b.20).
    let cancelled = false;
    const detach: Array<() => void> = [];

    function keep(unlisten: () => void) {
      if (cancelled) unlisten();
      else detach.push(unlisten);
    }

    void listen<ChangePayload>(EVENT_CHANGED, (event) => {
      // Invalidate rather than write the payload into the cache: the event
      // says *that* something changed, and re-reading keeps one set of rules
      // about what this window is allowed to see.
      if (event.payload.topic === "peers") {
        void qc.invalidateQueries({ queryKey: PEERS_KEY });
        return;
      }
      void invalidateHistoryQueries(qc);
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    }).then(keep);

    void listen<PushStatePayload>(EVENT_PUSH_STATE, (event) => {
      setLive(event.payload.live);
      // A stream that just came back may have missed changes while it was
      // down, so its return is itself a reason to re-read.
      if (event.payload.live) void invalidateHistoryQueries(qc);
    }).then(keep);

    void listen<boolean>(EVENT_PRIVATE_MODE_CHANGED, (event) => {
      qc.setQueryData<PrivateModeData>(PRIVATE_MODE_KEY, (cached) =>
        cached === undefined
          ? undefined
          : { ...cached, private_mode: event.payload },
      );
      void qc.invalidateQueries({ queryKey: PRIVATE_MODE_KEY });
      void qc.invalidateQueries({ queryKey: CONFIG_KEY });
      void invalidateHistoryQueries(qc);
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    }).then(keep);

    // The payload is the observed state, but the query is re-read rather than
    // written from it: one place decides what this setting is, the same rule
    // the change event above follows.
    void listen<boolean>(EVENT_AUTOSTART_CHANGED, () => {
      void qc.invalidateQueries({ queryKey: OPEN_AT_LOGIN_KEY });
    }).then(keep);

    return () => {
      cancelled = true;
      for (const unlisten of detach) unlisten();
    };
  }, [qc]);

  return live;
}
