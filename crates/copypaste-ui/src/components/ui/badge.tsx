import {
  Children,
  cloneElement,
  type ComponentProps,
  type ReactElement,
  type ReactNode,
} from "react";
import { Slot } from "@radix-ui/react-slot";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";
import styles from "./badge.module.css";

const badgeVariants = cva(
  styles.badge,
  {
    variants: {
      variant: {
        default: styles.primary,
        secondary: styles.secondary,
        destructive: styles.destructive,
        outline: styles.outline,
        /* Status tints: the fill is 15% of the status hue and the ink
         * is its AA-corrected *-strong variant. Using the base hue as the
         * foreground is the contrast bug A11Y-10 exists to prevent. */
        ok: styles.ok,
        warn: styles.warn,
        error: styles.error,
        info: styles.info,
      },
    },
    defaultVariants: { variant: "default" },
  },
);

function Badge({
  className,
  variant,
  asChild = false,
  children,
  ...props
}: ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
  const content = (value: ReactNode) => (
    <span data-slot="badge-content" className={styles.content}>
      {value}
    </span>
  );

  if (asChild) {
    const child = Children.only(children) as ReactElement<{
      children?: ReactNode;
    }>;
    return (
      <Slot
        data-slot="badge"
        className={cn(badgeVariants({ variant }), className)}
        {...props}
      >
        {cloneElement(child, undefined, content(child.props.children))}
      </Slot>
    );
  }

  return (
    <span
      data-slot="badge"
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    >
      {content(children)}
    </span>
  );
}

export { Badge, badgeVariants };
