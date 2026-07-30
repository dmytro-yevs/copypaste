/**
 * A screen with an offer, not an empty list: an empty list says "you have
 * copied nothing" when the truth is "nothing is listening" — the same defect as
 * bdac.2, one screen over.
 *
 * `start_service` is not routed yet, so a refusal swaps in the manual
 * instruction rather than reporting an error the user cannot act on. No
 * filesystem path appears in either branch (INV-12): `copypaste-daemon` is a
 * command name.
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
      await qc.invalidateQueries();
    } catch (raw) {
      if (isUnavailable(raw)) setManual(true);
    } finally {
      // INV-30: released whatever happened.
      setStarting(false);
    }
  }

  async function copyCommand() {
    try {
      await writeText(COMMAND);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // The command is on screen either way.
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
