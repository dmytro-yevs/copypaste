import { Channel } from "@tauri-apps/api/core";

import type { UpdateProgress, UpdateStatus } from "@/generated/ipc";
import { call, hasNativeBridge } from "@/lib/ipcCall";

export type { UpdateProgress, UpdateStatus };

export function getUpdateStatus(): Promise<UpdateStatus> {
  if (!hasNativeBridge()) return Promise.resolve({ state: "unsupported" });
  return call<UpdateStatus>("update_status");
}

export function checkForUpdate(): Promise<UpdateStatus> {
  if (!hasNativeBridge()) return Promise.resolve({ state: "unsupported" });
  return call<UpdateStatus>("check_for_update");
}

export function installUpdate(
  expectedVersion: string,
  onProgress: (progress: UpdateProgress) => void,
): Promise<UpdateStatus> {
  if (!hasNativeBridge()) return Promise.resolve({ state: "unsupported" });
  const progress = new Channel<UpdateProgress>();
  progress.onmessage = onProgress;
  return call<UpdateStatus>(
    "install_update",
    { expectedVersion, progress },
    { timeoutMs: 10 * 60_000 },
  );
}
