import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";

interface OnboardingStepLayoutProps {
  icon?: LucideIcon;
  title: string;
  body?: string;
  children?: ReactNode;
  primary: {
    label: string;
    onClick: () => void;
    disabled?: boolean;
  };
  skip?: {
    label: string;
    onClick: () => void;
  };
}

export function OnboardingStepLayout({
  icon: Icon,
  title,
  body,
  children,
  primary,
  skip,
}: OnboardingStepLayoutProps) {
  return (
    <section className="flex w-full max-w-[28rem] flex-col items-center px-s-4 py-s-5 text-center sm:px-s-6">
      {Icon ? (
        <span className="mb-s-4 flex size-12 items-center justify-center rounded-full border border-border bg-card text-foreground shadow-sm">
          <Icon size={21} aria-hidden="true" />
        </span>
      ) : null}

      <h1 className="text-balance text-xl font-medium text-foreground">{title}</h1>
      {body ? (
        <p className="mt-s-2 text-pretty text-sm text-muted-foreground">{body}</p>
      ) : null}

      {children ? <div className="mt-s-4 w-full text-left">{children}</div> : null}

      <div className="mt-s-5 flex w-full flex-col items-center gap-s-2">
        <Button className="w-full" disabled={primary.disabled} onClick={primary.onClick}>
          {primary.label}
        </Button>
        {skip ? (
          <Button className="w-full" variant="ghost" onClick={skip.onClick}>
            {skip.label}
          </Button>
        ) : null}
      </div>
    </section>
  );
}
