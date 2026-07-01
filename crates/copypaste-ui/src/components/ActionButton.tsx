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

export function ActionButton({
  onClick,
  disabled,
  pending,
  pendingLabel = "...",
  type = "button",
  title,
  "aria-label": ariaLabel,
  children,
}: ActionButtonProps) {
  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled ?? pending}
      title={title}
      aria-label={ariaLabel}
    >
      {pending ? pendingLabel : children}
    </button>
  );
}
