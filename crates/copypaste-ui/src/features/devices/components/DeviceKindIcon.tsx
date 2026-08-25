

import { Icon } from "@/components/ui";
import {
  deviceIconKind,
  type DevicePresentationIdentity,
} from "@/features/devices/model/devicePresentation";

export function DeviceKindIcon({
  identity,
  size = "md",
}: {
  identity: DevicePresentationIdentity;
  size?: "sm" | "md" | "lg";
}) {
  return <Icon name={deviceIconKind(identity)} size={size} />;
}
