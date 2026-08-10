import { Channel } from "@tauri-apps/api/core";

import type { UpdateProgress, UpdateStatus } from "@/generated/ipc";
import { call } from "@/lib/ipcCall";

export type { UpdateProgress, UpdateStatus };

export function getUpdateStatus(): Promise<UpdateStatus> {
  return call<UpdateStatus>("update_status");
}

export function checkForUpdate(): Promise<UpdateStatus> {
  return call<UpdateStatus>("check_for_update");
}

export function installUpdate(
  expectedVersion: string,
  onProgress: (progress: UpdateProgress) => void,
): Promise<UpdateStatus> {
  const progress = new Channel<UpdateProgress>();
  progress.onmessage = onProgress;
  return call<UpdateStatus>(
    "install_update",
    { expectedVersion, progress },
    { timeoutMs: 10 * 60_000 },
  );
}
