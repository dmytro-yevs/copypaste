import type { ComponentProps } from "react";
import { Slot } from "@radix-ui/react-slot";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "@/lib/cn";

const buttonVariants = cva(
  "inline-flex min-h-[var(--tap-min)] shrink-0 items-center justify-center gap-2 rounded-md text-sm font-medium whitespace-nowrap outline-none transition-all focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        // Hover tokens, not shadcn's alpha on the fill: an alpha dilutes the
        // fill toward whatever is behind it, which on one theme or the other
        // is toward the label — 4.02:1 worst. These mix away from the ink, so
        // hovering can only raise the label's contrast.
        default: "bg-primary text-primary-foreground shadow-xs hover:bg-primary-hover",
        destructive:
          "bg-destructive text-destructive-foreground shadow-xs hover:bg-destructive-hover focus-visible:ring-destructive",
        // --border-strong, not shadcn's --border: an outline button's boundary
        // is what identifies the control, and --border is 1.25:1 (WCAG 1.4.11).
        outline:
          "border border-border-strong bg-background shadow-xs hover:bg-accent hover:text-accent-foreground",
        secondary:
          "bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80",
        ghost: "hover:bg-accent hover:text-accent-foreground",
        // `--brand-2`, the text-safe accent step: `--accent` is a fill,
        // guaranteed only against `--on-accent` and the 3:1 ring floor, and as
        // text it drops to 3.12:1 on its worst surface.
        link: "text-brand-2 underline-offset-4 hover:underline",
      },
      size: {
        default: "h-9 px-4 py-2 has-[>svg]:px-3",
        sm: "h-8 gap-1.5 rounded-md px-3 has-[>svg]:px-2.5",
        lg: "h-10 rounded-md px-6 has-[>svg]:px-4",
        // Both grow to 48px under a finger: --sz-iconbtn carries the
        // coarse-pointer floor (Material 3), --tap-min the rest.
        icon: "size-[var(--sz-iconbtn)]",
        "icon-sm": "size-8 min-w-[var(--tap-min)]",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "button";
  return (
    <Comp
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
