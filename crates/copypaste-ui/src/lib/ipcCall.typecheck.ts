import { UI_COMMANDS } from "@/generated/ipc";
import { call } from "./ipcCall";

if (false) {
  void call(UI_COMMANDS.status);
  // @ts-expect-error Unknown commands are rejected before reaching a bridge.
  void call("future_command");
}
