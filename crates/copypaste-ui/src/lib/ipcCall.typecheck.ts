import { UI_COMMANDS } from "@/generated/ipc";
import { call } from "./ipcCall";

if (false) {
  const status = call(UI_COMMANDS.status);
  void status;

  // @ts-expect-error Unknown commands are rejected before reaching a bridge.
  void call("future_command");
  // @ts-expect-error List commands require their argument object.
  void call(UI_COMMANDS.list);
  // @ts-expect-error Every required argument is checked.
  void call(UI_COMMANDS.list, { limit: 20 });
  // @ts-expect-error Argument types come from the command contract.
  void call(UI_COMMANDS.copy_item, { id: 42 });
  // @ts-expect-error No-argument commands reject invented arguments.
  void call(UI_COMMANDS.status, {});
  // @ts-expect-error The caller cannot choose a result type.
  void call<string>(UI_COMMANDS.status);
  // @ts-expect-error The generated status result is not a string promise.
  const wrongResult: Promise<string> = call(UI_COMMANDS.status);
  void wrongResult;
}
