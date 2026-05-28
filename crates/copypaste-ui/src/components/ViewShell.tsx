import type { ReactNode } from "react";

/** Shared screen frame: a titled header bar + a scrollable content area. */
export function ViewShell({
  title,
  actions,
  children
}: {
  title: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full flex-col">
      <header className="flex h-11 shrink-0 items-center justify-between border-b border-ide-border px-4">
        <h1 className="text-[13px] font-medium tracking-wide text-ide-text">{title}</h1>
        <div className="flex items-center gap-2">{actions}</div>
      </header>
      <div className="min-h-0 flex-1 overflow-auto p-4">{children}</div>
    </div>
  );
}
