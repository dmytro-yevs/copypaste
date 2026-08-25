import type { ComponentType } from "react";

import { AppIcon } from "@/components/shared";
import { useSourceAppIcon } from "@/features/source-apps/hooks/useSourceAppIcon";

type FallbackIcon = ComponentType<{
  size?: number;
  className?: string;
  "aria-hidden"?: boolean;
}>;

interface SourceAppIconProps {
  bundleId: string | null;
  Fallback?: FallbackIcon;
  fallbackText?: string;
  size?: "xs" | "sm" | "md";
  shape?: "square" | "rounded" | "circle";
  className?: string;
}

export function SourceAppIcon({
  bundleId,
  Fallback,
  fallbackText,
  size = "sm",
  shape = "square",
  className,
}: SourceAppIconProps) {
  const icon = useSourceAppIcon(bundleId);
  return (
    <AppIcon
      pngBase64={icon.data?.png_base64}
      Fallback={Fallback}
      fallbackText={fallbackText}
      size={size}
      shape={shape}
      className={className}
    />
  );
}
