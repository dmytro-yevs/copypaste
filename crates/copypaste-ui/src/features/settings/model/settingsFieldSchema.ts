import type { ReactNode } from "react";

import type { IconName } from "@/components/ui/icon";
import type { StateMode } from "@/components/shared/StateView";
import type { SettingsCapabilities } from "./settingsNavigation";
import type { PreferenceSection } from "./preferenceSections";

export type SettingsPlatform = "desktop" | "android" | "windows";
export type SettingsDisclosureId = "clipboard-advanced" | "cloud-server";

/** A field has one stable identity for rendering, search and reveal. Controllers
 * supply the live value and callbacks; this description never owns effects. */
export interface SettingsFieldDefinition {
  readonly id: string;
  readonly section: PreferenceSection | "runtime-events";
  readonly title: string;
  readonly description?: string;
  readonly keywords?: readonly string[];
  readonly platforms?: readonly SettingsPlatform[];
  readonly capability?: Exclude<keyof SettingsCapabilities, "platform">;
  readonly disclosure?: SettingsDisclosureId;
  readonly kind: SettingsFieldKind;
}

export type SettingsFieldKind =
  | "boolean" | "choice" | "multi-choice" | "text" | "number"
  | "readonly" | "status" | "action" | "custom" | "dynamic" | "group";

interface FieldBase {
  readonly definition: SettingsFieldDefinition;
  readonly visible?: boolean;
  readonly disabled?: boolean;
  readonly busy?: boolean;
  readonly badge?: ReactNode;
  readonly note?: ReactNode;
  readonly help?: ReactNode;
}

export type SettingsField =
  | (FieldBase & { readonly kind: "boolean"; readonly value: boolean; readonly onChange: (value: boolean) => void; readonly controlId?: string })
  | (FieldBase & { readonly kind: "choice"; readonly value: string; readonly options: readonly { value: string; label: string; description?: string }[]; readonly onChange: (value: string, source?: HTMLElement) => void; readonly leadingIcon?: IconName; readonly validation?: { readonly min: number; readonly max?: number; readonly message: string }; readonly presentation?: "select" | "segmented" | "cards"; readonly controlClassName?: string; readonly optionClassName?: string; readonly titleClassName?: string; readonly renderOption?: (option: { value: string; label: string; description?: string }) => ReactNode })
  | (FieldBase & { readonly kind: "multi-choice"; readonly value: readonly string[]; readonly options: readonly { value: string; label: string }[]; readonly onChange: (value: readonly string[]) => void })
  | (FieldBase & { readonly kind: "text"; readonly value: string; readonly onChange: (value: string) => void; readonly placeholder?: string })
  | (FieldBase & { readonly kind: "number"; readonly value: number; readonly min: number; readonly max: number; readonly step?: number; readonly displayValue?: string; readonly onChange: (value: number) => void })
  | (FieldBase & { readonly kind: "readonly"; readonly value: ReactNode })
  | (FieldBase & { readonly kind: "status"; readonly value?: ReactNode; readonly mode?: StateMode; readonly ariaLabel?: string; readonly description?: ReactNode; readonly actions?: ReactNode })
  | (FieldBase & { readonly kind: "action"; readonly label: string; readonly onAction?: () => void; readonly href?: string; readonly tone?: "danger"; readonly variant?: "secondary" | "ghost"; readonly icon?: IconName; readonly extraActions?: ReactNode })
  | (FieldBase & { readonly kind: "custom"; readonly content: ReactNode; readonly rowless?: boolean });

export interface SettingsGroupSchema {
  readonly id: string;
  readonly title?: string;
  readonly description?: string;
  readonly surface?: boolean;
  readonly disclosure?: SettingsDisclosureId;
  readonly revealKey?: string;
  readonly fields: readonly SettingsField[];
}
