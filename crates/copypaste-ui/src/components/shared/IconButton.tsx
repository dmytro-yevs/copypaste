import type { ComponentProps } from "react";

import { Button } from "@/components/ui/button";

export type IconButtonProps = Omit<ComponentProps<typeof Button>, "children" | "size" | "icon" | "label"> & {
    icon: NonNullable<ComponentProps<typeof Button>["icon"]>;
    label: string;
    size?: "compact" | "regular";
};

/** Temporary compatibility export while consumers move to Button. */
export function IconButton({ size = "regular", type = "button", ...props }: IconButtonProps) {
    return <Button type={type} size={size === "compact" ? "compactIcon" : "icon"} {...props} />;
}
