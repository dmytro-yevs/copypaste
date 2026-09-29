import { UI_COMMANDS } from "@/generated/ipc";
import { call } from "./ipcCall";

if (false) {
  const status = call(UI_COMMANDS.status);
  void status;

  const writeAvailability = call(UI_COMMANDS.clipboard_write_availability, {
    contentType: "image/png",
    mode: "original",
  });
  const availabilityResult: Promise<
    "available" | "unsupported_content_type" | "unsupported_on_platform"
  > = writeAvailability;
  void availabilityResult;

  // @ts-expect-error Unknown commands are rejected before reaching a bridge.
  void call("future_command");
  // @ts-expect-error List commands require their argument object.
  void call(UI_COMMANDS.list);
  // @ts-expect-error Every required argument is checked.
  void call(UI_COMMANDS.list, { limit: 20 });
  // @ts-expect-error Argument types come from the command contract.
  void call(UI_COMMANDS.copy_item, { id: 42 });
  // @ts-expect-error The availability command takes a content type, not content.
  void call(UI_COMMANDS.clipboard_write_availability, { content: "invalid" });
  // @ts-expect-error The native command requires an explicit mode.
  void call(UI_COMMANDS.clipboard_write_availability, { contentType: "text" });
  void call(UI_COMMANDS.clipboard_write_availability, {
    contentType: "text",
    // @ts-expect-error The mode is a closed wire vocabulary.
    mode: "binary",
  });
  // @ts-expect-error No-argument commands reject invented arguments.
  void call(UI_COMMANDS.status, {});
  // @ts-expect-error The caller cannot choose a result type.
  void call<string>(UI_COMMANDS.status);
  // @ts-expect-error The generated status result is not a string promise.
  const wrongResult: Promise<string> = call(UI_COMMANDS.status);
  void wrongResult;
}
