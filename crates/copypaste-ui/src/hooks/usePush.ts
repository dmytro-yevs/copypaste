/**
 * The change stream, and the poll that backs it up.
 *
 * v1's clients subscribed; v2's app polled every three seconds, which costs up
 * to three seconds of lag on every copy and a constant trickle of IPC on an
 * idle machine (parity finding 15). The daemon now has `Method::Watch` and
 * `service::push` in the bridge turns it into two window events.
 *
 * **Push accelerates the poll; it does not replace it.** A subscription can die
 * in ways neither end notices promptly, and a UI that stopped polling because
 * it believed it was subscribed shows stale history for as long as the window
 * is open. So `live` slows the poll rather than stopping it — the same
 * conclusion `copypaste_cloud::sync::cadence` reached about Realtime, and the
 * obligation manifest 05 §5.4 calls "the only item that can silently
 * reintroduce data loss".
 */
import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";

import { HISTORY_KEY, STATUS_KEY } from "@/hooks/useHistory";
import { PEERS_KEY } from "@/hooks/useDevices";
import { hasBridge } from "@/lib/ipc";

/** Must match `service::push::EVENT_CHANGED`, which has a test asserting it. */
export const EVENT_CHANGED = "copypaste://changed";
/** Must match `service::push::EVENT_PUSH_STATE`. */
export const EVENT_PUSH_STATE = "copypaste://push-state";

/** What the bridge sends on a change. Carries no clipboard content: a
 *  subscriber re-reads through the ordinary commands. */
export interface ChangePayload {
  readonly topic: "items" | "peers";
  readonly item_count: number;
}

export interface PushStatePayload {
  readonly live: boolean;
}

/**
 * Subscribe to the change stream. Returns whether it is currently delivering.
 *
 * Mounted once, at the app root — not per screen. A second subscriber would
 * invalidate the same queries twice for one change.
 */
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
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    }).then(keep);

    void listen<PushStatePayload>(EVENT_PUSH_STATE, (event) => {
      setLive(event.payload.live);
      // A stream that just came back may have missed changes while it was
      // down, so its return is itself a reason to re-read.
      if (event.payload.live) void qc.invalidateQueries({ queryKey: HISTORY_KEY });
    }).then(keep);

    return () => {
      cancelled = true;
      for (const unlisten of detach) unlisten();
    };
  }, [qc]);

  return live;
}
