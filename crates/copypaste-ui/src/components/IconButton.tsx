/**
 * The one control primitive this window needs.
 *
 * A11Y-9: every icon-only control carries an `aria-label` **and** a matching
 * `title` — the label names it for assistive technology, the title is the
 * pointer user's affordance. The icon itself is decorative and hidden.
 */
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { type VariantProps, cva } from "class-variance-authority";

import { cn } from "../lib/cn";

const iconButton = cva(
  "inline-flex shrink-0 items-center justify-center rounded-ctl transition-colors duration-[var(--dur-fast)] disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      tone: {
        default: "text-dim hover:bg-hover hover:text-text",
        danger: "text-dim hover:bg-hover hover:text-err-strong",
      },
      size: {
        md: "size-[var(--sz-iconbtn)]",
        sm: "size-[var(--ctl-h-sm)]",
      },
    },
    defaultVariants: { tone: "default", size: "md" },
  },
);

interface IconButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "title">,
    VariantProps<typeof iconButton> {
  /** Used for both the accessible name and the pointer tooltip (A11Y-9). */
  label: string;
  children: ReactNode;
}

export function IconButton({
  label,
  tone,
  size,
  className,
  children,
  ...props
}: IconButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={cn(iconButton({ tone, size }), className)}
      {...props}
    >
      {children}
    </button>
  );
}
