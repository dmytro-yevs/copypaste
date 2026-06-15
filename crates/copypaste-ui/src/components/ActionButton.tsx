import React from "react";

// ---------------------------------------------------------------------------
// ActionButton — shared button component with design-system variants
//
// Variants mirror the Tailwind class sets already used inline across
// DevicesView.tsx and SettingsView.tsx (spec §7 / CopyPaste-zxv2).
//
// primary:      accent-fill + white text (Pair button, Match button)
// secondary:    elevated border + dim text (Cancel, Close, neutral actions)
// danger:       danger-tint fill (Unpair, Revoke — bg-ide-danger/15)
// danger-solid: solid danger fill + white text (primary destructive confirm)
// ghost:        no background, text-only (Cancel link style)
// ---------------------------------------------------------------------------

export type ActionButtonVariant =
  | "primary"
  | "secondary"
  | "danger"
  | "danger-solid"
  | "ghost";

interface ActionButtonProps {
  variant?: ActionButtonVariant;
  onClick?: () => void;
  disabled?: boolean;
  pending?: boolean;
  pendingLabel?: string;
  type?: "button" | "submit" | "reset";
  title?: string;
  "aria-label"?: string;
  className?: string;
  children: React.ReactNode;
  size?: "sm" | "md";
}

const BASE =
  "inline-flex items-center justify-center rounded-ide transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-ide-accent disabled:cursor-not-allowed disabled:opacity-40";

// Size modifiers
const SIZE_SM = "px-2.5 py-1 text-[12px]";
const SIZE_MD = "px-3 py-1.5 text-[13px]";

const VARIANT_CLS: Record<ActionButtonVariant, string> = {
  primary:
    "bg-ide-accent font-medium text-white hover:bg-ide-accentHover",
  secondary:
    "border border-ide-border bg-ide-elevated text-ide-dim hover:bg-ide-hover hover:text-ide-text",
  danger:
    "bg-ide-danger/15 text-ide-danger hover:bg-ide-danger/25",
  "danger-solid":
    "bg-ide-danger font-medium text-white hover:bg-ide-danger/85",
  ghost:
    "text-ide-faint hover:text-ide-dim",
};

export function ActionButton({
  variant = "secondary",
  onClick,
  disabled,
  pending,
  pendingLabel = "...",
  type = "button",
  title,
  "aria-label": ariaLabel,
  className,
  children,
  size = "md",
}: ActionButtonProps) {
  const sizeCls = size === "sm" ? SIZE_SM : SIZE_MD;
  const cls = [BASE, sizeCls, VARIANT_CLS[variant], className].filter(Boolean).join(" ");

  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled ?? pending}
      title={title}
      aria-label={ariaLabel}
      className={cls}
    >
      {pending ? pendingLabel : children}
    </button>
  );
}
