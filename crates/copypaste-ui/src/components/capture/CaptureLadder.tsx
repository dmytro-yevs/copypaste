/** Every step states where it stands in words — "Done", "Next", "Not done yet"
 *  — beside the glyph: a checklist whose only difference is a tick colour is
 *  not readable to everyone (A11Y-10). */
import { Check, Circle, Dot } from "lucide-react";

import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import type { LadderRung, LadderStep } from "@/lib/capture";

const LABEL = {
  install: "capture.setup.ladder.install",
  start: "capture.setup.ladder.start",
  permission: "capture.setup.ladder.permission",
  armed: "capture.setup.ladder.armed",
} as const satisfies Record<LadderStep, string>;

export function CaptureLadder({ rungs }: { rungs: readonly LadderRung[] }) {
  const { t } = useTranslation();

  return (
    <section
      data-settings-search-target={`section:${t("capture.setup.ladder.title")}`}
      className="flex flex-col gap-s-2"
    >
      <h2 className="text-sm font-medium">{t("capture.setup.ladder.title")}</h2>

      <ol
        aria-label={t("capture.setup.ladder.label")}
        className="flex flex-col gap-s-1"
      >
        {rungs.map((rung) => {
          const Icon = rung.done ? Check : rung.current ? Dot : Circle;
          return (
            <li
              key={rung.id}
              data-step={rung.id}
              className="flex flex-wrap items-center gap-s-2 border-b border-divider py-s-2 text-sm last:border-b-0"
            >
              <Icon
                size={15}
                aria-hidden="true"
                className={cn(
                  "shrink-0",
                  rung.done ? "text-ok-strong" : "text-muted-foreground",
                )}
              />
              <span className={cn("min-w-0 flex-1", rung.current && "font-medium")}>
                {t(LABEL[rung.id])}
              </span>
              <span className="shrink-0 text-xs text-muted-foreground">
                {t(
                  rung.done
                    ? "capture.setup.ladder.done"
                    : rung.current
                      ? "capture.setup.ladder.next"
                      : "capture.setup.ladder.todo",
                )}
              </span>
            </li>
          );
        })}
      </ol>
    </section>
  );
}
