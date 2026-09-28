import type { ComponentProps } from "react";

import { Button } from "@/components/ui/button";

/** Temporary compatibility export while consumers move to Button. */
export type ActionButtonProps = ComponentProps<typeof Button>;

export function ActionButton({ variant = "secondary", type = "button", ...props }: ActionButtonProps) {
    return <Button variant={variant} type={type} {...props} />;
}
