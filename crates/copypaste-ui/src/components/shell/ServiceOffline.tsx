/**
 * "The clipboard service isn't running."
 *
 * This is a **screen with an offer**, not an empty list. An empty list here is
 * a lie — it says "you have copied nothing" when the truth is "nothing is
 * listening" — and it was one of v1's recorded defects in the neighbouring case
 * (bdac.2: a loading state that fell through to the main layout and read as
 * "no devices").
 *
 * The offer degrades honestly. If the bridge routes `start_service`, the button
 * starts it. If it does not — the command is not implemented yet, see the table
 * in `lib/ipc.ts` — the failure is `unavailable`, and rather than reporting an
 * error we swap in the manual instruction and a button that copies the command
 * to the clipboard. Either way the user is told what to do next, and no
 * filesystem path appears in any of it (INV-12): `copypaste-daemon` is a
 * command name, not a path.
 */
import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Check, Copy, PlugZap, TriangleAlert } from "lucide-react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/EmptyState";
import { isUnavailable } from "@/lib/errors";
import { hasBridge, startService } from "@/lib/ipc";

const COMMAND = "copypaste-daemon";

export function ServiceOffline() {
  const qc = useQueryClient();
  const [starting, setStarting] = useState(false);
  const [manual, setManual] = useState(!hasBridge());
  const [copied, setCopied] = useState(false);

  async function start() {
    setStarting(true);
    try {
      await startService();
      // Every screen reads from the service; a successful start invalidates
      // all of it rather than one query.
      await qc.invalidateQueries();
    } catch (raw) {
      // Not routed by the bridge yet: fall back to telling the user how,
      // rather than showing them a failure they cannot act on.
      if (isUnavailable(raw)) setManual(true);
    } finally {
      // INV-30: the busy flag is released whatever happened.
      setStarting(false);
    }
  }

  async function copyCommand() {
    try {
      await writeText(COMMAND);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // A clipboard write is the one thing this screen can afford to lose
      // silently — the command is on screen either way.
    }
  }

  if (manual) {
    return (
      <EmptyState
        icon={TriangleAlert}
        title="Start the clipboard service"
        body="CopyPaste records your clipboard from a small background service. Run this command once and it will keep running:"
        secondary={
          <div className="flex flex-wrap items-center justify-center gap-s-2">
            <code className="rounded-md bg-muted px-s-2 py-s-1 font-mono text-sm text-foreground">
              {COMMAND}
            </code>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void copyCommand()}
            >
              {copied ? (
                <Check aria-hidden="true" />
              ) : (
                <Copy aria-hidden="true" />
              )}
              {copied ? "Copied" : "Copy command"}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => void qc.invalidateQueries()}
            >
              Check again
            </Button>
          </div>
        }
      />
    );
  }

  return (
    <EmptyState
      icon={PlugZap}
      busy={starting}
      title="The clipboard service isn't running"
      body="CopyPaste records your clipboard from a small background service. It isn't running, so nothing is being saved right now."
      action={{
        label: starting ? "Starting…" : "Start the service",
        onClick: () => void start(),
      }}
      secondary={
        <Button
          variant="ghost"
          size="sm"
          onClick={() => void qc.invalidateQueries()}
        >
          Check again
        </Button>
      }
    />
  );
}
