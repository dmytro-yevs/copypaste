import { invoke } from "@tauri-apps/api/core";
import type { CommandArgs, CommandResult, UiCommandName } from "@/generated/ipc";

/** Native transport only: no preview bridge, React, localization or logging.
 * Protected recovery entries must remain independent of the ordinary app. */
export function invokeNative<C extends UiCommandName>(command: C, args?: CommandArgs<C>): Promise<CommandResult<C>> {
  return invoke<CommandResult<C>>(command, args as Record<string, unknown> | undefined);
}
