import type { ComponentProps } from "react";

import { controlSurfaceVariants, type ControlSurfaceVariants } from "./control-surface";
import { cn } from "@/lib/cn";
import styles from "./input.module.css";

type InputProps = Omit<ComponentProps<"input">, "size"> &
  ControlSurfaceVariants & { surface?: "standalone" | "embedded" };

function Input({
  className,
  type,
  size,
  width = "fill",
  state,
  surface = "standalone",
  disabled,
  ...props
}: InputProps) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        styles.input,
        surface === "standalone" && controlSurfaceVariants({
          size,
          width,
          state: disabled ? "disabled" : state,
        }),
        surface === "embedded" && styles.embedded,
        className,
      )}
      disabled={disabled}
      aria-invalid={state === "invalid" || props["aria-invalid"] || undefined}
      {...props}
    />
  );
}

export { Input };
