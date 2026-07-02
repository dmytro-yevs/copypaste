// Bridge-mode invoke: used when the UI runs in a plain browser (Playwright,
// ?bridge=1) against the DEV /__ipc daemon bridge. The DATA path (ipc_call) is
// REAL — it hits the live daemon. OS-side commands (sound, notifications,
// accessibility, shortcuts, paste, window) are native side-effects with nowhere
// to run in a browser, so they resolve to harmless defaults. This is NOT mock
// data; only non-data native effects are stubbed.
import type { IpcReply } from "./ipc/types";

async function callBridge(method: string, params: unknown): Promise<IpcReply> {
  const res = await fetch("/__ipc", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ method, params: params ?? null }),
  });
  return (await res.json()) as IpcReply;
}

export async function bridgeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (cmd === "ipc_call") {
    const method = (args?.method as string) ?? "";
    const params = args?.params ?? null;
    return (await callBridge(method, params)) as unknown as T;
  }
  switch (cmd) {
    case "app_version":
      return "dev-bridge" as unknown as T;
    case "get_popup_shortcut":
    case "get_default_popup_shortcut":
    case "log_dir_path":
    case "read_logs":
      return "" as unknown as T;
    case "get_daemon_error":
      return null as unknown as T;
    case "check_accessibility_permission":
    case "check_notification_permission":
    case "get_allow_screenshots":
      return false as unknown as T;
    default:
      // Void OS-side effects: play_copy_sound, show_copy_notification,
      // focus_main_window, paste_plain_text, paste_to_frontmost, hide_popup,
      // set_popup_shortcut, set_allow_screenshots, restart_daemon,
      // request_accessibility_permission, open_item_file, etc.
      return undefined as unknown as T;
  }
}
