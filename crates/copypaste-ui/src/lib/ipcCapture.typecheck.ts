import type { CapturedPayload, CaptureSnapshot } from "./ipcCapture";

declare const snapshot: CaptureSnapshot;
declare const payload: CapturedPayload;

if (false) {
  // @ts-expect-error Capture state is immutable after crossing the bridge.
  snapshot.rung = "desktop";
  // @ts-expect-error Nested capture state is immutable too.
  snapshot.shizuku.running = false;
  // @ts-expect-error Tagged capture-health variants are immutable.
  snapshot.health.state = "working";
  // @ts-expect-error Capture event metadata is immutable.
  payload.id = "other";
}
