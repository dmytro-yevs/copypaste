import { getVersion } from "@tauri-apps/api/app";

import { hasNativeBridge } from "@/lib/ipcCall";

export async function appVersion(): Promise<string> {
  if (!hasNativeBridge()) return __COPYPASTE_APP_VERSION__;
  try {
    return await getVersion();
  } catch {
    return __COPYPASTE_APP_VERSION__;
  }
}
