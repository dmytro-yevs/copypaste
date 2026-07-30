/**
 * Settings › About — what is running, and one way to start over.
 *
 * `clipboard_backend` is surfaced because a fake backend must not be mistaken
 * for the real pasteboard: a build reading `fake` or `android-inprocess` says
 * so, in the warning colour, rather than looking like a working clipboard.
 *
 * Nothing here renders a path (INV-12), which is why the service is described
 * by version and backend rather than by where it lives.
 */
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useStatus } from "@/hooks/useHistory";
import { classifyError, friendlyError } from "@/lib/errors";
import { CURRENT_PROTOCOL_VERSION } from "@/lib/ipc";
import { usePrefs } from "@/store/prefs";
import { Row } from "@/components/settings/Row";

const REAL_BACKENDS = /pasteboard|nspasteboard|system/i;

export function AboutTab() {
  const status = useStatus();
  const resetPrefs = usePrefs((s) => s.reset);

  const backendIsReal = status.data
    ? REAL_BACKENDS.test(status.data.clipboard_backend)
    : true;
  const mismatch =
    status.data !== undefined &&
    status.data.protocol_version !== CURRENT_PROTOCOL_VERSION;

  return (
    <div className="flex flex-col">
      <Row title="Background service">
        {status.error ? (
          <span className="text-sm text-err-strong">
            {friendlyError(classifyError(status.error))}
          </span>
        ) : status.data ? (
          <span className="text-sm tabular-nums text-muted-foreground">
            CopyPaste {status.data.version}
          </span>
        ) : (
          <span className="text-sm text-muted-foreground">Connecting…</span>
        )}
      </Row>

      <Row
        title="Clipboard capture"
        description="Whether the service is watching the system clipboard right now."
      >
        {status.data ? (
          <Badge variant={status.data.capture_running ? "ok" : "warn"}>
            {status.data.capture_running ? "Running" : "Paused"}
          </Badge>
        ) : (
          <Badge variant="secondary">Unknown</Badge>
        )}
      </Row>

      <Row
        title="Clipboard backend"
        description="Which clipboard the service is reading. A test backend means this build is not touching your real clipboard."
      >
        {status.data ? (
          <Badge variant={backendIsReal ? "secondary" : "warn"}>
            {status.data.clipboard_backend}
          </Badge>
        ) : (
          <Badge variant="secondary">Unknown</Badge>
        )}
      </Row>

      <Row
        title="IPC protocol"
        description="The app and the service have to agree on this."
      >
        <Badge variant={mismatch ? "error" : "secondary"}>
          v{status.data?.protocol_version ?? CURRENT_PROTOCOL_VERSION}
          {mismatch ? ` · app speaks v${CURRENT_PROTOCOL_VERSION}` : ""}
        </Badge>
      </Row>

      <Row
        title="Items stored"
        description="Everything the service is holding for this device."
      >
        <span className="text-sm tabular-nums text-muted-foreground">
          {status.data ? status.data.item_count.toLocaleString() : "—"}
        </span>
      </Row>

      <Row
        title="Reset preferences"
        description="Puts theme, accent and list settings back to their defaults. Your clipboard history is not touched."
      >
        <Button variant="outline" size="sm" onClick={resetPrefs}>
          Reset preferences
        </Button>
      </Row>
    </div>
  );
}
