import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { StateView } from "@/components/shared/StateView";
import { Button, Dialog } from "@/components/ui";
import type { QuitFailureView } from "@/generated/ipc";

/** Separate entry root: independent of App, routing, onboarding and providers. */
export default function QuitFailureSurface() {
  const [failure, setFailure] = useState<QuitFailureView | null>(null);
  const [pending, setPending] = useState(false);
  const [readError, setReadError] = useState(false);

  useEffect(() => {
    let mounted = true;
    const read = async () => {
      try {
        const next = await invoke<QuitFailureView | null>("quit_failure_read");
        if (mounted) setFailure(next);
      } catch {
        if (mounted) setReadError(true);
      }
    };
    void read();
    const poll = window.setInterval(() => { void read(); }, 1000);
    return () => { mounted = false; window.clearInterval(poll); };
  }, []);

  const acknowledge = async () => {
    if (pending || failure === null) return;
    setPending(true);
    try {
      const acknowledged = await invoke<boolean>("quit_failure_ack", { id: failure.id });
      if (!acknowledged) setReadError(true);
    } catch {
      setReadError(true);
    } finally {
      setPending(false);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={() => {}}
      title="CopyPaste could not quit"
      description="The background service did not stop safely."
      showCloseButton={false}
      contentProps={{
        onEscapeKeyDown: (event) => event.preventDefault(),
        onPointerDownOutside: (event) => event.preventDefault(),
      }}
      footer={<Button type="button" disabled={pending || failure === null} onClick={() => void acknowledge()}>OK</Button>}
    >
      {failure ? (
        <StateView mode="error" title={failure.message} description="Acknowledge this message before trying to quit again." />
      ) : readError ? (
        <StateView mode="error" title="The quit failure could not be read. Keep this window open and try again." />
      ) : (
        <StateView mode="loading" title="Reading the quit result" />
      )}
    </Dialog>
  );
}
