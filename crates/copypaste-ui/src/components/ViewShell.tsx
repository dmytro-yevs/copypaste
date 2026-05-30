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
      {/* Header — IDE toolbar style: panel bg, hairline bottom border, depth shadow */}
      <header
        className={[
          "flex h-11 shrink-0 items-center justify-between px-4",
          "border-b border-ide-border bg-ide-panel",
          "shadow-ide-xs",
        ].join(" ")}
      >
        <h1 className="text-[13px] font-semibold tracking-wide text-ide-text">
          {title}
        </h1>
        <div className="flex items-center gap-2">{actions}</div>
      </header>

      {/* Content — scrollable, bg matches root */}
      <div className="min-h-0 flex-1 overflow-auto bg-ide-bg p-4">
        {children}
      </div>
    </div>
  );
}
