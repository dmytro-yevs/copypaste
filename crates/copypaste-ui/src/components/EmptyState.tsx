/**
 * The spinner and the icon are real, visible elements: v1 shipped classless
 * empty `<span>`s that rendered as nothing and were indistinguishable from a
 * layout bug (CopyPaste-8ebg.29, bdac.2). Loading copy is static text — a
 * pulsing one ignored reduced motion (MOT-21).
 */
import type { LucideIcon } from "lucide-react";
import { LoaderCircle } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";

type EmptyStateTone = "neutral" | "info" | "attention" | "danger" | "private";

interface EmptyStateProps {
  icon?: LucideIcon;
  busy?: boolean;
  tone?: EmptyStateTone;
  title: string;
  body?: string;
  action?: {
    label: string;
    onClick: () => void;
    icon?: LucideIcon;
    disabled?: boolean;
  };
  secondary?: ReactNode;
  secondaryPlacement?: "attached" | "separated";
  compact?: boolean;
}

export function EmptyState({
  icon: Icon,
  busy = false,
  tone = "neutral",
  title,
  body,
  action,
  secondary,
  secondaryPlacement = "separated",
  compact = false,
}: EmptyStateProps) {
  const ActionIcon = action?.icon;

  return (
    <div
      className={`flex min-h-0 flex-1 items-center justify-center overflow-y-auto px-s-3 sm:px-[var(--pad-empty)] ${compact ? "py-s-2 sm:py-s-3" : "py-s-5 sm:py-s-6"}`}
      role={busy ? "status" : undefined}
      aria-live={busy ? "polite" : undefined}
      aria-busy={busy || undefined}
    >
      <section
        data-slot="empty-state"
        className={`flex w-full flex-col items-center rounded-xl border border-border bg-card px-s-4 text-center shadow-sm sm:px-s-6 ${compact ? "max-w-[32rem] justify-center py-s-4 sm:py-s-5" : "max-w-[44rem] py-s-5 sm:py-8"}`}
      >
        <EmptyStateArtwork icon={Icon} busy={busy} tone={tone} />

        <div className="mt-s-4 flex w-full max-w-[40rem] min-w-0 flex-col gap-s-1">
          <p className="text-balance break-normal [overflow-wrap:anywhere] text-lg font-medium text-foreground">
            {title}
          </p>
          {body && (
            <p className="text-pretty break-normal [overflow-wrap:anywhere] text-sm text-muted-foreground">
              {body}
            </p>
          )}
        </div>

        {action && (
          <div className="mt-s-4 flex flex-wrap justify-center gap-s-2">
            <Button disabled={action.disabled} onClick={action.onClick}>
              {ActionIcon && <ActionIcon aria-hidden="true" />}
              {action.label}
            </Button>
          </div>
        )}

        {secondary && (
          <div
            className={
              secondaryPlacement === "attached"
                ? "mt-s-2 flex w-full justify-center"
                : "mt-s-4 flex w-full justify-center border-t border-divider pt-s-3"
            }
          >
            {secondary}
          </div>
        )}
      </section>
    </div>
  );
}

function EmptyStateArtwork({
  icon: Icon,
  busy,
  tone,
}: {
  icon?: LucideIcon;
  busy: boolean;
  tone: EmptyStateTone;
}) {
  const color = {
    neutral: "text-primary",
    info: "text-info-strong",
    attention: "text-warn-strong",
    danger: "text-err-strong",
    private: "text-muted-foreground",
  }[tone];

  return (
    <div aria-hidden="true" className={`relative h-28 w-40 ${color}`}>
      <svg
        viewBox="0 0 160 112"
        className="h-full w-full"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        <path
          d="M26 88C41 101 119 101 134 88"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          opacity=".16"
        />
        <path
          d="M39 75V29C39 23.477 43.477 19 49 19H63L68 13H92L97 19H111C116.523 19 121 23.477 121 29V75C121 80.523 116.523 85 111 85H49C43.477 85 39 80.523 39 75Z"
          fill="currentColor"
          opacity=".1"
        />
        <path
          d="M39 75V29C39 23.477 43.477 19 49 19H63L68 13H92L97 19H111C116.523 19 121 23.477 121 29V75C121 80.523 116.523 85 111 85H49C43.477 85 39 80.523 39 75Z"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinejoin="round"
          opacity=".78"
        />
        <path d="M57 38H103M57 51H95M57 64H87" stroke="currentColor" strokeWidth="3" strokeLinecap="round" opacity=".64" />
        <circle cx="124" cy="27" r="7" fill="currentColor" opacity=".18" />
        <circle cx="132" cy="45" r="3" fill="currentColor" opacity=".32" />
        <circle cx="30" cy="51" r="4" fill="currentColor" opacity=".24" />
      </svg>
      <span className="absolute bottom-0 left-1/2 flex size-12 -translate-x-1/2 items-center justify-center rounded-full border border-border bg-card text-foreground shadow-sm">
        {busy ? (
          <LoaderCircle size={21} className="animate-spin" />
        ) : Icon ? (
          <Icon size={21} />
        ) : null}
      </span>
    </div>
  );
}
